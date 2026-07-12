//! Billing: balance, traffic usage, tariffs (scope `billing`, read-only).

use crate::api::Client;
use crate::api::_models::billing::{
    Billing, BillingBalance, BillingDomainUsage, BillingTariffs, BillingTariffsGet, BillingUsage,
};
use crate::api::table::ProgramRes;
use crate::commands::domains::resolve_domain;
use crate::i18n::{self, M};
use crate::Context;
use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

#[derive(Subcommand)]
pub enum BillingCommand {
    /// Account balance per currency.
    Balance,
    /// Traffic usage of a domain vs the tariff limit.
    Usage { domain: String },
    /// Current and available tariffs of a domain.
    Tariffs { domain: String },
}

pub async fn run(ctx: &Context, cmd: BillingCommand) -> Result<ProgramRes> {
    let client = ctx.client()?;
    match cmd {
        BillingCommand::Balance => balance(&client).await.map(ProgramRes::from),
        BillingCommand::Usage { domain } => {
            usage(ctx, &client, &domain).await.map(ProgramRes::from)
        }
        BillingCommand::Tariffs { domain } => {
            tariffs(ctx, &client, &domain).await.map(ProgramRes::from)
        }
    }
}

async fn balance(client: &Client) -> Result<BillingBalance> {
    let payload = client.n_send::<Billing>(()).await?;

    Ok(payload)
}

async fn usage(ctx: &Context, client: &Client, domain: &str) -> Result<BillingDomainUsage> {
    let d = resolve_domain(client, domain).await?;
    let usage = client.n_send::<BillingUsage>(d.id).await?;
    // let payload: Value = client.get_json(&format!("domains/{}/usage", d.id)).await?;
    Ok(usage)
}

async fn tariffs(ctx: &Context, client: &Client, domain: &str) -> Result<BillingTariffsGet> {
    let d = resolve_domain(client, domain).await?;
    // let payload: Value = client
    //     .get_json(&format!("domains/{}/tariffs", d.id))
    //     .await?;

    let payload = client.n_send::<BillingTariffs>(d.id).await?;

    Ok(payload)
}

fn fmt_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => (if *b {
            i18n::tr(M::Yes)
        } else {
            i18n::tr(M::No)
        })
        .to_string(),
        other => other.to_string(),
    }
}
