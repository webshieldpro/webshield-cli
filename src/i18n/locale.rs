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

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
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

define_locale_codes! {
    En = "en",
    Ru = "ru",
}

fn parse_locale_str(l: &str) -> Option<&str> {
    if l.is_empty() {
        None
    } else {
        l.split(&['_', '-', '.']).next()
    }
}

fn get_locale_code() -> Option<LocaleCode> {
    let locale = sys_locale::get_locale()?;
    let code = parse_locale_str(&locale)?;
    LocaleCode::try_from(code).ok()
}

impl Default for LocaleCode {
    fn default() -> Self {
        get_locale_code().unwrap_or(Self::En)
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
    fn parses_locales() {
        assert_eq!(parse_locale_str("ru_RU.UTF-8"), Some("ru"));
        assert_eq!(parse_locale_str("en-US"), Some("en"));
        assert_eq!(parse_locale_str("en_US"), Some("en"));
        assert_eq!(parse_locale_str("fr"), Some("fr"));
        assert_eq!(parse_locale_str(""), None);
    }
}
