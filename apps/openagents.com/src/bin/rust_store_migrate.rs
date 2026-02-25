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
    auth_store: Option<PathBuf>,
    codex_thread_store: Option<PathBuf>,
    domain_store: Option<PathBuf>,
    backup_dir: PathBuf,
    manifest_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct Manifest {
    schema: &'static str,
    generated_at: String,
    stores: Vec<StoreManifestEntry>,
}

#[derive(Debug, Serialize)]
struct StoreManifestEntry {
    store: String,
    path: String,
    existed: bool,
    backup_path: Option<String>,
    before_sha256: Option<String>,
    after_sha256: String,
    changed: bool,
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
    fs::create_dir_all(&args.backup_dir)
        .map_err(|error| format!("failed to create backup directory: {error}"))?;

    let mut stores = Vec::new();

    if let Some(path) = args.auth_store.as_deref() {
        stores.push(process_store(
            "auth",
            path,
            &args.backup_dir,
            migrate_auth_store,
            auth_counts,
        )?);
    }

    if let Some(path) = args.codex_thread_store.as_deref() {
        stores.push(process_store(
            "codex_threads",
            path,
            &args.backup_dir,
            migrate_codex_thread_store,
            codex_thread_counts,
        )?);
    }

    if let Some(path) = args.domain_store.as_deref() {
        stores.push(process_store(
            "domain",
            path,
            &args.backup_dir,
            migrate_domain_store,
            domain_counts,
        )?);
    }

    if stores.is_empty() {
        return Err("no store paths supplied; pass at least one --auth-store/--codex-thread-store/--domain-store".to_string());
    }

    let manifest = Manifest {
        schema: "openagents.rust_store_migration.v1",
        generated_at: Utc::now().to_rfc3339(),
        stores,
    };

    if let Some(parent) = args.manifest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create manifest directory: {error}"))?;
    }

    let manifest_payload = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("failed to encode manifest: {error}"))?;
    fs::write(&args.manifest_path, manifest_payload)
        .map_err(|error| format!("failed to write manifest: {error}"))?;

    println!(
        "rust-store-migrate: wrote manifest to {}",
        args.manifest_path.display()
    );
    Ok(())
}

fn parse_args<I>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = String>,
{
    let mut auth_store = None;
    let mut codex_thread_store = None;
    let mut domain_store = None;
    let mut backup_dir = None;
    let mut manifest_path = None;

    let args: Vec<String> = args.into_iter().collect();
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
            "--auth-store" => auth_store = Some(PathBuf::from(value)),
            "--codex-thread-store" => codex_thread_store = Some(PathBuf::from(value)),
            "--domain-store" => domain_store = Some(PathBuf::from(value)),
            "--backup-dir" => backup_dir = Some(PathBuf::from(value)),
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag: {flag}")),
        }

        index += 2;
    }

    Ok(Args {
        auth_store,
        codex_thread_store,
        domain_store,
        backup_dir: backup_dir
            .unwrap_or_else(|| PathBuf::from("./storage/app/rust-store-migrate/backups")),
        manifest_path: manifest_path
            .unwrap_or_else(|| PathBuf::from("./storage/app/rust-store-migrate/manifest.json")),
    })
}

fn print_help() {
    println!(
        "Usage: rust_store_migrate [flags]\n\n\
Flags:\n\
  --auth-store <path>           Auth store JSON path\n\
  --codex-thread-store <path>   Codex thread store JSON path\n\
  --domain-store <path>         Domain store JSON path\n\
  --backup-dir <path>           Backup directory (default: ./storage/app/rust-store-migrate/backups)\n\
  --manifest <path>             Manifest output path (default: ./storage/app/rust-store-migrate/manifest.json)\n\
  --help                        Show this help"
    );
}

