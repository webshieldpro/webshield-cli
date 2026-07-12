//! Domain (zone) management.

use crate::api::Client;
use crate::api::_models::domain::{
    DomainAdd, DomainAddReq, DomainCheckDelegation, DomainDelete, DomainInner, DomainList, Domains,
};
use crate::api::table::ProgramRes;
use crate::commands::confirm;
use crate::i18n::{self, M};
use crate::Context;
use anyhow::Result;
use clap::Subcommand;

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

pub async fn run(ctx: &Context, cmd: DomainsCommand) -> Result<ProgramRes> {
    let client = ctx.client()?;
    match cmd {
        DomainsCommand::List => list(&client).await.map(ProgramRes::from),
        DomainsCommand::Add { name, import } => {
            add(&client, name, import).await.map(ProgramRes::from)
        }
        DomainsCommand::Get { name } => get(&client, &name).await.map(ProgramRes::from),

        DomainsCommand::Remove { name } => {
            let domain = resolve_domain(&client, &name).await?;
            confirm(
                ctx.yes,
                &i18n::f(M::ConfirmDeleteDomain, &[("name", &domain.name)]),
            )?;
            client.n_send::<DomainDelete>(domain.id).await?;
            // success(&i18n::f(M::DomainDeleted, &[("name", &domain.name)]));
            Ok(ProgramRes::from(i18n::f(
                M::DomainDeleted,
                &[("name", &domain.name)],
            )))
        }
        DomainsCommand::Check { name } => check(&client, &name).await.map(ProgramRes::from),
    }
}

async fn check(client: &Client, name: &String) -> Result<String> {
    let domain = resolve_domain(&client, &name).await?;

    let result = client.n_send::<DomainCheckDelegation>(domain.id).await?;

    let check: crate::api::models::DelegationCheck = serde_json::from_value(result)?;
    match check.delegated {
        Some(true) => Ok(i18n::f(M::DelegationOk, &[("name", &domain.name)])),
        Some(false) => {
            let mut buf = Vec::new();

            buf.push(i18n::f(
                M::DelegationNotDelegated,
                &[("name", &domain.name)],
            ));

            // warn(&i18n::f(
            //     M::DelegationNotDelegated,
            //     &[("name", &domain.name)],
            // ));

            if !check.current_ns.is_empty() {
                // info(&i18n::f(
                //     M::DelegationCurrentNs,
                //     &[("ns", &check.current_ns.join(", "))],
                // ));
                buf.push(i18n::f(
                    M::DelegationCurrentNs,
                    &[("ns", &check.current_ns.join(", "))],
                ));
            }

            if !check.missing_ns.is_empty() {
                // warn(&i18n::f(
                //     M::DelegationMissingNs,
                //     &[("ns", &check.missing_ns.join(", "))],
                // ));

                buf.push(i18n::f(
                    M::DelegationMissingNs,
                    &[("ns", &check.missing_ns.join(", "))],
                ));
            }
            if !check.extra_ns.is_empty() {
                // warn(&i18n::f(
                //     M::DelegationExtraNs,
                //     &[("ns", &check.extra_ns.join(", "))],
                // ));
                buf.push(i18n::f(
                    M::DelegationExtraNs,
                    &[("ns", &check.extra_ns.join(", "))],
                ));
            }
            if check.missing_ns.is_empty() && check.extra_ns.is_empty() {
                // warn(i18n::tr(M::DelegationNoNs));
                buf.push(i18n::tr(M::DelegationNoNs).to_string());
            }
            buf.push(i18n::tr(M::DelegationPropagationNote).to_string());
            Ok(buf.join("\n\n"))
        }
        None => Ok(i18n::tr(M::DelegationUnknown).to_string()),
    }
}

async fn get(client: &Client, name: &String) -> Result<DomainInner> {
    let domain = resolve_domain(&client, &name).await?;
    Ok(domain)
}

async fn add(client: &Client, name: String, import: String) -> Result<String> {
    // let created: crate::api::models::Domain = client.post_json("domains", &body).await?;
    let created = client
        .n_send_ser::<DomainAdd>(
            DomainAddReq {
                name,
                import_method: import,
            },
            (),
        )
        .await?;

    // success(&i18n::f(
    //     M::DomainCreated,
    //     &[("name", &created.name), ("id", &created.id.to_string())],
    // ))

    Ok(i18n::f(
        M::DomainCreated,
        &[("name", &created.name), ("id", &created.id.to_string())],
    ))

    // if ctx.output == OutputFormat::Json {
    //     return print_json(&created);
    // }
}

async fn list(client: &Client) -> Result<DomainList> {
    // let domains: Vec<crate::api::models::Domain> = client.list_all("domains").await?;
    let domains = client.n_send::<Domains>(()).await?;
    // if ctx.output == OutputFormat::Json {
    //     return print_json(&domains);
    // }
    Ok(domains)
}

/// Resolves the user's domain by name (case-insensitive, trailing dot ignored).
pub async fn resolve_domain(client: &Client, name: &str) -> Result<DomainInner> {
    let needle = name.trim().trim_end_matches('.').to_lowercase();

    let domains = client.n_send::<Domains>(()).await?;
    domains
        .results
        .into_iter()
        .find(|d| d.name.trim_end_matches('.').eq_ignore_ascii_case(&needle))
        .ok_or_else(|| anyhow::anyhow!(i18n::f(M::DomainNotFound, &[("name", name)])))
}
