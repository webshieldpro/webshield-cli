#![allow(refining_impl_trait_reachable)]

use crate::api::request_desc::{ListRequestDesc, RequestDesc};
use crate::api::table::DisplayTable;
use crate::i18n;
use crate::i18n::M;
use crate::util::output::success;
use reqwest::multipart::Form;
use reqwest::Method;
use serde::{Deserialize, Serialize};

pub struct SiteAdd;
#[derive(Serialize)]
pub struct SiteAddReq {
    pub hostname: String,
    pub domain_id: i64,
}
#[derive(Deserialize, Serialize)]
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
    #[serde(default)]
    pub publish_error: Option<String>,
}

impl DisplayTable for SitesListInner {
    fn headers(&self) -> Vec<&'static str> {
        unreachable!()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        unreachable!()
    }

    fn display_as_table(&self) {
        success(i18n::f(
            M::SiteCreated,
            &[("host", &self.hostname), ("id", &self.id.to_string())],
        ));
    }
}

impl RequestDesc for SiteAdd {
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

#[derive(Deserialize, Serialize)]
pub struct SitesList {
    pub results: Vec<SitesListInner>,
}

impl ListRequestDesc for Sites {
    type Params = ();
    type Item = SitesListInner;

    fn get_url(_: ()) -> &'static str {
        "static-sites"
    }
}

impl DisplayTable for SitesList {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            i18n::tr(M::HId),
            i18n::tr(M::HHost),
            i18n::tr(M::HDomain),
            i18n::tr(M::HStatus),
            i18n::tr(M::HVersion),
            i18n::tr(M::HSize),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.results
            .iter()
            .map(|s| {
                vec![
                    s.id.to_string(),
                    s.hostname.clone(),
                    s.domain_name.clone().unwrap_or_default(),
                    s.status.clone().unwrap_or_default(),
                    s.content_version.map(|v| v.to_string()).unwrap_or_default(),
                    s.size_bytes
                        .map(crate::util::output::fmt_size)
                        .unwrap_or_default(),
                ]
            })
            .collect()
    }
}

pub struct SiteFiles;
#[derive(Deserialize, Serialize)]
pub struct FilesResponseSite {
    #[serde(default)]
    pub files: Vec<ServerFileSite>,
}

impl DisplayTable for FilesResponseSite {
    fn headers(&self) -> Vec<&'static str> {
        vec![i18n::tr(M::HPath), i18n::tr(M::HEtag)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.files
            .iter()
            .filter(|f| !f.is_dir)
            .map(|f| vec![f.path.clone(), f.etag.clone().unwrap_or_default()])
            .collect()
    }
}

impl RequestDesc for SiteFiles {
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

#[derive(Deserialize, Serialize)]
pub struct ServerFileSite {
    pub path: String,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub is_dir: bool,
}

pub struct SiteDisable;

impl RequestDesc for SiteDisable {
    type Params = i64;
    type Request = ();
    // The response body is irrelevant — only the status matters.
    type Response = serde::de::IgnoredAny;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{}/disable", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct SitePublish;

impl RequestDesc for SitePublish {
    type Params = i64;
    type Request = ();
    type Response = serde::de::IgnoredAny;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{}/publish", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct SiteGet;

impl RequestDesc for SiteGet {
    type Params = i64;
    type Request = ();
    type Response = SitesListInner;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{}", id)
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct SitePublishFromBucket;
#[derive(Debug, Serialize)]
pub struct SitePublishBucketReq {
    pub bucket: String,
    pub path: String,
}

impl RequestDesc for SitePublishFromBucket {
    type Params = i64;
    type Request = SitePublishBucketReq;
    type Response = SitesListInner;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{}/publish-from-bucket", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct SiteFilesUploadBatch;
impl RequestDesc for SiteFilesUploadBatch {
    type Params = i64;
    type Request = Form;
    type Response = serde::de::IgnoredAny;

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
    pub paths: Vec<String>,
}

impl RequestDesc for SiteFilesDeleteBatch {
    type Params = i64;
    type Request = SiteFilesPaths;
    type Response = serde::de::IgnoredAny;

    fn get_url(site_id: Self::Params) -> impl AsRef<str> {
        format!("static-sites/{site_id}/delete-files")
    }

    fn method() -> Method {
        Method::POST
    }
}
