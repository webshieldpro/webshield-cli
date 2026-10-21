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
    /// Monthly spending cap in the account currency; `None` — no cap.
    #[serde(default)]
    pub spend_cap: Option<String>,
    /// Why the project is paused: `debt`, `cap` or `None`.
    #[serde(default)]
    pub paused: Option<String>,
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

/// Projects filtered by name (`None` — all of them).
pub struct ProjectsByName;
impl RequestDesc for ProjectsByName {
    type Params = Option<String>;
    type Request = ();
    type Response = ProjectList;

    fn get_url(name: Option<String>) -> impl AsRef<str> {
        match name {
            Some(name) => format!("compute/projects?name={}", q(&name)),
            None => "compute/projects".into(),
        }
    }

    fn method() -> Method {
        Method::GET
    }
}

/// App environment: the server replaces the whole map on PUT.
#[derive(Deserialize, Serialize, Default)]
pub struct Env {
    pub variables: std::collections::BTreeMap<String, String>,
    /// Print values, not only names (they are secrets more often than not).
    #[serde(skip)]
    pub show_values: bool,
}

impl DisplayTable for Env {
    fn headers(&self) -> Vec<&'static str> {
        if self.show_values {
            vec![t!(h_name), t!(h_value)]
        } else {
            vec![t!(h_name)]
        }
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.variables
            .iter()
            .map(|(name, value)| {
                if self.show_values {
                    vec![name.clone(), value.clone()]
                } else {
                    vec![name.clone()]
                }
            })
            .collect()
    }
}

pub struct EnvGet;
impl RequestDesc for EnvGet {
    type Params = i64;
    type Request = ();
    type Response = Env;

    fn get_url(service: i64) -> impl AsRef<str> {
        format!("compute/services/{service}/env")
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct EnvPut;
impl RequestDesc for EnvPut {
    type Params = i64;
    type Request = Env;
    type Response = Env;

    fn get_url(service: i64) -> impl AsRef<str> {
        format!("compute/services/{service}/env")
    }

    fn method() -> Method {
        Method::PUT
    }
}

/// Postgres database of a project.
#[derive(Deserialize, Serialize, Clone)]
pub struct Database {
    pub id: i64,
    pub project: i64,
    pub name: String,
    pub size_gb: i64,
    pub instances: i64,
    pub status: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub restore_source: String,
}

#[derive(Deserialize, Serialize)]
pub struct DatabaseList {
    pub results: Vec<Database>,
}

impl DisplayTable for DatabaseList {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            t!(h_name),
            t!(h_status),
            t!(h_size),
            t!(h_instances),
            t!(h_reference),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.results
            .iter()
            .map(|db| {
                vec![
                    db.name.clone(),
                    status_with_error(&db.status, &db.error),
                    format!("{} GB", db.size_gb),
                    db.instances.to_string(),
                    db.reference.clone(),
                ]
            })
            .collect()
    }
}

impl DisplayTable for Database {
    fn headers(&self) -> Vec<&'static str> {
        DatabaseList { results: vec![] }.headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        DatabaseList {
            results: vec![self.clone()],
        }
        .rows()
    }
}

/// Valkey store of a project.
#[derive(Deserialize, Serialize, Clone)]
pub struct KeyValue {
    pub id: i64,
    pub project: i64,
    pub name: String,
    pub memory_mb: i64,
    pub persistent: bool,
    pub status: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub reference: String,
}

#[derive(Deserialize, Serialize)]
pub struct KeyValueList {
    pub results: Vec<KeyValue>,
}

impl DisplayTable for KeyValueList {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            t!(h_name),
            t!(h_status),
            t!(h_memory),
            t!(h_on_disk),
            t!(h_reference),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.results
            .iter()
            .map(|kv| {
                vec![
                    kv.name.clone(),
                    status_with_error(&kv.status, &kv.error),
                    format!("{} MB", kv.memory_mb),
                    if kv.persistent { "+" } else { "—" }.into(),
                    kv.reference.clone(),
                ]
            })
            .collect()
    }
}

