//! Output formatting: human-readable tables and JSON for scripts.

use anyhow::Result;
use clap::ValueEnum;
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use console::style;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table (default).
    Table,
    /// Machine-readable JSON.
    Json,
}

/// Prints a value as JSON (used with `-o json`).
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Builds and prints a table with headers and rows.
pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("{}", style(crate::i18n::tr(crate::i18n::M::Empty)).dim());
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers.iter().map(Cell::new));
    for row in rows {
        table.add_row(row.iter().map(Cell::new));
    }
    println!("{table}");
}

pub fn success(msg: &str) {
    eprintln!("{} {}", style("✓").green().bold(), msg);
}

pub fn info(msg: &str) {
    eprintln!("{}", style(msg).dim());
}
