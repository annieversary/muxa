use backtrace::Backtrace;
use std::panic::Location;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub struct ErrResponse {
    status_code: StatusCode,
    message: String,
    location: &'static Location<'static>,
    backtrace: Backtrace,
}

impl ErrResponse {
    #[track_caller]
    pub fn new(status_code: StatusCode, message: String) -> Self {
        Self {
            status_code,
            message,
            location: Location::caller(),
            backtrace: Backtrace::new(),
        }
    }
}

impl<E> From<E> for ErrResponse
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[track_caller]
    fn from(err: E) -> Self {
        internal_error(err)
    }
}

#[track_caller]
pub fn internal_error<E>(err: E) -> ErrResponse
where
    E: std::error::Error,
{
    ErrResponse::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

impl IntoResponse for ErrResponse {
    fn into_response(self) -> Response {
        if self.status_code == StatusCode::NOT_FOUND {
            return (StatusCode::NOT_FOUND, "page not found").into_response();
        }

        tracing::error!(
            message = %self.message,
            error.file = self.location.file(),
            error.line = self.location.line(),
            error.col = self.location.column(),
            error.backtrace = ?self.backtrace,
        );

        let s = if cfg!(debug_assertions) {
            format!(
                "error: {}\n\n{}, line {}, col {}\n\n{:?}",
                self.message,
                self.location.file(),
                self.location.line(),
                self.location.column(),
                self.backtrace,
            )
        } else {
            self.message
        };

        (self.status_code, s).into_response()
    }
}

/// sets up panic hook
pub fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|panic| {
        let b = Backtrace::new();
        if let Some(location) = panic.location() {
            tracing::error!(
              message = %panic,
              panic.file = location.file(),
              panic.line = location.line(),
              panic.column = location.column(),
              backtrace = ?b,
            );
        } else {
            tracing::error!(message = %panic, backtrace = ?b);
        }
    }));
}
