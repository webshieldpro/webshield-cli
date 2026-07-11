//! HTTP client for the `/api/v1` API. Authentication — Bearer with a personal `wsk_…` token.
//! DRF errors (`{"detail": …}` or a field error map) are unwrapped into readable text.

use super::models::Page;
use crate::api::get_url::MakeReq;
use crate::i18n::{self, M};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::{Method, RequestBuilder, Response, StatusCode};
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

    /// Sends a request and deserializes the JSON body. Empty body (204) → `null`.
    pub async fn send_json<T: DeserializeOwned>(&self, rb: RequestBuilder) -> Result<T> {
        let value = self.send_value(rb).await?;
        serde_json::from_value(value).context(i18n::tr(M::ErrParse))
    }

    fn n_make_request<T: DeserializeOwned + MakeReq>(&self, params: T::Params) -> RequestBuilder {
        self.request(T::method(), T::get_url(params).as_ref())
    }

    pub async fn n_send<T: DeserializeOwned + MakeReq>(&self, params: T::Params) -> Result<T> {
        self.send_json(self.n_make_request::<T>(params)).await
    }

    pub async fn n_send_json<T: DeserializeOwned + MakeReq, D: Serialize>(
        &self,
        dt: &D,
        params: T::Params,
    ) -> Result<T> {
        self.send_json(self.n_make_request::<T>(params).json(dt)).await
    }

    /// Sends a request and returns raw JSON (or `Null` for an empty body).
    pub async fn send_value(&self, rb: RequestBuilder) -> Result<Value> {
        println!("DEBUG BUILDER {:?}", rb);
        let resp = rb.send().await.context(i18n::tr(M::ErrNetwork))?;
        let resp = check_status(resp).await?;
        let text = resp.text().await.context(i18n::tr(M::ErrReadBody))?;
        println!("DEBUG RESPONSE: {}", text);
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context(i18n::tr(M::ErrParse))
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send_json(self.request(Method::GET, path)).await
    }

    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.send_json(self.request(Method::POST, path).json(body))
            .await
    }

    pub async fn post_empty(&self, path: &str) -> Result<Value> {
        self.send_value(self.request(Method::POST, path)).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value> {
        self.send_value(self.request(Method::DELETE, path)).await
    }

    /// Fetches every page of a list endpoint (follows `next`).
    /// Tolerates both the `{results:[…]}` envelope and a bare array.
    pub async fn list_all<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>> {
        let mut out = Vec::new();
        let mut next_url = self.url(path);
        loop {
            let rb = self
                .http
                .request(Method::GET, &next_url)
                .bearer_auth(&self.token);
            let value = self.send_value(rb).await?;
            match value {
                Value::Array(_) => {
                    let items: Vec<T> =
                        serde_json::from_value(value).context("failed to parse the list")?;
                    out.extend(items);
                    break;
                }
                Value::Object(_) => {
                    let page: Page<T> =
                        serde_json::from_value(value).context("failed to parse the list page")?;
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
}

/// Checks the HTTP status; on error extracts a readable message from the DRF body.
async fn check_status(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    let detail = extract_detail(&body).unwrap_or_else(|| body.clone());
    match status {
        StatusCode::UNAUTHORIZED => {
            bail!(
                "401 Unauthorized: {detail}\n{}",
                i18n::tr(M::ErrUnauthorized)
            )
        }
        StatusCode::FORBIDDEN => {
            bail!("403 Forbidden: {detail}\n{}", i18n::tr(M::ErrForbidden))
        }
        _ => Err(anyhow!(
            "{} {}: {detail}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("")
        )),
    }
}

/// Extracts `detail` or joins DRF serializer field errors into a single line.
fn extract_detail(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let obj = value.as_object()?;
    if let Some(detail) = obj.get("detail").and_then(Value::as_str) {
        return Some(detail.to_string());
    }
    // A {field: [messages]} map — join into "field: message".
    let mut parts = Vec::new();
    for (field, errs) in obj {
        let joined = match errs {
            Value::Array(items) => items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
                .join("; "),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        parts.push(format!("{field}: {joined}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(base: &str) -> Client {
        Client::new(base, "wsk_test").unwrap()
    }

    #[test]
    fn url_joins_base_and_path() {
        let c = client("https://example.com");
        assert_eq!(c.url("domains"), "https://example.com/api/v1/domains");
        // A trailing slash in the base and a leading slash in the path do not double up.
        let c = client("https://example.com/");
        assert_eq!(c.url("/domains"), "https://example.com/api/v1/domains");
    }

    #[test]
    fn extract_detail_reads_drf_detail() {
        assert_eq!(
            extract_detail(r#"{"detail": "Not found."}"#),
            Some("Not found.".to_string())
        );
    }

    #[test]
    fn extract_detail_joins_field_errors() {
        let got = extract_detail(r#"{"name": ["This field is required.", "Too short."]}"#);
        assert_eq!(
            got,
            Some("name: This field is required.; Too short.".to_string())
        );
    }

    #[test]
    fn extract_detail_ignores_non_json() {
        assert_eq!(extract_detail("<html>502</html>"), None);
        assert_eq!(extract_detail("{}"), None);
    }

    #[tokio::test]
    async fn list_all_follows_pagination() {
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

        let items: Vec<Value> = client(&server.uri()).list_all("things").await.unwrap();
        let vals: Vec<i64> = items.iter().map(|i| i["v"].as_i64().unwrap()).collect();
        assert_eq!(vals, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn list_all_accepts_bare_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/things"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"v": 1}])))
            .mount(&server)
            .await;

        let items: Vec<Value> = client(&server.uri()).list_all("things").await.unwrap();
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn send_value_maps_empty_body_to_null() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/things/1/disable"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let got = client(&server.uri())
            .post_empty("things/1/disable")
            .await
            .unwrap();
        assert_eq!(got, Value::Null);
    }

    #[tokio::test]
    async fn error_401_includes_detail_and_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/domains"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(json!({"detail": "Invalid token."})),
            )
            .mount(&server)
            .await;

        let err = client(&server.uri())
            .get_json::<Value>("domains")
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("401 Unauthorized"), "got: {msg}");
        assert!(msg.contains("Invalid token."), "got: {msg}");
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
