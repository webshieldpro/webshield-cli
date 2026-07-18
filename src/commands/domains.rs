//! Domain (zone) management.

use crate::api::models::domain::{
    DomainAdd, DomainAddReq, DomainCheckDelegation, DomainDelete, DomainInner, DomainList, Domains,
};
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::confirm;
use crate::i18n::{self, M};
use crate::util::output::{info, success, warn};
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
            success(i18n::f(M::DomainDeleted, &[("name", &domain.name)]));

            Ok(ProgramRes::Idle)
        }
        DomainsCommand::Check { name } => check(&client, &name).await.map(ProgramRes::from),
    }
}

async fn check(client: &Client, name: &str) -> Result<()> {
    let domain = resolve_domain(client, name).await?;

    let result = client.n_send::<DomainCheckDelegation>(domain.id).await?;

    match result.delegated {
        Some(true) => success(i18n::f(M::DelegationOk, &[("name", &domain.name)])),
        Some(false) => {
            warn(i18n::f(
                M::DelegationNotDelegated,
                &[("name", &domain.name)],
            ));

            if !result.current_ns.is_empty() {
                info(i18n::f(
                    M::DelegationCurrentNs,
                    &[("ns", &result.current_ns.join(", "))],
                ));
            }

            if !result.missing_ns.is_empty() {
                warn(i18n::f(
                    M::DelegationMissingNs,
                    &[("ns", &result.missing_ns.join(", "))],
                ));
            }
            if !result.extra_ns.is_empty() {
                warn(i18n::f(
                    M::DelegationExtraNs,
                    &[("ns", &result.extra_ns.join(", "))],
                ));
            }
            if result.missing_ns.is_empty() && result.extra_ns.is_empty() {
                warn(i18n::tr(M::DelegationNoNs));
            }
            info(i18n::tr(M::DelegationPropagationNote));
        }
        None => {
            info(i18n::tr(M::DelegationUnknown));
        }
    };
    Ok(())
}

async fn get(client: &Client, name: &str) -> Result<DomainInner> {
    resolve_domain(client, name).await
}

async fn add(client: &Client, name: String, import: String) -> Result<()> {
    let created = client
        .n_send_ser::<DomainAdd>(
            DomainAddReq {
                name,
                import_method: import,
            },
            (),
        )
        .await?;

    success(i18n::f(
        M::DomainCreated,
        &[("name", &created.name), ("id", &created.id.to_string())],
    ));

    Ok(())
}

async fn list(client: &Client) -> Result<DomainList> {
    let results = client.n_list::<Domains>(()).await?;
    Ok(DomainList { results })
}

/// Resolves the user's domain by name (case-insensitive, trailing dot ignored).
pub async fn resolve_domain(client: &Client, name: &str) -> Result<DomainInner> {
    let needle = name.trim().trim_end_matches('.').to_lowercase();

    let domains = client.n_list::<Domains>(()).await?;
    domains
        .into_iter()
        .find(|d| d.name.trim_end_matches('.').eq_ignore_ascii_case(&needle))
        .ok_or_else(|| anyhow::anyhow!(i18n::f(M::DomainNotFound, &[("name", name)])))
}
