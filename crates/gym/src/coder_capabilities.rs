//! Coder One's executor capability matrix, as `coder-one capabilities`
//! wrote it.
//!
//! Each executor adapter demonstrates some of five session capabilities:
//! start, observe, stop with a cleanup acknowledgement, resume, and steer.
//! A policy may use only what its adapter demonstrated. The matrix file,
//! `~/.openagents/coder-one/capabilities.json`
//! (`openagents.coder-one.capabilities.v1`), names the tests behind each
//! cell and, when `coder-one capabilities --demonstrate` ran, the installed
//! CLI's own demonstration against a local model server. This module reads
//! it for the runbooks view and `gym coder capabilities`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema of the matrix file.
pub const MATRIX_SCHEMA: &str = "openagents.coder-one.capabilities.v1";

/// The schema of this module's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-capabilities.v1";

/// The capabilities, in the matrix's order.
pub const CAPABILITIES: [&str; 5] = ["start", "observe", "stop", "resume", "steer"];

/// Where `coder-one capabilities` writes the matrix.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/capabilities.json"))
}

/// Reads a matrix file.
///
/// # Errors
///
/// Returns a message when the file is missing or isn't a matrix.
pub fn load(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "cannot read {}: {error}; write it with `coder-one capabilities`",
            path.display()
        )
    })?;
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    if document.get("schema").and_then(Value::as_str) != Some(MATRIX_SCHEMA) {
        return Err(format!("{} is not a {MATRIX_SCHEMA} file", path.display()));
    }
    Ok(document)
}

fn cell(adapter: &Value, capability: &str) -> &'static str {
    let declared = adapter["capabilities"][capability]["demonstrated"].as_bool();
    let real = adapter["demonstration"]["capabilities"][capability]["demonstrated"].as_bool();
    match (declared, real) {
        (Some(true), Some(false)) => "yes!",
        (Some(true), _) => "yes",
        (Some(false), _) => "refused",
        _ => "—",
    }
}

