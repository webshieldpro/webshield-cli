#![allow(refining_impl_trait_reachable)]

//! Apps (compute): projects, apps, deployments and logs.

use crate::api::request_desc::RequestDesc;
use crate::commands::display_table::DisplayTable;
use crate::t;
use reqwest::multipart::Form;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// Query-string value: our names are `[a-z0-9-]`, but encode anyway.
fn q(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Deserialize, Serialize)]
pub struct ProjectList {
    pub results: Vec<Project>,
}

pub struct Projects;
impl RequestDesc for Projects {
    type Params = ();
    type Request = ();
    type Response = ProjectList;

    fn get_url(_: ()) -> impl AsRef<str> {
        "compute/projects"
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DeploymentSummary {
    pub id: i64,
    pub status: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct App {
    pub id: i64,
    pub project: i64,
    pub name: String,
    pub port: i64,
    pub min_scale: i64,
    pub max_scale: i64,
    #[serde(default)]
    pub active_deployment: Option<DeploymentSummary>,
}

#[derive(Deserialize, Serialize)]
pub struct AppList {
    pub results: Vec<App>,
    #[serde(skip)]
    pub project_names: Vec<(i64, String)>,
}

impl DisplayTable for AppList {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            t!(h_id),
            t!(h_project),
            t!(h_app),
            t!(h_port),
            t!(h_deployment),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.results
            .iter()
            .map(|app| {
                let project = self
                    .project_names
                    .iter()
                    .find(|(id, _)| *id == app.project)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| app.project.to_string());
                let active = app
                    .active_deployment
                    .as_ref()
                    .map(|d| format!("#{}", d.id))
                    .unwrap_or_else(|| "—".into());
                vec![
                    app.id.to_string(),
                    project,
                    app.name.clone(),
                    app.port.to_string(),
                    active,
                ]
            })
            .collect()
    }
}

/// Apps filtered by project and app names (`None` — no filter).
pub struct AppsByName;
impl RequestDesc for AppsByName {
    type Params = (Option<String>, Option<String>);
    type Request = ();
    type Response = AppList;

    fn get_url((project, name): Self::Params) -> impl AsRef<str> {
        let mut url = String::from("compute/services?");
        if let Some(project) = project {
            url.push_str(&format!("project_name={}&", q(&project)));
        }
        if let Some(name) = name {
            url.push_str(&format!("name={}&", q(&name)));
        }
        url.trim_end_matches(['?', '&']).to_string()
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Deployment {
    pub id: i64,
    pub service: i64,
    pub status: String,
    #[serde(default)]
    pub source_kind: String,
    #[serde(default)]
    pub image_digest: String,
    #[serde(default)]
    pub builder: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub build_log: Option<String>,
}

impl Deployment {
    pub fn in_progress(&self) -> bool {
        matches!(self.status.as_str(), "queued" | "building" | "deploying")
    }
}

impl DisplayTable for Deployment {
    fn headers(&self) -> Vec<&'static str> {
        vec![t!(h_id), t!(h_status), t!(h_source), t!(h_created)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        vec![vec![
            self.id.to_string(),
            self.status.clone(),
            self.source_kind.clone(),
            self.created_at.clone(),
        ]]
    }
}

#[derive(Deserialize, Serialize)]
pub struct DeploymentList {
    pub results: Vec<Deployment>,
}

impl DisplayTable for DeploymentList {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            t!(h_id),
            t!(h_status),
            t!(h_source),
            t!(h_created),
            t!(h_error),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.results
            .iter()
            .map(|d| {
                vec![
                    d.id.to_string(),
                    d.status.clone(),
                    d.source_kind.clone(),
                    d.created_at.clone(),
                    d.error.clone(),
                ]
            })
            .collect()
    }
}

pub struct Deployments;
impl RequestDesc for Deployments {
    type Params = i64;
    type Request = ();
    type Response = DeploymentList;

    fn get_url(service: i64) -> impl AsRef<str> {
        format!("compute/deployments?service={service}")
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct DeploymentGet;
impl RequestDesc for DeploymentGet {
    type Params = i64;
    type Request = ();
    type Response = Deployment;

    fn get_url(id: i64) -> impl AsRef<str> {
        format!("compute/deployments/{id}")
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct Redeploy;
impl RequestDesc for Redeploy {
    type Params = i64;
    type Request = ();
    type Response = Deployment;

    fn get_url(id: i64) -> impl AsRef<str> {
        format!("compute/deployments/{id}/redeploy")
    }

    fn method() -> Method {
        Method::POST
    }
}

#[derive(Serialize)]
pub struct DeployUrlReq {
    pub service: i64,
    pub source_url: String,
    pub source_subdir: String,
}

pub struct DeployUrl;
impl RequestDesc for DeployUrl {
    type Params = ();
    type Request = DeployUrlReq;
    type Response = Deployment;

    fn get_url(_: ()) -> impl AsRef<str> {
        "compute/deployments"
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct DeployUpload;
impl RequestDesc for DeployUpload {
    type Params = i64;
    type Request = Form;
    type Response = Deployment;

    fn get_url(service: i64) -> impl AsRef<str> {
        format!("compute/services/{service}/upload")
    }

    fn method() -> Method {
        Method::POST
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct LogLine {
    pub ts: String,
    #[serde(default)]
    pub stream: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Deserialize, Serialize)]
pub struct LogLines {
    pub lines: Vec<LogLine>,
}

pub struct Logs;
impl RequestDesc for Logs {
    /// (app id, period, search text, continue after this timestamp)
    type Params = (i64, String, Option<String>, Option<String>);
    type Request = ();
    type Response = LogLines;

    fn get_url((service, since, search, start): Self::Params) -> impl AsRef<str> {
        let mut url = format!(
            "compute/services/{service}/logs?since={}&limit=500",
            q(&since)
        );
        if let Some(search) = search {
            url.push_str(&format!("&q={}", q(&search)));
        }
        if let Some(start) = start {
            url.push_str(&format!("&start={}", q(&start)));
        }
        url
    }

    fn method() -> Method {
        Method::GET
    }
}
