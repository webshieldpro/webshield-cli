//! Billing: balance, traffic usage, tariffs (scope `billing`, read-only).

use crate::api::models::billing::{
    Billing, BillingBalance, BillingDomainUsage, BillingTariffs, BillingTariffsGet, BillingUsage,
};
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::t;
use crate::util::context::Context;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum BillingCommand {
    #[command(about = t!(cmd_billing_balance))]
    Balance,
    #[command(about = t!(cmd_billing_usage))]
    Usage {
        #[arg(help = t!(arg_domain))]
        domain: String,
    },
    #[command(about = t!(cmd_billing_tariffs))]
    Tariffs {
        #[arg(help = t!(arg_domain))]
        domain: String,
    },
}

impl Run for BillingCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let client = ctx.client()?;
        match self {
            Self::Balance => balance(client).await.map(ProgramRes::from),
            Self::Usage { domain } => usage(client, &domain).await.map(ProgramRes::from),
            Self::Tariffs { domain } => tariffs(client, &domain).await.map(ProgramRes::from),
        }
    }
}

async fn balance(client: &Client<'_>) -> Result<BillingBalance> {
    client.send::<Billing>(()).await
}

async fn usage(client: &Client<'_>, domain: &str) -> Result<BillingDomainUsage> {
    let d = resolve_domain(client, domain).await?;
    let usage = client.send::<BillingUsage>(d.id).await?;
    Ok(usage)
}

async fn tariffs(client: &Client<'_>, domain: &str) -> Result<BillingTariffsGet> {
    let d = resolve_domain(client, domain).await?;
    let payload = client.send::<BillingTariffs>(d.id).await?;

    Ok(payload)
}
