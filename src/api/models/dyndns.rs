//! Dynamic DNS hostnames (`/dyndns/hosts`, scope `dns`).
//!
//! The update token itself is returned exactly once — when the hostname is created
//! and when the token is rotated. Afterwards only its prefix is available, so the
//! value has to be stored by the caller right away.

use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::t;
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct DynDnsHost {
    pub id: i64,
    pub hostname: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ttl: i64,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub token_prefix: String,
    #[serde(default)]
    pub last_ipv4: Option<String>,
    #[serde(default)]
    pub last_ipv6: Option<String>,
    #[serde(default)]
    pub last_update_at: Option<String>,
    #[serde(default)]
    pub updates_count: i64,
    /// Present only in the response that issued it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

fn host_headers() -> Vec<&'static str> {
    vec![
        t!(h_id),
        t!(h_host),
        t!(h_address),
        t!(h_ttl),
        t!(h_last_seen),
    ]
}

impl DynDnsHost {
    fn row(&self) -> Vec<String> {
        let addresses = [self.last_ipv4.as_deref(), self.last_ipv6.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
        vec![
            self.id.to_string(),
            self.hostname.clone(),
            addresses,
            self.ttl.to_string(),
            self.last_update_at.clone().unwrap_or_default(),
        ]
    }
}

impl DisplayTable for DynDnsHost {
    fn headers(&self) -> Vec<&'static str> {
        match self.token {
            // The token is the point of this response: show it instead of the state.
            Some(_) => vec![t!(h_host), t!(h_token)],
            None => host_headers(),
        }
    }

    fn rows(&self) -> Vec<Vec<String>> {
        match self.token.as_deref() {
            Some(token) => vec![vec![self.hostname.clone(), token.to_string()]],
            None => vec![self.row()],
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct DynDnsHostList {
    pub results: Vec<DynDnsHost>,
}

impl DisplayTable for DynDnsHostList {
    fn headers(&self) -> Vec<&'static str> {
        host_headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.results.iter().map(DynDnsHost::row).collect()
    }
}

pub struct DynDnsHosts;

impl RequestDesc for DynDnsHosts {
    /// Optional domain id filter.
    type Params = Option<i64>;
    type Request = ();
    type Response = DynDnsHostList;

    fn get_url(domain_id: Self::Params) -> impl AsRef<str> {
        match domain_id {
            Some(id) => format!("dyndns/hosts?domain={id}"),
            None => "dyndns/hosts".to_string(),
        }
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Serialize)]
pub struct DynDnsNewReq {
    pub domain_id: i64,
    pub hostname: String,
    pub ttl: i64,
    pub comment: String,
}

pub struct DynDnsNew;

impl RequestDesc for DynDnsNew {
    type Params = ();
    type Request = DynDnsNewReq;
    type Response = DynDnsHost;

    fn get_url(_: ()) -> impl AsRef<str> {
        "dyndns/hosts"
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct DynDnsRotate;

impl RequestDesc for DynDnsRotate {
    type Params = i64;
    type Request = ();
    type Response = DynDnsHost;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("dyndns/hosts/{id}/rotate-token")
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct DynDnsDelete;

impl RequestDesc for DynDnsDelete {
    type Params = i64;
    type Request = ();
    type Response = serde::de::IgnoredAny;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("dyndns/hosts/{id}")
    }

    fn method() -> Method {
        Method::DELETE
    }
}
