use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::util::context::Context;
use crate::util::output::OutputFormat;
use crate::{commands, t};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    about = t!(app_about),
    propagate_version = true
)]
pub struct Cli {
    #[arg(long, short = 'p', global = true, env = "WS_PROFILE", help = t!(arg_profile))]
    pub(crate) profile: Option<String>,

    #[arg(long, global = true, env = "WS_API_URL", help = t!(arg_api_url))]
    pub(crate) api_url: Option<String>,

    #[arg(long, global = true, env = "WS_TOKEN", hide_env_values = true, help = t!(arg_token))]
    pub(crate) token: Option<String>,

    #[arg(long, short = 'o', global = true, value_enum, default_value_t = OutputFormat::Table, help = t!(arg_output))]
    pub(crate) output: OutputFormat,

    #[arg(long, short = 'y', global = true, help = t!(arg_yes))]
    pub(crate) yes: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

macro_rules! define_run_command_enum {
    (
            $(
                $(#[$variant_meta:meta])*
                $variant:ident($ty:ty),
            )*

    ) => {
        #[derive(Subcommand)]
        pub enum RunCommand {
            $(
                $(#[$variant_meta])*
                $variant($ty),
            )*
        }

        impl Run for RunCommand {
            async fn run<'a>(self, ctx: &'a mut Context<'a>) -> anyhow::Result<ProgramRes> {
                match self {
                    $(
                        Self::$variant(cmd) => cmd.run(ctx).await,
                    )*
                }
            }
        }
    };
}

define_run_command_enum! {
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
}

#[derive(Subcommand)]
pub enum Command {
    #[command(flatten)]
    RunCommand(RunCommand),

    #[command(about = t!(cmd_completion))]
    Completion {
        #[arg(help = t!(arg_shell))]
        shell: CompletionShell,
    },
}

/// Shells we can emit completions for. Wraps clap_complete's built-in `Shell`
/// and adds Nushell, whose generator lives in a separate crate.
#[derive(Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
    Nushell,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// clap's self-check: catches conflicting flags, bad defaults and other
    /// structural mistakes in the whole command tree at test time.
    #[test]
    fn cli_structure_is_valid() {
        Cli::command().debug_assert();
    }
}
