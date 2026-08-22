use crate::api::Client;
use crate::t;
use crate::util::config;
use crate::util::config::{Profile, ProfileConfig};
use anyhow::bail;
use std::borrow::Cow;

/// Resolved execution context: API access and output settings.
pub struct Context<'c> {
    profile: Option<&'c str>,
    api_url: Option<String>,
    token: Option<String>,
    pub yes: bool,
    pub cfg: ProfileConfig<'c>,
    client: Option<Client<'c>>,
}

impl<'c> Context<'c> {
    pub fn new(
        profile: Option<&'c str>,
        api_url: Option<String>,
        token: Option<String>,
        yes: bool,
        cfg: ProfileConfig<'c>,
    ) -> Self {
        Self {
            profile,
            api_url,
            token,
            yes,
            cfg,
            client: None,
        }
    }

    pub fn profile_name(&self) -> Option<&'c str> {
        self.profile
    }

    pub fn api_url_override(&self) -> Option<&str> {
        self.api_url.as_deref()
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub fn get_profile(&self) -> Option<&Profile> {
        get_profile_from_cfg(&self.cfg, self.profile)
    }

    fn new_client(&self) -> anyhow::Result<Client<'c>> {
        let profile = self.get_profile();
        let token = self
            .token
            .clone()
            .or_else(|| profile.and_then(|p| p.token.clone()));

        let Some(token) = token else {
            bail!(t!(no_token, &format!("{:?}", self.profile)));
        };

        let api_url = self
            .api_url
            .as_deref()
            .or_else(|| profile.and_then(|p| p.api_url.as_deref()))
            .unwrap_or(config::DEFAULT_API_URL);

        Client::new(api_url, token)
    }

    /// get the HTTP client, resolving URL and token from flags/env/profile.
    pub fn client(&mut self) -> anyhow::Result<&'c mut Client<'_>> {
        if self.client.is_none() {
            self.client = Some(self.new_client()?);
        }
        Ok(self.client.as_mut().unwrap())
    }
}

pub fn get_profile_from_cfg<'a>(
    cfg: &'a ProfileConfig,
    profile_string: Option<&'a str>,
) -> Option<&'a Profile> {
    cfg.profile(get_profile_name(cfg, profile_string).as_ref())
}

pub fn get_profile_name<'a>(
    cfg: &'a ProfileConfig,
    profile_string: Option<&'a str>,
) -> Cow<'a, str> {
    cfg.active_profile_name(profile_string)
}
