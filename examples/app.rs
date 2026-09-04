//! A small app wired up the way muxa expects, kept here so that `cargo test` and
//! `cargo clippy --all-targets` compile the `routes!` and `default_layers!` macros.
//! Both expand to `muxa::` paths, so nothing inside the library itself can exercise them.
//!
//! `cargo run --example app`, then http://127.0.0.1:3000.

use axum::{
    extract::{Extension, FromRequestParts},
    response::Redirect,
    routing::{get, post},
    Router,
};
use http::request::Parts;
use maud::{html, Markup};
use muxa::{
    config::Config,
    errors::ErrResponse,
    extractors::multipart::{Multipart, UploadedFile},
    html::{HtmlContext, HtmlContextBuilder, Template},
    reexports::TypedPath,
    router::RouterExtension,
    sessions::UserSession,
    theme::{ThemeCookie, ThemeTrait},
};
use serde::Deserialize;
use sqlx::sqlite::SqlitePoolOptions;
use std::convert::Infallible;

muxa::routes! {
    "/" => Home {}
    "/users/{id}" => User { id: i64 }
    "/upload" => Upload {}
}

/// `routes!` should expand without the caller importing anything, so this second
/// invocation lives in a module that imports nothing
#[allow(dead_code)]
mod hygiene {
    muxa::routes! {
        "/nothing/{slug}" => Nothing { slug: String }
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum Theme {
    #[default]
    Light,
    Dark,
}

impl ThemeTrait for Theme {
    fn css_url(&self) -> &'static str {
        match self {
            Theme::Light => "/static/light.css",
            Theme::Dark => "/static/dark.css",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }
}

/// The app's template. `HtmlMiddleware` builds one per request and stores it in the
/// request extensions, so it needs `FromRequestParts` and `Clone`.
#[derive(Clone)]
struct Tpl {
    theme: Theme,
}

impl<S: Send + Sync> FromRequestParts<S> for Tpl {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ThemeCookie(theme) = ThemeCookie::from_request_parts(parts, state).await?;
        Ok(Self { theme })
    }
}

impl Template<NamedRoute> for Tpl {
    fn head(ctx: &HtmlContext<Self, NamedRoute>) -> Markup {
        html! {
            title { (ctx.get_title()) }
            link rel="stylesheet" href=(ctx.inner.theme.css_url());
        }
    }

    fn body(ctx: &HtmlContext<Self, NamedRoute>) -> Markup {
        html! {
            nav {
                // a NamedRoute renders as its href, and `matches` ignores the params,
                // so any user page marks the "users" link as current
                a href=(route_home()) .current[ctx.route.matches(&route_home())] { "home" }
                a href=(route_user(1)) .current[ctx.route.matches(&route_user(1))] { "users" }
            }
            @if let Some(flash) = &ctx.session_flash {
                p .flash { (flash) }
            }
            main { (ctx.content) }
            (ctx.section_get("scripts"))
        }
    }
}

type Builder = HtmlContextBuilder<Tpl, NamedRoute>;

async fn home(Extension(builder): Extension<Builder>) -> HtmlContext<Tpl, NamedRoute> {
    builder
        .build(html! {
            h1 { "hello" }
            form action=(route_upload()) method="post" enctype="multipart/form-data" {
                input type="text" name="name";
                input type="file" name="file";
                button { "upload" }
            }
        })
        .with_title("home")
}

async fn user(
    UserPath { id }: UserPath,
    Extension(builder): Extension<Builder>,
) -> HtmlContext<Tpl, NamedRoute> {
    builder
        .build(html! { h1 { "user " (id) } })
        .with_title(format!("user {id}"))
        .with_description("a single user")
        .section_append("scripts", html! { script { "console.log('user page')" } })
}

#[derive(Debug, Deserialize)]
struct UploadForm {
    name: String,
    file: Option<UploadedFile>,
}

async fn upload(
    Extension(mut session): Extension<UserSession>,
    Multipart(form): Multipart<UploadForm>,
) -> Result<Redirect, ErrResponse> {
    let msg = match form.file {
        Some(file) => format!("{} uploaded {}", form.name, file.filename),
        None => format!("{} uploaded nothing", form.name),
    };
    // flash lives for exactly one further request, so it survives this redirect
    session.flash(msg).await?;

    Ok(route_home().redirect())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = muxa::tracing::setup_tracing(std::env::temp_dir());

    let config = Config::new(
        std::env::temp_dir().join("muxa-example-uploads"),
        "static".into(),
        "http://127.0.0.1:3000".to_string(),
        "muxa example".to_string(),
        "/uploaded".to_string(),
    );
    std::fs::create_dir_all(config.get_upload_path())?;

    // a single connection, so every query sees the same in-memory database
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY NOT NULL,
            expires DATETIME NOT NULL,
            session TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    let app = Router::new()
        .route(HomePath::PATH, get(home))
        .route(UserPath::PATH, get(user))
        .route(UploadPath::PATH, post(upload))
        .upload_dir(&config)
        .layer(muxa::default_layers! {
            builder: Builder,
            pool: pool,
            config: config,
            extensions: [],
        });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
