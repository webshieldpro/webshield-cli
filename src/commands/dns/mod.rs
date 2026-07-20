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

mod rr_type;

use crate::api::models::dns::{
    ChangeType, DNSDomainRecords, DNSDomainRecordsPost, DnsRecords, DnssecDelete, DnssecGet,
    DnssecPost, DnssecResp, RRSet, RRSetList, RecordItem,
};
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::i18n::{self, M};
use crate::Context;
use anyhow::{bail, Result};
use clap::Subcommand;
use rr_type::RrType;
use std::borrow::Cow;

#[derive(Subcommand)]
pub enum DnsCommand {
    // TODO Duplicated code fragment
    /// List DNS records of a domain.
    List {
        domain: String,
        /// Filter by record type (A, AAAA, CNAME, TXT, MX, …).
        // #[arg(long = "type")]
        rr_type: Option<RrType>,
    },
    /// Add value(s) to a record (appends for A/AAAA/TXT).
    Add {
        domain: String,
        /// Record name relative to the domain, or `@` for the apex.
        name: String,
        /// Record type.
        #[arg(name = "TYPE")]
        rr_type: RrType,
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
        rr_type: RrType,
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
        rr_type: RrType,
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
    let client = ctx.new_client()?;
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
        } => change(&client, &domain, name, rr_type, &value, ttl, Op::Add)
            .await
            .map(ProgramRes::from),
        DnsCommand::Set {
            domain,
            name,
            rr_type,
            value,
            ttl,
        } => change(&client, &domain, name, rr_type, &value, ttl, Op::Set)
            .await
            .map(ProgramRes::from),
        DnsCommand::Remove {
            domain,
            name,
            rr_type,
            value,
        } => change(&client, &domain, name, rr_type, &value, 0, Op::Remove)
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

async fn post_rrset(client: &Client, domain_id: i64, rrset: RRSet<'_>) -> Result<()> {
    client
        .send_json::<DNSDomainRecordsPost>(
            DnsRecords {
                rrsets: vec![rrset],
                records_used: None,
                records_limit: None,
            },
            domain_id,
        )
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

fn find_rrset<'a>(records: &'a [RRSet<'a>], fqdn: &str, rr_type: &str) -> Option<&'a RRSet<'a>> {
    let want = fqdn.trim_end_matches('.');
    let ty = rr_type.to_uppercase();
    records.iter().find(|r| {
        r.name.trim_end_matches('.').eq_ignore_ascii_case(want)
            && r.rr_type.eq_ignore_ascii_case(&ty)
    })
}

async fn list(
    client: &Client,
    domain: &str,
    rr_type: Option<RrType>,
) -> Result<RRSetList<'static>> {
    let d = resolve_domain(client, domain).await?;
    let resp = client.send::<DNSDomainRecords>(d.id).await?;
    let rrsets: RRSetList = resp
        .rrsets
        .into_iter()
        .filter(|r| {
            rr_type
                .as_ref()
                .is_none_or(|f| r.rr_type.eq_ignore_ascii_case(f.as_str()))
        })
        .collect::<Vec<RRSet>>()
        .into();

    Ok(rrsets)
}

fn content(vl: Vec<String>) -> Vec<RecordItem<'static>> {
    vl.into_iter()
        .map(|c| RecordItem {
            content: Cow::Owned(c),
            disabled: false,
        })
        .collect()
}

fn dns_set<'a>(name: &'a str, ty: &'a str, ttl: i64, values: Vec<String>) -> RRSet<'a> {
    RRSet {
        name: Cow::Borrowed(name),
        rr_type: Cow::Borrowed(ty),
        records: content(values),
        ttl: Some(ttl),
        change_type: None,
        proxied: false,
    }
}

fn dns_req_del<'a>(name: &'a str, ty: &'a str, stale: Vec<String>) -> RRSet<'a> {
    RRSet {
        name: Cow::Borrowed(name),
        rr_type: Cow::Borrowed(ty),
        records: content(stale),
        ttl: None,
        change_type: Some(ChangeType::DELETE),
        proxied: false,
    }
}

/// Single entry point for add/set/remove — they differ only in the rrsets they build.
async fn change(
    client: &Client,
    domain: &str,
    name: String,
    rr_type: RrType,
    values: &[String],
    ttl: i64,
    op: Op,
) -> Result<String> {
    // TODO
    let d = resolve_domain(client, domain).await?;

    let ty = rr_type.as_str().to_uppercase();

    let values = values
        .iter()
        .map(|v| rr_type.normalize(v))
        .collect::<Vec<String>>();

    let (msg, count) = match op {
        Op::Add => {
            let l = values.len();

            let rrset = dns_set(&name, &ty, ttl, values);
            post_rrset(client, d.id, rrset).await?;
            (M::DnsAdded, l)
        }
        Op::Set => {
            let fqdn = to_fqdn(&name, &d.name);
            let resp = client.send::<DNSDomainRecords>(d.id).await?;

            let current = find_rrset(&resp.rrsets, &fqdn, &ty)
                .map(|r| {
                    r.records
                        .iter()
                        .map(|rec| rec.content.to_string())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            // Remove values that are absent from the target set.
            let stale: Vec<String> = current
                .into_iter()
                .filter(|c| !values.contains(c))
                .collect();
            if !stale.is_empty() {
                let del = dns_req_del(&name, &ty, stale);

                post_rrset(client, d.id, del).await?;
            }
            let l = values.len();
            let rrset = dns_set(&name, &ty, ttl, values);
            post_rrset(client, d.id, rrset).await?;
            (M::DnsSet, l)
        }
        Op::Remove => {
            let targets: Vec<String> = if values.is_empty() {
                let fqdn = to_fqdn(&name, &d.name);
                let resp = client.send::<DNSDomainRecords>(d.id).await?;
                let rrset = find_rrset(&resp.rrsets, &fqdn, &ty).ok_or_else(|| {
                    anyhow::anyhow!(i18n::f(
                        M::RecordNotFound,
                        &[("name", &name), ("type", &ty)]
                    ))
                })?;
                rrset
                    .records
                    .iter()
                    .map(|r| r.content.to_string())
                    .collect()
            } else {
                values
            };
            if targets.is_empty() {
                bail!(i18n::f(
                    M::NothingToDelete,
                    &[("name", &name), ("type", &ty)]
                ));
            }

            let l = targets.len();
            let del = dns_req_del(&name, &ty, targets);
            post_rrset(client, d.id, del).await?;
            (M::DnsRemoved, l)
        }
    };

    Ok(i18n::f(
        msg,
        &[
            ("name", &name),
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

            let r = client.send::<DnssecGet>(d.id).await?;
            Ok(r)
        }
        DnssecCommand::Enable { domain } => {
            let d = resolve_domain(client, &domain).await?;
            let r = client.send::<DnssecPost>(d.id).await?;
            Ok(r)
        }
        DnssecCommand::Disable { domain, force } => {
            let d = resolve_domain(client, &domain).await?;
            let has_force = if force {
                Some("?force=true".to_string())
            } else {
                None
            };

            let r = client.send::<DnssecDelete>((d.id, has_force)).await?;
            Ok(r)
        }
    }
}
