//! Hostnames of domains connected via DNS records (`/external-hosts`, scope `domains`).

use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::t;
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct HostRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub rr_type: String,
    pub value: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct HostRecords {
    pub target: String,
    /// `_acme-challenge.<hostname>` CNAME: proves ownership and renews the certificate.
    pub ownership: HostRecord,
    /// The hostname itself: CNAME, or ALIAS/ANAME/flattening for the zone apex.
    pub traffic: HostRecord,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ExternalHost {
    pub id: i64,
    pub domain_id: i64,
    pub domain_name: String,
    pub hostname: String,
    pub mode: String,
    pub status: String,
    pub records: HostRecords,
    #[serde(default)]
    pub certificate_status: String,
    #[serde(default)]
    pub certificate_error: String,
    #[serde(default)]
    pub traffic_ok: bool,
    #[serde(default)]
    pub removal_at: Option<String>,
}

fn host_headers() -> Vec<&'static str> {
    vec![
        t!(h_id),
        t!(h_host),
        t!(h_status),
        t!(h_mode),
        t!(h_certificate),
        t!(h_traffic),
        t!(h_target),
    ]
}

impl ExternalHost {
    fn row(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            self.hostname.clone(),
            self.status.clone(),
            self.mode.clone(),
            self.certificate_status.clone(),
            if self.traffic_ok { t!(yes) } else { t!(no) }.to_string(),
            self.records.target.clone(),
        ]
    }
}

impl DisplayTable for ExternalHost {
    fn headers(&self) -> Vec<&'static str> {
        host_headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        vec![self.row()]
    }
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub struct ExternalHostList(pub Vec<ExternalHost>);

impl DisplayTable for ExternalHostList {
    fn headers(&self) -> Vec<&'static str> {
        host_headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.0.iter().map(ExternalHost::row).collect()
    }
}

impl DisplayTable for HostRecords {
    fn headers(&self) -> Vec<&'static str> {
        vec![t!(h_name), t!(h_type), t!(h_value)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        [&self.ownership, &self.traffic]
            .into_iter()
            .map(|r| vec![r.name.clone(), r.rr_type.clone(), r.value.clone()])
            .collect()
    }
}

pub struct Hosts;

impl RequestDesc for Hosts {
    /// Optional domain id filter.
    type Params = Option<i64>;
    type Request = ();
    type Response = ExternalHostList;

    fn get_url(domain_id: Self::Params) -> impl AsRef<str> {
        match domain_id {
            Some(id) => format!("external-hosts?domain_id={}", id),
            None => "external-hosts".to_string(),
        }
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Serialize)]
pub struct HostNewReq {
    pub domain_id: i64,
    pub hostname: String,
    pub mode: String,
    pub origin: String,
    pub origin_ssl: bool,
    pub redirect_target: String,
}

pub struct HostNew;

impl RequestDesc for HostNew {
    type Params = ();
    type Request = HostNewReq;
    type Response = ExternalHost;

    fn get_url(_: ()) -> impl AsRef<str> {
        "external-hosts"
    }

    fn method() -> Method {
        Method::POST
    }
}

#[derive(Deserialize, Serialize)]
pub struct HostCheck {
    pub code: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Deserialize, Serialize)]
pub struct HostVerifyRes {
    #[serde(default)]
    pub check: Option<HostCheck>,
    pub host: ExternalHost,
}

pub struct HostVerify;

impl RequestDesc for HostVerify {
    type Params = i64;
    type Request = ();
    type Response = HostVerifyRes;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("external-hosts/{}/verify", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct HostDelete;

impl RequestDesc for HostDelete {
    type Params = i64;
    type Request = ();
    type Response = serde::de::IgnoredAny;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("external-hosts/{}", id)
    }

    fn method() -> Method {
        Method::DELETE
    }
}
