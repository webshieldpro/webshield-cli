//! Locale selection and access.
//!
//! Every language is one `Locale` value literally included from `locales/<code>.iro`
//! at compile time, so a missing or extra key is a compile error rather than a
//! runtime surprise. The active locale is resolved once at startup (flag → env →
//! profile → system locale) and read through the `t!` macro.

use crate::i18n::Locale;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

#[derive(Debug)]
pub struct UnknownLocale(pub String);

impl Display for UnknownLocale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown language `{}` (expected en or ru)", self.0)
    }
}

impl std::error::Error for UnknownLocale {}

macro_rules! define_locale_codes {

    ($($lang:ident = $value:literal),* $(,)?) => {

        #[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum LocaleCode {
            $($lang,)*
        }

        impl LocaleCode {

            // pub const fn as_str(&self) -> &'static str {
            //     match self {
            //         $(Self::$lang => $value,)*
            //     }
            // }

            pub const fn locale(&self) -> Locale {
                match self {
                    $(
                        Self::$lang => include!( concat!("./locales/", $value, ".iro") ),
                    )*
                }
            }
        }

        impl TryFrom<&str> for LocaleCode {
            type Error = UnknownLocale;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                match s.trim().to_lowercase().as_str() {
                    $($value => Ok(Self::$lang),)*
                    other => Err(UnknownLocale(other.to_string())),
                }
            }
        }
    }
}

// The first variant is the default one (English).
define_locale_codes! {
    En = "en",
    Ru = "ru",
}

#[allow(clippy::derivable_impls)]
impl Default for LocaleCode {
    fn default() -> Self {
        Self::En
    }
}

static LOCALE: OnceLock<Locale> = OnceLock::new();

#[cfg(test)]
/// Active locale for tests. Falls back to the default one when nobody called `set_locale`
/// (unit tests building the clap tree, for instance).
pub fn locale() -> &'static Locale {
    LOCALE.get_or_init(|| LocaleCode::default().locale())
}

#[cfg(not(test))]
/// Active locale.
pub fn locale() -> &'static Locale {
    LOCALE.get().expect("locale not initialized")
}

/// Code of the active locale — for showing and storing it.
/// Localized string: `t!(key)` for a fixed one, `t!(key, arg, …)` for a template.
#[macro_export]
macro_rules! t {
    ($name:ident) => {
        $crate::i18n::locale::locale().$name
    };
    ($name:ident, $($arg:expr),+ $(,)?) => {
        $crate::t!($name)($($arg),+)
    };
}

/// Fixes the active locale. Only the first call wins — the language must be
/// resolved before the clap tree is built, everything after that is a no-op.
pub fn set_locale(code: LocaleCode) {
    LOCALE
        .set(code.locale())
        .expect("locale already initialized");
}

/// Language of an environment value like `ru_RU.UTF-8` or `en`.
fn from_env_value(value: &str) -> Option<LocaleCode> {
    let v = value.trim().to_lowercase();
    if v.is_empty() || v == "c" || v == "posix" {
        return None;
    }
    match v.split(['_', '-', '.']).next().unwrap_or_default() {
        "ru" | "rus" | "russian" => Some(LocaleCode::Ru),
        "en" | "eng" | "english" => Some(LocaleCode::En),
        // A known-but-unsupported locale still means "not English by accident".
        _ => Some(LocaleCode::En),
    }
}

/// Pre-extracts a flag value from the raw arguments: help has to be printed in the
/// right language, and by then clap has not parsed anything yet. `names` lists the
/// spellings of one flag, e.g. `["--profile", "-p"]`.
pub fn prescan_flag(args: &[String], names: &[&str]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        for name in names {
            if a == name {
                return it.next().cloned();
            }
            if let Some(v) = a.strip_prefix(&format!("{name}=")) {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[allow(dead_code)]
/// Resolves the language: `--lang` → `WS_LANG` → profile → system locale → English.
pub fn resolve(args: &[String], profile_lang: impl FnOnce() -> Option<LocaleCode>) -> LocaleCode {
    if let Some(code) = prescan_flag(args, &["--lang"]).and_then(|v| LocaleCode::try_from(&*v).ok())
    {
        return code;
    }
    if let Some(code) = std::env::var("WS_LANG")
        .ok()
        .and_then(|v| from_env_value(&v))
    {
        return code;
    }
    if let Some(code) = profile_lang() {
        return code;
    }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(code) = std::env::var(key).ok().and_then(|v| from_env_value(&v)) {
            return code;
        }
    }
    LocaleCode::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_parse_case_insensitively() {
        assert_eq!(LocaleCode::try_from("RU").unwrap(), LocaleCode::Ru);
        assert_eq!(LocaleCode::try_from(" en ").unwrap(), LocaleCode::En);
        assert!(LocaleCode::try_from("klingon").is_err());
    }

    #[test]
    fn env_values_carry_a_region_and_charset() {
        assert_eq!(from_env_value("ru_RU.UTF-8"), Some(LocaleCode::Ru));
        assert_eq!(from_env_value("en-GB"), Some(LocaleCode::En));
        // "No locale set" must not shadow the next source in the chain.
        assert_eq!(from_env_value("C"), None);
        assert_eq!(from_env_value(""), None);
    }

    #[test]
    fn flag_is_prescanned_in_both_spellings() {
        let args = |s: &str| s.split(' ').map(str::to_string).collect::<Vec<_>>();
        assert_eq!(
            prescan_flag(&args("webshield --lang ru domains list"), &["--lang"]),
            Some("ru".to_string())
        );
        assert_eq!(
            prescan_flag(&args("webshield --lang=ru domains list"), &["--lang"]),
            Some("ru".to_string())
        );
        assert_eq!(
            prescan_flag(&args("webshield domains list"), &["--lang"]),
            None
        );
        // A short spelling counts too — `-p work` selects the profile.
        assert_eq!(
            prescan_flag(
                &args("webshield -p work domains list"),
                &["--profile", "-p"]
            ),
            Some("work".to_string())
        );
    }
}
