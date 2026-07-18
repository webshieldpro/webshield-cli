//! HTTP client for the `/api/v1` API. Authentication — Bearer with a personal `wsk_…` token.
//! DRF errors (`{"detail": …}` or a field error map) are unwrapped into readable text.

use super::_models::Page;
use crate::api::error::check_status;
use crate::api::request_desc::{ListRequestDesc, RequestDesc};
use crate::i18n::{self, M};
use anyhow::{Context, Result};
use reqwest::multipart::Form;
use reqwest::{Method, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl Client {
    pub fn new(api_url: &str, token: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("webshield-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build the HTTP client")?;
        Ok(Self {
            http,
            base: api_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}/api/v1/{}", self.base, path.trim_start_matches('/'))
    }

    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, self.url(path))
            .bearer_auth(&self.token)
    }

    fn n_make_request<R: RequestDesc>(&self, params: R::Params) -> RequestBuilder {
        self.request(R::method(), R::get_url(params).as_ref())
    }

    pub async fn n_send<R: RequestDesc<Request = ()>>(
        &self,
        params: R::Params,
    ) -> Result<R::Response> {
        self.send_json(self.n_make_request::<R>(params)).await
    }

    pub async fn n_send_multipart<R: RequestDesc<Request = Form>>(
        &self,
        params: R::Params,
        form: Form,
    ) -> Result<R::Response> {
        self.send_json(self.n_make_request::<R>(params).multipart(form))
            .await
    }

    pub async fn n_send_ser<R: RequestDesc>(
        &self,
        req: R::Request,
        params: R::Params,
    ) -> Result<R::Response>
    where
        R::Request: Serialize,
    {
        self.send_json(self.n_make_request::<R>(params).json(&req))
            .await
    }

    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.send_json(self.request(Method::POST, path).json(body))
            .await
    }

    /// Fetches every page of a GET list endpoint (follows `next`).
    /// Tolerates both the `{results:[…]}` envelope and a bare array.
    pub async fn n_list<R: ListRequestDesc>(&self, params: R::Params) -> Result<Vec<R::Item>> {
        let mut out = Vec::new();
        let mut next_url = self.url(R::get_url(params).as_ref());
        loop {
            let rb = self
                .http
                .request(Method::GET, &next_url)
                .bearer_auth(&self.token);
            let value = self.send_value(rb).await?;
            println!("{:?}", value);
            match value {
                Value::Array(_) => {
                    let items: Vec<R::Item> =
                        serde_json::from_value(value).context(i18n::tr(M::ErrParse))?;
                    out.extend(items);
                    break;
                }
                Value::Object(_) => {
                    let page: Page<R::Item> =
                        serde_json::from_value(value).context(i18n::tr(M::ErrParse))?;
                    out.extend(page.results);
                    match page.next {
                        Some(url) if !url.is_empty() => next_url = url,
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        Ok(out)
    }

    /// Sends a request and deserializes the JSON body. Empty body (204) → `null`.
    async fn send_json<T: DeserializeOwned>(&self, rb: RequestBuilder) -> Result<T> {
        let value = self.send_value(rb).await?;
        serde_json::from_value(value).context(i18n::tr(M::ErrParse))
    }

    /// Sends a request and returns raw JSON (or `Null` for an empty body).
    async fn send_value(&self, rb: RequestBuilder) -> Result<Value> {
        let resp = rb.send().await.context(i18n::tr(M::ErrNetwork))?;
        let resp = check_status(resp).await?;
        let text = resp.text().await.context(i18n::tr(M::ErrReadBody))?;
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context(i18n::tr(M::ErrParse))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::error::HttpError;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(base: &str) -> Client {
        Client::new(base, "wsk_test").unwrap()
    }

    struct Things;
    impl ListRequestDesc for Things {
        type Params = ();
        type Item = Value;

        fn get_url(_: ()) -> impl AsRef<str> {
            "things"
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
    async fn n_list_follows_pagination() {
        let server = MockServer::start().await;
        let page2_url = format!("{}/api/v1/things?page=2", server.uri());
        Mock::given(method("GET"))
            .and(path("/api/v1/things"))
            .and(query_param_is_missing("page"))
            .and(header("authorization", "Bearer wsk_test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "next": page2_url,
                "results": [{"v": 1}, {"v": 2}],
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/things"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "next": null,
                "results": [{"v": 3}],
            })))
            .expect(1)
            .mount(&server)
            .await;

        let items = client(&server.uri()).n_list::<Things>(()).await.unwrap();
        let vals: Vec<i64> = items.iter().map(|i| i["v"].as_i64().unwrap()).collect();
        assert_eq!(vals, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn n_list_accepts_bare_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/things"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"v": 1}])))
            .mount(&server)
            .await;

        let items = client(&server.uri()).n_list::<Things>(()).await.unwrap();
        assert_eq!(items.len(), 1);
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
            .n_send::<ThingDisable>(())
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
            .n_send::<ThingDisable>(())
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

        let err = client(&server.uri())
            .n_list::<Things>(())
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("401 Unauthorized"), "got: {msg}");
        assert!(msg.contains("Invalid token."), "got: {msg}");
        // `auth status` relies on recovering the code from the error chain.
        let http = err.downcast_ref::<HttpError>().expect("HttpError in chain");
        assert_eq!(http.status.as_u16(), 401);
    }

    #[tokio::test]
    async fn error_400_joins_serializer_field_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/domains"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({"name": ["This field is required."]})),
            )
            .mount(&server)
            .await;

        let err = client(&server.uri())
            .post_json::<_, Value>("domains", &json!({}))
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("400"), "got: {msg}");
        assert!(msg.contains("name: This field is required."), "got: {msg}");
    }
}
