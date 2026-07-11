#![allow(refining_impl_trait_reachable)]

use crate::api::get_url::MakeReq;
use reqwest::Method;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SitesListInner {
    pub id: i64,
    pub hostname: String,
    #[serde(default)]
    pub domain_name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub content_version: Option<i64>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
}

impl MakeReq for SitesListInner {
    type Params = ();

    fn get_url(_: ()) -> &'static str {
        "static-sites"
    }

    fn method() -> Method {
        Method::POST
    }
}

#[derive(Deserialize)]
pub struct SitesList {
    pub results: Vec<SitesListInner>,
}

impl MakeReq for SitesList {
    type Params = ();

    fn get_url(_: ()) -> &'static str {
        "static-sites"
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Debug, Deserialize)]
pub struct FilesResponseSite {
    #[serde(default)]
    pub files: Vec<ServerFileSite>,
}

impl MakeReq for FilesResponseSite {
    type Params = i64;

    fn get_url(params: Self::Params) -> String {
        format!("static-sites/{}/files", params)
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerFileSite {
    pub path: String,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub is_dir: bool,
}
