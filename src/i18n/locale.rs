use crate::i18n::structs::Locale;
use clap::ValueEnum;
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

#[derive(Debug)]
pub struct UnknownLocale;

impl Display for UnknownLocale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self)
    }
}

impl std::error::Error for UnknownLocale {}

macro_rules! define_locale_codes {

    ($($lang:ident = $value:literal),* $(,)?) => {

        #[derive(Debug, Clone, Copy,  ValueEnum)]
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

            // pub const ITEMS: &'static [Self] = &[$(Self::$lang),*];

        }

        impl TryFrom<&str> for LocaleCode {
            type Error = UnknownLocale;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                match s {
                    $($value => Ok(Self::$lang),)*
                    _ => Err(UnknownLocale),
                }
            }
        }
    }
}

define_locale_codes! {
    En = "en",
    Ru = "ru",
}

pub static LOCALE: OnceLock<Locale> = OnceLock::new();

#[macro_export]
macro_rules! t {
    ($name:ident) => {
        LOCALE.get().unwrap().$name
    };
}

pub fn load_locale(loc: &str) -> Result<(), UnknownLocale> {
    let l = LocaleCode::try_from(loc)?;
    Ok(set_locale(l))
}

pub fn set_locale(loc: LocaleCode) {
    LOCALE
        .set(loc.locale())
        .expect("locale already initialized");
}
