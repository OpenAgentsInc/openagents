use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

#[derive(Debug)]
struct Args {
    fixture_path: PathBuf,
    auth_store_path: PathBuf,
    codex_thread_store_path: PathBuf,
    domain_store_path: PathBuf,
    manifest_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct SeedManifest {
    schema: &'static str,
    generated_at: String,
    fixture_path: String,
    fixture_sha256: String,
    outputs: Vec<SeedOutput>,
}

#[derive(Debug, Serialize)]
struct SeedOutput {
    store: String,
    path: String,
    sha256: String,
    counts: HashMap<String, u64>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args(env::args().skip(1))?;

    let fixture_bytes = fs::read(&args.fixture_path).map_err(|error| {
        format!(
            "failed to read fixture {}: {error}",
            args.fixture_path.display()
        )
    })?;
    let fixture_sha256 = sha256_hex(&fixture_bytes);

    let fixture = serde_json::from_slice::<Value>(&fixture_bytes)
        .map_err(|error| format!("failed to parse fixture JSON: {error}"))?;

    let rust_stores = fixture
        .get("rust_stores")
        .and_then(Value::as_object)
        .ok_or_else(|| "fixture missing rust_stores object".to_string())?;

    let auth = rust_stores
        .get("auth")
        .cloned()
        .ok_or_else(|| "fixture rust_stores.auth missing".to_string())?;
    let codex_threads = rust_stores
        .get("codex_threads")
        .cloned()
        .ok_or_else(|| "fixture rust_stores.codex_threads missing".to_string())?;
    let domain = rust_stores
        .get("domain")
        .cloned()
        .ok_or_else(|| "fixture rust_stores.domain missing".to_string())?;

    let outputs = vec![
        write_store(
            "auth",
            &args.auth_store_path,
            canonicalize_json_value(auth),
            auth_counts,
        )?,
        write_store(
            "codex_threads",
            &args.codex_thread_store_path,
            canonicalize_json_value(codex_threads),
            codex_thread_counts,
        )?,
        write_store(
            "domain",
            &args.domain_store_path,
            canonicalize_json_value(domain),
            domain_counts,
        )?,
    ];

    if let Some(manifest_path) = args.manifest_path.as_ref() {
        let manifest = SeedManifest {
            schema: "openagents.webparity.seed_manifest.v1",
            generated_at: Utc::now().to_rfc3339(),
            fixture_path: args.fixture_path.display().to_string(),
            fixture_sha256,
            outputs,
        };

        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create manifest directory: {error}"))?;
        }

        fs::write(
            manifest_path,
            serde_json::to_vec_pretty(&manifest)
                .map_err(|error| format!("failed to encode seed manifest: {error}"))?,
        )
        .map_err(|error| format!("failed to write seed manifest: {error}"))?;

        println!(
            "seed-parity-fixtures: wrote manifest {}",
            manifest_path.display()
        );
    }

    Ok(())
}

fn parse_args<I>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    if args.is_empty() {
        return Err("missing required flags; run with --help".to_string());
    }

    let mut fixture_path = None;
    let mut auth_store_path = None;
    let mut codex_thread_store_path = None;
    let mut domain_store_path = None;
    let mut manifest_path = None;

    let mut index = 0usize;
    while index < args.len() {
        let flag = &args[index];
        if flag == "--help" || flag == "-h" {
            print_help();
            std::process::exit(0);
        }

        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for flag: {flag}"))?
            .to_string();

        match flag.as_str() {
            "--fixture" => fixture_path = Some(PathBuf::from(value)),
            "--auth-store" => auth_store_path = Some(PathBuf::from(value)),
            "--codex-thread-store" => codex_thread_store_path = Some(PathBuf::from(value)),
            "--domain-store" => domain_store_path = Some(PathBuf::from(value)),
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag: {flag}")),
        }

        index += 2;
    }

    Ok(Args {
        fixture_path: fixture_path.ok_or_else(|| "--fixture is required".to_string())?,
        auth_store_path: auth_store_path.ok_or_else(|| "--auth-store is required".to_string())?,
        codex_thread_store_path: codex_thread_store_path
            .ok_or_else(|| "--codex-thread-store is required".to_string())?,
        domain_store_path: domain_store_path
            .ok_or_else(|| "--domain-store is required".to_string())?,
        manifest_path,
    })
}

fn print_help() {
    println!(
        "Usage: seed_parity_fixtures --fixture <path> --auth-store <path> --codex-thread-store <path> --domain-store <path> [--manifest <path>]"
    );
}

fn write_store<C>(store: &str, path: &Path, value: Value, count: C) -> Result<SeedOutput, String>
where
    C: Fn(&Value) -> HashMap<String, u64>,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create {} parent directory: {error}",
                path.display()
            )
        })?;
    }

    let payload = serde_json::to_vec_pretty(&value)
        .map_err(|error| format!("failed to encode {} store payload: {error}", store))?;
    fs::write(path, &payload).map_err(|error| {
        format!(
            "failed to write {} store {}: {error}",
            store,
            path.display()
        )
    })?;

    Ok(SeedOutput {
        store: store.to_string(),
        path: path.display().to_string(),
        sha256: sha256_hex(&payload),
        counts: count(&value),
    })
}

fn auth_counts(value: &Value) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    counts.insert(
        "users".to_string(),
        value
            .get("users_by_id")
            .and_then(Value::as_object)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts.insert(
        "personal_access_tokens".to_string(),
        value
            .get("personal_access_tokens")
            .and_then(Value::as_object)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts
}

fn codex_thread_counts(value: &Value) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    counts.insert(
        "threads".to_string(),
        value
            .get("threads")
            .and_then(Value::as_object)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts.insert(
        "messages".to_string(),
        value
            .get("messages_by_thread")
            .and_then(Value::as_object)
            .map(|rows| {
                rows.values()
                    .filter_map(Value::as_array)
                    .map(|messages| messages.len() as u64)
                    .sum()
            })
            .unwrap_or(0),
    );
    counts
}

fn domain_counts(value: &Value) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    counts.insert(
        "autopilots".to_string(),
        value
            .get("autopilots")
            .and_then(Value::as_object)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts.insert(
        "paywalls".to_string(),
        value
            .get("l402_paywalls")
            .and_then(Value::as_object)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts.insert(
        "integrations".to_string(),
        value
            .get("user_integrations")
            .and_then(Value::as_object)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts.insert(
        "shouts".to_string(),
        value
            .get("shouts")
            .and_then(Value::as_array)
            .map(|rows| rows.len() as u64)
            .unwrap_or(0),
    );
    counts
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json_value(value)))
                .collect();
            let mut object = Map::new();
            for (key, value) in sorted {
                object.insert(key, value);
            }
            Value::Object(object)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_json_value).collect())
        }
        scalar => scalar,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();

    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(hex_char(byte >> 4));
        output.push(hex_char(byte & 0x0f));
    }
    output
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => '0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_sorts_nested_objects() {
        let value = serde_json::json!({
            "b": {"z": 1, "a": 2},
            "a": [
                {"d": 1, "c": 2}
            ]
        });

        let canonical = canonicalize_json_value(value);
        let keys: Vec<String> = canonical
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }
}
