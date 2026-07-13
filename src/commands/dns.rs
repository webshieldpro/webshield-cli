//! DNS record management.
//!
//! Backend semantics (`apply_dns_changes`) are NOT a full replace as in vanilla PowerDNS:
//!  * the default operation (no changetype) for A/AAAA/TXT/MX/… **merges** incoming
//!    values with the existing ones (missing values are not removed). CNAME/NS/PTR
//!    are not merged — the set is replaced as a whole.
//!  * `changetype=DELETE` with a non-empty `records` removes **only the listed** values.
//!
//! Commands: `add` — default POST (the server merges), `remove` — DELETE of specific
//! values (or the whole rrset), `set` — client-side reconcile (DELETE extras + POST targets).

use crate::api::_models::dns::{
    DNSDomainRecords, DnssecDelete, DnssecGet, DnssecPost, DnssecResp, RRSet, RRSetList,
};
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::i18n::{self, M};
use crate::Context;
use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::{json, Value};

#[derive(Subcommand)]
pub enum DnsCommand {
    /// List DNS records of a domain.
    List {
        domain: String,
        /// Filter by record type (A, AAAA, CNAME, TXT, MX, …).
        #[arg(long = "type")]
        rr_type: Option<String>,
    },
    /// Add value(s) to a record (appends for A/AAAA/TXT).
    Add {
        domain: String,
        /// Record name relative to the domain, or `@` for the apex.
        name: String,
        /// Record type.
        #[arg(name = "TYPE")]
        rr_type: String,
        /// One or more values.
        #[arg(required = true)]
        value: Vec<String>,
        #[arg(long, default_value_t = 300)]
        ttl: i64,
    },
    /// Replace a record with exactly the given values.
    Set {
        domain: String,
        name: String,
        #[arg(name = "TYPE")]
        rr_type: String,
        #[arg(required = true)]
        value: Vec<String>,
        #[arg(long, default_value_t = 300)]
        ttl: i64,
    },
    /// Remove record value(s); without values — remove the whole set.
    Remove {
        domain: String,
        name: String,
        #[arg(name = "TYPE")]
        rr_type: String,
        /// Specific values to remove (otherwise the whole rrset is removed).
        value: Vec<String>,
    },
    /// DNSSEC management.
    #[command(subcommand)]
    Dnssec(DnssecCommand),
}

#[derive(Subcommand)]
pub enum DnssecCommand {
    /// DNSSEC status and DS records for the registrar.
    Status { domain: String },
    /// Enable online zone signing.
    Enable { domain: String },
    /// Disable DNSSEC (blocked while a DS is visible in the parent; see --force).
    Disable {
        domain: String,
        /// Remove signing even with a live DS in the parent (risk of SERVFAIL).
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(ctx: &Context, cmd: DnsCommand) -> Result<ProgramRes> {
    let client = ctx.client()?;
    match cmd {
        DnsCommand::List { domain, rr_type } => {
            list(&client, &domain, rr_type).await.map(ProgramRes::from)
        }
        DnsCommand::Add {
            domain,
            name,
            rr_type,
            value,
            ttl,
        } => change(&client, &domain, &name, &rr_type, &value, ttl, Op::Add)
            .await
            .map(ProgramRes::from),
        DnsCommand::Set {
            domain,
            name,
            rr_type,
            value,
            ttl,
        } => change(&client, &domain, &name, &rr_type, &value, ttl, Op::Set)
            .await
            .map(ProgramRes::from),
        DnsCommand::Remove {
            domain,
            name,
            rr_type,
            value,
        } => change(&client, &domain, &name, &rr_type, &value, 0, Op::Remove)
            .await
            .map(ProgramRes::from),
        DnsCommand::Dnssec(sub) => dnssec(&client, sub).await.map(ProgramRes::from),
    }
}

enum Op {
    Add,
    Set,
    Remove,
}

async fn post_rrset(client: &Client, domain_id: i64, rrset: Value) -> Result<()> {
    let body = json!({ "rrsets": [rrset] });
    let _: Value = client
        .post_json(&format!("domains/{domain_id}/records"), &body)
        .await?;
    Ok(())
}

/// Normalizes a name to an FQDN with a trailing dot to match the API response.
fn to_fqdn(name: &str, domain: &str) -> String {
    let n = name.trim().trim_end_matches('.').to_lowercase();
    let d = domain.trim_end_matches('.').to_lowercase();
    if n.is_empty() || n == "@" {
        format!("{d}.")
    } else if n == d || n.ends_with(&format!(".{d}")) {
        format!("{n}.")
    } else {
        format!("{n}.{d}.")
    }
}

/// Normalizes a record value to the canonical form the API validates and stores
/// (mirrors the control panel): CNAME/NS/PTR — FQDN with a trailing dot,
/// MX/SRV — trailing dot on the target, TXT — wrapped in quotes.
fn normalize_content(rr_type: &str, value: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return v.to_string();
    }
    match rr_type {
        "CNAME" | "NS" | "PTR" => ensure_trailing_dot(v),
        "TXT" => {
            if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
                v.to_string()
            } else {
                format!("\"{v}\"")
            }
        }
        "MX" => dot_nth_token(v, 1),
        "SRV" => dot_nth_token(v, 3),
        _ => v.to_string(),
    }
}

