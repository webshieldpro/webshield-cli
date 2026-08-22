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
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::t;
use crate::util::context::Context;
use anyhow::{bail, Result};
use clap::Subcommand;
use rr_type::RrType;
use std::borrow::Cow;

#[derive(Subcommand)]
pub enum DnsCommand {
    #[command(about = t!(cmd_dns_list))]
    List {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(help = t!(arg_record_type_filter))]
        rr_type: Option<RrType>,
    },
    #[command(about = t!(cmd_dns_add))]
    Add {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(help = t!(arg_record_name))]
        name: String,
        #[arg(name = "TYPE", help = t!(arg_record_type))]
        rr_type: RrType,
        #[arg(required = true, help = t!(arg_record_value))]
        value: Vec<String>,
        #[arg(long, default_value_t = 300, help = t!(arg_record_ttl))]
        ttl: i64,
    },
    #[command(about = t!(cmd_dns_set))]
    Set {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(help = t!(arg_record_name))]
        name: String,
        #[arg(name = "TYPE", help = t!(arg_record_type))]
        rr_type: RrType,
        #[arg(required = true, help = t!(arg_record_value))]
        value: Vec<String>,
        #[arg(long, default_value_t = 300, help = t!(arg_record_ttl))]
        ttl: i64,
    },
    #[command(about = t!(cmd_dns_remove))]
    Remove {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(help = t!(arg_record_name))]
        name: String,
        #[arg(name = "TYPE", help = t!(arg_record_type))]
        rr_type: RrType,
        #[arg(help = t!(arg_record_value_remove))]
        value: Vec<String>,
    },
    #[command(subcommand)]
    #[command(about = t!(cmd_dns_dnssec))]
    Dnssec(DnssecCommand),
}

#[derive(Subcommand)]
pub enum DnssecCommand {
    #[command(about = t!(cmd_dnssec_status))]
    Status {
        #[arg(help = t!(arg_domain))]
        domain: String,
    },
    #[command(about = t!(cmd_dnssec_enable))]
    Enable {
        #[arg(help = t!(arg_domain))]
        domain: String,
    },
    #[command(about = t!(cmd_dnssec_disable))]
    Disable {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(long, help = t!(arg_dnssec_force))]
        force: bool,
    },
}

impl Run for DnsCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let client = ctx.client()?;
        match self {
            Self::List { domain, rr_type } => {
                list(client, &domain, rr_type).await.map(ProgramRes::from)
            }
            Self::Add {
                domain,
                name,
                rr_type,
                value,
                ttl,
            } => change(client, &domain, name, rr_type, &value, ttl, Op::Add)
                .await
                .map(ProgramRes::from),
            Self::Set {
                domain,
                name,
                rr_type,
                value,
                ttl,
            } => change(client, &domain, name, rr_type, &value, ttl, Op::Set)
                .await
                .map(ProgramRes::from),
            Self::Remove {
                domain,
                name,
                rr_type,
                value,
            } => change(client, &domain, name, rr_type, &value, 0, Op::Remove)
                .await
                .map(ProgramRes::from),
            Self::Dnssec(sub) => dnssec(client, sub).await.map(ProgramRes::from),
        }
    }
}

enum Op {
    Add,
    Set,
    Remove,
}

async fn post_rrset(client: &Client<'_>, domain_id: i64, rrset: RRSet<'_>) -> Result<()> {
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
    client: &Client<'_>,
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
        change_type: Some(ChangeType::Delete),
        proxied: false,
    }
}

/// Single entry point for add/set/remove — they differ only in the rrsets they build.
async fn change(
    client: &Client<'_>,
    domain: &str,
    name: String,
    rr_type: RrType,
    values: &[String],
    ttl: i64,
    op: Op,
) -> Result<String> {
    let d = resolve_domain(client, domain).await?;

    let ty = rr_type.as_str().to_uppercase();

    let values = values
        .iter()
        .map(|v| rr_type.normalize(v))
        .collect::<Vec<String>>();

    // The message differs per operation; the locale gives back a formatter.
    let (msg, count) = match op {
        Op::Add => {
            let l = values.len();

            let rrset = dns_set(&name, &ty, ttl, values);
            post_rrset(client, d.id, rrset).await?;
            (t!(dns_added), l)
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
            (t!(dns_set), l)
        }
        Op::Remove => {
            let targets: Vec<String> = if values.is_empty() {
                let fqdn = to_fqdn(&name, &d.name);
                let resp = client.send::<DNSDomainRecords>(d.id).await?;
                let rrset = find_rrset(&resp.rrsets, &fqdn, &ty)
                    .ok_or_else(|| anyhow::anyhow!(t!(record_not_found, &name, &ty)))?;
                rrset
                    .records
                    .iter()
                    .map(|r| r.content.to_string())
                    .collect()
            } else {
                values
            };
            if targets.is_empty() {
                bail!(t!(nothing_to_delete, &name, &ty));
            }

            let l = targets.len();
            let del = dns_req_del(&name, &ty, targets);
            post_rrset(client, d.id, del).await?;
            (t!(dns_removed), l)
        }
    };

    Ok(msg(&name, &ty, &d.name, &count.to_string()))
}

async fn dnssec(client: &Client<'_>, cmd: DnssecCommand) -> Result<DnssecResp> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rrset(name: &str, ty: &str) -> RRSet<'static> {
        RRSet {
            name: Cow::Owned(name.to_string()),
            rr_type: Cow::Owned(ty.to_string()),
            ttl: Some(300),
            records: Vec::new(),
            proxied: false,
            change_type: None,
        }
    }

    #[test]
    fn to_fqdn_expands_names_relative_to_the_domain() {
        assert_eq!(to_fqdn("@", "example.com"), "example.com.");
        assert_eq!(to_fqdn("", "example.com"), "example.com.");
        assert_eq!(to_fqdn("www", "example.com"), "www.example.com.");
        // An already qualified name is not doubled.
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
    fn find_rrset_matches_case_and_dot_insensitively() {
        let rrsets = vec![rrset("www.example.com.", "A")];
        assert!(find_rrset(&rrsets, "www.example.com.", "a").is_some());
        assert!(find_rrset(&rrsets, "www.example.com", "A").is_some());
        assert!(find_rrset(&rrsets, "other.example.com.", "A").is_none());
        assert!(find_rrset(&rrsets, "www.example.com.", "TXT").is_none());
    }
}
