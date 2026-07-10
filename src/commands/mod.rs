//! Subcommand implementations. Each module is thin: argument parsing + API calls.

pub mod auth;
pub mod billing;
pub mod dns;
pub mod domains;
pub mod proxy;
pub mod sites;
pub mod stats;

use anyhow::{bail, Result};

use crate::api::models::Domain;
use crate::api::Client;
use crate::i18n::{self, M};

/// Resolves the user's domain by name (case-insensitive, trailing dot ignored).
pub async fn resolve_domain(client: &Client, name: &str) -> Result<Domain> {
    let needle = name.trim().trim_end_matches('.').to_lowercase();
    let domains: Vec<Domain> = client.list_all("domains").await?;
    domains
        .into_iter()
        .find(|d| d.name.trim_end_matches('.').eq_ignore_ascii_case(&needle))
        .ok_or_else(|| anyhow::anyhow!(i18n::f(M::DomainNotFound, &[("name", name)])))
}

/// Asks for confirmation (y/N) unless `--yes` was passed.
pub fn confirm(yes: bool, prompt: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    use std::io::Write;
    print!("{prompt} {}: ", i18n::tr(M::ConfirmSuffix));
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !matches!(answer.trim(), "y" | "Y" | "yes" | "д" | "да") {
        bail!(i18n::tr(M::ConfirmCancelled));
    }
    Ok(())
}
