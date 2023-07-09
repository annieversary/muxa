use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Config(Arc<ConfigInner>);

#[derive(Clone, Debug)]
struct ConfigInner {
    upload_path: PathBuf,
    static_path: PathBuf,
    base_url: String,

    upload_route: String,

    app_name: String,
}

impl Config {
    /// Get a reference to the config inner's upload path.
    #[must_use]
    pub fn get_upload_path(&self) -> &PathBuf {
        &self.0.upload_path
    }

    #[must_use]
    pub fn get_upload_route(&self) -> &str {
        self.0.upload_route.as_ref()
    }

    /// Get a reference to the config inner's base url.
    #[must_use]
    pub fn get_base_url(&self) -> &str {
        self.0.base_url.as_ref()
    }

    /// Get a reference to the config inner's static path.
    #[must_use]
    pub fn get_static_path(&self) -> &PathBuf {
        &self.0.static_path
    }

    /// Get a reference to the config inner's app name.
    #[must_use]
    pub fn get_app_name(&self) -> &str {
        self.0.app_name.as_ref()
    }

    pub fn new(
        upload_path: PathBuf,
        static_path: PathBuf,
        base_url: String,
        app_name: String,
        upload_route: String,
    ) -> Self {
        Self(Arc::new(ConfigInner {
            upload_path,
            static_path,
            base_url,
            upload_route,
            app_name,
        }))
    }

    /// Panics if env variables are not set
    pub fn from_env() -> Self {
        let upload_path = std::env::var("UPLOAD_PATH")
            .expect("failed to get UPLOAD_PATH")
            .into();
        let upload_route = std::env::var("UPLOAD_ROUTE").unwrap_or_else(|_| "/upload".to_string());
        let static_path = std::env::var("STATIC_PATH")
            .expect("failed to get STATIC_PATH")
            .into();
        let base_url = std::env::var("BASE_URL").expect("failed to get BASE_URL");
        let app_name = std::env::var("APP_NAME").expect("failed to get APP_NAME");
        Config(Arc::new(ConfigInner {
            upload_path,
            static_path,
            base_url,
            upload_route,
            app_name,
        }))
    }

    pub fn upload_path(&self, p: impl AsRef<Path>) -> PathBuf {
        let mut a = self.0.upload_path.clone();
        a.push(p.as_ref());
        a
    }

    pub fn get_random_folder(&self) -> Result<PathBuf, crate::errors::ErrResponse> {
        let mut temp_path = self.0.upload_path.clone();
        let uuid = uuid::Uuid::new_v4();
        temp_path.push(uuid.to_string());

        std::fs::create_dir_all(&temp_path)?;

        Ok(temp_path)
    }

    /// returns a relative url to a file
    /// eg: `/uploaded/aaaaaaaaaa/image.png`
    pub fn uploaded_url(&self, s: &str) -> String {
        format!("{}/{s}", self.0.upload_route)
    }

    /// returns absolute url to a file
    /// eg: `https://example.com/uploaded/aaaaaaaaaa/image.png`
    pub fn absolute_uploaded_url(&self, s: &str) -> String {
        self.absolute_url(&self.uploaded_url(s))
    }
    pub fn absolute_url(&self, s: &str) -> String {
        format!("{}{}", self.0.base_url, s)
    }
}