fn process_store<F, C>(
    store: &str,
    path: &Path,
    backup_dir: &Path,
    migrate: F,
    count: C,
) -> Result<StoreManifestEntry, String>
where
    F: FnOnce(Value) -> Value,
    C: Fn(&Value) -> HashMap<String, u64>,
{
    let existed = path.exists();
    let before_bytes = if existed {
        Some(fs::read(path).map_err(|error| {
            format!("failed to read {} store {}: {error}", store, path.display())
        })?)
    } else {
        None
    };

    let before_sha256 = before_bytes.as_ref().map(|bytes| sha256_hex(bytes));

    let backup_path = if let Some(bytes) = before_bytes.as_ref() {
        let backup_path = backup_dir.join(format!(
            "{}-{}-{}.bak.json",
            store,
            Utc::now().format("%Y%m%dT%H%M%SZ"),
            short_sha256(bytes)
        ));
        fs::write(&backup_path, bytes).map_err(|error| {
            format!(
                "failed to write backup for {} store {}: {error}",
                store,
                backup_path.display()
            )
        })?;
        Some(backup_path)
    } else {
        None
    };

    let current_value = match before_bytes {
        Some(bytes) => serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Object(Map::new())),
        None => Value::Object(Map::new()),
    };

    let migrated = canonicalize_json_value(migrate(current_value));
    let payload = serde_json::to_vec_pretty(&migrated)
        .map_err(|error| format!("failed to encode {} store JSON: {error}", store))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create {} store directory: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(path, &payload).map_err(|error| {
        format!(
            "failed to write {} store {}: {error}",
            store,
            path.display()
        )
    })?;

    let after_sha256 = sha256_hex(&payload);
    let changed = before_sha256
        .as_ref()
        .map(|before| before != &after_sha256)
        .unwrap_or(true);

    Ok(StoreManifestEntry {
        store: store.to_string(),
        path: path.display().to_string(),
        existed,
        backup_path: backup_path.map(|path| path.display().to_string()),
        before_sha256,
        after_sha256,
        changed,
        counts: count(&migrated),
    })
}

fn migrate_auth_store(input: Value) -> Value {
    let mut root = ensure_object(input);
    ensure_object_field(&mut root, "challenges");
    ensure_object_field(&mut root, "sessions");
    ensure_object_field(&mut root, "access_index");
    ensure_object_field(&mut root, "refresh_index");
    ensure_object_field(&mut root, "revoked_refresh_tokens");
    ensure_object_field(&mut root, "revoked_refresh_token_ids");
    ensure_object_field(&mut root, "users_by_id");
    ensure_object_field(&mut root, "users_by_email");
    ensure_object_field(&mut root, "users_by_workos_id");
    ensure_object_field(&mut root, "personal_access_tokens");
    Value::Object(root)
}

fn migrate_codex_thread_store(input: Value) -> Value {
    let mut root = ensure_object(input);
    ensure_object_field(&mut root, "threads");
    ensure_object_field(&mut root, "messages_by_thread");
    Value::Object(root)
}

fn migrate_domain_store(input: Value) -> Value {
    let mut root = ensure_object(input);

    ensure_object_field(&mut root, "autopilots");
    ensure_object_field(&mut root, "autopilot_profiles");
    ensure_object_field(&mut root, "autopilot_policies");
    ensure_object_field(&mut root, "autopilot_runtime_bindings");
    ensure_object_field(&mut root, "runtime_driver_overrides");
    ensure_object_field(&mut root, "l402_credentials");
    ensure_object_field(&mut root, "l402_paywalls");
    ensure_object_field(&mut root, "user_spark_wallets");
    ensure_object_field(&mut root, "user_integrations");
    ensure_array_field(&mut root, "user_integration_audits");
    ensure_object_field(&mut root, "comms_webhook_events");
    ensure_object_field(&mut root, "comms_delivery_projections");
    ensure_array_field(&mut root, "shouts");
    ensure_array_field(&mut root, "whispers");

    ensure_counter_field_with_map_max(
        &mut root,
        "next_user_integration_id",
        "user_integrations",
        "id",
    );
    ensure_counter_field_with_array_max(
        &mut root,
        "next_user_integration_audit_id",
        "user_integration_audits",
        "id",
    );
    ensure_counter_field_with_map_max(
        &mut root,
        "next_comms_webhook_event_id",
        "comms_webhook_events",
        "id",
    );
    ensure_counter_field_with_map_max(
        &mut root,
        "next_comms_delivery_projection_id",
        "comms_delivery_projections",
        "id",
    );
    ensure_counter_field_with_array_max(&mut root, "next_shout_id", "shouts", "id");
    ensure_counter_field_with_array_max(&mut root, "next_whisper_id", "whispers", "id");

    Value::Object(root)
}

fn auth_counts(value: &Value) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    counts.insert("users".to_string(), object_len(value, "users_by_id") as u64);
    counts.insert("sessions".to_string(), object_len(value, "sessions") as u64);
    counts.insert(
        "personal_access_tokens".to_string(),
        object_len(value, "personal_access_tokens") as u64,
    );
    counts
}

fn codex_thread_counts(value: &Value) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    counts.insert("threads".to_string(), object_len(value, "threads") as u64);

    let messages = value
        .get("messages_by_thread")
        .and_then(Value::as_object)
        .map(|map| {
            map.values()
                .filter_map(Value::as_array)
                .map(|rows| rows.len() as u64)
                .sum::<u64>()
        })
        .unwrap_or(0);
    counts.insert("messages".to_string(), messages);
    counts
}

