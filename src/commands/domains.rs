//! Domain (zone) management.

use crate::api::Client;
use crate::api::_models::domain::{Domain, DomainInner};
use crate::commands::confirm;
use crate::i18n::{self, M};
use crate::output::{info, print_json, print_table, success, warn, OutputFormat};
use crate::Context;
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

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
            // let domains: Vec<crate::api::models::Domain> = client.list_all("domains").await?;
            let domains: Domain = client.n_send(()).await?;
            // if ctx.output == OutputFormat::Json {
            //     return print_json(&domains);
            // }
            let yes = i18n::tr(M::Yes);
            let no = i18n::tr(M::No);
            let dash = i18n::tr(M::Dash);
            let rows = domains
                .results
                .iter()
                .map(|d| {
                    vec![
                        d.id.to_string(),
                        d.name.clone(),
                        match d.delegated {
                            Some(true) => yes.into(),
                            Some(false) => no.into(),
                            None => dash.into(),
                        },
                        d.current_tariff
                            .as_ref()
                            .map(|t| t.name.clone())
                            .unwrap_or_else(|| dash.into()),
                    ]
                })
                .collect();
            print_table(
                &[
                    i18n::tr(M::HId),
                    i18n::tr(M::HDomain),
                    i18n::tr(M::HDelegated),
                    i18n::tr(M::HTariff),
                ],
                rows,
            );
        }
        DomainsCommand::Add { name, import } => {
            let body = json!({ "name": name, "import_method": import });
            // let created: crate::api::models::Domain = client.post_json("domains", &body).await?;
            let created: DomainInner = client.n_send_json(&body, ()).await?;
            success(&i18n::f(
                M::DomainCreated,
                &[("name", &created.name), ("id", &created.id.to_string())],
            ));

            // if ctx.output == OutputFormat::Json {
            //     return print_json(&created);
            // }
        }
        DomainsCommand::Get { name } => {
            let domain = resolve_domain(&client, &name).await?; // TODO
                                                                // if ctx.output == OutputFormat::Json {
                                                                //     return print_json(&domain);
                                                                // }

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
                        match domain.delegated {
                            Some(true) => yes.into(),
                            Some(false) => no.into(),
                            None => dash.into(),
                        },
                    ],
                    vec![
                        i18n::tr(M::HTariff).into(),
                        domain
                            .current_tariff
                            .as_ref()
                            .map(|t| t.name.clone())
                            .unwrap_or_else(|| dash.into()),
                    ],
                ],
            );
        }
        DomainsCommand::Remove { name } => {
            let domain = resolve_domain(&client, &name).await?;
            confirm(
                ctx.yes,
                &i18n::f(M::ConfirmDeleteDomain, &[("name", &domain.name)]),
            )?;
            client.delete(&format!("domains/{}", domain.id)).await?;
            success(&i18n::f(M::DomainDeleted, &[("name", &domain.name)]));
        }
        DomainsCommand::Check { name } => {
            let domain = resolve_domain(&client, &name).await?;

            let result = client
                .post_empty(&format!("domains/{}/check-delegation", domain.id))
                .await?;

            if ctx.output == OutputFormat::Json {
                return print_json(&result);
            }
            let check: crate::api::models::DelegationCheck = serde_json::from_value(result)?;
            match check.delegated {
                Some(true) => success(&i18n::f(M::DelegationOk, &[("name", &domain.name)])),
                Some(false) => {
                    warn(&i18n::f(
                        M::DelegationNotDelegated,
                        &[("name", &domain.name)],
                    ));
                    if !check.current_ns.is_empty() {
                        info(&i18n::f(
                            M::DelegationCurrentNs,
                            &[("ns", &check.current_ns.join(", "))],
                        ));
                    }
                    if !check.missing_ns.is_empty() {
                        warn(&i18n::f(
                            M::DelegationMissingNs,
                            &[("ns", &check.missing_ns.join(", "))],
                        ));
                    }
                    if !check.extra_ns.is_empty() {
                        warn(&i18n::f(
                            M::DelegationExtraNs,
                            &[("ns", &check.extra_ns.join(", "))],
                        ));
                    }
                    if check.missing_ns.is_empty() && check.extra_ns.is_empty() {
                        warn(i18n::tr(M::DelegationNoNs));
                    }
                    info(i18n::tr(M::DelegationPropagationNote));
                }
                None => info(i18n::tr(M::DelegationUnknown)),
            }
        }
    }
    Ok(())
}

/// Resolves the user's domain by name (case-insensitive, trailing dot ignored).
pub async fn resolve_domain(client: &Client, name: &str) -> Result<DomainInner> {
    let needle = name.trim().trim_end_matches('.').to_lowercase();
    // let domains: Vec<models::Domain> = client.list_all("domains").await?;
    let domains: Domain = client.n_send(()).await?;
    domains
        .results
        .into_iter()
        .find(|d| d.name.trim_end_matches('.').eq_ignore_ascii_case(&needle))
        .ok_or_else(|| anyhow::anyhow!(i18n::f(M::DomainNotFound, &[("name", name)])))
}
