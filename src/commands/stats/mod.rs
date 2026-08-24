//! Domain statistics and protection (scope `stats`, read-only).

use crate::api::models::stats::{BanStats, StatBans, StatDomains, SummaryStats};
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::t;
use crate::util::context::Context;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
#[command(about = t!(cmd_stats))]
pub enum StatsCommand {
    #[command(about = t!(cmd_stats_summary))]
    Summary {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(long, default_value = "24h", help = t!(arg_range))]
        range: String,
    },
    #[command(about = t!(cmd_stats_bans))]
    Bans {
        #[arg(help = t!(arg_domain))]
        domain: String,
        #[arg(long, default_value = "24h", help = t!(arg_range))]
        range: String,
    },
}

impl Run for StatsCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let client = ctx.client()?;
        match self {
            Self::Summary { domain, range } => {
                summary(client, &domain, &range).await.map(ProgramRes::from)
            }
            Self::Bans { domain, range } => {
                bans(client, &domain, &range).await.map(ProgramRes::from)
            }
        }
    }
}

async fn summary(client: &Client<'_>, domain: &str, range: &str) -> Result<SummaryStats> {
    let d = resolve_domain(client, domain).await?;
    // The summary is complex (charts and aggregates) — print it as JSON.
    let payload: SummaryStats = client
        .send::<StatDomains>((d.id, range.to_string()))
        .await?;
    Ok(payload)
}

async fn bans(client: &Client<'_>, domain: &str, range: &str) -> Result<BanStats> {
    let d = resolve_domain(client, domain).await?;

    let payload: BanStats = client.send::<StatBans>((d.id, range.to_string())).await?;

    Ok(payload)
}
