//! HTTP client for the `/api/v1` API. Authentication — Bearer with a personal `wsk_…` token.
//! DRF errors (`{"detail": …}` or a field error map) are unwrapped into readable text.

use crate::api::error::check_status;
use crate::api::request_desc::RequestDesc;
use crate::t;
use anyhow::{Context, Result};
use reqwest::multipart::Form;
use reqwest::{IntoUrl, Method, RequestBuilder};
use serde::Serialize;
use std::borrow::Cow;

pub struct Client<'s> {
    http: reqwest::Client,
    base: Cow<'s, str>,
    token: Cow<'s, str>,
}

impl<'s> Client<'s> {
    pub fn new(mut api_url: &str, token: impl Into<Cow<'s, str>>) -> Result<Self> {
        let http = Self::build_reqwest_client()?;
        while api_url.ends_with('/') {
            api_url = &api_url[..api_url.len() - 1];
        }
        Ok(Self {
            http,
            base: format!("{}/api/v1", api_url).into(),
            token: token.into(),
        })
    }

    fn build_reqwest_client() -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .user_agent(concat!("webshield-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build the HTTP client")
    }

    fn url(&self, path: impl AsRef<str>) -> String {
        format!("{}/{}", self.base, path.as_ref().trim_start_matches('/'))
    }

    fn request(&self, method: Method, full_path: impl IntoUrl) -> RequestBuilder {
        self.http
            .request(method, full_path)
            .bearer_auth(&self.token)
    }

    fn make_request<R: RequestDesc>(&self, params: R::Params) -> RequestBuilder {
        self.request(R::method(), self.url(R::get_url(params)))
    }

    async fn send_data<R: RequestDesc>(&self, rb: RequestBuilder) -> Result<R::Response> {
        let resp = rb.send().await.context(t!(err_network))?;
        let resp = check_status(resp).await?;

        // A bit of a hack
        let dt: R::Response = {
            let full = resp.bytes().await.context(t!(err_network))?;

            if full.is_empty() {
                serde_json::from_slice(b"null")
            } else {
                serde_json::from_slice(&full)
            }
        }
        .context(t!(err_parse))?;

        Ok(dt)
    }

    pub async fn send<R: RequestDesc<Request = ()>>(
        &self,
        params: R::Params,
    ) -> Result<R::Response> {
        self.send_data::<R>(self.make_request::<R>(params)).await
    }

    pub async fn send_multipart<R: RequestDesc<Request = Form>>(
        &self,
        params: R::Params,
        form: Form,
    ) -> Result<R::Response> {
        self.send_data::<R>(self.make_request::<R>(params).multipart(form))
            .await
    }

    pub async fn send_json<R: RequestDesc>(
        &self,
        req: R::Request,
        params: R::Params,
    ) -> Result<R::Response>
    where
        R::Request: Serialize,
    {
        self.send_data::<R>(self.make_request::<R>(params).json(&req))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::error::HttpError;
    use serde_json::{json, Value};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(base: &str) -> Client<'_> {
        Client::new(base, "wsk_test").unwrap()
    }

    struct Things;
    impl RequestDesc for Things {
        type Params = ();
        type Request = ();
        type Response = Value;

        fn get_url(_: ()) -> impl AsRef<str> {
            "things"
        }

        fn method() -> Method {
            Method::GET
        }
    }

    struct ThingDisable;
    impl RequestDesc for ThingDisable {
        type Params = ();
        type Request = ();
        type Response = serde::de::IgnoredAny;

        fn get_url(_: ()) -> impl AsRef<str> {
            "things/1/disable"
        }

        fn method() -> Method {
            Method::POST
        }
    }

    #[test]
    fn url_joins_base_and_path() {
        let c = client("https://example.com");
        assert_eq!(c.url("domains"), "https://example.com/api/v1/domains");
        // A trailing slash in the base and a leading slash in the path do not double up.
        let c = client("https://example.com/");
        assert_eq!(c.url("/domains"), "https://example.com/api/v1/domains");
    }

    #[tokio::test]
    async fn n_send_tolerates_empty_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/things/1/disable"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        client(&server.uri())
            .send::<ThingDisable>(())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn n_send_ignores_unneeded_json_body() {
        // Endpoints whose body the CLI discards (upload/publish/…) must accept any JSON.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/things/1/disable"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
            .mount(&server)
            .await;

        client(&server.uri())
            .send::<ThingDisable>(())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn error_401_includes_detail_and_hint_and_downcasts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/things"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(json!({"detail": "Invalid token."})),
            )
            .mount(&server)
            .await;

        let err = client(&server.uri()).send::<Things>(()).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("401 Unauthorized"), "got: {msg}");
        assert!(msg.contains("Invalid token."), "got: {msg}");
        // `auth status` relies on recovering the code from the error chain.
        let http = err.downcast_ref::<HttpError>().expect("HttpError in chain");
        assert_eq!(http.status.as_u16(), 401);
    }
}