fn ensure_trailing_dot(s: &str) -> String {
    if s.ends_with('.') {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

/// Appends a trailing dot to the n-th whitespace-separated token (the FQDN
/// target of MX/SRV). Values with fewer tokens are left as-is for the server
/// to report a format error.
fn dot_nth_token(value: &str, n: usize) -> String {
    let mut parts: Vec<String> = value.split_whitespace().map(str::to_string).collect();
    if let Some(target) = parts.get_mut(n) {
        *target = ensure_trailing_dot(target);
    }
    parts.join(" ")
}

fn find_rrset<'a>(records: &'a [RRSet], fqdn: &str, rr_type: &str) -> Option<&'a RRSet> {
    let want = fqdn.trim_end_matches('.');
    let ty = rr_type.to_uppercase();
    records.iter().find(|r| {
        r.name.trim_end_matches('.').eq_ignore_ascii_case(want)
            && r.rr_type.eq_ignore_ascii_case(&ty)
    })
}

async fn list(client: &Client, domain: &str, rr_type: Option<String>) -> Result<RRSetList> {
    let d = resolve_domain(client, domain).await?;
    let resp = client.n_send::<DNSDomainRecords>(d.id).await?;
    let filter = rr_type.map(|t| t.to_uppercase());
    let rrsets: RRSetList = resp
        .rrsets
        .into_iter()
        .filter(|r| {
            filter
                .as_ref()
                .is_none_or(|f| r.rr_type.eq_ignore_ascii_case(f))
        })
        .collect::<Vec<RRSet>>()
        .into();

    Ok(rrsets)
}

/// Single entry point for add/set/remove — they differ only in the rrsets they build.
async fn change(
    client: &Client,
    domain: &str,
    name: &str,
    rr_type: &str,
    values: &[String],
    ttl: i64,
    op: Op,
) -> Result<String> {
    let d = resolve_domain(client, domain).await?;
    let ty = rr_type.to_uppercase();
    // Canonical form also makes set/remove match the server-stored values.
    let values: Vec<String> = values.iter().map(|v| normalize_content(&ty, v)).collect();
    let values = values.as_slice();
    let items =
        |vals: &[String]| -> Vec<Value> { vals.iter().map(|v| json!({ "content": v })).collect() };

    let (msg, count) = match op {
        Op::Add => {
            let rrset = json!({ "name": name, "type": ty, "ttl": ttl, "records": items(values) });
            post_rrset(client, d.id, rrset).await?;
            (M::DnsAdded, values.len())
        }
        Op::Set => {
            let fqdn = to_fqdn(name, &d.name);
            let resp = client.n_send::<DNSDomainRecords>(d.id).await?;
            let current: Vec<String> = find_rrset(&resp.rrsets, &fqdn, &ty)
                .map(|r| r.records.iter().map(|rec| rec.content.clone()).collect())
                .unwrap_or_default();
            // Remove values that are absent from the target set.
            let stale: Vec<String> = current
                .into_iter()
                .filter(|c| !values.contains(c))
                .collect();
            if !stale.is_empty() {
                let del = json!({ "name": name, "type": ty, "changetype": "DELETE", "records": items(&stale) });
                post_rrset(client, d.id, del).await?;
            }
            let rrset = json!({ "name": name, "type": ty, "ttl": ttl, "records": items(values) });
            post_rrset(client, d.id, rrset).await?;
            (M::DnsSet, values.len())
        }
        Op::Remove => {
            let targets: Vec<String> = if values.is_empty() {
                let fqdn = to_fqdn(name, &d.name);
                let resp = client.n_send::<DNSDomainRecords>(d.id).await?;
                let rrset = find_rrset(&resp.rrsets, &fqdn, &ty).ok_or_else(|| {
                    anyhow::anyhow!(i18n::f(M::RecordNotFound, &[("name", name), ("type", &ty)]))
                })?;
                rrset.records.iter().map(|r| r.content.clone()).collect()
            } else {
                values.to_vec()
            };
            if targets.is_empty() {
                bail!(i18n::f(
                    M::NothingToDelete,
                    &[("name", name), ("type", &ty)]
                ));
            }
            let del = json!({ "name": name, "type": ty, "changetype": "DELETE", "records": items(&targets) });
            post_rrset(client, d.id, del).await?;
            (M::DnsRemoved, targets.len())
        }
    };

    Ok(i18n::f(
        msg,
        &[
            ("name", name),
            ("type", &ty),
            ("domain", &d.name),
            ("count", &count.to_string()),
        ],
    ))
}

