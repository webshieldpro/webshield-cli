//! Edge settings of proxied/redirect hosts (`/nginx-configs`, scope `proxy`).

use crate::api::_models::proxy::{
    Proxies, ProxyData, ProxyDecl, ProxyDelete, ProxyInfo, ProxyNew, ProxyPatch, ProxyResolve,
};
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::confirm;
use crate::commands::domains::resolve_domain;
use crate::i18n::{self, M};
use crate::Context;
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct SetImpl {
    pub hostname: String,
    /// Owner domain (required when creating).
    #[arg(long)]
    pub domain: String,

    #[command(flatten)]
    pub info: ProxyInfo,
}

#[derive(Subcommand)]
pub enum ProxyCommand {
    /// List proxy/redirect host configs.
    List,
    /// Show a host config.
    Get { hostname: String },
    /// Create or update a host config (partial update if it exists).
    Set(SetImpl),
    /// Remove a host config.
    Remove { hostname: String },
}

pub async fn run(ctx: &Context, cmd: ProxyCommand) -> Result<ProgramRes> {
    let client = ctx.client()?;
    match cmd {
        ProxyCommand::List => list(&client).await.map(ProgramRes::from),
        ProxyCommand::Get { hostname } => resolve_proxy(&client, &hostname)
            .await
            .map(ProgramRes::from),
        ProxyCommand::Set(s) => set(&client, s).await.map(ProgramRes::from),

        ProxyCommand::Remove { hostname } => {
            let cfg = resolve_proxy(&client, &hostname).await?;

            confirm(
                ctx.yes,
                &i18n::f(M::ConfirmRemoveProxy, &[("host", &hostname)]),
            )?;

            client.n_send::<ProxyDelete>(cfg.id).await?;
            Ok(ProgramRes::from(i18n::f(
                M::ProxyRemoved,
                &[("host", &hostname)],
            )))
        }
    }
}

async fn _find_config(client: &Client, hostname: &str) -> Result<ProxyData> {
    let configs = client.n_list::<ProxyResolve>(()).await?;
    let c = configs
        .into_iter()
        .find(|c| c.hostname.eq_ignore_ascii_case(hostname));
    c.ok_or_else(|| anyhow::anyhow!(i18n::f(M::NotFoundProxy, &[("host", hostname)])))
}

async fn resolve_proxy(client: &Client, hostname: &str) -> Result<ProxyData> {
    let needle = hostname.trim().to_lowercase();
    _find_config(client, &needle).await
}

async fn list(client: &Client) -> Result<Proxies> {
    let configs = client.n_list::<ProxyResolve>(()).await?;
    Ok(Proxies::from(configs))
}

/// Upsert: PATCH when the config already exists, otherwise POST (domain required).
async fn set(client: &Client, set: SetImpl) -> Result<String> {
    let hostname = set.hostname;
    let existing = _find_config(client, &hostname).await.ok();

    let res = if let Some(cfg) = existing {
        // Partial update of an existing config.
        client.n_send_ser::<ProxyPatch>(set.info, cfg.id).await?;
        i18n::f(M::ProxyUpdated, &[("host", &hostname)])
    } else {
        let d = resolve_domain(client, &set.domain).await?;

        client
            .n_send_ser::<ProxyNew>(
                ProxyDecl {
                    hostname: hostname.clone(),
                    domain_id: d.id,
                    inner: set.info,
                },
                (),
            )
            .await?;
        i18n::f(M::ProxyCreated, &[("host", &hostname)])
    };
    Ok(res)
}
