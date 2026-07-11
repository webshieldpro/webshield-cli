#![allow(refining_impl_trait_reachable)]

use crate::api::get_url::MakeReq;
use reqwest::Method;
use reqwest::multipart::Form;
use serde::{Deserialize, Serialize};

pub struct SiteAdd;
#[derive(Serialize)]
pub struct SiteAddReq {
    pub hostname: String,
    pub domain_id: i64
}
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

impl MakeReq for SiteAdd {
    type Params = ();
    type Request = SiteAddReq;
    type Response = SitesListInner;

    fn get_url(_: ()) -> &'static str {
        "static-sites"
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct Sites;

#[derive(Deserialize)]
pub struct SitesList {
    pub results: Vec<SitesListInner>,
}

impl MakeReq for Sites {
    type Params = ();
    type Request = ();
    type Response = SitesList;

    fn get_url(_: ()) -> &'static str {
        "static-sites"
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct SiteFiles;
#[derive(Debug, Deserialize)]
pub struct FilesResponseSite {
    #[serde(default)]
    pub files: Vec<ServerFileSite>,
}

impl MakeReq for SiteFiles {
    type Params = i64;
    type Request = ();
    type Response = FilesResponseSite;

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

pub struct SiteDisable;

impl MakeReq for SiteDisable {
    type Params = i64;
    type Request = ();
    type Response = ();

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{}/disable", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct SitePublish;

impl MakeReq for SitePublish {
    type Params = i64;
    type Request = ();
    type Response = ();

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{}/publish", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct SiteFilesUploadBatch;
impl MakeReq for SiteFilesUploadBatch {
    type Params = i64;
    type Request = Form;
    type Response = ();

    fn get_url(site_id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{site_id}/upload")
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct SiteFilesDeleteBatch;
#[derive(Debug, Serialize)]
pub struct SiteFilesPaths {
    pub paths: Vec<String>
}

impl MakeReq for SiteFilesDeleteBatch {
    type Params = i64;
    type Request = SiteFilesPaths;
    type Response = serde_json::Value;

    fn get_url(site_id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{site_id}/delete-files")
    }

    fn method() -> Method {
        Method::POST
    }
}