//! CLI configuration: profiles in `~/.config/webshield/config.toml`.
//!
//! A profile stores the API base URL and (optionally) a personal `wsk_…` token.
//! Source precedence during resolution: command-line flags/env → active profile.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::i18n::LocaleCode;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_API_URL: &str = "https://webshield.pro";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Default profile name (when `--profile`/env is not set).
    #[serde(default)]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    /// Personal `wsk_…` token. Stored in plain text — same as `~/.aws/credentials`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Interface language of this profile. Unset = follow the environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<LocaleCode>,
}

/// Path to the configuration file (`$XDG_CONFIG_HOME/webshield/config.toml`).
pub fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("failed to locate the configuration directory")?;
    Ok(base.join("webshield").join("config.toml"))
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("invalid TOML in {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self).context("failed to serialize the configuration")?;
        std::fs::write(&path, raw)
            .with_context(|| format!("failed to write {}", path.display()))?;
        // The file contains a token — restrict access to the owner (0600) on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
        Ok(())
    }

    /// Active profile name, honoring the override.
    pub fn active_profile_name(&self, override_name: Option<&str>) -> String {
        override_name
            .map(str::to_string)
            .or_else(|| self.default_profile.clone())
            .unwrap_or_else(|| "default".to_string())
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profiles_and_optional_fields() {
        let cfg: Config = toml::from_str(
            r#"
            default_profile = "work"

            [profiles.work]
            api_url = "https://staging.example.com"
            token = "wsk_abc"
            lang = "ru"

            [profiles.home]
            token = "wsk_def"
            "#,
        )
        .unwrap();
        let work = cfg.profile("work").unwrap();
        assert_eq!(work.api_url.as_deref(), Some("https://staging.example.com"));
        assert_eq!(work.lang, Some(LocaleCode::Ru));
        // An omitted api_url/lang stays empty — the caller falls back to the default.
        let home = cfg.profile("home").unwrap();
        assert_eq!(home.api_url, None);
        assert_eq!(home.lang, None);
        assert!(cfg.profile("missing").is_none());
    }

    #[test]
    fn active_profile_precedence() {
        let mut cfg = Config::default();
        // No override, no default → literal "default".
        assert_eq!(cfg.active_profile_name(None), "default");
        cfg.default_profile = Some("work".into());
        assert_eq!(cfg.active_profile_name(None), "work");
        // An explicit override (flag/env) wins over the config default.
        assert_eq!(cfg.active_profile_name(Some("home")), "home");
    }

    #[test]
    fn token_is_omitted_from_serialized_config_when_absent() {
        let mut cfg = Config::default();
        cfg.profiles.insert("p".into(), Profile::default());
        let raw = toml::to_string_pretty(&cfg).unwrap();
        assert!(!raw.contains("token"), "got: {raw}");
    }
}