impl DisplayTable for KeyValue {
    fn headers(&self) -> Vec<&'static str> {
        KeyValueList { results: vec![] }.headers()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        KeyValueList {
            results: vec![self.clone()],
        }
        .rows()
    }
}

fn status_with_error(status: &str, error: &str) -> String {
    if error.is_empty() {
        status.to_string()
    } else {
        format!("{status}: {error}")
    }
}

#[derive(Serialize)]
pub struct DatabaseCreateReq {
    pub project: i64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_gb: Option<i64>,
    pub instances: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_from: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_time: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct RecoveryWindow {
    pub earliest: Option<String>,
    pub latest: Option<String>,
}

impl DisplayTable for RecoveryWindow {
    fn headers(&self) -> Vec<&'static str> {
        vec![t!(h_earliest), t!(h_latest)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "—".into());
        vec![vec![show(&self.earliest), show(&self.latest)]]
    }
}

pub struct DatabaseRecoveryWindow;
impl RequestDesc for DatabaseRecoveryWindow {
    type Params = i64;
    type Request = ();
    type Response = RecoveryWindow;

    fn get_url(id: i64) -> impl AsRef<str> {
        format!("compute/databases/{id}/recovery-window")
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Serialize)]
pub struct KeyValueCreateReq {
    pub project: i64,
    pub name: String,
    pub memory_mb: i64,
    pub persistent: bool,
}

/// Databases (`db`) and key-value stores (`kv`) share the REST shape.
pub struct ResourceList<T>(std::marker::PhantomData<T>);
pub struct ResourceCreate<T>(std::marker::PhantomData<T>);
pub struct ResourceDelete<T>(std::marker::PhantomData<T>);

pub trait ProjectResource {
    const PATH: &'static str;
    type List: serde::de::DeserializeOwned;
    type Item: serde::de::DeserializeOwned;
    type Create: Serialize;

    /// (id, name) of each listed item.
    fn entries(list: &Self::List) -> Vec<(i64, String)>;
}

impl ProjectResource for Database {
    const PATH: &'static str = "compute/databases";
    type List = DatabaseList;
    type Item = Database;
    type Create = DatabaseCreateReq;

    fn entries(list: &DatabaseList) -> Vec<(i64, String)> {
        list.results
            .iter()
            .map(|x| (x.id, x.name.clone()))
            .collect()
    }
}

impl ProjectResource for KeyValue {
    const PATH: &'static str = "compute/kv";
    type List = KeyValueList;
    type Item = KeyValue;
    type Create = KeyValueCreateReq;

    fn entries(list: &KeyValueList) -> Vec<(i64, String)> {
        list.results
            .iter()
            .map(|x| (x.id, x.name.clone()))
            .collect()
    }
}

impl<T: ProjectResource> RequestDesc for ResourceList<T> {
    /// Project id.
    type Params = i64;
    type Request = ();
    type Response = T::List;

    fn get_url(project: i64) -> impl AsRef<str> {
        format!("{}?project={project}", T::PATH)
    }

    fn method() -> Method {
        Method::GET
    }
}

impl<T: ProjectResource> RequestDesc for ResourceCreate<T> {
    type Params = ();
    type Request = T::Create;
    type Response = T::Item;

    fn get_url(_: ()) -> impl AsRef<str> {
        T::PATH
    }

    fn method() -> Method {
        Method::POST
    }
}

impl<T: ProjectResource> RequestDesc for ResourceDelete<T> {
    type Params = i64;
    type Request = ();
    // 204 — only the status matters.
    type Response = serde::de::IgnoredAny;

    fn get_url(id: i64) -> impl AsRef<str> {
        format!("{}/{id}", T::PATH)
    }

    fn method() -> Method {
        Method::DELETE
    }
}

/// One day of project spending; amounts and hours are decimal strings.
#[derive(Deserialize, Serialize)]
pub struct UsageDay {
    pub date: String,
    pub cost: String,
    pub vcpu_hours: String,
    pub ram_gb_hours: String,
    pub disk_gb_hours: String,
    #[serde(default)]
    pub backup_gb_hours: String,
}

