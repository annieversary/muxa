use crate::{config::Config, errors::*, sessions::UserSession};
use axum::{
    extract::{FromRequest, Query, RequestParts},
    http::Request,
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use maud::{html, Markup, DOCTYPE};
use std::{collections::HashMap, fmt::Debug, marker::PhantomData};

#[derive(Clone)]
pub struct HtmlContextBuilder<T> {
    query: HashMap<String, String>,
    pub session_flash: Option<String>,
    config: Config,
    inner: T,
}

pub struct HtmlMiddleware<B, T>(PhantomData<(B, T)>);

impl<B, T> HtmlMiddleware<B, T>
where
    B: Send,
    T: FromRequest<B> + Send + Sync + 'static,
{
    pub async fn html_context_middleware(req: Request<B>, next: Next<B>) -> impl IntoResponse {
        // extractors need a RequestParts
        let mut req = RequestParts::new(req);

        let Query(query) = Query::<HashMap<String, String>>::from_request(&mut req)
            .await
            .unwrap();
        let session_flash = req.extensions().get::<UserSession>().unwrap().get_flash();
        let config = req.extensions().get::<Config>().unwrap().clone();
        let inner = T::from_request(&mut req)
            .await
            .ok()
            .expect("inner to be available in the request");

        let mut req = req.try_into_request()?;

        req.extensions_mut().insert(HtmlContextBuilder {
            query,
            session_flash,
            config,
            inner,
        });

        let res = next.run(req).await;

        Ok::<_, ErrResponse>(res)
    }
}

impl<T> HtmlContextBuilder<T> {
    pub fn build(self, content: Markup) -> HtmlContext<T> {
        HtmlContext {
            content,
            query: self.query,
            session_flash: self.session_flash,
            config: self.config,
            title: None,
            description: None,
            image: None,

            inner: self.inner,
        }
    }
}

#[derive(Debug)]
pub struct HtmlContext<T> {
    pub content: Markup,
    pub query: HashMap<String, String>,
    pub session_flash: Option<String>,
    pub config: Config,

    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,

    pub inner: T,
}

impl<T> HtmlContext<T> {
    /// sets the title for this page
    /// will be `Config::app_name` by default
    pub fn with_title(mut self, s: impl ToString) -> Self {
        self.title = Some(s.to_string());
        self
    }
    pub fn get_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.config.get_app_name())
    }

    /// sets the description for this page
    pub fn with_description(mut self, s: impl ToString) -> Self {
        self.description = Some(s.to_string());
        self
    }
    pub fn get_description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[allow(dead_code)]
    /// sets the image for this page
    /// `s` is a relative path, will be
    pub fn with_image(mut self, s: impl ToString) -> Self {
        self.image = Some(s.to_string());
        self
    }
    /// sets the image for this page
    /// `s` is a relative path, will be
    pub fn with_optional_image(mut self, s: Option<impl ToString>) -> Self {
        self.image = s.map(|s| s.to_string());
        self
    }
    /// returns absolute path to image
    pub fn get_image(&self) -> Option<String> {
        self.image
            .as_deref()
            .map(|s| self.config.absolute_uploaded_url(s))
    }
}

impl<T: Template> IntoResponse for HtmlContext<T> {
    fn into_response(self) -> Response {
        let m = T::base(self);
        Html(m.into_string()).into_response()
    }
}

pub trait Template
where
    Self: Sized,
{
    fn base(ctx: HtmlContext<Self>) -> Markup {
        html! {
            (DOCTYPE)
            head {
              (Self::head(&ctx))
            }
            body {
              (Self::body(&ctx))
            }
        }
    }

    fn head(ctx: &HtmlContext<Self>) -> Markup;
    fn body(ctx: &HtmlContext<Self>) -> Markup;
}
