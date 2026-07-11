use crate::api::get_url::MakeReq;
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
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
#[derive(Deserialize)]
pub struct DomainInner {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub delegated: Option<bool>,
    #[serde(default)]
    pub current_tariff: Option<Tariff>,
}

impl MakeReq for DomainAdd {
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
#[derive(Deserialize)]
pub struct DomainList {
    pub results: Vec<DomainInner>,
}

impl MakeReq for Domains {
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

impl MakeReq for DomainDelete {
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
impl MakeReq for DomainCheckDelegation {
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