use crate::api::get_url::MakeReq;
use reqwest::Method;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Tariff {
    #[serde(default)]
    pub name: String,
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

impl MakeReq for DomainInner {
    type Params = ();

    fn get_url(_: Self::Params) -> impl AsRef<str> {
        "domains"
    }

    fn method() -> Method {
        Method::POST
    }
}

#[derive(Deserialize)]
pub struct Domain {
    pub results: Vec<DomainInner>,
}

impl MakeReq for Domain {
    type Params = ();

    fn get_url(_: Self::Params) -> impl AsRef<str> {
        "domains"
    }

    fn method() -> Method {
        Method::GET
    }
}
