//! CLI localization (English/Russian).
//!
//! The language is resolved once at startup: `--lang` → env `WS_LANG` → profile →
//! system locale (`LC_ALL`/`LC_MESSAGES`/`LANG`) → English. It is stored in a global
//! `OnceLock` and read via the `t!` macro, so the language does not have to be
//! threaded through every call. clap help is localized the same way: the `about`/
//! `help` attributes are `t!` expressions evaluated when the command tree is built,
//! which is why the locale must be set before that happens.

pub mod locale;
mod structs;

pub use locale::{set_locale, LocaleCode};
pub use structs::Locale;
