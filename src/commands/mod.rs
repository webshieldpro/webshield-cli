//! Subcommand implementations. Each module is thin: argument parsing + API calls.

mod util;

pub mod auth;
pub mod billing;
pub mod dns;
pub mod domains;
pub mod lang;
pub mod proxy;
pub mod sites;
pub mod stats;
