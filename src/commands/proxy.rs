//! Edge settings of proxied/redirect hosts (`/nginx-configs`, scope `proxy`).

use anyhow::{bail, Result};
use clap::Subcommand;
use serde_json::{json, Map, Value};

use crate::api::models::ProxyConfig;
use crate::api::Client;
use crate::commands::{confirm, resolve_domain};
use crate::i18n::{self, M};
use crate::output::{print_json, print_table, success, OutputFormat};
use crate::Context;

#[derive(Subcommand)]
pub enum ProxyCommand {
    /// List proxy/redirect host configs.
    List,
    /// Show a host config.
    Get { hostname: String },
    /// Create or update a host config (partial update if it exists).
    Set {
        hostname: String,
        /// Owner domain (required when creating).
        #[arg(long)]
        domain: Option<String>,
        /// Mode: proxy | redirect.
        #[arg(long)]
        mode: Option<String>,
        /// Redirect target hostname (for mode=redirect).
        #[arg(long)]
        redirect_target: Option<String>,
        /// Require HTTPS (true/false).
        #[arg(long)]
        ssl: Option<bool>,
        /// Bot protection (true/false).
        #[arg(long)]
        bot_protection: Option<bool>,
        /// Captcha check (true/false).
        #[arg(long)]
        captcha: Option<bool>,
        /// HTTP/2 (true/false).
        #[arg(long)]
        http2: Option<bool>,
        /// HTTP/3 (true/false).
        #[arg(long)]
        http3: Option<bool>,
        /// Max request body size in MB (0 = unlimited).
        #[arg(long)]
        max_body_mb: Option<i64>,
        /// Blocked bot slugs, comma-separated (see `webshield bots`).
        #[arg(long)]
        block_bots: Option<String>,
    },
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
        ProxyCommand::Set {
            hostname,
            domain,
            mode,
            redirect_target,
            ssl,
            bot_protection,
            captcha,
            http2,
            http3,
            max_body_mb,
            block_bots,
        } => {
            let mut fields = Map::new();
            if let Some(v) = mode {
                fields.insert("mode".into(), json!(v));
            }
            if let Some(v) = redirect_target {
                fields.insert("redirect_target".into(), json!(v));
            }
            if let Some(v) = ssl {
                fields.insert("ssl_required".into(), json!(v));
            }
            if let Some(v) = bot_protection {
                fields.insert("bot_protection_enabled".into(), json!(v));
            }
            if let Some(v) = captcha {
                fields.insert("captcha_check_enabled".into(), json!(v));
            }
            if let Some(v) = http2 {
                fields.insert("http2_enabled".into(), json!(v));
            }
            if let Some(v) = http3 {
                fields.insert("http3_enabled".into(), json!(v));
            }
            if let Some(v) = max_body_mb {
                fields.insert("max_body_size_mb".into(), json!(v));
            }
            if let Some(v) = block_bots {
                let slugs: Vec<&str> = v.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
                fields.insert("blocked_bots".into(), json!(slugs));
            }
            set(&client, &hostname, domain.as_deref(), fields).await
        }
        ProxyCommand::Remove { hostname } => {
            let cfg = resolve_proxy(&client, &hostname).await?;
            confirm(ctx.yes, &i18n::f(M::ConfirmRemoveProxy, &[("host", &hostname)]))?;
            client.delete(&format!("nginx-configs/{}", cfg.id)).await?;
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
                if c.ssl_required.unwrap_or(false) { yes.into() } else { String::new() },
                if c.bot_protection_enabled.unwrap_or(false) { yes.into() } else { String::new() },
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
async fn set(client: &Client, hostname: &str, domain: Option<&str>, mut fields: Map<String, Value>) -> Result<()> {
    let existing = {
        let configs: Vec<ProxyConfig> = client.list_all("nginx-configs").await?;
        configs.into_iter().find(|c| c.hostname.eq_ignore_ascii_case(hostname))
    };

    if let Some(cfg) = existing {
        // Partial update of an existing config.
        let _: Value = client
            .send_json(
                client
                    .request(reqwest::Method::PATCH, &format!("nginx-configs/{}", cfg.id))
                    .json(&Value::Object(fields)),
            )
            .await?;
        success(&i18n::f(M::ProxyUpdated, &[("host", hostname)]));
    } else {
        let Some(domain) = domain else {
            bail!(i18n::tr(M::NeedDomain));
        };
        let d = resolve_domain(client, domain).await?;
        fields.insert("hostname".into(), json!(hostname));
        fields.insert("domain_id".into(), json!(d.id));
        let _: Value = client.post_json("nginx-configs", &Value::Object(fields)).await?;
        success(&i18n::f(M::ProxyCreated, &[("host", hostname)]));
    }
    Ok(())
}
