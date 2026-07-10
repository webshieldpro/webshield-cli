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

use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::{json, Value};

use crate::api::models::{RRSet, RecordsResponse};
use crate::api::Client;
use crate::commands::resolve_domain;
use crate::i18n::{self, M};
use crate::output::{print_json, print_table, success, OutputFormat};
use crate::Context;

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

pub async fn run(ctx: &Context, cmd: DnsCommand) -> Result<()> {
    let client = ctx.client()?;
    match cmd {
        DnsCommand::List { domain, rr_type } => list(ctx, &client, &domain, rr_type).await,
        DnsCommand::Add { domain, name, rr_type, value, ttl } => {
            change(&client, &domain, &name, &rr_type, &value, ttl, Op::Add).await
        }
        DnsCommand::Set { domain, name, rr_type, value, ttl } => {
            change(&client, &domain, &name, &rr_type, &value, ttl, Op::Set).await
        }
        DnsCommand::Remove { domain, name, rr_type, value } => {
            change(&client, &domain, &name, &rr_type, &value, 0, Op::Remove).await
        }
        DnsCommand::Dnssec(sub) => dnssec(&client, sub).await,
    }
}

enum Op {
    Add,
    Set,
    Remove,
}

async fn fetch_records(client: &Client, domain_id: i64) -> Result<RecordsResponse> {
    client.get_json(&format!("domains/{domain_id}/records")).await
}

async fn post_rrset(client: &Client, domain_id: i64, rrset: Value) -> Result<()> {
    let body = json!({ "rrsets": [rrset] });
    let _: Value = client.post_json(&format!("domains/{domain_id}/records"), &body).await?;
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

fn find_rrset<'a>(records: &'a [RRSet], fqdn: &str, rr_type: &str) -> Option<&'a RRSet> {
    let want = fqdn.trim_end_matches('.');
    let ty = rr_type.to_uppercase();
    records.iter().find(|r| {
        r.name.trim_end_matches('.').eq_ignore_ascii_case(want) && r.rr_type.eq_ignore_ascii_case(&ty)
    })
}

async fn list(ctx: &Context, client: &Client, domain: &str, rr_type: Option<String>) -> Result<()> {
    let d = resolve_domain(client, domain).await?;
    let resp = fetch_records(client, d.id).await?;
    let filter = rr_type.map(|t| t.to_uppercase());
    let rrsets: Vec<&RRSet> = resp
        .rrsets
        .iter()
        .filter(|r| filter.as_ref().is_none_or(|f| r.rr_type.eq_ignore_ascii_case(f)))
        .collect();

    if ctx.output == OutputFormat::Json {
        return print_json(&rrsets);
    }
    let yes = i18n::tr(M::Yes);
    let rows = rrsets
        .iter()
        .map(|r| {
            let values = r.records.iter().map(|rec| rec.content.clone()).collect::<Vec<_>>().join(", ");
            vec![
                r.name.clone(),
                r.rr_type.clone(),
                r.ttl.map(|t| t.to_string()).unwrap_or_default(),
                if r.proxied { yes.into() } else { String::new() },
                values,
            ]
        })
        .collect();
    print_table(
        &[
            i18n::tr(M::HName),
            i18n::tr(M::HType),
            i18n::tr(M::HTtl),
            i18n::tr(M::HProxy),
            i18n::tr(M::HValues),
        ],
        rows,
    );
    if let Some(used) = resp.records_used {
        let limit = resp.records_limit.map(|l| l.to_string()).unwrap_or_else(|| "∞".into());
        crate::output::info(&i18n::f(M::RecordsCount, &[("used", &used.to_string()), ("limit", &limit)]));
    }
    Ok(())
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
) -> Result<()> {
    let d = resolve_domain(client, domain).await?;
    let ty = rr_type.to_uppercase();
    let items = |vals: &[String]| -> Vec<Value> { vals.iter().map(|v| json!({ "content": v })).collect() };

    let (msg, count) = match op {
        Op::Add => {
            let rrset = json!({ "name": name, "type": ty, "ttl": ttl, "records": items(values) });
            post_rrset(client, d.id, rrset).await?;
            (M::DnsAdded, values.len())
        }
        Op::Set => {
            let fqdn = to_fqdn(name, &d.name);
            let resp = fetch_records(client, d.id).await?;
            let current: Vec<String> = find_rrset(&resp.rrsets, &fqdn, &ty)
                .map(|r| r.records.iter().map(|rec| rec.content.clone()).collect())
                .unwrap_or_default();
            // Remove values that are absent from the target set.
            let stale: Vec<String> = current.into_iter().filter(|c| !values.contains(c)).collect();
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
                let resp = fetch_records(client, d.id).await?;
                let rrset = find_rrset(&resp.rrsets, &fqdn, &ty).ok_or_else(|| {
                    anyhow::anyhow!(i18n::f(M::RecordNotFound, &[("name", name), ("type", &ty)]))
                })?;
                rrset.records.iter().map(|r| r.content.clone()).collect()
            } else {
                values.to_vec()
            };
            if targets.is_empty() {
                bail!(i18n::f(M::NothingToDelete, &[("name", name), ("type", &ty)]));
            }
            let del = json!({ "name": name, "type": ty, "changetype": "DELETE", "records": items(&targets) });
            post_rrset(client, d.id, del).await?;
            (M::DnsRemoved, targets.len())
        }
    };

    success(&i18n::f(
        msg,
        &[("name", name), ("type", &ty), ("domain", &d.name), ("count", &count.to_string())],
    ));
    Ok(())
}

async fn dnssec(client: &Client, cmd: DnssecCommand) -> Result<()> {
    match cmd {
        DnssecCommand::Status { domain } => {
            let d = resolve_domain(client, &domain).await?;
            let result: Value = client.get_json(&format!("domains/{}/dnssec", d.id)).await?;
            print_json(&result)
        }
        DnssecCommand::Enable { domain } => {
            let d = resolve_domain(client, &domain).await?;
            let result = client.post_empty(&format!("domains/{}/dnssec", d.id)).await?;
            success(i18n::tr(M::DnssecEnabled));
            print_json(&result)
        }
        DnssecCommand::Disable { domain, force } => {
            let d = resolve_domain(client, &domain).await?;
            let path = if force {
                format!("domains/{}/dnssec?force=true", d.id)
            } else {
                format!("domains/{}/dnssec", d.id)
            };
            let result = client.delete(&path).await?;
            success(i18n::tr(M::DnssecDisabled));
            print_json(&result)
        }
    }
}
