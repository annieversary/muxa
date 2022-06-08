use async_session::Session;
use axum::{
    headers::{Cookie, HeaderMapExt},
    http::{header::SET_COOKIE, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use chrono::{Duration, NaiveDateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use validator::{ValidationError, ValidationErrors};

use crate::{
    cookies::SameSite,
    errors::{internal_error, ErrResponse},
};

// implemented following
// https://github.com/tokio-rs/axum/blob/main/examples/sessions/src/main.rs
// https://docs.rs/async-session/latest/src/async_session/memory_store.rs.html#17-52
// https://github.com/AscendingCreations/AxumSessions/blob/main/src/session_store.rs
// https://github.com/AscendingCreations/AxumSessions/blob/main/src/databases/mysql.rs

pub const SESSION_COOKIE_NAME: &str = "muxa_session";

const FLASH_KEY: &str = "internal-key-flash";
const FLASH_KEY_TRACKER: &str = "internal-key-flash-tracker";
const ERRORS_KEY: &str = "internal-key-errors";
const ERRORS_KEY_TRACKER: &str = "internal-key-errors-tracker";
const OLD_KEY: &str = "internal-key-old";
const OLD_KEY_TRACKER: &str = "internal-key-old-tracker";

#[cfg(feature = "mysql")]
type DbPool = sqlx::MySqlPool;
#[cfg(feature = "sqlite")]
type DbPool = sqlx::SqlitePool;

#[derive(Clone)]
pub struct DbSessionStore {
    pool: DbPool,
    /// SameSite value to use for session cookies
    same_site: SameSite,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct InternalSession {
    id: String,
    expires: NaiveDateTime,
    session: String,
}

impl DbSessionStore {
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            same_site: SameSite::Strict,
        }
    }

    pub fn with_same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    pub async fn load_session(&self, cookie_value: String) -> Result<Option<Session>, ErrResponse> {
        let id = Session::id_from_cookie_value(&cookie_value).map_err(|_| {
            ErrResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "couldn't get id out of cookie value".to_string(),
            )
        })?;
        let result: Option<InternalSession> = sqlx::query_as(
            "SELECT * FROM sessions
        WHERE id = ? AND (expires IS NULL OR expires > ?)",
        )
        .bind(id)
        .bind(Utc::now())
        .fetch_optional(&self.pool)
        .await?;

        result
            .map(|r| serde_json::from_str(&r.session))
            .transpose()
            .map_err(internal_error)
    }

    pub async fn store_session(&self, session: Session) -> Result<Option<String>, ErrResponse> {
        let session_string = serde_json::to_string(&session)?;

        #[cfg(feature = "mysql")]
        let q = "INSERT INTO sessions
                (id, session, expires) VALUES(?, ?, ?)
            ON DUPLICATE KEY UPDATE
                expires = VALUES(expires),
                session = VALUES(session)";
        #[cfg(feature = "sqlite")]
        let q = "INSERT INTO sessions
                (id, session, expires) VALUES(?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                session = excluded.session,
                expires = excluded.expires";

        sqlx::query(q)
            .bind(session.id().to_string())
            .bind(&session_string)
            .bind(Utc::now() + Duration::hours(6))
            .execute(&self.pool)
            .await?;

        Ok(session.into_cookie_value())
    }
}

#[derive(Clone)]
pub struct UserSession {
    session: Session,
    store: DbSessionStore,
}
impl std::fmt::Debug for UserSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserSession")
            .field("session", &self.session)
            .field("store", &"DbSessionStore".to_string())
            .finish()
    }
}

impl UserSession {
    /// stores the session in the DbSessionStore
    /// returns the header value which will set the corresponding cookie
    ///
    /// should only be called on the original session, not any cloned
    /// cloned sessions don't contain the cookie value
    async fn save_and_get_cookie(self) -> Result<HeaderValue, ErrResponse> {
        let cookie = self
            .store
            .store_session(self.session)
            .await?
            .expect("calling save_and_get_cookie on a cloned session, this is not allowed");
        HeaderValue::from_str(
            format!(
                "{}={}; SameSite={}; Secure; Path=/",
                SESSION_COOKIE_NAME, cookie, self.store.same_site
            )
            .as_str(),
        )
        .map_err(internal_error)
    }

    /// stores the session in the DbSessionStore
    pub async fn save(&self) -> Result<(), ErrResponse> {
        // when we clone the session, the cookie value is not set, so we can ignore it
        let _cookie = self.store.store_session(self.session.clone()).await?;
        Ok(())
    }

    pub async fn insert(
        &mut self,
        key: &str,
        value: impl serde::Serialize,
    ) -> Result<(), ErrResponse> {
        self.session.insert(key, value)?;
        self.clone().save().await
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.session.get(key)
    }

