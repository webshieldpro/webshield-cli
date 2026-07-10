//! WebShield CLI — command-line client for domains, DNS records, proxying and static
//! site publishing via the `/api/v1` API. Help text is English by default; runtime
//! output and help follow `--lang`/`WS_LANG`/system locale (see `i18n`).

mod api;
mod commands;
mod config;
mod i18n;
mod output;

use anyhow::{bail, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::Shell;

use api::Client;
use config::Config;
use i18n::Lang;
use output::OutputFormat;

#[derive(Parser)]
#[command(name = "webshield", version, about = "WebShield command-line client", propagate_version = true)]
struct Cli {
    /// Config profile (defaults to the active one from config.toml).
    #[arg(long, short = 'p', global = true, env = "WS_PROFILE")]
    profile: Option<String>,

    /// Base API URL (overrides the profile).
    #[arg(long, global = true, env = "WS_API_URL")]
    api_url: Option<String>,

    /// Personal token `wsk_…` (overrides the profile).
    #[arg(long, global = true, env = "WS_TOKEN", hide_env_values = true)]
    token: Option<String>,

    /// Interface language (en/ru); defaults to WS_LANG or system locale.
    #[arg(long, global = true, value_enum)]
    lang: Option<Lang>,

    /// Output format.
    #[arg(long, short = 'o', global = true, value_enum, default_value_t = OutputFormat::Table)]
    output: OutputFormat,

    /// Do not ask for confirmation on destructive operations.
    #[arg(long, short = 'y', global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authentication and profiles.
    #[command(subcommand)]
    Auth(commands::auth::AuthCommand),
    /// Domains (zones).
    #[command(subcommand)]
    Domains(commands::domains::DomainsCommand),
    /// DNS records.
    #[command(subcommand)]
    Dns(commands::dns::DnsCommand),
    /// Static sites and publishing.
    #[command(subcommand)]
    Sites(commands::sites::SitesCommand),
    /// Proxy/redirect host edge settings.
    #[command(subcommand)]
    Proxy(commands::proxy::ProxyCommand),
    /// Statistics and protection.
    #[command(subcommand)]
    Stats(commands::stats::StatsCommand),
    /// Billing: balance, usage, tariffs.
    #[command(subcommand)]
    Billing(commands::billing::BillingCommand),
    /// Generate a shell completion script.
    Completion {
        /// Shell: bash, zsh, fish, powershell, elvish.
        shell: Shell,
    },
}

/// Resolved execution context: API access and output settings.
pub struct Context {
    profile: Option<String>,
    api_url: Option<String>,
    token: Option<String>,
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
    pub fn client(&self) -> Result<Client> {
        let cfg = Config::load()?;
        let profile_name = cfg.active_profile_name(self.profile.as_deref());
        let profile = cfg.profile(&profile_name);

        let api_url = self
            .api_url
            .clone()
            .or_else(|| profile.map(|p| p.api_url.clone()))
            .unwrap_or_else(|| config::DEFAULT_API_URL.to_string());

        let token = self
            .token
            .clone()
            .or_else(|| profile.and_then(|p| p.token.clone()));

        let Some(token) = token else {
            bail!(i18n::f(i18n::M::NoToken, &[("profile", &profile_name)]));
        };
        Client::new(&api_url, &token)
    }
}

#[tokio::main]
async fn main() {
    // The language is needed before parsing so that help prints in the right language.
    let raw: Vec<String> = std::env::args().collect();
    i18n::set(i18n::resolve(i18n::prescan_lang(&raw).as_deref()));

    if let Err(err) = run().await {
        eprintln!("{} {err:#}", console::style(i18n::tr(i18n::M::ErrorPrefix)).red().bold());
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cmd = i18n::localize_help(Cli::command());
    let matches = cmd.get_matches();
    let cli = Cli::from_arg_matches(&matches)?;

    let ctx = Context {
        profile: cli.profile,
        api_url: cli.api_url,
        token: cli.token,
        output: cli.output,
        yes: cli.yes,
    };

    match cli.command {
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
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(())
        }
    }
}
