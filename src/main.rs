//! WebShield CLI — command-line client for domains, DNS records, proxying and static
//! site publishing via the `/api/v1` API. Both runtime output and help follow
//! `--lang`/`WS_LANG`/profile/system locale (see `i18n`).

mod api;
mod commands;
mod config;
mod i18n;
mod util;

use anyhow::{bail, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use clap_complete_nushell::Nushell;

use crate::api::table::ProgramRes;
use crate::config::Config;
use crate::i18n::locale::{prescan_flag, resolve};
use crate::i18n::{set_locale, LocaleCode};
use api::Client;
use util::output::OutputFormat;

#[derive(Parser)]
#[command(
    name = "webshield",
    version,
    about = t!(app_about),
    propagate_version = true
)]
struct Cli {
    #[arg(long, short = 'p', global = true, env = "WS_PROFILE", help = t!(arg_profile))]
    profile: Option<String>,

    #[arg(long, global = true, env = "WS_API_URL", help = t!(arg_api_url))]
    api_url: Option<String>,

    #[arg(long, global = true, env = "WS_TOKEN", hide_env_values = true, help = t!(arg_token))]
    token: Option<String>,

    #[arg(long, global = true, value_enum, help = t!(arg_lang))]
    lang: Option<LocaleCode>,

    #[arg(long, short = 'o', global = true, value_enum, default_value_t = OutputFormat::Table, help = t!(arg_output))]
    output: OutputFormat,

    #[arg(long, short = 'y', global = true, help = t!(arg_yes))]
    yes: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    #[command(about = t!(cmd_auth))]
    Auth(commands::auth::AuthCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_domains))]
    Domains(commands::domains::DomainsCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_dns))]
    Dns(commands::dns::DnsCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_sites))]
    Sites(commands::sites::SitesCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_proxy))]
    Proxy(commands::proxy::ProxyCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_stats))]
    Stats(commands::stats::StatsCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_billing))]
    Billing(commands::billing::BillingCommand),
    #[command(about = t!(cmd_completion))]
    Completion {
        #[arg(help = t!(arg_shell))]
        shell: CompletionShell,
    },
}

/// Shells we can emit completions for. Wraps clap_complete's built-in `Shell`
/// and adds Nushell, whose generator lives in a separate crate.
#[derive(Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
    Nushell,
}

/// Resolved execution context: API access and output settings.
pub struct Context {
    profile: Option<String>,
    api_url: Option<String>,
    token: Option<String>,
    pub lang: Option<LocaleCode>,
    pub output: OutputFormat,
    pub yes: bool,
}

impl Context {
    pub fn profile_name(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    pub fn api_url_override(&self) -> Option<&str> {
        self.api_url.as_deref()
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// Builds the HTTP client, resolving URL and token from flags/env/profile.
    pub fn new_client(&self) -> Result<Client> {
        let cfg = Config::load()?;
        let profile_name = cfg.active_profile_name(self.profile.as_deref());
        let profile = cfg.profile(&profile_name);

        let api_url = self
            .api_url
            .clone()
            .or_else(|| profile.and_then(|p| p.api_url.clone()))
            .unwrap_or_else(|| config::DEFAULT_API_URL.to_string());

        let token = self
            .token
            .clone()
            .or_else(|| profile.and_then(|p| p.token.clone()));

        let Some(token) = token else {
            bail!(t!(no_token, &profile_name));
        };
        Client::new(api_url, token)
    }
}

/// Language stored in the profile that is about to be used. Best effort: a broken
/// config must not blow up before clap gets a chance to report the real problem.
fn profile_lang(args: &[String]) -> Option<LocaleCode> {
    let cfg = Config::load().ok()?;
    let name = prescan_flag(args, &["--profile", "-p"])
        .or_else(|| std::env::var("WS_PROFILE").ok())
        .unwrap_or_else(|| cfg.active_profile_name(None));
    cfg.profile(&name).and_then(|p| p.lang)
}

#[tokio::main]
async fn main() {
    // The language is needed before parsing: help is localized as well, and clap
    // reads the `t!` attributes while building the command tree.
    let args: Vec<String> = std::env::args().collect();
    set_locale(resolve(&args, || profile_lang(&args)));

    if let Err(err) = run().await {
        eprintln!("{} {err:#}", console::style(t!(error_prefix)).red().bold());
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    let ctx = Context {
        profile: cli.profile,
        api_url: cli.api_url,
        token: cli.token,
        lang: cli.lang,
        output: cli.output,
        yes: cli.yes,
    };

    let result: Result<ProgramRes> = match cli.command {
        Command::Auth(cmd) => commands::auth::run(&ctx, cmd).await,
        Command::Domains(cmd) => commands::domains::run(&ctx, cmd).await,
        Command::Dns(cmd) => commands::dns::run(&ctx, cmd).await,
        Command::Sites(cmd) => commands::sites::run(&ctx, cmd).await,
        Command::Proxy(cmd) => commands::proxy::run(&ctx, cmd).await,
        Command::Stats(cmd) => commands::stats::run(&ctx, cmd).await,
        Command::Billing(cmd) => commands::billing::run(&ctx, cmd).await,
        Command::Completion { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            let mut out = std::io::stdout();
            // Nushell's generator lives in its own crate; the rest are clap_complete's.
            match shell {
                CompletionShell::Bash => {
                    clap_complete::generate(Shell::Bash, &mut cmd, name, &mut out)
                }
                CompletionShell::Zsh => {
                    clap_complete::generate(Shell::Zsh, &mut cmd, name, &mut out)
                }
                CompletionShell::Fish => {
                    clap_complete::generate(Shell::Fish, &mut cmd, name, &mut out)
                }
                CompletionShell::Powershell => {
                    clap_complete::generate(Shell::PowerShell, &mut cmd, name, &mut out)
                }
                CompletionShell::Elvish => {
                    clap_complete::generate(Shell::Elvish, &mut cmd, name, &mut out)
                }
                CompletionShell::Nushell => {
                    clap_complete::generate(Nushell, &mut cmd, name, &mut out)
                }
            }
            Ok(ProgramRes::Idle)
        }
    };

    match result? {
        ProgramRes::Str(s) => println!("{s}"),
        ProgramRes::Table(tb) => {
            if ctx.output == OutputFormat::Table {
                tb.display_as_table()
            } else {
                println!("{}", serde_json::to_string_pretty(&tb.as_json()?)?)
            }
        }
        ProgramRes::Idle => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// clap's self-check: catches conflicting flags, bad defaults and other
    /// structural mistakes in the whole command tree at test time.
    #[test]
    fn cli_structure_is_valid() {
        Cli::command().debug_assert();
    }
}
