//! API error handling: HTTP status checking and unwrapping DRF error bodies
//! (`{"detail": …}` or a field error map) into readable text.

use crate::i18n::{self, M};
use anyhow::Result;
use reqwest::{Response, StatusCode};
use serde_json::Value;
use std::fmt;

/// HTTP error with the DRF `detail` already extracted from the body.
/// Kept as a typed error so callers (`auth status`) can branch on the
/// status code via `anyhow::Error::downcast_ref`.
#[derive(Debug)]
pub struct HttpError {
    pub status: StatusCode,
    pub detail: String,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status {
            StatusCode::UNAUTHORIZED => write!(
                f,
                "401 Unauthorized: {}\n{}",
                self.detail,
                i18n::tr(M::ErrUnauthorized)
            ),
            StatusCode::FORBIDDEN => write!(
                f,
                "403 Forbidden: {}\n{}",
                self.detail,
                i18n::tr(M::ErrForbidden)
            ),
            status => write!(
                f,
                "{} {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or(""),
                self.detail
            ),
        }
    }
}

impl std::error::Error for HttpError {}

/// Checks the HTTP status; on error extracts a readable message from the DRF body.
pub(crate) async fn check_status(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    let detail = extract_detail(&body).unwrap_or(body);
    Err(anyhow::Error::new(HttpError { status, detail }))
}

/// Extracts `detail` or joins DRF serializer field errors into a single line.
fn extract_detail(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let obj = value.as_object()?;
    if let Some(detail) = obj.get("detail").and_then(Value::as_str) {
        return Some(detail.to_string());
    }
    // A {field: [messages]} map — join into "field: message".
    let mut parts = Vec::new();
    for (field, errs) in obj {
        let joined = flatten_errors(errs);
        if joined.is_empty() {
            continue;
        }
        parts.push(format!("{field}: {joined}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

/// Recursively collects message strings from a DRF error value. Nested
/// serializers produce structures like {"rrsets": [{"non_field_errors": ["…"]}]}
/// where valid items appear as empty objects — those are skipped.
fn flatten_errors(value: &Value) -> String {
    let join = |it: &mut dyn Iterator<Item = String>| {
        it.filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ")
    };
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => join(&mut items.iter().map(flatten_errors)),
        Value::Object(map) => join(&mut map.values().map(flatten_errors)),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_detail_reads_drf_detail() {
        assert_eq!(
            extract_detail(r#"{"detail": "Not found."}"#),
            Some("Not found.".to_string())
        );
    }

    #[test]
    fn extract_detail_joins_field_errors() {
        let got = extract_detail(r#"{"name": ["This field is required.", "Too short."]}"#);
        assert_eq!(
            got,
            Some("name: This field is required.; Too short.".to_string())
        );
    }

    #[test]
    fn extract_detail_unwraps_nested_serializer_errors() {
        // Bulk rrsets validation: valid items are empty objects, only messages survive.
        let got = extract_detail(
            r#"{"rrsets": [{}, {"non_field_errors": ["For CNAME/NS/PTR the target must be a FQDN ending with a dot."]}]}"#,
        );
        assert_eq!(
            got,
            Some(
                "rrsets: For CNAME/NS/PTR the target must be a FQDN ending with a dot.".to_string()
            )
        );
    }

    #[test]
    fn extract_detail_ignores_non_json() {
        assert_eq!(extract_detail("<html>502</html>"), None);
        assert_eq!(extract_detail("{}"), None);
    }
}
