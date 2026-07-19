//! API access layer: HTTP client and typed response models.

pub mod client;
pub mod error;
pub mod models;
mod request_desc;
pub mod table;

pub use client::Client;
