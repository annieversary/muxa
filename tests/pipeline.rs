//! Drives a router wired up with `default_layers!` through `oneshot`, so the pieces
//! that only exist once every layer has run get exercised.

use axum::{
    extract::{Extension, FromRequestParts},
    response::Redirect,
    routing::{get, post},
    Router,
};
use chrono::{Duration, Utc};
use http::request::Parts;
use maud::{html, Markup};
use muxa::{
    config::Config,
    errors::ErrResponse,
    extractors::multipart::{Multipart, UploadedFile},
    html::{HtmlContext, HtmlContextBuilder, Template},
    reexports::TypedPath,
    sessions::{DbSessionStore, UserSession},
    tests::helpers::{empty_get, RouterExt},
};
use serde::Deserialize;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::{convert::Infallible, path::PathBuf};

muxa::routes! {
    "/" => Home {}
    "/upload" => Upload {}
}

#[derive(Clone)]
struct Tpl;

impl<S: Send + Sync> FromRequestParts<S> for Tpl {
    type Rejection = Infallible;

    async fn from_request_parts(_: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self)
    }
}

impl Template<NamedRoute> for Tpl {
    fn head(ctx: &HtmlContext<Self, NamedRoute>) -> Markup {
        html! { title { (ctx.get_title()) } }
    }

    fn body(ctx: &HtmlContext<Self, NamedRoute>) -> Markup {
        html! {
            @if let Some(flash) = &ctx.session_flash {
                p .flash { (flash) }
            }
            main { (ctx.content) }
        }
    }
}

type Builder = HtmlContextBuilder<Tpl, NamedRoute>;

async fn home(Extension(builder): Extension<Builder>) -> HtmlContext<Tpl, NamedRoute> {
    builder.build(html! { h1 { "hello" } })
}

#[derive(Debug, Deserialize)]
struct UploadForm {
    file: Option<UploadedFile>,
}

async fn upload(
    Extension(mut session): Extension<UserSession>,
    Multipart(form): Multipart<UploadForm>,
) -> Result<Redirect, ErrResponse> {
    let msg = match &form.file {
        Some(file) => format!("stored at {}", file.upload_path),
        None => "nothing".to_string(),
    };
    session.flash(msg).await?;

    Ok(route_home().redirect())
}

/// one connection, so every query sees the same in-memory database
async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY NOT NULL,
            expires DATETIME NOT NULL,
            session TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

async fn app(upload_path: PathBuf) -> Router {
    app_with(upload_path, pool().await)
}

fn app_with(upload_path: PathBuf, pool: SqlitePool) -> Router {
    let config = Config::new(
        upload_path,
        "static".into(),
        "http://example.com".to_string(),
        "muxa test".to_string(),
        "/uploaded".to_string(),
    );

    Router::new()
        .route(HomePath::PATH, get(home))
        .route(UploadPath::PATH, post(upload))
        .layer(muxa::default_layers! {
            builder: Builder,
            pool: pool,
            config: config,
            extensions: [],
        })
}

