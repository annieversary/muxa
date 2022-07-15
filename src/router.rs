use axum::{http::StatusCode, routing::get_service, Router};
use tower_http::services::ServeDir;

use crate::config::Config;

pub trait RouterExtension {
    fn static_dir(self, config: &Config, folder_name: &str) -> Self;
    fn static_dirs(self, config: &Config, folders: &[&str]) -> Self;
    /// mount `Config::get_upload_path` at `path`
    fn upload_dir(self, config: &Config) -> Self;
}

impl RouterExtension for Router {
    fn static_dir(self, config: &Config, folder_name: &str) -> Self {
        let mut path = config.get_static_path().clone();
        path.push(folder_name);

        self.nest(
            &format!("/{folder_name}"),
            get_service(ServeDir::new(path)).handle_error(|error: std::io::Error| async move {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                )
            }),
        )
    }

    fn static_dirs(mut self, config: &Config, folders: &[&str]) -> Self {
        for folder in folders {
            self = self.static_dir(config, folder);
        }
        self
    }

    /// registers the upload directory at $UPLOAD_ROUTE
    /// or "/uploaded"
    fn upload_dir(self, config: &Config) -> Self {
        self.nest(
            config.get_upload_route(),
            get_service(ServeDir::new(&config.get_upload_path())).handle_error(
                |error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                },
            ),
        )
    }
}

/// adds the layers required by muxa, plus app-related extensions
#[macro_export]
macro_rules! default_layers {
    (
      builder: $builder:ident,
      pool: $pool:expr,
      config: $config:expr,
      extensions: [ $($ext:expr),* $(,)? ],
    ) => {
        tower::ServiceBuilder::new()
            .layer(tower_http::trace::TraceLayer::new_for_http())
            .layer(axum::extract::Extension($pool.clone()))
            .layer(axum::extract::Extension($config))
            $(
                .layer(axum::extract::Extension($ext))
            )*
            .layer(axum::extract::Extension(
                muxa::sessions::DbSessionStore::new($pool).with_same_site(muxa::cookies::SameSite::Lax),
            ))
            .layer(axum::middleware::from_fn(muxa::sessions::session_middleware))
            .layer(axum::middleware::from_fn(
              <$builder as muxa::html::AssociatedMiddleware<_>>::Middleware::html_context_middleware,
            ))
    };
}
