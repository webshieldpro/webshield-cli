//! API access layer: HTTP client and typed response _models.

pub mod client;
pub mod _models;
pub mod models;
mod get_url;

pub use client::Client;
