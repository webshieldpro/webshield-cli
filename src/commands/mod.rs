//! Subcommand implementations. Each module is thin: argument parsing + API calls.

// TODO
// It would be nice to: separate the logic for sending requests to the server and displaying the
// response to the user, so that the entire 'api' module and ready-made implementations for sending
// requests could be moved into a separate library and reused in other projects.

mod util;

pub mod auth;
pub mod billing;
pub mod display_table;
pub mod dns;
pub mod domains;
pub mod dyndns;
pub mod hosts;
pub mod lang;
pub mod prog_res;
pub mod proxy;
pub mod run;
pub mod sites;
pub mod stats;
