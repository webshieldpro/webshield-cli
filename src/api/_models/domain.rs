use crate::api::request_desc::RequestDesc;
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

impl DisplayTable for DomainInner {
    fn headers(&self) -> Vec<&'static str> {
        vec![i18n::tr(M::HField), i18n::tr(M::HValue)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let yes = i18n::tr(M::Yes);
        let no = i18n::tr(M::No);
        let dash = i18n::tr(M::Dash);

        vec![
            vec![i18n::tr(M::HId).into(), self.id.to_string()],
            vec![i18n::tr(M::HDomain).into(), self.name.clone()],
            vec![
                i18n::tr(M::HDelegated).into(),
                match self.delegated {
                    Some(true) => yes.into(),
                    Some(false) => no.into(),
                    None => dash.into(),
                },
            ],
            vec![
                i18n::tr(M::HTariff).into(),
                self.current_tariff
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| dash.into()),
            ],
        ]
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
#[derive(Deserialize, Serialize)]
pub struct DomainList {
    pub results: Vec<DomainInner>,
}

impl DisplayTable for DomainList {
    fn headers(&self) -> Vec<&'static str> {
        self.results.iter().next().unwrap().headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let mut buf = Vec::with_capacity(self.results.len());
        for res in &self.results {
            buf.extend(res.rows());
        }
        buf
    }
}

impl RequestDesc for Domains {
    type Params = ();
    type Request = ();
    type Response = DomainList;

    fn get_url(_: ()) -> impl AsRef<str> {
        "domains"
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct DomainDelete;

impl RequestDesc for DomainDelete {
    // TODO: request the DomainInner structure
    type Params = i64;
    type Request = ();
    type Response = ();

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
