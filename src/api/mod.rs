//! API access layer: HTTP client and typed response models.

pub mod _models;
pub mod models;
pub mod client;
pub mod error;
mod request_desc;
pub mod table;

pub use client::Client;
