use axum::{
    async_trait,
    extract::FromRequestParts,
    headers::{Cookie, HeaderMapExt},
};
use http::request::Parts;
use std::convert::Infallible;

// TODO we could add something to html, but making it optional would be a big pain in the ass
// cause there's generics, so trait bounds would give problems

/// Extracts the selected theme from the cookie
/// Will be set to T::default if the cookie is not set or the value is not correct
#[derive(Debug, Clone, Copy)]
pub struct ThemeCookie<T: Default>(pub T);

pub trait ThemeTrait {
    fn css_url(&self) -> &'static str;
    fn from_str(s: &str) -> Option<Self>
    where
        Self: Sized;
}

#[async_trait]
impl<S, T: Default + ThemeTrait> FromRequestParts<S> for ThemeCookie<T>
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(req: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let cookie = req.headers.typed_get::<Cookie>();

        let theme = match cookie {
            Some(cookie) => cookie
                .get("theme")
                .and_then(T::from_str)
                .unwrap_or_default(),
            _ => T::default(),
        };

        Ok(ThemeCookie(theme))
    }
}
