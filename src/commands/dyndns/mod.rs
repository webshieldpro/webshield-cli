//! Hostnames whose address is reported by a router or a scheduled job (dyndns2).
//!
//! This command only manages the hostnames and their tokens. The updates themselves
//! go to a separate endpoint (`dyn.webshield.pro`) with the update token, not with a
//! personal `wsk_…` token — see the documentation for the router and cron setup.

use crate::api::models::dyndns::{
    DynDnsDelete, DynDnsHost, DynDnsHostList, DynDnsHosts, DynDnsNew, DynDnsNewReq, DynDnsRotate,
};
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::commands::prog_res::ProgramRes;
use crate::commands::run::Run;
use crate::t;
use crate::util::context::Context;
use crate::util::input::confirm;
use crate::util::output::{info, success};
use anyhow::Result;
use clap::Subcommand;

/// Matches the backend default: a dynamic address lives for minutes.
const DEFAULT_TTL: i64 = 60;

#[derive(Subcommand)]
#[command(about = t!(cmd_dyndns))]
pub enum DynDnsCommand {
    #[command(about = t!(cmd_dyndns_list))]
    List {
        #[arg(long, help = t!(arg_dyndns_domain_filter))]
        domain: Option<String>,
    },
    #[command(about = t!(cmd_dyndns_add))]
    Add {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
        #[arg(long, help = t!(arg_dyndns_domain))]
        domain: String,
        #[arg(long, default_value_t = DEFAULT_TTL, help = t!(arg_dyndns_ttl))]
        ttl: i64,
        #[arg(long, help = t!(arg_dyndns_comment))]
        comment: Option<String>,
    },
    #[command(about = t!(cmd_dyndns_rotate))]
    Rotate {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
    #[command(about = t!(cmd_dyndns_remove))]
    Remove {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
}

impl Run for DynDnsCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let yes = ctx.yes;
        let client = ctx.client()?;
        match self {
            Self::List { domain } => list(client, domain).await.map(ProgramRes::from),
            Self::Add {
                hostname,
                domain,
                ttl,
                comment,
            } => {
                let d = resolve_domain(client, &domain).await?;
                let host = client
                    .send_json::<DynDnsNew>(
                        DynDnsNewReq {
                            domain_id: d.id,
                            hostname,
                            ttl,
                            comment: comment.unwrap_or_default(),
                        },
                        (),
                    )
                    .await?;
                success(t!(dyndns_added, &host.hostname));
                info(t!(dyndns_token_once));
                Ok(ProgramRes::from(host))
            }
            Self::Rotate { hostname } => {
                let host = resolve_host(client, &hostname).await?;
                confirm(yes, &t!(confirm_rotate_dyndns, &host.hostname))?;
                let host = client.send::<DynDnsRotate>(host.id).await?;
                success(t!(dyndns_rotated, &host.hostname));
                info(t!(dyndns_token_once));
                Ok(ProgramRes::from(host))
            }
            Self::Remove { hostname } => {
                let host = resolve_host(client, &hostname).await?;
                confirm(yes, &t!(confirm_remove_dyndns, &host.hostname))?;
                client.send::<DynDnsDelete>(host.id).await?;
                Ok(ProgramRes::from(t!(dyndns_removed, &host.hostname)))
            }
        }
    }
}

async fn list(client: &Client<'_>, domain: Option<String>) -> Result<DynDnsHostList> {
    let domain_id = match domain {
        Some(name) => Some(resolve_domain(client, &name).await?.id),
        None => None,
    };
    client.send::<DynDnsHosts>(domain_id).await
}

/// Resolves a hostname of the account to its dynamic DNS entry.
async fn resolve_host(client: &Client<'_>, hostname: &str) -> Result<DynDnsHost> {
    let needle = hostname.trim().trim_end_matches('.').to_lowercase();
    let hosts = client.send::<DynDnsHosts>(None).await?;
    hosts
        .results
        .into_iter()
        .find(|h| h.hostname == needle)
        .ok_or_else(|| anyhow::anyhow!(t!(dyndns_not_found, hostname)))
}