async fn dnssec(client: &Client, cmd: DnssecCommand) -> Result<DnssecResp> {
    match cmd {
        DnssecCommand::Status { domain } => {
            let d = resolve_domain(client, &domain).await?;

            let r = client.n_send::<DnssecGet>(d.id).await?;
            Ok(r)
        }
        DnssecCommand::Enable { domain } => {
            let d = resolve_domain(client, &domain).await?;
            let r = client.n_send::<DnssecPost>(d.id).await?;
            Ok(r)
        }
        DnssecCommand::Disable { domain, force } => {
            let d = resolve_domain(client, &domain).await?;
            let has_force = if force {
                Some("?force=true".to_string())
            } else {
                None
            };

            let r = client.n_send::<DnssecDelete>((d.id, has_force)).await?;
            Ok(r)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn to_fqdn_normalizes_names() {
        // Apex aliases.
        assert_eq!(to_fqdn("@", "example.com"), "example.com.");
        assert_eq!(to_fqdn("", "example.com"), "example.com.");
        assert_eq!(to_fqdn("example.com", "example.com"), "example.com.");
        // Relative and absolute forms converge.
        assert_eq!(to_fqdn("www", "example.com"), "www.example.com.");
        assert_eq!(
            to_fqdn("www.example.com", "example.com"),
            "www.example.com."
        );
        // Case and trailing dots are ignored.
        assert_eq!(
            to_fqdn("WWW.Example.COM.", "example.com."),
            "www.example.com."
        );
    }

    #[test]
    fn normalize_content_matches_backend_canonical_forms() {
        // CNAME/NS/PTR targets get a trailing dot (the backend rejects bare names).
        assert_eq!(
            normalize_content("CNAME", "target.example.com"),
            "target.example.com."
        );
        assert_eq!(
            normalize_content("CNAME", "target.example.com."),
            "target.example.com."
        );
        assert_eq!(
            normalize_content("NS", "ns1.example.com"),
            "ns1.example.com."
        );
        // MX/SRV: only the FQDN target token is dotted.
        assert_eq!(
            normalize_content("MX", "10 mail.example.com"),
            "10 mail.example.com."
        );
        assert_eq!(
            normalize_content("MX", "10 mail.example.com."),
            "10 mail.example.com."
        );
        assert_eq!(
            normalize_content("SRV", "5 0 5060 sip.example.com"),
            "5 0 5060 sip.example.com."
        );
        // Malformed MX/SRV are passed through for the server to report the format error.
        assert_eq!(normalize_content("MX", "10"), "10");
        // TXT values are quoted, already-quoted values are untouched.
        assert_eq!(normalize_content("TXT", "v=spf1 -all"), "\"v=spf1 -all\"");
        assert_eq!(
            normalize_content("TXT", "\"v=spf1 -all\""),
            "\"v=spf1 -all\""
        );
        // Address records are untouched.
        assert_eq!(normalize_content("A", "1.1.1.1"), "1.1.1.1");
    }

    #[test]
    fn find_rrset_matches_case_and_dot_insensitively() {
        let rrsets = vec![RRSet {
            name: "www.example.com.".into(),
            rr_type: "A".into(),
            ttl: Some(300),
            records: vec![],
            proxied: false,
        }];
        assert!(find_rrset(&rrsets, "www.example.com.", "a").is_some());
        assert!(find_rrset(&rrsets, "www.example.com", "A").is_some());
        assert!(find_rrset(&rrsets, "other.example.com.", "A").is_none());
        assert!(find_rrset(&rrsets, "www.example.com.", "TXT").is_none());
    }

    /// Mounts the domain list every `change()` call starts from (`resolve_domain`).
    async fn mount_domain(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api/v1/domains"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!([{"id": 7, "name": "example.com"}])),
            )
            .mount(server)
            .await;
    }

    async fn mount_records(server: &MockServer, rrsets: Value) {
        Mock::given(method("GET"))
            .and(path("/api/v1/domains/7/records"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "rrsets": rrsets })))
            .mount(server)
            .await;
    }

    fn client(server: &MockServer) -> Client {
        Client::new(&server.uri(), "wsk_test").unwrap()
    }

    #[tokio::test]
    async fn add_posts_default_rrset_without_fetching_records() {
        let server = MockServer::start().await;
        mount_domain(&server).await;
        // The server merges on the default changetype, so `add` must NOT read
        // the current records and must NOT send a DELETE.
        Mock::given(method("POST"))
            .and(path("/api/v1/domains/7/records"))
            .and(body_json(json!({
                "rrsets": [{
                    "name": "www", "type": "A", "ttl": 300,
                    "records": [{"content": "1.1.1.1"}],
                }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        change(
            &client(&server),
            "example.com",
            "www",
            "a",
            &["1.1.1.1".to_string()],
            300,
            Op::Add,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn add_normalizes_cname_target_to_fqdn_with_dot() {
        let server = MockServer::start().await;
        mount_domain(&server).await;
        // The bare target must reach the API with a trailing dot, as the panel does.
        Mock::given(method("POST"))
            .and(path("/api/v1/domains/7/records"))
            .and(body_json(json!({
                "rrsets": [{
                    "name": "www", "type": "CNAME", "ttl": 300,
                    "records": [{"content": "target.example.com."}],
                }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        change(
            &client(&server),
            "example.com",
            "www",
            "cname",
            &["target.example.com".to_string()],
            300,
            Op::Add,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn set_reconciles_deletes_stale_then_posts_targets() {
        let server = MockServer::start().await;
        mount_domain(&server).await;
        // Current server state: 1.1.1.1 (stale) + 2.2.2.2 (kept).
        mount_records(
            &server,
            json!([{
                "name": "www.example.com.", "type": "A", "ttl": 300,
                "records": [{"content": "1.1.1.1"}, {"content": "2.2.2.2"}],
            }]),
        )
        .await;
        // Reconcile step 1: DELETE only the stale value.
        Mock::given(method("POST"))
            .and(path("/api/v1/domains/7/records"))
            .and(body_json(json!({
                "rrsets": [{
                    "name": "www", "type": "A", "changetype": "DELETE",
                    "records": [{"content": "1.1.1.1"}],
                }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        // Reconcile step 2: default POST of the full target set (server merges).
        Mock::given(method("POST"))
            .and(path("/api/v1/domains/7/records"))
            .and(body_json(json!({
                "rrsets": [{
                    "name": "www", "type": "A", "ttl": 300,
                    "records": [{"content": "2.2.2.2"}, {"content": "3.3.3.3"}],
                }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        change(
            &client(&server),
            "example.com",
            "www",
            "a",
            &["2.2.2.2".to_string(), "3.3.3.3".to_string()],
            300,
            Op::Set,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn set_without_existing_rrset_sends_no_delete() {
        let server = MockServer::start().await;
        mount_domain(&server).await;
        mount_records(&server, json!([])).await;
        // Only the default POST is mounted; an unexpected DELETE would 404 and fail.
        Mock::given(method("POST"))
            .and(path("/api/v1/domains/7/records"))
            .and(body_json(json!({
                "rrsets": [{
                    "name": "www", "type": "A", "ttl": 300,
                    "records": [{"content": "1.1.1.1"}],
                }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        change(
            &client(&server),
            "example.com",
            "www",
            "a",
            &["1.1.1.1".to_string()],
            300,
            Op::Set,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn remove_without_values_deletes_whole_rrset() {
        let server = MockServer::start().await;
        mount_domain(&server).await;
        mount_records(
            &server,
            json!([{
                "name": "www.example.com.", "type": "A", "ttl": 300,
                "records": [{"content": "1.1.1.1"}, {"content": "2.2.2.2"}],
            }]),
        )
        .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/domains/7/records"))
            .and(body_json(json!({
                "rrsets": [{
                    "name": "www", "type": "A", "changetype": "DELETE",
                    "records": [{"content": "1.1.1.1"}, {"content": "2.2.2.2"}],
                }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        change(
            &client(&server),
            "example.com",
            "www",
            "a",
            &[],
            0,
            Op::Remove,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn remove_of_missing_rrset_fails() {
        let server = MockServer::start().await;
        mount_domain(&server).await;
        mount_records(&server, json!([])).await;

        let err = change(
            &client(&server),
            "example.com",
            "www",
            "a",
            &[],
            0,
            Op::Remove,
        )
        .await
        .unwrap_err();
        assert!(format!("{err:#}").contains("www"), "got: {err:#}");
    }

    #[tokio::test]
    async fn unknown_domain_fails_before_any_write() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/domains"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let err = change(
            &client(&server),
            "missing.com",
            "www",
            "a",
            &["1.1.1.1".to_string()],
            300,
            Op::Add,
        )
        .await
        .unwrap_err();
        assert!(format!("{err:#}").contains("missing.com"), "got: {err:#}");
    }
}
