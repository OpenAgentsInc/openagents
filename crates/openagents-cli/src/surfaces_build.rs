//! Build and check the staged coder text surfaces.
//!
//! `surfaces/coder/` is the source. This module rebuilds `index.json`, the
//! catalog-lines mirror, and `src/surfaces.rs`. `--check` (the default) refuses
//! a tree where those outputs are stale. `--write` rebuilds them.

use regex::Regex;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const INDEX_SCHEMA: &str = "openagents.coder_surface_index.v1";

struct Surface {
    id: &'static str,
    #[allow(dead_code)]
    file: &'static str,
    #[allow(dead_code)]
    authored: bool,
}

const SURFACES: [Surface; 3] = [
    Surface {
        id: "system-prompt",
        file: "system-prompt.v1.json",
        authored: true,
    },
    Surface {
        id: "tool-descriptions",
        file: "tool-descriptions.v1.json",
        authored: true,
    },
    Surface {
        id: "catalog-lines",
        file: "catalog-lines.v1.json",
        authored: false,
    },
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn surfaces_dir(root: &Path) -> PathBuf {
    root.join("surfaces/coder")
}

fn rust_module_path(root: &Path) -> PathBuf {
    root.join("crates/openagents-cli/src/surfaces.rs")
}

fn pretty_object(fields: &[(String, String)], indent: usize) -> String {
    let pad = " ".repeat(indent);
    let inner = " ".repeat(indent + 2);
    let mut out = String::from("{\n");
    for (i, (key, value)) in fields.iter().enumerate() {
        out.push_str(&inner);
        out.push_str(&serde_json::to_string(key).unwrap());
        out.push_str(": ");
        let rendered = value.replace('\n', &format!("\n{inner}"));
        out.push_str(&rendered);
        if i + 1 != fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push('}');
    out
}

fn digest_of(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn text_entries_in_file_order(raw: &str) -> Vec<(String, String)> {
    let parsed: serde_json::Value = serde_json::from_str(raw).expect("surface json");
    let text = parsed
        .get("text")
        .and_then(|v| v.as_object())
        .expect("surface text");
    let start = raw.find("\"text\"").expect("text field");
    let rest = &raw[start..];
    let brace = rest.find('{').expect("text object");
    let body = &rest[brace + 1..];
    let re = Regex::new(r#""((?:\\.|[^"\\])*)"\s*:"#).expect("key regex");
    let mut keys = Vec::new();
    for cap in re.captures_iter(body) {
        let key = cap[1].to_string();
        if text.contains_key(&key) && !keys.iter().any(|existing| existing == &key) {
            keys.push(key);
        }
        if keys.len() == text.len() {
            break;
        }
    }
    keys.into_iter()
        .map(|key| {
            let value = text
                .get(&key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (key, value)
        })
        .collect()
}

fn catalog_entries(root: &Path) -> Vec<(String, String)> {
    let plugins = root.join("plugins");
    let mut names: Vec<String> = fs::read_dir(&plugins)
        .unwrap_or_else(|err| panic!("read {}: {err}", plugins.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();

    let mut entries = Vec::new();
    for name in names {
        let manifest_path = plugins.join(&name).join("manifest.json");
        let Ok(raw) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(plugin_name) = manifest.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(description) = manifest.get("description").and_then(|v| v.as_str()) else {
            continue;
        };
        entries.push((plugin_name.to_string(), description.to_string()));
    }
    entries
}

fn catalog_json(entries: &[(String, String)]) -> String {
    let text_fields: Vec<(String, String)> = entries
        .iter()
        .map(|(key, value)| (key.clone(), serde_json::to_string(value).unwrap()))
        .collect();
    let fields = vec![
        (
            "schema".to_string(),
            serde_json::to_string("openagents.coder_surface.catalog_lines.v1").unwrap(),
        ),
        (
            "surface".to_string(),
            serde_json::to_string("catalog-lines").unwrap(),
        ),
        (
            "source".to_string(),
            serde_json::to_string("plugins/<id>/manifest.json#description").unwrap(),
        ),
        ("text".to_string(), pretty_object(&text_fields, 2)),
    ];
    format!("{}\n", pretty_object(&fields, 0))
}

fn rust_const(key: &str) -> String {
    key.replace(['.', '-'], "_").to_ascii_uppercase()
}

fn rust_module(
    system: &[(String, String)],
    tools: &[(String, String)],
    digests: &[(String, String)],
) -> String {
    let mut lines = vec![
        "// @generated by cargo run -p openagents-cli --bin coder-surfaces -- --write — do not edit.".to_string(),
        "//".to_string(),
        "// The staged coder text surfaces (`surfaces/coder/`), embedded at build".to_string(),
        "// time. Editing a sentence here is editing a build output: the artifact".to_string(),
        "// is the source. Rebuild with `cargo run -p openagents-cli --bin coder-surfaces -- --write`.".to_string(),
        "// `cargo test -p openagents-cli --test coder_surfaces_embed` refuses a tree".to_string(),
        "// where the two disagree.".to_string(),
        "//".to_string(),
        "//! Staged coder text surfaces, embedded from `surfaces/coder/`.".to_string(),
        String::new(),
        "/// The system prompt surface: `surfaces/coder/system-prompt.v1.json`.".to_string(),
        "pub mod system_prompt {".to_string(),
    ];
    push_text_consts(&mut lines, system);
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(
        "/// The tool-description surface: `surfaces/coder/tool-descriptions.v1.json`.".to_string(),
    );
    lines.push("pub mod tool_descriptions {".to_string());
    push_text_consts(&mut lines, tools);
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(
        "/// Every staged surface and the digest of the artifact this was built from.".to_string(),
    );
    lines.push("///".to_string());
    lines.push(
        "/// A run records these so a bench row names exactly which text produced it.".to_string(),
    );
    lines.push(format!(
        "pub const SURFACE_DIGESTS: [(&str, &str); {}] = [",
        digests.len()
    ));
    for (id, digest) in digests {
        lines.push(format!(
            "    ({}, {}),",
            serde_json::to_string(id).unwrap(),
            serde_json::to_string(digest).unwrap()
        ));
    }
    lines.push("];".to_string());
    lines.push(String::new());
    rustfmt(&lines.join("\n"))
}

fn push_text_consts(lines: &mut Vec<String>, entries: &[(String, String)]) {
    for (key, value) in entries {
        lines.push(format!("    /// `{key}`"));
        lines.push(format!(
            "    pub const {}: &str = {};",
            rust_const(key),
            serde_json::to_string(value).unwrap()
        ));
    }
}

fn rustfmt(source: &str) -> String {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rustfmt");
    child
        .stdin
        .as_mut()
        .expect("rustfmt stdin")
        .write_all(source.as_bytes())
        .expect("write rustfmt");
    let output = child.wait_with_output().expect("rustfmt");
    if !output.status.success() {
        panic!(
            "rustfmt failed while building coder surfaces: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).expect("rustfmt utf8")
}

struct Built {
    files: Vec<(String, String)>,
    index: String,
    rust: String,
}

fn surface_index_entry(file: &str, schema: &str, keys: usize, digest: &str) -> String {
    pretty_object(
        &[
            ("file".to_string(), serde_json::to_string(file).unwrap()),
            ("schema".to_string(), serde_json::to_string(schema).unwrap()),
            ("keys".to_string(), keys.to_string()),
            ("digest".to_string(), serde_json::to_string(digest).unwrap()),
        ],
        4,
    )
}

fn build(root: &Path) -> Built {
    let dir = surfaces_dir(root);
    let system_raw = fs::read_to_string(dir.join("system-prompt.v1.json")).expect("system-prompt");
    let tools_raw =
        fs::read_to_string(dir.join("tool-descriptions.v1.json")).expect("tool-descriptions");
    let system_entries = text_entries_in_file_order(&system_raw);
    let tool_entries = text_entries_in_file_order(&tools_raw);
    let catalog = catalog_entries(root);
    let catalog_raw = catalog_json(&catalog);

    let files = vec![
        ("system-prompt.v1.json".to_string(), system_raw.clone()),
        ("tool-descriptions.v1.json".to_string(), tools_raw.clone()),
        ("catalog-lines.v1.json".to_string(), catalog_raw.clone()),
    ];

    let system_schema = serde_json::from_str::<serde_json::Value>(&system_raw)
        .ok()
        .and_then(|v| v.get("schema").and_then(|s| s.as_str()).map(str::to_string))
        .unwrap_or_default();
    let tools_schema = serde_json::from_str::<serde_json::Value>(&tools_raw)
        .ok()
        .and_then(|v| v.get("schema").and_then(|s| s.as_str()).map(str::to_string))
        .unwrap_or_default();

    let digests: Vec<(String, String)> = vec![
        ("system-prompt".to_string(), digest_of(&system_raw)),
        ("tool-descriptions".to_string(), digest_of(&tools_raw)),
        ("catalog-lines".to_string(), digest_of(&catalog_raw)),
    ];

    let surface_fields = vec![
        (
            "system-prompt".to_string(),
            surface_index_entry(
                "system-prompt.v1.json",
                &system_schema,
                system_entries.len(),
                &digests[0].1,
            ),
        ),
        (
            "tool-descriptions".to_string(),
            surface_index_entry(
                "tool-descriptions.v1.json",
                &tools_schema,
                tool_entries.len(),
                &digests[1].1,
            ),
        ),
        (
            "catalog-lines".to_string(),
            surface_index_entry(
                "catalog-lines.v1.json",
                "openagents.coder_surface.catalog_lines.v1",
                catalog.len(),
                &digests[2].1,
            ),
        ),
    ];
    let index = format!(
        "{}\n",
        pretty_object(
            &[
                (
                    "schema".to_string(),
                    serde_json::to_string(INDEX_SCHEMA).unwrap(),
                ),
                ("surfaces".to_string(), pretty_object(&surface_fields, 2)),
            ],
            0,
        )
    );

    Built {
        rust: rust_module(&system_entries, &tool_entries, &digests),
        files,
        index,
    }
}

/// Rebuild artifacts and the embedded Rust module on disk.
pub fn write() {
    let root = repo_root();
    let built = build(&root);
    let dir = surfaces_dir(&root);
    for (file, text) in &built.files {
        fs::write(dir.join(file), text).expect("write surface");
    }
    fs::write(dir.join("index.json"), &built.index).expect("write index");
    fs::write(rust_module_path(&root), &built.rust).expect("write rust module");
    println!(
        "coder surfaces rebuilt: {} + index and Rust module",
        SURFACES.iter().map(|s| s.id).collect::<Vec<_>>().join(", ")
    );
}

/// Refuse a tree whose staged artifacts and embedded module are stale.
pub fn check() -> Result<(), Vec<String>> {
    let root = repo_root();
    let built = build(&root);
    let dir = surfaces_dir(&root);
    let mut failures = Vec::new();
    let hint = "cargo run -p openagents-cli --bin coder-surfaces -- --write";

    for (file, expected) in &built.files {
        let path = dir.join(file);
        compare(
            &path,
            expected,
            &format!("surface {file}"),
            hint,
            &mut failures,
        );
    }
    compare(
        &dir.join("index.json"),
        &built.index,
        "surface digest index",
        hint,
        &mut failures,
    );
    compare(
        &rust_module_path(&root),
        &built.rust,
        "embedded Rust module",
        hint,
        &mut failures,
    );

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn compare(path: &Path, expected: &str, label: &str, hint: &str, failures: &mut Vec<String>) {
    match fs::read_to_string(path) {
        Ok(found) if found == expected => {}
        Ok(_) => failures.push(format!(
            "{label}: {} is stale — it does not match what the staged artifacts build to. Run `{hint}`.",
            path.display()
        )),
        Err(_) => failures.push(format!(
            "{label}: {} does not exist. Run `{hint}`.",
            path.display()
        )),
    }
}
