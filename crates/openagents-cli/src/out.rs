//! Command output: one JSON document per command under `--json`, and a
//! short text rendering otherwise. Errors go to stderr in both modes; under
//! `--json` they are also a `{"error": ...}` document on stdout.

use serde_json::Value;

/// The operation was refused or failed.
pub const EXIT_FAILURE: u8 = 1;
/// The command line was invalid.
pub const EXIT_USAGE: u8 = 64;

#[derive(Clone, Copy, Debug)]
pub struct Output {
    json: bool,
}

impl Output {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    pub fn json(self) -> bool {
        self.json
    }

    /// Print `value` as JSON, or as the text `render` makes of it.
    pub fn emit(self, value: &Value, render: impl FnOnce(&Value) -> String) {
        if self.json {
            println!("{value}");
        } else {
            let text = render(value);
            if !text.is_empty() {
                println!("{text}");
            }
        }
    }

    /// Print a stream item: one JSON line, or the text `render` makes of it.
    pub fn line(self, value: &Value, render: impl FnOnce(&Value) -> String) {
        self.emit(value, render);
    }

    /// Report a failure and return the failure exit code.
    pub fn fail(self, command: &str, message: &str) -> u8 {
        eprintln!("openagents {command}: {message}");
        if self.json {
            println!("{}", serde_json::json!({ "error": message }));
        }
        EXIT_FAILURE
    }

    /// Report a usage error, print `usage`, and return the usage exit code.
    pub fn usage(self, command: &str, message: &str, usage: &str) -> u8 {
        eprintln!("openagents {command}: {message}\n\n{usage}");
        if self.json {
            println!("{}", serde_json::json!({ "error": message, "usage": true }));
        }
        EXIT_USAGE
    }
}

/// A readable table: rows of cells, columns padded to the widest cell.
pub fn table(rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if widths.len() <= index {
                widths.push(0);
            }
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(index, cell)| {
                    if index + 1 == row.len() {
                        cell.clone()
                    } else {
                        format!("{cell:<width$}", width = widths[index])
                    }
                })
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
