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
    /// A usage error in one line, for mistakes the whole usage would only
    /// bury: points at `openagents COMMAND --help` instead.
    pub fn refuse(self, command: &str, message: &str) -> u8 {
        eprintln!("openagents {command}: {message}");
        if self.json {
            println!("{}", serde_json::json!({ "error": message, "usage": true }));
        }
        EXIT_USAGE
    }

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

/// `2026-10-02` for Unix seconds, in UTC.
pub(crate) fn date(at: u64) -> String {
    let days = i64::try_from(at / 86_400).unwrap_or(0);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