/// Spending of a project in the current calendar month.
#[derive(Deserialize, Serialize)]
pub struct ProjectUsage {
    pub currency: String,
    pub month_start: String,
    pub cost: String,
    pub spend_cap: Option<String>,
    pub days: Vec<UsageDay>,
}

impl DisplayTable for ProjectUsage {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            t!(h_date),
            t!(h_cost),
            t!(h_vcpu_hours),
            t!(h_ram_hours),
            t!(h_disk_hours),
            t!(h_backup_hours),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let sum = |pick: fn(&UsageDay) -> &str| {
            let total: f64 = self
                .days
                .iter()
                .map(|d| pick(d).parse::<f64>().unwrap_or(0.0))
                .sum();
            format!("{total:.3}")
        };
        let mut rows: Vec<Vec<String>> = self
            .days
            .iter()
            .map(|d| {
                vec![
                    d.date.clone(),
                    d.cost.clone(),
                    d.vcpu_hours.clone(),
                    d.ram_gb_hours.clone(),
                    d.disk_gb_hours.clone(),
                    d.backup_gb_hours.clone(),
                ]
            })
            .collect();
        let cost = match &self.spend_cap {
            Some(cap) => format!("{} / {cap} {}", self.cost, self.currency),
            None => format!("{} {}", self.cost, self.currency),
        };
        rows.push(vec![
            t!(usage_total).to_string(),
            cost,
            sum(|d| &d.vcpu_hours),
            sum(|d| &d.ram_gb_hours),
            sum(|d| &d.disk_gb_hours),
            sum(|d| &d.backup_gb_hours),
        ]);
        rows
    }
}

pub struct ProjectUsageGet;
impl RequestDesc for ProjectUsageGet {
    type Params = i64;
    type Request = ();
    type Response = ProjectUsage;

    fn get_url(id: i64) -> impl AsRef<str> {
        format!("compute/projects/{id}/usage")
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Deserialize, Serialize)]
pub struct Prices {
    pub vcpu: String,
    pub ram_gb: String,
    pub disk_gb: String,
    #[serde(default)]
    pub backup_gb: String,
}

/// Prepaid apps: balance, charges not debited yet, what is available, prices per 30 days.
#[derive(Deserialize, Serialize)]
pub struct ComputeBilling {
    pub currency: String,
    pub balance: String,
    pub unbilled: String,
    pub available: String,
    pub pause_at: Option<String>,
    pub prices: Prices,
}

impl DisplayTable for ComputeBilling {
    fn headers(&self) -> Vec<&'static str> {
        vec![t!(h_metric), t!(h_value)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let money = |v: &str| format!("{v} {}", self.currency);
        let mut rows = vec![
            vec![t!(h_balance).to_string(), money(&self.balance)],
            vec![t!(h_unbilled).to_string(), money(&self.unbilled)],
            vec![t!(h_available).to_string(), money(&self.available)],
        ];
        if let Some(at) = &self.pause_at {
            rows.push(vec![t!(h_pause_at).to_string(), at.clone()]);
        }
        rows.extend([
            vec![t!(h_price_vcpu).to_string(), money(&self.prices.vcpu)],
            vec![t!(h_price_ram).to_string(), money(&self.prices.ram_gb)],
            vec![t!(h_price_disk).to_string(), money(&self.prices.disk_gb)],
            vec![
                t!(h_price_backup).to_string(),
                money(&self.prices.backup_gb),
            ],
        ]);
        rows
    }
}

pub struct ComputeBillingGet;
impl RequestDesc for ComputeBillingGet {
    type Params = ();
    type Request = ();
    type Response = ComputeBilling;

    fn get_url(_: ()) -> impl AsRef<str> {
        "compute/billing"
    }

    fn method() -> Method {
        Method::GET
    }
}

/// `null` removes the cap, so the field is always sent.
#[derive(Serialize)]
pub struct SpendCapReq {
    pub spend_cap: Option<String>,
}

pub struct ProjectSpendCap;
impl RequestDesc for ProjectSpendCap {
    type Params = i64;
    type Request = SpendCapReq;
    type Response = Project;

    fn get_url(id: i64) -> impl AsRef<str> {
        format!("compute/projects/{id}")
    }

    fn method() -> Method {
        Method::PATCH
    }
}
