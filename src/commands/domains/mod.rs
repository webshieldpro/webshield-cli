//! Domain (zone) management.

use crate::api::models::domain::{
    DomainAdd, DomainAddReq, DomainCheckDelegation, DomainDelete, DomainInner, DomainList, Domains,
    ResolveDomains,
};
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::util::Page;
use crate::t;
use crate::util::context::Context;
use crate::util::input::confirm;
use crate::util::output::{info, success, warn};
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DomainsCommand {
    #[command(about = t!(cmd_domains_list))]
    List(Page),
    #[command(about = t!(cmd_domains_add))]
    Add {
        #[arg(help = t!(arg_domain_name))]
        name: String,
        #[arg(long, default_value = "scan", help = t!(arg_domains_import))]
        import: String,
    },
    #[command(about = t!(cmd_domains_get))]
    Get {
        #[arg(help = t!(arg_domain_name))]
        name: String,
    },
    #[command(about = t!(cmd_domains_remove))]
    Remove {
        #[arg(help = t!(arg_domain_name))]
        name: String,
    },
    #[command(about = t!(cmd_domains_check))]
    Check {
        #[arg(help = t!(arg_domain_name))]
        name: String,
    },
}

impl Run for DomainsCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let yes = ctx.yes;
        let client = ctx.client()?;
        match self {
            Self::List(page) => list(client, page.into()).await.map(ProgramRes::from),
            Self::Add { name, import } => add(client, name, import).await.map(ProgramRes::from),
            Self::Get { name } => get(client, &name).await.map(ProgramRes::from),
            Self::Remove { name } => remove(yes, client, &name).await.map(ProgramRes::from),
            Self::Check { name } => check(client, &name).await.map(ProgramRes::from),
        }
    }
}

async fn remove(yes: bool, client: &Client<'_>, name: &str) -> Result<()> {
    let domain = resolve_domain(client, name).await?;
    confirm(yes, &t!(confirm_delete_domain, &domain.name))?;

    client.send::<DomainDelete>(domain.id).await?;

    success(t!(domain_deleted, &domain.name));
    Ok(())
}

async fn check(client: &Client<'_>, name: &str) -> Result<()> {
    let domain = resolve_domain(client, name).await?;

    let result = client.send::<DomainCheckDelegation>(domain.id).await?;

    match result.delegated {
        Some(true) => success(t!(delegation_ok, &domain.name)),
        Some(false) => {
            warn(t!(delegation_not_delegated, &domain.name));

            if !result.current_ns.is_empty() {
                info(t!(delegation_current_ns, &result.current_ns.join(", ")));
            }

            if !result.missing_ns.is_empty() {
                warn(t!(delegation_missing_ns, &result.missing_ns.join(", ")));
            }
            if !result.extra_ns.is_empty() {
                warn(t!(delegation_extra_ns, &result.extra_ns.join(", ")));
            }
            if result.missing_ns.is_empty() && result.extra_ns.is_empty() {
                warn(t!(delegation_no_ns));
            }
            info(t!(delegation_propagation_note));
        }
        None => {
            info(t!(delegation_unknown));
        }
    };
    Ok(())
}

async fn get(client: &Client<'_>, name: &str) -> Result<DomainInner> {
    resolve_domain(client, name).await
}

async fn add(client: &Client<'_>, name: String, import: String) -> Result<()> {
    let created = client
        .send_json::<DomainAdd>(
            DomainAddReq {
                name,
                import_method: import,
            },
            (),
        )
        .await?;

    success(t!(domain_created, &created.name, &created.id.to_string()));

    Ok(())
}

async fn list(client: &Client<'_>, page: u32) -> Result<DomainList> {
    client.send::<Domains>(page).await
}

/// Resolves the user's domain by name.
pub async fn resolve_domain(client: &Client<'_>, name: &str) -> Result<DomainInner> {
    let needle = name.trim().trim_end_matches('.').to_lowercase();

    let domains = client.send::<ResolveDomains>(needle).await?;

    domains
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!(t!(domain_not_found, name)))
}