fn domain_counts(value: &Value) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    counts.insert(
        "autopilots".to_string(),
        object_len(value, "autopilots") as u64,
    );
    counts.insert(
        "l402_paywalls".to_string(),
        object_len(value, "l402_paywalls") as u64,
    );
    counts.insert(
        "integrations".to_string(),
        object_len(value, "user_integrations") as u64,
    );
    counts.insert("shouts".to_string(), array_len(value, "shouts") as u64);
    counts.insert("whispers".to_string(), array_len(value, "whispers") as u64);
    counts
}

fn ensure_object(input: Value) -> Map<String, Value> {
    match input {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn ensure_object_field(root: &mut Map<String, Value>, key: &str) {
    let needs_reset = !matches!(root.get(key), Some(Value::Object(_)));
    if needs_reset {
        root.insert(key.to_string(), Value::Object(Map::new()));
    }
}

fn ensure_array_field(root: &mut Map<String, Value>, key: &str) {
    let needs_reset = !matches!(root.get(key), Some(Value::Array(_)));
    if needs_reset {
        root.insert(key.to_string(), Value::Array(Vec::new()));
    }
}

fn ensure_counter_field_with_map_max(
    root: &mut Map<String, Value>,
    counter_key: &str,
    map_key: &str,
    id_field: &str,
) {
    let current = root
        .get(counter_key)
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let max_seen = root
        .get(map_key)
        .and_then(Value::as_object)
        .map(|map| {
            map.values()
                .map(|row| {
                    row.get(id_field)
                        .and_then(Value::as_u64)
                        .or_else(|| {
                            row.get(id_field)
                                .and_then(Value::as_str)
                                .and_then(|raw| raw.parse::<u64>().ok())
                        })
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    if current == 0 {
        root.insert(
            counter_key.to_string(),
            Value::from(max_seen.saturating_add(1)),
        );
    }
}

fn ensure_counter_field_with_array_max(
    root: &mut Map<String, Value>,
    counter_key: &str,
    array_key: &str,
    id_field: &str,
) {
    let current = root
        .get(counter_key)
        .and_then(Value::as_u64)
        .unwrap_or_default();

    let max_seen = root
        .get(array_key)
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.get(id_field)
                        .and_then(Value::as_u64)
                        .or_else(|| {
                            row.get(id_field)
                                .and_then(Value::as_str)
                                .and_then(|raw| raw.parse::<u64>().ok())
                        })
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    if current == 0 {
        root.insert(
            counter_key.to_string(),
            Value::from(max_seen.saturating_add(1)),
        );
    }
}

fn object_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|rows| rows.len())
        .unwrap_or(0)
}

fn array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|rows| rows.len())
        .unwrap_or(0)
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

fn short_sha256(bytes: &[u8]) -> String {
    let full = sha256_hex(bytes);
    full.chars().take(12).collect()
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
    fn auth_migration_fills_missing_keys() {
        let migrated = migrate_auth_store(Value::Null);
        assert_eq!(object_len(&migrated, "users_by_id"), 0);
        assert_eq!(object_len(&migrated, "sessions"), 0);
    }

    #[test]
    fn codex_migration_fills_missing_keys() {
        let migrated = migrate_codex_thread_store(Value::Object(Map::new()));
        assert_eq!(object_len(&migrated, "threads"), 0);
        assert_eq!(object_len(&migrated, "messages_by_thread"), 0);
    }

    #[test]
    fn domain_migration_normalizes_counters() {
        let input = serde_json::json!({
            "user_integrations": {
                "u1::resend": {"id": 7}
            },
            "user_integration_audits": [
                {"id": 10}
            ],
            "comms_webhook_events": {
                "idemp": {"id": 5}
            },
            "comms_delivery_projections": {
                "scope": {"id": 8}
            },
            "shouts": [{"id": 9}],
            "whispers": [{"id": 11}]
        });

        let migrated = migrate_domain_store(input);
        assert_eq!(
            migrated
                .get("next_user_integration_id")
                .and_then(Value::as_u64),
            Some(8)
        );
        assert_eq!(
            migrated
                .get("next_user_integration_audit_id")
                .and_then(Value::as_u64),
            Some(11)
        );
        assert_eq!(
            migrated
                .get("next_comms_webhook_event_id")
                .and_then(Value::as_u64),
            Some(6)
        );
        assert_eq!(
            migrated
                .get("next_comms_delivery_projection_id")
                .and_then(Value::as_u64),
            Some(9)
        );
        assert_eq!(
            migrated.get("next_shout_id").and_then(Value::as_u64),
            Some(10)
        );
        assert_eq!(
            migrated.get("next_whisper_id").and_then(Value::as_u64),
            Some(12)
        );
    }
}
