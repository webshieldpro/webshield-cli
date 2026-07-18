//! Shared typed models of `/api/v1` responses (endpoint-specific models live
//! in `models/`). Fields the server may omit are marked `#[serde(default)]`
//! so the CLI does not break on schema extensions.

use serde::{Deserialize, Serialize};

/// DRF pagination envelope (`PageNumberPagination`).
#[derive(Debug, Deserialize)]
pub struct Page<T> {
    #[serde(default)]
    pub next: Option<String>,
    pub results: Vec<T>,
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
