//! Subcommand implementations. Each module is thin: argument parsing + API calls.

pub mod auth;
pub mod billing;
pub mod dns;
pub mod domains;
pub mod proxy;
pub mod sites;
pub mod stats;

use anyhow::{bail, Result};

use crate::i18n::{self, M};

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