fn upload_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("muxa-test-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[tokio::test]
async fn renders_a_named_route() {
    let res = app(upload_dir("render")).await.req(empty_get("/")).await;

    assert!(res.is_ok());
    assert!(res.contains_str("hello"));
    assert!(res.contains_str("<title>muxa test</title>"));
}

#[tokio::test]
async fn session_cookie_is_http_only() {
    let res = app(upload_dir("cookie")).await.req(empty_get("/")).await;

    let cookie = res
        .parts
        .headers
        .get(http::header::SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .unwrap();

    assert!(cookie.starts_with("muxa_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Secure"));
}

/// a request that matched no route gets no builder, and the fallback answers it
#[tokio::test]
async fn unknown_route_falls_through() {
    let res = app(upload_dir("unknown"))
        .await
        .req(empty_get("/does-not-exist"))
        .await;

    assert_eq!(res.status(), http::StatusCode::NOT_FOUND);
}

fn multipart_body(file_name: &str) -> (String, axum::body::Body) {
    let boundary = "muxaboundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
         Content-Type: text/plain\r\n\
         \r\n\
         pwned\r\n\
         --{boundary}--\r\n"
    );

    (
        format!("multipart/form-data; boundary={boundary}"),
        axum::body::Body::from(body),
    )
}

/// the file name comes straight off the wire, so it must not be able to steer the
/// write out of the upload directory, by climbing out of it or by being absolute
#[tokio::test]
async fn upload_stays_inside_the_upload_directory() {
    // one level up lands back in the upload directory, so the traversal needs two
    let escaped = std::env::temp_dir().join("muxa-test-pwned.txt");
    let hostile_names = [
        "../../muxa-test-pwned.txt".to_string(),
        escaped.display().to_string(),
    ];

    for (i, file_name) in hostile_names.iter().enumerate() {
        let dir = upload_dir(&format!("traversal-{i}"));
        let _ = std::fs::remove_file(&escaped);

        let (content_type, body) = multipart_body(file_name);
        let req = http::Request::builder()
            .method("POST")
            .uri("/upload")
            .header(http::header::CONTENT_TYPE, content_type)
            .body(body)
            .unwrap();

        let res = app(dir.clone()).await.req(req).await;
        assert_eq!(res.status(), http::StatusCode::SEE_OTHER, "for {file_name}");

        assert!(
            !escaped.exists(),
            "{file_name} escaped the upload directory to {}",
            escaped.display()
        );

        // it landed in the random folder under the upload directory instead
        let stored: Vec<_> = walk(&dir).collect();
        assert_eq!(stored.len(), 1, "for {file_name}, got {stored:?}");
        assert_eq!(stored[0].file_name().unwrap(), "muxa-test-pwned.txt");
        assert_eq!(stored[0].parent().unwrap().parent().unwrap(), dir);
    }
}

/// a file part whose name has no usable component at all is skipped, not written
#[tokio::test]
async fn upload_with_an_unusable_name_is_skipped() {
    let dir = upload_dir("unusable");

    let (content_type, body) = multipart_body("..");
    let req = http::Request::builder()
        .method("POST")
        .uri("/upload")
        .header(http::header::CONTENT_TYPE, content_type)
        .body(body)
        .unwrap();

    let res = app(dir.clone()).await.req(req).await;
    assert_eq!(res.status(), http::StatusCode::SEE_OTHER);
    assert_eq!(walk(&dir).count(), 0);
}

#[tokio::test]
async fn pruning_only_removes_expired_sessions() {
    let pool = pool().await;
    let store = DbSessionStore::new(pool.clone());

    let rows = [
        ("stale", Utc::now() - Duration::days(1)),
        ("just-expired", Utc::now() - Duration::seconds(1)),
        ("live", Utc::now() + Duration::days(180)),
    ];
    for (id, expires) in rows {
        sqlx::query("INSERT INTO sessions (id, expires, session) VALUES (?, ?, ?)")
            .bind(id)
            .bind(expires)
            .bind("{}")
            .execute(&pool)
            .await
            .unwrap();
    }

    assert_eq!(store.prune_expired().await.unwrap(), 2);

    let left: Vec<String> = sqlx::query_scalar("SELECT id FROM sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(left, ["live"]);

    // nothing left to remove, so a second run is a no-op
    assert_eq!(store.prune_expired().await.unwrap(), 0);
}

/// the sqlite backend stores `expires` as text, so pruning is a string comparison.
/// this checks it against a row the store actually wrote, not a hand made one, because
/// a format mismatch here would delete live sessions
#[tokio::test]
async fn pruning_keeps_a_session_the_store_just_wrote() {
    let pool = pool().await;
    let store = DbSessionStore::new(pool.clone());

    let res = app_with(upload_dir("prune-live"), pool.clone())
        .req(empty_get("/"))
        .await;
    assert!(res.is_ok());

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "the request should have written a session");

    assert_eq!(store.prune_expired().await.unwrap(), 0);
}

/// the pruner runs once as soon as it is spawned, rather than waiting out a full interval
#[tokio::test]
async fn spawned_pruner_runs_immediately() {
    let pool = pool().await;
    let store = DbSessionStore::new(pool.clone());

    sqlx::query("INSERT INTO sessions (id, expires, session) VALUES (?, ?, ?)")
        .bind("stale")
        .bind(Utc::now() - Duration::days(1))
        .bind("{}")
        .execute(&pool)
        .await
        .unwrap();

    let pruner = store.spawn_pruner(std::time::Duration::from_secs(3600));

    let count = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
                .fetch_one(&pool)
                .await
                .unwrap();
            if count == 0 {
                return count;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the pruner should have run without waiting for the second tick");

    assert_eq!(count, 0);
    pruner.abort();
}

fn walk(dir: &std::path::Path) -> impl Iterator<Item = PathBuf> + '_ {
    std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk_owned(path)
            } else {
                vec![path]
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
}

fn walk_owned(dir: PathBuf) -> Vec<PathBuf> {
    walk(&dir).collect()
}
