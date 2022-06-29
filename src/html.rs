use crate::{config::Config, errors::*, sessions::UserSession};
use axum::{
    extract::{FromRequest, Query, RequestParts},
    http::Request,
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use maud::{html, Markup, DOCTYPE};
use std::{collections::HashMap, convert::Infallible, fmt::Debug, marker::PhantomData};

/// gets inserted as an extension into the request by `HtmlMiddleware`
/// use the `build` method to provide it the html content
#[derive(Clone)]
pub struct HtmlContextBuilder<T, R> {
    query: HashMap<String, String>,
    pub session_flash: Option<String>,
    config: Config,
    route: R,
    inner: T,
}

pub struct HtmlMiddleware<B, T, R>(PhantomData<(B, T, R)>);

impl<B, T, R> HtmlMiddleware<B, T, R>
where
    B: Send,
    T: FromRequest<B> + Send + Sync + 'static,
    R: FromRequest<B> + Send + Sync + 'static,
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
        let route = R::from_request(&mut req)
            .await
            .ok()
            .expect("route to be available in the request");

        let mut req = req.try_into_request()?;

        req.extensions_mut().insert(HtmlContextBuilder {
            query,
            session_flash,
            config,
            route,
            inner,
        });

        let res = next.run(req).await;

        Ok::<_, ErrResponse>(res)
    }
}

impl<T, R> HtmlContextBuilder<T, R> {
    pub fn build(self, content: Markup) -> HtmlContext<T, R> {
        HtmlContext {
            content,
            query: self.query,
            session_flash: self.session_flash,
            config: self.config,
            route: self.route,

            title: None,
            description: None,
            image: None,

            sections: Default::default(),

            inner: self.inner,
        }
    }
}

#[derive(Debug)]
pub struct HtmlContext<T, R> {
    pub content: Markup,
    pub query: HashMap<String, String>,
    pub session_flash: Option<String>,
    pub config: Config,
    pub route: R,

    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,

    pub sections: HashMap<String, Vec<Markup>>,

    pub inner: T,
}

impl<T, R> HtmlContext<T, R> {
    /// sets the title for this page
    /// will be `Config::app_name` by default
    pub fn with_title(mut self, s: impl ToString) -> Self {
        self.title = Some(s.to_string());
        self
    }
    pub fn get_title(&self) -> &str {
        self.title
            .as_deref()
            .unwrap_or_else(|| self.config.get_app_name())
    }

    /// sets the description for this page
    pub fn with_description(mut self, s: impl ToString) -> Self {
        self.description = Some(s.to_string());
        self
    }
    pub fn get_description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// change the route that was automatically detected
    /// useful in cases when automatic detection messes up
    pub fn with_route(mut self, s: R) -> Self {
        self.route = s;
        self
    }

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

    /// add a piece of markdown to a section, similar to laravel's `@push`
    /// usually used for adding `script`s at the bottom of the page
    pub fn section_append(mut self, key: impl ToString, m: Markup) -> Self {
        let section = self.sections.entry(key.to_string()).or_default();
        section.push(m);
        self
    }

    pub fn section_get(&self, key: &str) -> Markup {
        let section: &[Markup] = self
            .sections
            .get(key)
            .map(AsRef::as_ref)
            .unwrap_or_default();
        html! {
            @for i in section {(*i)}
        }
    }
}

impl<T: Template<R>, R> IntoResponse for HtmlContext<T, R> {
    fn into_response(self) -> Response {
        let m = T::base(self);
        Html(m.into_string()).into_response()
    }
}

pub trait Template<R>
where
    Self: Sized,
{
    fn base(ctx: HtmlContext<Self, R>) -> Markup {
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

    fn head(ctx: &HtmlContext<Self, R>) -> Markup;
    fn body(ctx: &HtmlContext<Self, R>) -> Markup;
}

/// for when there is no `NamedRoute` or it isn't wanted
/// implements `FromRequest` so it can be used in `HtmlContext` and `HtmlContextBuilder`
pub struct NoRoute;
#[axum::async_trait]
impl<B> FromRequest<B> for NoRoute
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        Ok(NoRoute)
    }
}
