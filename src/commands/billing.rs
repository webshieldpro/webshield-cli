//! Billing: balance, traffic usage, tariffs (scope `billing`, read-only).

use anyhow::Result;
use clap::Subcommand;
use serde_json::Value;

use crate::api::Client;
use crate::commands::resolve_domain;
use crate::i18n::{self, M};
use crate::output::{print_json, print_table, OutputFormat};
use crate::Context;

#[derive(Subcommand)]
pub enum BillingCommand {
    /// Account balance per currency.
    Balance,
    /// Traffic usage of a domain vs the tariff limit.
    Usage { domain: String },
    /// Current and available tariffs of a domain.
    Tariffs { domain: String },
}

pub async fn run(ctx: &Context, cmd: BillingCommand) -> Result<()> {
    let client = ctx.client()?;
    match cmd {
        BillingCommand::Balance => balance(ctx, &client).await,
        BillingCommand::Usage { domain } => usage(ctx, &client, &domain).await,
        BillingCommand::Tariffs { domain } => tariffs(ctx, &client, &domain).await,
    }
}

async fn balance(ctx: &Context, client: &Client) -> Result<()> {
    let payload: Value = client.get_json("billing/balance").await?;
    if ctx.output == OutputFormat::Json {
        return print_json(&payload);
    }
    let rows = payload
        .get("balances")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(cur, val)| vec![cur.clone(), val.as_str().unwrap_or("").to_string()])
                .collect()
        })
        .unwrap_or_default();
    print_table(&[i18n::tr(M::HCurrency), i18n::tr(M::HBalance)], rows);
    Ok(())
}

async fn usage(ctx: &Context, client: &Client, domain: &str) -> Result<()> {
    let d = resolve_domain(client, domain).await?;
    let payload: Value = client.get_json(&format!("domains/{}/usage", d.id)).await?;
    if ctx.output == OutputFormat::Json {
        return print_json(&payload);
    }
    let get = |k: &str| payload.get(k).map(fmt_value).unwrap_or_default();
    let rows = vec![
        vec!["bytes_used".into(), get("bytes_used")],
        vec!["limit_gb".into(), get("limit_gb")],
        vec!["used_ratio".into(), get("used_ratio")],
        vec!["over_limit".into(), get("over_limit")],
        vec!["throttled".into(), get("throttled")],
        vec!["tariff".into(), get("tariff")],
        vec!["currency".into(), get("currency")],
    ];
    print_table(&[i18n::tr(M::HMetric), i18n::tr(M::HValue)], rows);
    Ok(())
}

async fn tariffs(ctx: &Context, client: &Client, domain: &str) -> Result<()> {
    let d = resolve_domain(client, domain).await?;
    let payload: Value = client.get_json(&format!("domains/{}/tariffs", d.id)).await?;
    if ctx.output == OutputFormat::Json {
        return print_json(&payload);
    }
    let current = payload
        .get("current_tariff")
        .and_then(|t| t.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let list = payload.get("tariffs").and_then(Value::as_array).cloned().unwrap_or_default();
    let rows = list
        .iter()
        .map(|t| {
            let s = |k: &str| t.get(k).map(fmt_value).unwrap_or_default();
            let name = s("name");
            let marker = if name == current { "*" } else { "" };
            vec![marker.into(), name, s("price"), s("currency"), s("period")]
        })
        .collect();
    print_table(
        &["", i18n::tr(M::HName), "price", i18n::tr(M::HCurrency), "period"],
        rows,
    );
    Ok(())
}

fn fmt_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => (if *b { i18n::tr(M::Yes) } else { i18n::tr(M::No) }).to_string(),
        other => other.to_string(),
    }
}
