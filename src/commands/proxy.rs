//! Edge settings of proxied/redirect hosts (`/nginx-configs`, scope `proxy`).

use crate::api::_models::proxy::{ProxyDecl, ProxyDelete, ProxyInfo, ProxyNew, ProxyPatch};
use crate::api::models::ProxyConfig;
use crate::api::Client;
use crate::commands::confirm;
use crate::commands::domains::resolve_domain;
use crate::i18n::{self, M};
use crate::output::{print_json, print_table, success, OutputFormat};
use crate::Context;
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
struct SetImpl {
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

pub async fn run(ctx: &Context, cmd: ProxyCommand) -> Result<()> {
    let client = ctx.client()?;
    match cmd {
        ProxyCommand::List => list(ctx, &client).await,
        ProxyCommand::Get { hostname } => {
            let cfg = resolve_proxy(&client, &hostname).await?;
            print_json(&cfg)
        }
        ProxyCommand::Set(s) => set(&client, s).await,
        ProxyCommand::Remove { hostname } => {
            let cfg = resolve_proxy(&client, &hostname).await?;
            confirm(
                ctx.yes,
                &i18n::f(M::ConfirmRemoveProxy, &[("host", &hostname)]),
            )?;
            client.n_send::<ProxyDelete>(cfg.id).await?;
            success(&i18n::f(M::ProxyRemoved, &[("host", &hostname)]));
            Ok(())
        }
    }
}

async fn resolve_proxy(client: &Client, hostname: &str) -> Result<ProxyConfig> {
    let needle = hostname.trim().to_lowercase();
    let configs: Vec<ProxyConfig> = client.list_all("nginx-configs").await?;
    configs
        .into_iter()
        .find(|c| c.hostname.eq_ignore_ascii_case(&needle))
        .ok_or_else(|| anyhow::anyhow!(i18n::f(M::NotFoundProxy, &[("host", hostname)])))
}

async fn list(ctx: &Context, client: &Client) -> Result<()> {
    let configs: Vec<ProxyConfig> = client.list_all("nginx-configs").await?;
    if ctx.output == OutputFormat::Json {
        return print_json(&configs);
    }
    let yes = i18n::tr(M::Yes);
    let rows = configs
        .iter()
        .map(|c| {
            vec![
                c.hostname.clone(),
                c.domain_name.clone().unwrap_or_default(),
                c.mode.clone().unwrap_or_default(),
                c.redirect_target.clone().unwrap_or_default(),
                if c.ssl_required.unwrap_or(false) {
                    yes.into()
                } else {
                    String::new()
                },
                if c.bot_protection_enabled.unwrap_or(false) {
                    yes.into()
                } else {
                    String::new()
                },
            ]
        })
        .collect();
    print_table(
        &[
            i18n::tr(M::HHost),
            i18n::tr(M::HDomain),
            i18n::tr(M::HMode),
            i18n::tr(M::HTarget),
            i18n::tr(M::HSsl),
            i18n::tr(M::HBotProt),
        ],
        rows,
    );
    Ok(())
}

/// Upsert: PATCH when the config already exists, otherwise POST (domain required).
async fn set(client: &Client, mut set: SetImpl) -> Result<()> {
    let hostname = set.hostname;
    let existing = {
        let configs: Vec<ProxyConfig> = client.list_all("nginx-configs").await?;
        configs
            .into_iter()
            .find(|c| c.hostname.eq_ignore_ascii_case(&hostname))
    };

    if let Some(cfg) = existing {
        // Partial update of an existing config.
        client.n_send_ser::<ProxyPatch>(set.info, cfg.id).await?;
        success(&i18n::f(M::ProxyUpdated, &[("host", &hostname)]));
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
        success(&i18n::f(M::ProxyCreated, &[("host", &hostname)]));
    }
    Ok(())
}
