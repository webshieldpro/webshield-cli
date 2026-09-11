//! WebShield CLI — command-line client for domains, DNS records, proxying and static
//! site publishing via the `/api/v1` API. Both runtime output and help follow
//! `--lang`/`WS_LANG`/profile/system locale (see `i18n`).

mod api;
mod cmd;
mod commands;
mod i18n;
mod util;

use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::i18n::locale::LocaleCode;
use crate::i18n::set_locale;
use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::Shell;
use clap_complete_nushell::Nushell;
use cmd::{Cli, Command, CompletionShell};
use util::config::ProfileConfig;
use util::context::Context;
use util::output::OutputFormat;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{} {err:#}", console::style(t!(error_prefix)).red().bold());
        std::process::exit(1);
    }
}

/// Language from argv: `--lang ru` or `--lang=ru`.
///
/// Parsed by hand because clap help strings are built through `t!`: the locale must
/// be fixed before `Cli::parse()` runs, so clap cannot be the one to read the flag.
/// An invalid value is ignored here — clap reports it during the actual parse.
fn lang_from_args() -> Option<LocaleCode> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        let value = match arg.strip_prefix("--lang") {
            Some("") => args.next()?,
            Some(rest) => rest.strip_prefix('=')?.to_string(),
            None => continue,
        };
        return LocaleCode::try_from(value.as_str()).ok();
    }
    None
}

fn lang_from_env() -> Option<LocaleCode> {
    LocaleCode::try_from(std::env::var("WS_LANG").ok()?.as_str()).ok()
}

async fn run() -> Result<()> {
    let cfg = ProfileConfig::load()?;

    // Source order: flag -> environment -> profile -> system locale -> English.
    set_locale(
        lang_from_args()
            .or_else(lang_from_env)
            .or(cfg.lang)
            .unwrap_or_default(),
    );

    let cli = Cli::parse();

    let mut ctx = Context::new(cli.profile.as_deref(), cli.api_url, cli.token, cli.yes, cfg);

    let result: Result<ProgramRes> = match cli.command {
        Command::RunCommand(cmd) => cmd.run(&mut ctx).await,
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
        ProgramRes::Data(tb) => match cli.output {
            OutputFormat::Table => tb.display_as_table(),
            OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&tb.as_json()?)?),
        },
        ProgramRes::Idle => {}
    }

    Ok(())
}
