use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::i18n;
use crate::i18n::M;
use clap::Args;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Args, Clone)]
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
    pub inner: ProxyInfo,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ProxyData {
    pub hostname: String,
    pub domain_name: String,
    #[serde(flatten)]
    pub inner: ProxyInfo,
    pub domain_id: i64,
}

impl DisplayTable for ProxyData {
    fn headers(&self) -> Vec<&'static str> {
        Proxies(vec![]).headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        Proxies(vec![ProxyData::clone(self)]).rows()
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
    type Response = ();

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("nginx-configs/{}", id)
    }

    fn method() -> Method {
        Method::DELETE
    }
}

#[derive(Deserialize, Serialize)]
pub struct Proxies(Vec<ProxyData>);

impl Proxies {
    pub fn into_inner(self) -> Vec<ProxyData> {
        self.0
    }
}

impl DisplayTable for Proxies {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            i18n::tr(M::HHost),
            i18n::tr(M::HDomain),
            i18n::tr(M::HMode),
            i18n::tr(M::HTarget),
            i18n::tr(M::HSsl),
            i18n::tr(M::HBotProt),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let yes = i18n::tr(M::Yes);
        self.0
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
    type Params = ();
    type Request = ();
    type Response = Proxies;

    fn get_url(_: ()) -> impl AsRef<str> {
        "nginx-configs"
    }

    fn method() -> Method {
        Method::GET
    }
}
