//! HTTP client for the `/api/v1` API. Authentication — Bearer with a personal `wsk_…` token.
//! DRF errors (`{"detail": …}` or a field error map) are unwrapped into readable text.

use super::models::Page;
use crate::api::request_desc::RequestDesc;
use crate::i18n::{self, M};
use anyhow::{Context, Result};
use reqwest::multipart::Form;
use reqwest::{Method, RequestBuilder, Result as ReqResult};
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
    async fn send_json<T: DeserializeOwned>(&self, rb: RequestBuilder) -> Result<T> {
        let value = self.send_value(rb).await?;
        serde_json::from_value(value).context(i18n::tr(M::ErrParse))
    }

    fn n_make_request<R: RequestDesc>(&self, params: R::Params) -> RequestBuilder {
        self.request(R::method(), R::get_url(params).as_ref())
    }

    pub async fn n_send<R: RequestDesc<Request = ()>>(
        &self,
        params: R::Params,
    ) -> ReqResult<R::Response> {
        self.n_send_req(self.n_make_request::<R>(params)).await
    }

    pub async fn n_send_multipart<R: RequestDesc<Request = Form>>(
        &self,
        params: R::Params,
        form: Form,
    ) -> ReqResult<R::Response> {
        self.n_send_req(self.n_make_request::<R>(params).multipart(form))
            .await
    }

    pub async fn n_send_ser<R: RequestDesc>(
        &self,
        req: R::Request,
        params: R::Params,
    ) -> ReqResult<R::Response>
    where
        R::Request: Serialize,
    {
        self.n_send_req(self.n_make_request::<R>(params).json(&req))
            .await
    }

    async fn n_send_req<D: DeserializeOwned>(&self, rb: RequestBuilder) -> ReqResult<D> {
        println!("DEBUG BUILDER {:?}", rb);
        let resp = rb.send().await?;

        resp.error_for_status_ref()?;

        resp.json::<D>().await
    }

    // async fn n_send_json<R: DeserializeOwned + RequestDesc, D: Serialize>(
    //     &self,
    //     dt: &D,
    //     params: R::Params,
    // ) -> Result<R> {
    //     self.send_json(self.n_make_request::<R>(params).json(dt)).await
    // }

    /// Sends a request and returns raw JSON (or `Null` for an empty body).
    async fn send_value(&self, rb: RequestBuilder) -> Result<Value> {
        println!("DEBUG BUILDER {:?}", rb);
        let resp = rb.send().await.context(i18n::tr(M::ErrNetwork))?;

        resp.error_for_status_ref()?;

        let text = resp.text().await.context(i18n::tr(M::ErrReadBody))?;
        println!("DEBUG RESPONSE: {}", text);
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context(i18n::tr(M::ErrParse))
    }

    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.send_json(self.request(Method::POST, path).json(body))
            .await
    }

    /// Fetches every page of a list endpoint (follows `next`).
    /// Tolerates both the `{results:[…]}` envelope and a bare array.
    // FIXME: what an awful API
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
