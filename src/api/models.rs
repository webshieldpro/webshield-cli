//! Typed models of `/api/v1` responses. Fields the server may omit are marked
//! `#[serde(default)]` so the CLI does not break on schema extensions.

use serde::{Deserialize, Serialize};

/// DRF pagination envelope (`PageNumberPagination`).
#[derive(Debug, Deserialize)]
pub struct Page<T> {
    #[serde(default)]
    pub next: Option<String>,
    pub results: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: i64,
    pub name: String,
    /// Delegation status: `None` until the domain's NS delegation is checked.
    #[serde(default)]
    pub delegated: Option<bool>,
    #[serde(default)]
    pub current_tariff: Option<Tariff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tariff {
    #[serde(default)]
    pub name: String,
}

/// Result of `POST domains/{id}/check-delegation`. The check is strict:
/// the NS set at the parent zone must match the WebShield set exactly, so
/// `missing_ns`/`extra_ns` explain why `delegated` is `false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationCheck {
    #[serde(default)]
    pub delegated: Option<bool>,
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub current_ns: Vec<String>,
    #[serde(default)]
    pub missing_ns: Vec<String>,
    #[serde(default)]
    pub extra_ns: Vec<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

/// A single record value (content + disabled flag).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordItem {
    pub content: String,
    #[serde(default)]
    pub disabled: bool,
}

/// A set of records sharing one name and type (rrset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RRSet {
    pub name: String,
    #[serde(rename = "type")]
    pub rr_type: String,
    #[serde(default)]
    pub ttl: Option<i64>,
    #[serde(default)]
    pub records: Vec<RecordItem>,
    /// Proxying flag (for A/AAAA/CNAME).
    #[serde(default)]
    pub proxied: bool,
}

#[derive(Debug, Deserialize)]
pub struct RecordsResponse {
    #[serde(default)]
    pub rrsets: Vec<RRSet>,
    #[serde(default)]
    pub records_used: Option<i64>,
    #[serde(default)]
    pub records_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticSite {
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

/// Response of `GET /static-sites/<id>/files` — flat list of draft files with etags.
#[derive(Debug, Deserialize)]
pub struct FilesResponse {
    #[serde(default)]
    pub files: Vec<ServerFile>,
}

#[derive(Debug, Deserialize)]
pub struct ServerFile {
    pub path: String,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub is_dir: bool,
}

/// Edge configuration of a proxied/redirect host (`/nginx-configs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub id: i64,
    pub hostname: String,
    #[serde(default)]
    pub domain_name: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub redirect_target: Option<String>,
    #[serde(default)]
    pub ssl_required: Option<bool>,
    #[serde(default)]
    pub bot_protection_enabled: Option<bool>,
    #[serde(default)]
    pub captcha_check_enabled: Option<bool>,
    #[serde(default)]
    pub http2_enabled: Option<bool>,
    #[serde(default)]
    pub http3_enabled: Option<bool>,
    #[serde(default)]
    pub max_body_size_mb: Option<i64>,
    #[serde(default)]
    pub blocked_bots: Option<Vec<String>>,
}
