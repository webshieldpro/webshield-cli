use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::t;
use clap::Args;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Args, Clone)]
pub struct ProxyInfo {
    #[arg(long, help = t!(arg_proxy_mode))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[arg(long, help = t!(arg_proxy_redirect_target))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_target: Option<String>,
    #[arg(long, help = t!(arg_proxy_ssl))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl: Option<bool>,
    #[arg(long, help = t!(arg_proxy_bot_protection))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot_protection: Option<bool>,
    #[arg(long, help = t!(arg_proxy_captcha))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captcha: Option<bool>,
    #[arg(long, help = t!(arg_proxy_http2))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http2: Option<bool>,
    #[arg(long, help = t!(arg_proxy_http3))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http3: Option<bool>,
    #[arg(long, help = t!(arg_proxy_max_body))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_mb: Option<i64>,
    #[arg(long, help = t!(arg_proxy_block_bots))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_bots: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct ProxyDecl {
    pub hostname: String,
    pub domain_id: i64,
    #[serde(flatten)]
    pub inner: ProxyInfo,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ProxyData {
    /// Config id — the `/nginx-configs/{id}` PATCH/DELETE key (NOT domain_id).
    pub id: i64,
    pub hostname: String,
    pub domain_name: String,
    #[serde(flatten)]
    pub inner: ProxyInfo,
    pub domain_id: i64,
}

impl DisplayTable for ProxyData {
    fn headers(&self) -> Vec<&'static str> {
        Proxies::from(vec![]).headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        Proxies::from(vec![ProxyData::clone(self)]).rows()
    }
}

pub struct ProxyPatch;

impl RequestDesc for ProxyPatch {
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

impl RequestDesc for ProxyNew {
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
impl RequestDesc for ProxyDelete {
    type Params = i64;
    type Request = ();
    // The body (204 or a JSON payload) is irrelevant — only the status matters.
    type Response = serde::de::IgnoredAny;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("nginx-configs/{}", id)
    }

    fn method() -> Method {
        Method::DELETE
    }
}

#[derive(Serialize, Deserialize)]
pub struct Proxies {
    pub results: Vec<ProxyData>,
}

impl From<Vec<ProxyData>> for Proxies {
    fn from(configs: Vec<ProxyData>) -> Self {
        Self { results: configs }
    }
}

impl DisplayTable for Proxies {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            t!(h_host),
            t!(h_domain),
            t!(h_mode),
            t!(h_target),
            t!(h_ssl),
            t!(h_bot_prot),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let yes = t!(yes);
        self.results
            .iter()
            .map(|c| {
                vec![
                    c.hostname.clone(),
                    c.domain_name.clone(),
                    c.inner.mode.clone().unwrap_or_default(),
                    c.inner.redirect_target.clone().unwrap_or_default(),
                    if c.inner.ssl.unwrap_or(false) {
                        yes.into()
                    } else {
                        String::new()
                    },
                    if c.inner.bot_protection.unwrap_or(false) {
                        yes.into()
                    } else {
                        String::new()
                    },
                ]
            })
            .collect()
    }
}

pub struct ProxyResolve;

impl RequestDesc for ProxyResolve {
    type Params = String;
    type Request = ();
    type Response = Proxies;

    fn get_url(hostname: Self::Params) -> impl AsRef<str> {
        format!("nginx-configs?hostname={}", hostname)
    }
    fn method() -> Method {
        Method::GET
    }
}

pub struct Proxy;

impl RequestDesc for Proxy {
    type Params = u32;
    type Request = ();
    type Response = Proxies;

    fn get_url(page: Self::Params) -> impl AsRef<str> {
        format!("nginx-configs?page={}", page)
    }
    fn method() -> Method {
        Method::GET
    }
}
