use crate::api::request_desc::{ListRequestDesc, RequestDesc};
use crate::api::table::DisplayTable;
use crate::i18n;
use crate::i18n::M;
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Tariff {
    #[serde(default)]
    pub name: String,
}
pub struct DomainAdd;
#[derive(Serialize)]
pub struct DomainAddReq {
    pub name: String,
    pub import_method: String,
}
#[derive(Deserialize, Serialize)]
pub struct DomainInner {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub delegated: Option<bool>,
    #[serde(default)]
    pub current_tariff: Option<Tariff>,
}

fn domain_headers() -> Vec<&'static str> {
    vec![
        i18n::tr(M::HId),
        i18n::tr(M::HDomain),
        i18n::tr(M::HDelegated),
        i18n::tr(M::HTariff),
    ]
}

impl DisplayTable for DomainInner {
    fn headers(&self) -> Vec<&'static str> {
        domain_headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let yes = i18n::tr(M::Yes);
        let no = i18n::tr(M::No);
        let dash = i18n::tr(M::Dash);

        vec![vec![
            self.id.to_string(),
            self.name.clone(),
            match self.delegated {
                Some(true) => yes.into(),
                Some(false) => no.into(),
                None => dash.into(),
            },
            self.current_tariff
                .as_ref()
                .map(|t| t.name.clone())
                .unwrap_or_else(|| dash.into()),
        ]]
    }
}

impl RequestDesc for DomainAdd {
    type Params = ();
    type Request = DomainAddReq;
    type Response = DomainInner;

    fn get_url(_: ()) -> impl AsRef<str> {
        "domains"
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct Domains;
#[derive(Serialize)]
pub struct DomainList {
    pub results: Vec<DomainInner>,
}

impl DisplayTable for DomainList {
    fn headers(&self) -> Vec<&'static str> {
        domain_headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let mut buf = Vec::with_capacity(self.results.len());
        for res in &self.results {
            buf.extend(res.rows());
        }
        buf
    }
}

impl ListRequestDesc for Domains {
    type Params = ();
    type Item = DomainInner;

    fn get_url(_: ()) -> impl AsRef<str> {
        "domains"
    }
}

pub struct DomainDelete;

impl RequestDesc for DomainDelete {
    type Params = i64;
    type Request = ();
    // The body (204 or a JSON payload) is irrelevant — only the status matters.
    type Response = serde::de::IgnoredAny;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("domains/{}", id)
    }

    fn method() -> Method {
        Method::DELETE
    }
}

pub struct DomainCheckDelegation;
impl RequestDesc for DomainCheckDelegation {
    type Params = i64;
    type Request = ();
    type Response = serde_json::Value;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/check-delegation", id)
    }

    fn method() -> Method {
        Method::POST
    }
}
