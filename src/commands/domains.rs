//! Domain (zone) management.

use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::commands::{confirm, resolve_domain};
use crate::i18n::{self, M};
use crate::output::{print_json, print_table, success, OutputFormat};
use crate::Context;

#[derive(Subcommand)]
pub enum DomainsCommand {
    /// List your domains.
    List,
    /// Add a domain (create the zone).
    Add {
        /// Domain name (e.g. example.com).
        name: String,
        /// Import existing records on creation: scan | none.
        #[arg(long, default_value = "scan")]
        import: String,
    },
    /// Show a domain.
    Get { name: String },
    /// Delete a domain and its zone.
    Remove { name: String },
    /// Check delegation (NS point to us).
    Check { name: String },
}

pub async fn run(ctx: &Context, cmd: DomainsCommand) -> Result<()> {
    let client = ctx.client()?;
    match cmd {
        DomainsCommand::List => {
            let domains: Vec<crate::api::models::Domain> = client.list_all("domains").await?;
            if ctx.output == OutputFormat::Json {
                return print_json(&domains);
            }
            let yes = i18n::tr(M::Yes);
            let no = i18n::tr(M::No);
            let dash = i18n::tr(M::Dash);
            let rows = domains
                .iter()
                .map(|d| {
                    vec![
                        d.id.to_string(),
                        d.name.clone(),
                        if d.delegated { yes.into() } else { no.into() },
                        d.current_tariff.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| dash.into()),
                    ]
                })
                .collect();
            print_table(
                &[i18n::tr(M::HId), i18n::tr(M::HDomain), i18n::tr(M::HDelegated), i18n::tr(M::HTariff)],
                rows,
            );
        }
        DomainsCommand::Add { name, import } => {
            let body = json!({ "name": name, "import_method": import });
            let created: crate::api::models::Domain = client.post_json("domains", &body).await?;
            success(&i18n::f(M::DomainCreated, &[("name", &created.name), ("id", &created.id.to_string())]));
            if ctx.output == OutputFormat::Json {
                return print_json(&created);
            }
        }
        DomainsCommand::Get { name } => {
            let domain = resolve_domain(&client, &name).await?;
            if ctx.output == OutputFormat::Json {
                return print_json(&domain);
            }
            let yes = i18n::tr(M::Yes);
            let no = i18n::tr(M::No);
            let dash = i18n::tr(M::Dash);
            print_table(
                &[i18n::tr(M::HField), i18n::tr(M::HValue)],
                vec![
                    vec![i18n::tr(M::HId).into(), domain.id.to_string()],
                    vec![i18n::tr(M::HDomain).into(), domain.name.clone()],
                    vec![
                        i18n::tr(M::HDelegated).into(),
                        if domain.delegated { yes.into() } else { no.into() },
                    ],
                    vec![
                        i18n::tr(M::HTariff).into(),
                        domain.current_tariff.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| dash.into()),
                    ],
                ],
            );
        }
        DomainsCommand::Remove { name } => {
            let domain = resolve_domain(&client, &name).await?;
            confirm(ctx.yes, &i18n::f(M::ConfirmDeleteDomain, &[("name", &domain.name)]))?;
            client.delete(&format!("domains/{}", domain.id)).await?;
            success(&i18n::f(M::DomainDeleted, &[("name", &domain.name)]));
        }
        DomainsCommand::Check { name } => {
            let domain = resolve_domain(&client, &name).await?;
            let result = client.post_empty(&format!("domains/{}/check-delegation", domain.id)).await?;
            print_json(&result)?;
        }
    }
    Ok(())
}
