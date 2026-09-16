//! Hostnames of domains connected via DNS records, without delegating nameservers.
//!
//! Flow: `hosts add` → set the ownership record → `hosts verify` (confirms and issues
//! the certificate) → point the hostname itself at the same target.

use crate::api::models::hosts::{
    ExternalHost, ExternalHostList, HostDelete, HostNew, HostNewReq, HostVerify, Hosts,
};
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::t;
use crate::util::context::Context;
use crate::util::input::confirm;
use crate::util::output::{info, success, warn};
use anyhow::Result;
use clap::{Subcommand, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
pub enum HostMode {
    Proxy,
    Static,
    Redirect,
}

impl HostMode {
    fn as_api(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::Static => "static",
            Self::Redirect => "redirect",
        }
    }
}

#[derive(Subcommand)]
#[command(about = t!(cmd_hosts))]
pub enum HostsCommand {
    #[command(about = t!(cmd_hosts_list))]
    List {
        #[arg(long, help = t!(arg_hosts_domain_filter))]
        domain: Option<String>,
    },
    #[command(about = t!(cmd_hosts_add))]
    Add {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
        #[arg(long, help = t!(arg_proxy_domain))]
        domain: String,
        #[arg(long, value_enum, default_value_t = HostMode::Proxy, help = t!(arg_hosts_mode))]
        mode: HostMode,
        #[arg(long, help = t!(arg_hosts_origin))]
        origin: Option<String>,
        #[arg(long, help = t!(arg_hosts_origin_http))]
        origin_http: bool,
        #[arg(long, help = t!(arg_proxy_redirect_target))]
        redirect_target: Option<String>,
    },
    #[command(about = t!(cmd_hosts_records))]
    Records {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
    #[command(about = t!(cmd_hosts_verify))]
    Verify {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
    #[command(about = t!(cmd_hosts_remove))]
    Remove {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
}

impl Run for HostsCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let yes = ctx.yes;
        let client = ctx.client()?;
        match self {
            Self::List { domain } => list(client, domain).await.map(ProgramRes::from),
            Self::Add {
                hostname,
                domain,
                mode,
                origin,
                origin_http,
                redirect_target,
            } => {
                let d = resolve_domain(client, &domain).await?;
                let host = client
                    .send_json::<HostNew>(
                        HostNewReq {
                            domain_id: d.id,
                            hostname,
                            mode: mode.as_api().to_string(),
                            origin: origin.unwrap_or_default(),
                            origin_ssl: !origin_http,
                            redirect_target: redirect_target.unwrap_or_default(),
                        },
                        (),
                    )
                    .await?;
                success(t!(host_added, &host.hostname));
                info(t!(host_next_ownership));
                Ok(ProgramRes::from(host.records))
            }
            Self::Records { hostname } => {
                let host = resolve_host(client, &hostname).await?;
                Ok(ProgramRes::from(host.records))
            }
            Self::Verify { hostname } => verify(client, &hostname).await.map(ProgramRes::from),
            Self::Remove { hostname } => {
                let host = resolve_host(client, &hostname).await?;
                confirm(yes, &t!(confirm_remove_host, &host.hostname))?;
                client.send::<HostDelete>(host.id).await?;
                Ok(ProgramRes::from(t!(host_removed, &host.hostname)))
            }
        }
    }
}

async fn list(client: &Client<'_>, domain: Option<String>) -> Result<ExternalHostList> {
    let domain_id = match domain {
        Some(name) => Some(resolve_domain(client, &name).await?.id),
        None => None,
    };
    client.send::<Hosts>(domain_id).await
}

async fn verify(client: &Client<'_>, hostname: &str) -> Result<ExternalHost> {
    let host = resolve_host(client, hostname).await?;
    let res = client.send::<HostVerify>(host.id).await?;
    let host = res.host;
    match res.check.as_ref().map(|c| c.code.as_str()) {
        Some("ok") => {
            success(t!(host_verified, &host.hostname));
            if host.certificate_status == "issued" {
                info(t!(
                    host_next_traffic,
                    &host.records.traffic.name,
                    &host.records.target
                ));
            } else {
                info(t!(host_cert_pending));
            }
        }
        Some("mismatch") => warn(t!(
            host_check_mismatch,
            &host.records.ownership.name,
            &res.check
                .as_ref()
                .map(|c| c.detail.clone())
                .unwrap_or_default()
        )),
        Some("error") => warn(t!(host_check_error)),
        _ => warn(t!(host_check_missing, &host.records.ownership.name)),
    }
    Ok(host)
}

/// Resolves a connected (not removed) hostname of the account.
async fn resolve_host(client: &Client<'_>, hostname: &str) -> Result<ExternalHost> {
    let needle = hostname.trim().trim_end_matches('.').to_lowercase();
    let hosts = client.send::<Hosts>(None).await?;
    hosts
        .0
        .into_iter()
        .find(|h| h.hostname == needle && h.status != "removed")
        .ok_or_else(|| anyhow::anyhow!(t!(host_not_found, hostname)))
}