    pub async fn remove(&mut self, key: &str) -> Result<(), ErrResponse> {
        self.session.remove(key);
        self.clone().save().await
    }

    pub async fn flash(&mut self, value: impl AsRef<str>) -> Result<(), ErrResponse> {
        self.session.insert(FLASH_KEY, value.as_ref())?;
        self.session.insert(FLASH_KEY_TRACKER, true)?;
        self.clone().save().await
    }

    pub fn get_flash(&self) -> Option<String> {
        self.session.get(FLASH_KEY)
    }

    // lmao
    pub async fn errors<M, L, K, V>(&mut self, value: M) -> Result<(), ErrResponse>
    where
        M: Into<HashMap<K, L>>,
        K: ToString,
        L: Into<Vec<V>>,
        V: ToString,
    {
        let map: HashMap<K, L> = value.into();
        let map: HashMap<String, Vec<String>> = map
            .into_iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    v.into().iter().map(ToString::to_string).collect::<Vec<_>>(),
                )
            })
            .collect();
        self.session.insert(ERRORS_KEY, map)?;
        self.session.insert(ERRORS_KEY_TRACKER, true)?;
        self.clone().save().await
    }

    pub async fn validation_errors(&mut self, value: ValidationErrors) -> Result<(), ErrResponse> {
        let r = value
            .field_errors()
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    v.iter()
                        .map(|v: &ValidationError| {
                            v.message
                                .to_owned()
                                .unwrap_or_else(|| "invalid".into())
                                .to_string()
                        })
                        .collect(),
                )
            })
            .collect::<HashMap<&str, Vec<String>>>();

        self.errors(r).await
    }

    pub async fn get_errors(&mut self) -> Result<HashMap<String, Vec<String>>, ErrResponse> {
        Ok(self.session.get(ERRORS_KEY).unwrap_or_default())
    }

    pub async fn old<T: Serialize>(&mut self, old: T) -> Result<(), ErrResponse> {
        self.session.insert(OLD_KEY, old)?;
        self.session.insert(OLD_KEY_TRACKER, true)?;
        self.clone().save().await
    }

    pub fn get_old(&mut self) -> Result<HashMap<String, String>, ErrResponse> {
        let old: HashMap<String, Value> = match self.session.get(OLD_KEY) {
            Some(s) => s,
            None => return Ok(Default::default()),
        };
        let old: HashMap<String, String> = old
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    // if we do to_string directly, strings get "" around them
                    if let Value::String(s) = v {
                        s
                    } else {
                        v.to_string()
                    },
                )
            })
            .collect();
        Ok(old)
    }
}

async fn get_session_from_cookie(store: &DbSessionStore, cookie: Option<String>) -> Session {
    if let Some(c) = cookie {
        match store.load_session(c.clone()).await {
            Ok(Some(mut session)) => {
                session.set_cookie_value(c);
                return session;
            }
            Ok(None) => {}
            Err(err) => tracing::error!("error getting session {err:?}"),
        }
    }

    tracing::trace!("no session found, creating new");

    Session::new()
}

pub async fn session_middleware<B>(mut req: Request<B>, next: Next<B>) -> impl IntoResponse {
    tracing::trace!("starting request");
    let store = req
        .extensions()
        .get::<DbSessionStore>()
        .expect("`DbSessionStore` extension missing")
        .clone();

    let session_cookie = req
        .headers()
        .typed_get::<Cookie>()
        .as_ref()
        .and_then(|c| c.get(SESSION_COOKIE_NAME))
        .map(|a| a.to_string());

    // get the session and add it as a req extension
    let session = get_session_from_cookie(&store, session_cookie).await;
    let mut user_session = UserSession { session, store };
    req.extensions_mut().insert(user_session.clone());

    // keep going and get the response
    let mut res = next.run(req).await;

    // to make sure we don't delete the flash and errors we just set, we check the tracker
    let hard_coded_keys = [
        (FLASH_KEY, FLASH_KEY_TRACKER),
        (ERRORS_KEY, ERRORS_KEY_TRACKER),
        (OLD_KEY, OLD_KEY_TRACKER),
    ];
    for (k, t) in hard_coded_keys {
        if user_session.session.get(t) != Some(true) {
            user_session.session.remove(k);
        }
        user_session.session.remove(t);
    }

    // consume the session and get the cookie value
    let cookie = user_session.save_and_get_cookie().await?;

    res.headers_mut().insert(SET_COOKIE, cookie);
    tracing::trace!("ending request");
    Ok::<_, ErrResponse>(res)
}
