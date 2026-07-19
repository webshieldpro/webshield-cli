//! Output formatting: human-readable tables and JSON for scripts.

use crate::i18n::{self, Lang};
use clap::ValueEnum;
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use console::style;
use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table (default).
    Table,
    /// Machine-readable JSON.
    Json,
}

/// Formats a byte count with binary units (1023 B, 1.5 KB, …).
pub fn fmt_size(bytes: i64) -> String {
    let units: [&str; 4] = match i18n::get() {
        Lang::Ru => ["Б", "КБ", "МБ", "ГБ"],
        Lang::En => ["B", "KB", "MB", "GB"],
    };

    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} {}", units[0])
    } else {
        format!("{v:.1} {}", units[i])
    }
}

/// Builds and prints a table with headers and rows.
pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("{}", style(i18n::tr(i18n::M::Empty)).dim());
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
    fn fmt_size_uses_binary_units() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(1023), "1023 B");
        assert_eq!(fmt_size(1536), "1.5 KB");
        assert_eq!(fmt_size(5 * 1024 * 1024), "5.0 MB");
    }
}
