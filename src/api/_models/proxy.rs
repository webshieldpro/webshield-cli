use clap::Args;
use reqwest::Method;
use serde::Serialize;
use serde_json::Value;
use crate::api::get_url::MakeReq;

#[derive(Serialize, Args)]
pub struct ProxyInfo {
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_target: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl: Option<bool>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot_protection: Option<bool>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captcha: Option<bool>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http2: Option<bool>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http3: Option<bool>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_mb: Option<i64>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_bots: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct ProxyDecl {
    pub hostname: String,
    pub domain_id: i64,
    #[serde(flatten)]
    pub inner: ProxyInfo
}

pub struct ProxyPatch;

impl MakeReq for ProxyPatch {
    type Params = i64;
    type Request = ProxyInfo;
    type Response = Value;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("nginx-configs/{}", id)
    }

    fn method() -> Method {
        Method::PATCH
    }
}

pub struct ProxyNew;

impl MakeReq for ProxyNew {
    type Params = ();
    type Request = ProxyDecl;
    type Response = Value;

    fn get_url(_: ()) -> impl AsRef<str> {
        "nginx-configs"
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct ProxyDelete;
impl MakeReq for ProxyDelete {
    type Params = i64;
    type Request = ();
    type Response = ();

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("nginx-configs/{}", id)
    }

    fn method() -> Method {
        Method::DELETE
    }
}