//! Output formatting: human-readable tables and JSON for scripts.

use crate::t;
use clap::ValueEnum;
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use console::style;
use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[value(help = t!(arg_output_table))]
    Table,
    #[value(help = t!(arg_output_json))]
    Json,
}

/// Formats a byte count with decimal units (999 B, 1.5 kB, …) — the same scale
/// traffic and storage are billed in.
pub fn fmt_size(bytes: i64) -> String {
    const UNITS: [&str; 6] = ["B", "kB", "MB", "GB", "TB", "PB"];

    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Builds and prints a table with headers and rows.
pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("{}", style(t!(empty)).dim());
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

pub fn success<T: Display>(msg: T) {
    eprintln!("{} {}", style("✓").green().bold(), msg);
}

pub fn info<T: Display>(msg: T) {
    eprintln!("{}", style(msg).dim());
}

pub fn warn<T: Display>(msg: T) {
    eprintln!("{} {}", style("!").yellow().bold(), msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_size_uses_decimal_units() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(999), "999 B");
        assert_eq!(fmt_size(1536), "1.5 kB");
        assert_eq!(fmt_size(5 * 1000 * 1000), "5.0 MB");
    }
}