/// The matrix as text rows; `detail` adds each cell's tests and the real
/// demonstration's evidence.
#[must_use]
pub fn lines(document: &Value, detail: bool) -> Vec<String> {
    let mut lines = vec![
        "Executor capability matrix · a policy may use only what its adapter demonstrated"
            .to_owned(),
        format!(
            "  {:<12} {:<7} {:<7} {:<7} {:<7} {:<8} real CLI demonstration",
            "adapter", "start", "observe", "stop", "resume", "steer"
        ),
    ];
    let adapters = document["adapters"].as_array().cloned().unwrap_or_default();
    for adapter in &adapters {
        let demonstration = &adapter["demonstration"];
        let real = if demonstration.is_null() {
            "none recorded".to_owned()
        } else {
            format!(
                "{} at {}{}",
                demonstration["cli_version"]
                    .as_str()
                    .unwrap_or("unknown version"),
                demonstration["at"].as_str().unwrap_or("—"),
                if demonstration["agrees_with_matrix"] == json!(true) {
                    ", agrees"
                } else {
                    ", DISAGREES"
                }
            )
        };
        lines.push(format!(
            "  {:<12} {:<7} {:<7} {:<7} {:<7} {:<8} {real}",
            adapter["adapter"].as_str().unwrap_or("?"),
            cell(adapter, "start"),
            cell(adapter, "observe"),
            cell(adapter, "stop"),
            cell(adapter, "resume"),
            cell(adapter, "steer"),
        ));
    }
    lines.push(
        "  yes: demonstrated by tests · refused: the host refuses it · yes!: the real CLI did not show it"
            .to_owned(),
    );
    if detail {
        for adapter in &adapters {
            lines.push(String::new());
            lines.push(format!(
                "{}: {}",
                adapter["adapter"].as_str().unwrap_or("?"),
                adapter["note"].as_str().unwrap_or_default()
            ));
            for capability in CAPABILITIES {
                let tests: Vec<&str> = adapter["capabilities"][capability]["tests"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                lines.push(format!(
                    "  {capability:<8} {:<8} tests: {}",
                    cell(adapter, capability),
                    tests.join(", ")
                ));
                if let Some(evidence) = adapter["demonstration"]["capabilities"][capability]
                    .get("evidence")
                    .filter(|evidence| !evidence.is_null())
                {
                    let evidence = evidence
                        .as_str()
                        .map_or_else(|| evidence.to_string(), str::to_owned);
                    lines.push(format!(
                        "           real CLI: {}",
                        evidence.chars().take(160).collect::<String>()
                    ));
                }
            }
        }
    }
    lines
}

/// The runbooks view's section: the matrix, or how to write it.
#[must_use]
pub fn runbook_lines(path: Option<&Path>) -> Vec<String> {
    match path.map(load) {
        Some(Ok(document)) => lines(&document, false),
        Some(Err(error)) => vec![
            "Executor capability matrix".to_owned(),
            format!("  {error}"),
        ],
        None => vec![
            "Executor capability matrix".to_owned(),
            "  No HOME; write the matrix with `coder-one capabilities`.".to_owned(),
        ],
    }
}

const HELP: &str = "\
gym coder capabilities [--path PATH] [--json]

Shows each Coder One executor adapter's capability matrix: start, observe,
stop with a cleanup acknowledgement, resume, and steer, with the tests
behind each cell and the installed CLI's last demonstration against a local
model server. Reads ~/.openagents/coder-one/capabilities.json unless --path
names another file. Write it with `coder-one capabilities`, and add
--demonstrate to drive the installed CLIs.";

/// `gym coder capabilities …`.
///
/// # Errors
///
/// Returns a message for an unknown option or a file that doesn't read.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut path = default_path();
    let mut json_output = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--path" => {
                index += 1;
                path = Some(PathBuf::from(
                    args.get(index).ok_or("--path needs a value")?,
                ));
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let path = path.ok_or("no --path and no HOME")?;
    let document = load(&path)?;
    if json_output {
        let value = json!({
            "schema": SCHEMA,
            "path": path.display().to_string(),
            "matrix": document,
        });
        serde_json::to_writer_pretty(&mut *out, &value).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in lines(&document, true) {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        let cells = |steer: bool| {
            json!({
                "start": { "demonstrated": true, "tests": ["adapter::tests::start"] },
                "observe": { "demonstrated": true, "tests": ["adapter::tests::observe"] },
                "stop": { "demonstrated": true, "tests": ["adapter::tests::stop"] },
                "resume": { "demonstrated": true, "tests": ["adapter::tests::resume"] },
                "steer": { "demonstrated": steer, "tests": ["adapter::tests::steer"] },
            })
        };
        json!({
            "schema": MATRIX_SCHEMA,
            "adapters": [
                { "adapter": "claude-code", "capabilities": cells(true), "note": "all five",
                  "demonstration": { "cli_version": "2.1.280 (Claude Code)", "at": "2026-09-22T00:00:00Z",
                    "agrees_with_matrix": true,
                    "capabilities": { "steer": { "demonstrated": true, "evidence": "done: sent a 30-character message" } } } },
                { "adapter": "codex", "capabilities": cells(false), "note": "no steer", "demonstration": null },
            ],
        })
    }

    #[test]
    fn the_matrix_reads_as_text_and_json() {
        let dir = std::env::temp_dir().join(format!("gym-capabilities-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("capabilities.json");
        std::fs::write(&path, fixture().to_string()).unwrap();
        let text = runbook_lines(Some(&path)).join("\n");
        assert!(text.contains("claude-code"), "{text}");
        assert!(text.contains("2.1.280 (Claude Code) at 2026-09-22T00:00:00Z, agrees"));
        let codex = text.lines().find(|line| line.contains("codex")).unwrap();
        assert!(
            codex.contains("refused") && codex.contains("none recorded"),
            "{codex}"
        );
        let mut out = Vec::new();
        command(&["--path".to_owned(), path.display().to_string()], &mut out).unwrap();
        let detail = String::from_utf8(out).unwrap();
        assert!(detail.contains("tests: adapter::tests::steer"));
        assert!(detail.contains("real CLI: done: sent a 30-character message"));
        let mut out = Vec::new();
        command(
            &[
                "--path".to_owned(),
                path.display().to_string(),
                "--json".to_owned(),
            ],
            &mut out,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["matrix"]["adapters"][1]["adapter"], "codex");
        let missing = runbook_lines(Some(&dir.join("absent.json"))).join("\n");
        assert!(missing.contains("coder-one capabilities"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
