//! Authentication and profile management.
//!
//! Primary mode — a personal token `wsk_…` (created in the dashboard, least-privilege
//! scopes). JWT email login is not implemented yet (needed for token/S3 management).

use crate::api::error::HttpError;
use crate::api::models::billing::Billing;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::config::{Config, Profile, DEFAULT_API_URL};
use crate::i18n::active_code;
use crate::t;
use crate::util::output::{info, success};
use crate::Context;
use anyhow::{Context as _, Result};
use clap::Subcommand;
use console::style;

#[derive(Subcommand)]
pub enum AuthCommand {
    #[command(about = t!(cmd_auth_login))]
    Login {
        #[arg(long, help = t!(arg_login_token))]
        token: Option<String>,
        #[arg(long, default_value = DEFAULT_API_URL, help = t!(arg_login_api_url))]
        api_url: String,
    },
    #[command(about = t!(cmd_auth_status))]
    Status,
    #[command(about = t!(cmd_auth_logout))]
    Logout,
}

pub async fn run(ctx: &Context, cmd: AuthCommand) -> Result<ProgramRes> {
    match cmd {
        AuthCommand::Login { token, api_url } => {
            login(ctx, token, api_url).await.map(ProgramRes::from)
        }
        AuthCommand::Status => status(ctx).await.map(ProgramRes::from),
        AuthCommand::Logout => logout(ctx).map(ProgramRes::from),
    }
}

async fn login(ctx: &Context, token: Option<String>, api_url: String) -> Result<()> {
    let token = match token {
        Some(t) => t,
        None => rpassword::prompt_password(t!(token_prompt)).context("failed to read the token")?,
    };
    let token = token.trim().to_string();

    if !token.starts_with("wsk_") {
        info(t!(token_warn_prefix));
    }

    let mut cfg = Config::load()?;
    let name = cfg.active_profile_name(ctx.profile_name());

    let verdict = probe(&api_url, &token).await;

    let profile = cfg
        .profiles
        .entry(name.clone())
        .or_insert_with(Profile::default);
    profile.api_url = Some(api_url);
    profile.token = Some(token.clone());
    // Only an explicit `--lang` is persisted: an ambient locale must not stick.
    if let Some(lang) = ctx.lang {
        profile.lang = Some(lang);
    }
    if cfg.default_profile.is_none() {
        cfg.default_profile = Some(name.clone());
    }
    cfg.save()?;

    // Reported only once the token really is on disk — the messages say "saved".
    match verdict {
        Ok(code) if code.is_success() => success(t!(token_saved_ok, &name)),
        Ok(code) if code.as_u16() == 403 => success(t!(token_saved_scoped, &name)),
        Ok(code) => info(t!(token_saved_code, &code.as_u16().to_string())),
        Err(err) => info(t!(token_saved_probe_fail, &err.to_string())),
    }

    Ok(())
}

async fn status(ctx: &Context) -> Result<()> {
    let cfg = Config::load()?;
    let name = cfg.active_profile_name(ctx.profile_name());
    let profile = cfg.profile(&name);
    let api_url = ctx
        .api_url_override()
        .map(str::to_string)
        .or_else(|| profile.and_then(|p| p.api_url.clone()))
        .unwrap_or_else(|| DEFAULT_API_URL.to_string());
    let has_token = ctx.has_token() || profile.and_then(|p| p.token.as_ref()).is_some();

    let ht = if has_token {
        style(t!(token_set)).green().to_string()
    } else {
        style(t!(token_unset)).red().to_string()
    };

    println!(
        "{prof} {nm}\n{url} {api_url}\n{lbl_token} {ht}\n{lbl_lang} {lang}\n",
        prof = t!(lbl_profile),
        nm = style(&name).bold(),
        url = t!(lbl_api_url),
        lbl_token = t!(lbl_token),
        lbl_lang = t!(lbl_lang),
        lang = active_code().as_str()
    );

    if !has_token {
        info(t!(login_hint));
    } else {
        let client = ctx.new_client()?;
        let resp = client.send::<Billing>(()).await; // Any route

        let verdict = match resp {
            Ok(_) => style(t!(access_ok).to_string()).green(),
            // The HTTP code is recovered from the typed error in the anyhow chain.
            Err(err) => match err
                .downcast_ref::<HttpError>()
                .map(|http| http.status.as_u16())
            {
                Some(401) => style(t!(access_invalid).to_string()).red(),
                Some(403) => style(t!(access_forbidden).to_string()).yellow(),

                other => style(t!(access_unexpected, &format!("{:?}", other))).yellow(),
            },
        };

        println!("{} {verdict}\n", t!(lbl_access));
    }
    Ok(())
}

fn logout(ctx: &Context) -> Result<()> {
    let mut cfg = Config::load()?;
    let name = cfg.active_profile_name(ctx.profile_name());

    if let Some(profile) = cfg.profiles.get_mut(&name) {
        profile.token = None;
        cfg.save()?;
        success(t!(token_removed, &name));
    } else {
        info(t!(profile_not_found, &name));
    }
    Ok(())
}

/// Lightweight check of a token that is not stored yet: one GET, only the status
/// code matters — an HTTP error is an answer here, not a failure.
async fn probe(api_url: &str, token: &str) -> Result<reqwest::StatusCode> {
    let client = Client::new(api_url.to_string(), token.to_string())?;
    match client.send::<Billing>(()).await {
        Ok(_) => Ok(reqwest::StatusCode::OK),
        Err(err) => match err.downcast_ref::<HttpError>() {
            Some(http) => Ok(http.status),
            None => Err(err),
        },
    }
}
