//! Domain statistics and protection (scope `stats`, read-only).

use crate::api::_models::stats::{BanStats, StatBans, StatDomains, SummaryStats};
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::Context;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum StatsCommand {
    /// Traffic/requests summary of a domain.
    Summary {
        domain: String,
        /// Time range, e.g. 24h, 7d.
        #[arg(long, default_value = "24h")]
        range: String,
    },
    /// Active bans/challenges for a domain.
    Bans {
        domain: String,
        #[arg(long, default_value = "24h")]
        range: String,
    },
}

pub async fn run(ctx: &Context, cmd: StatsCommand) -> Result<ProgramRes> {
    let client = ctx.client()?;
    match cmd {
        StatsCommand::Summary { domain, range } => summary(&client, &domain, &range)
            .await
            .map(ProgramRes::from),
        StatsCommand::Bans { domain, range } => {
            bans(&client, &domain, &range).await.map(ProgramRes::from)
        }
    }
}

async fn summary(client: &Client, domain: &str, range: &str) -> Result<SummaryStats> {
    let d = resolve_domain(client, domain).await?;
    // The summary is complex (charts and aggregates) — print it as JSON.
    let payload: SummaryStats = client
        .n_send::<StatDomains>((d.id, range.to_string()))
        .await?;
    Ok(payload)
}

async fn bans(client: &Client, domain: &str, range: &str) -> Result<BanStats> {
    let d = resolve_domain(client, domain).await?;

    let payload: BanStats = client.n_send::<StatBans>((d.id, range.to_string())).await?;

    Ok(payload)
}
