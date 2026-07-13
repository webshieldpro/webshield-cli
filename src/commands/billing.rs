//! Billing: balance, traffic usage, tariffs (scope `billing`, read-only).

use crate::api::_models::billing::{
    Billing, BillingBalance, BillingDomainUsage, BillingTariffs, BillingTariffsGet, BillingUsage,
};
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::Context;
use anyhow::Result;
use clap::Subcommand;

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
        BillingCommand::Usage { domain } => usage(&client, &domain).await.map(ProgramRes::from),
        BillingCommand::Tariffs { domain } => tariffs(&client, &domain).await.map(ProgramRes::from),
    }
}

async fn balance(client: &Client) -> Result<BillingBalance> {
    let payload = client.n_send::<Billing>(()).await?;

    Ok(payload)
}

async fn usage(client: &Client, domain: &str) -> Result<BillingDomainUsage> {
    let d = resolve_domain(client, domain).await?;
    let usage = client.n_send::<BillingUsage>(d.id).await?;
    Ok(usage)
}

async fn tariffs(client: &Client, domain: &str) -> Result<BillingTariffsGet> {
    let d = resolve_domain(client, domain).await?;
    let payload = client.n_send::<BillingTariffs>(d.id).await?;

    Ok(payload)
}
