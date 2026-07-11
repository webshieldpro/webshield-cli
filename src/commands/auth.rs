//! Authentication and profile management.
//!
//! Primary mode — a personal token `wsk_…` (created in the dashboard, least-privilege
//! scopes). JWT email login is not implemented yet (needed for token/S3 management).

use anyhow::{Context as _, Result};
use clap::Subcommand;
use console::style;
use reqwest::Method;

use crate::config::{Config, Profile, DEFAULT_API_URL};
use crate::i18n::{self, M};
use crate::Context;

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Store a `wsk_…` token in the profile and verify it.
    Login {
        /// Token `wsk_…` (prompted interactively if omitted).
        #[arg(long)]
        token: Option<String>,
        /// Base API URL for the profile.
        #[arg(long, default_value = DEFAULT_API_URL)]
        api_url: String,
    },
    /// Show the active profile and verify API access.
    Status,
    /// Remove the token from the active profile.
    Logout,
}

pub async fn run(ctx: &Context, cmd: AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Login { token, api_url } => login(ctx, token, api_url).await,
        AuthCommand::Status => status(ctx).await,
        AuthCommand::Logout => logout(ctx),
    }
}

async fn login(ctx: &Context, token: Option<String>, api_url: String) -> Result<()> {
    let token = match token {
        Some(t) => t,
        None => rpassword::prompt_password(i18n::tr(M::TokenPrompt))
            .context("failed to read the token")?,
    };
    let token = token.trim().to_string();
    if !token.starts_with("wsk_") {
        crate::output::info(i18n::tr(M::TokenWarnPrefix));
    }

    let mut cfg = Config::load()?;
    let name = cfg.active_profile_name(ctx.profile_name());
    let profile = cfg
        .profiles
        .entry(name.clone())
        .or_insert_with(Profile::default);
    profile.api_url = api_url.clone();
    profile.token = Some(token.clone());
    if cfg.default_profile.is_none() {
        cfg.default_profile = Some(name.clone());
    }
    cfg.save()?;

    match probe(&api_url, &token).await {
        Ok(code) if code.is_success() => {
            crate::output::success(&i18n::f(M::TokenSavedOk, &[("profile", &name)]))
        }
        Ok(code) if code.as_u16() == 403 => {
            crate::output::success(&i18n::f(M::TokenSavedScoped, &[("profile", &name)]))
        }
        Ok(code) => crate::output::info(&i18n::f(
            M::TokenSavedCode,
            &[("code", &code.as_u16().to_string())],
        )),
        Err(err) => crate::output::info(&i18n::f(
            M::TokenSavedProbeFail,
            &[("err", &err.to_string())],
        )),
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
        .or_else(|| profile.map(|p| p.api_url.clone()))
        .unwrap_or_else(|| DEFAULT_API_URL.to_string());
    let has_token = ctx.has_token() || profile.and_then(|p| p.token.as_ref()).is_some();

    println!("{}  {}", i18n::tr(M::LblProfile), style(&name).bold());
    println!("{}  {api_url}", i18n::tr(M::LblApiUrl));
    println!(
        "{}  {}",
        i18n::tr(M::LblToken),
        if has_token {
            style(i18n::tr(M::TokenSet)).green().to_string()
        } else {
            style(i18n::tr(M::TokenUnset)).red().to_string()
        }
    );

    if !has_token {
        crate::output::info(i18n::tr(M::LoginHint));
        return Ok(());
    }

    let client = ctx.client()?;
    let resp = client
        .request(Method::GET, "domains")
        .send()
        .await
        .context("failed to reach the API")?;
    let verdict = match resp.status().as_u16() {
        200 => style(i18n::tr(M::AccessOk).to_string()).green(),
        401 => style(i18n::tr(M::AccessInvalid).to_string()).red(),
        403 => style(i18n::tr(M::AccessForbidden).to_string()).yellow(),
        other => style(i18n::f(
            M::AccessUnexpected,
            &[("code", &other.to_string())],
        ))
        .yellow(),
    };
    println!("{}  {verdict}", i18n::tr(M::LblAccess));
    Ok(())
}

fn logout(ctx: &Context) -> Result<()> {
    let mut cfg = Config::load()?;
    let name = cfg.active_profile_name(ctx.profile_name());
    if let Some(profile) = cfg.profiles.get_mut(&name) {
        profile.token = None;
        cfg.save()?;
        crate::output::success(&i18n::f(M::TokenRemoved, &[("profile", &name)]));
    } else {
        crate::output::info(&i18n::f(M::ProfileNotFound, &[("profile", &name)]));
    }
    Ok(())
}

/// Lightweight token check: GET /domains, only the status code matters.
async fn probe(api_url: &str, token: &str) -> Result<reqwest::StatusCode> {
    let url = format!("{}/api/v1/domains", api_url.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await?;
    Ok(resp.status())
}
