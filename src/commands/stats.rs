//! Domain statistics and protection (scope `stats`, read-only).

use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api::Client;
use crate::commands::resolve_domain;
use crate::i18n::{self, M};
use crate::output::{print_json, print_table};
use crate::Context;

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

pub async fn run(ctx: &Context, cmd: StatsCommand) -> Result<()> {
    let client = ctx.client()?;
    match cmd {
        StatsCommand::Summary { domain, range } => summary(&client, &domain, &range).await,
        StatsCommand::Bans { domain, range } => bans(&client, &domain, &range).await,
    }
}

async fn summary(client: &Client, domain: &str, range: &str) -> Result<()> {
    let d = resolve_domain(client, domain).await?;
    // The summary is complex (charts and aggregates) — print it as JSON.
    let payload: Value = client
        .get_json(&format!("domains/{}/stats?range={range}&step=1h", d.id))
        .await?;
    print_json(&payload)
}

async fn bans(client: &Client, domain: &str, range: &str) -> Result<()> {
    let d = resolve_domain(client, domain).await?;
    let payload: Value = client
        .get_json(&format!("domains/{}/protection/bans?range={range}", d.id))
        .await?;

    let bans = payload
        .get("bans")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if bans.is_empty() {
        crate::output::info(i18n::tr(M::NoBans));
        return Ok(());
    }
    let rows = bans
        .iter()
        .map(|b| {
            let s = |k: &str| b.get(k).map(fmt_value).unwrap_or_default();
            vec![
                s("ip"),
                s("type"),
                s("reason"),
                s("last_seen"),
                s("requests"),
            ]
        })
        .collect();
    print_table(
        &[
            i18n::tr(M::HIp),
            i18n::tr(M::HType),
            i18n::tr(M::HReason),
            i18n::tr(M::HLastSeen),
            i18n::tr(M::HRequests),
        ],
        rows,
    );
    Ok(())
}

fn fmt_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
