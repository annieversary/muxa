use axum::{http::StatusCode, routing::get_service, Router};
use tower_http::services::ServeDir;

use crate::config::Config;

pub trait RouterExtension {
    fn static_dir(self, config: &Config, folder_name: &str) -> Self;
    fn static_dirs(self, config: &Config, folders: &[&str]) -> Self;
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
            .layer(Extension($pool.clone()))
            .layer(Extension($config))
            $(
                .layer(Extension($ext))
            )*
            .layer(Extension(
                muxa::sessions::DbSessionStore::new($pool).with_same_site(muxa::cookies::SameSite::Lax),
            ))
            .layer(middleware::from_fn(muxa::sessions::session_middleware))
            .layer(middleware::from_fn(
              <$builder as muxa::html::AssociatedMiddleware<_>>::Middleware::html_context_middleware,
            ))
    };
}
