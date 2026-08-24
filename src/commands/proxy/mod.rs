//! Edge settings of proxied/redirect hosts (`/nginx-configs`, scope `proxy`).

use crate::api::models::proxy::{
    Proxies, Proxy, ProxyData, ProxyDecl, ProxyDelete, ProxyInfo, ProxyNew, ProxyPatch,
    ProxyResolve,
};
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::commands::util::Page;
use crate::t;
use crate::util::context::Context;
use crate::util::input::confirm;
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct SetImpl {
    #[arg(help = t!(arg_hostname))]
    pub hostname: String,
    #[arg(long, help = t!(arg_proxy_domain))]
    pub domain: String,

    #[command(flatten)]
    pub info: ProxyInfo,
}

#[derive(Subcommand)]
#[command(about = t!(cmd_proxy))]
pub enum ProxyCommand {
    #[command(about = t!(cmd_proxy_list))]
    List(Page),
    #[command(about = t!(cmd_proxy_get))]
    Get {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
    #[command(about = t!(cmd_proxy_set))]
    Set(SetImpl),
    #[command(about = t!(cmd_proxy_remove))]
    Remove {
        #[arg(help = t!(arg_hostname))]
        hostname: String,
    },
}

impl Run for ProxyCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let yes = ctx.yes;
        let client = ctx.client()?;
        match self {
            Self::List(page) => list(client, page.into()).await.map(ProgramRes::from),
            Self::Get { hostname } => resolve_proxy(client, &hostname).await.map(ProgramRes::from),
            Self::Set(s) => set(client, s).await.map(ProgramRes::from),

            Self::Remove { hostname } => {
                let cfg = resolve_proxy(client, &hostname).await?;

                confirm(yes, &t!(confirm_remove_proxy, &hostname))?;

                client.send::<ProxyDelete>(cfg.id).await?;
                Ok(ProgramRes::from(t!(proxy_removed, &hostname)))
            }
        }
    }
}

async fn find_config(client: &Client<'_>, hostname: String) -> Result<ProxyData> {
    let config = client.send::<ProxyResolve>(hostname.clone()).await?;

    config
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!(t!(not_found_proxy, &hostname)))
}

async fn resolve_proxy(client: &Client<'_>, hostname: &str) -> Result<ProxyData> {
    let needle = hostname.trim().to_lowercase();
    find_config(client, needle).await
}

async fn list(client: &Client<'_>, page: u32) -> Result<Proxies> {
    client.send::<Proxy>(page).await
}

/// Upsert: PATCH when the config already exists, otherwise POST (domain required).
async fn set(client: &Client<'_>, set: SetImpl) -> Result<String> {
    let hostname = set.hostname;
    let existing = find_config(client, hostname.clone()).await.ok();

    let res = if let Some(cfg) = existing {
        // Partial update of an existing config.
        client.send_json::<ProxyPatch>(set.info, cfg.id).await?;
        t!(proxy_updated, &hostname)
    } else {
        let d = resolve_domain(client, &set.domain).await?;

        client
            .send_json::<ProxyNew>(
                ProxyDecl {
                    hostname: hostname.clone(),
                    domain_id: d.id,
                    inner: set.info,
                },
                (),
            )
            .await?;
        t!(proxy_created, &hostname)
    };
    Ok(res)
}
