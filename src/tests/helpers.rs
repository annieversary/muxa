use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http::{response::Parts, uri::Uri};
use tower::ServiceExt; // for `app.oneshot()`

pub fn get<T, B>(uri: T, body: B) -> Request<B>
where
    Uri: TryFrom<T>,
    <Uri as TryFrom<T>>::Error: Into<http::Error>,
{
    Request::builder().uri(uri).body(body).unwrap()
}

pub fn empty_get<T>(uri: T) -> Request<Body>
where
    Uri: TryFrom<T>,
    <Uri as TryFrom<T>>::Error: Into<http::Error>,
{
    get(uri, Body::empty())
}

#[allow(dead_code)]
pub fn post<T, B>(uri: T, body: B) -> Request<B>
where
    Uri: TryFrom<T>,
    <Uri as TryFrom<T>>::Error: Into<http::Error>,
{
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(body)
        .unwrap()
}

pub struct TestResponse {
    pub parts: Parts,
    pub bytes: bytes::Bytes,
}

impl TestResponse {
    #[allow(dead_code)]
    pub fn status(&self) -> StatusCode {
        self.parts.status
    }

    pub fn is_ok(&self) -> bool {
        self.parts.status == StatusCode::OK
    }

    pub fn contains_str(&self, s: &str) -> bool {
        let output = std::str::from_utf8(&self.bytes).unwrap();
        output.contains(s)
    }

    pub fn doesnt_contain_str(&self, s: &str) -> bool {
        !self.contains_str(s)
    }
}

pub trait RouterExt {
    fn req(self, req: Request<Body>) -> impl std::future::Future<Output = TestResponse> + Send;
}
impl RouterExt for Router {
    async fn req(self, req: Request<Body>) -> TestResponse {
        let (parts, body) = self.oneshot(req).await.unwrap().into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        TestResponse { parts, bytes }
    }
}
