//! Explicit trajectory exchange; credentials come from the environment or a file.
use crate::{Form, MAX_BYTES, Options, Result, carriage, prepare, relay};
use nostr::domain::RelaySigner;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const USAGE: &str = "openagents trajectory pack|publish FILE --bundle FILE [--to NPUB | --publish-public] [--document] [--metadata FILE] [--key-file FILE] [--relay URL]\nopenagents trajectory retry BUNDLE --relay URL [--key-file FILE]\nopenagents trajectory fetch EVENT --relay URL --output FILE [--source-host HOST] [--key-file FILE]";

pub async fn run(words: Vec<String>) -> Result<Option<Value>> {
    if words.is_empty() || words.iter().any(|w| matches!(w.as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return Ok(None);
    }
    let command = words.first().ok_or(USAGE)?;
    let input = words.get(1).filter(|s| !s.starts_with('-')).ok_or(USAGE)?;
    let mut options: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut index = 2;
    while index < words.len() {
        let name = &words[index];
        if matches!(name.as_str(), "--document" | "--publish-public") {
            options.entry(name.clone()).or_default();
            index += 1;
            continue;
        }
        if !matches!(
            name.as_str(),
            "--bundle"
                | "--to"
                | "--metadata"
                | "--key-file"
                | "--relay"
                | "--output"
                | "--source-host"
        ) {
            return Err(format!("unknown option {name}"));
        }
        let value = words
            .get(index + 1)
            .filter(|s| !s.starts_with("--"))
            .ok_or("option needs a value")?;
        let slot = options.entry(name.clone()).or_default();
        if name != "--source-host" && !slot.is_empty() {
            return Err(format!("option repeated: {name}"));
        }
        slot.push(value.clone());
        index += 2;
    }
    let option = |name: &str| {
        options
            .get(name)
            .and_then(|v| v.first())
            .map(String::as_str)
    };
    let credential = if let Some(path) = option("--key-file") {
        let path = Path::new(path);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(path)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("key file must be readable only by its owner".into());
            }
        }
        String::from_utf8(read(path, 4096)?).map_err(|_| "key file is not UTF-8")?
    } else {
        std::env::var("OPENAGENTS_NOSTR_SECRET")
            .map_err(|_| "set OPENAGENTS_NOSTR_SECRET or use --key-file")?
    };
    let secret = if credential.trim().starts_with("nsec1") {
        SecretKey::from_byte_array(
            nostr::nip19::decode_nsec(credential.trim()).map_err(|_| "invalid Nostr key")?,
        )
        .map_err(|_| "invalid Nostr key")?
    } else {
        credential
            .trim()
            .parse::<SecretKey>()
            .map_err(|_| "invalid Nostr key")?
    };
    let url = option("--relay");
    let result = match command.as_str() {
        "pack" | "publish" => {
            if options.contains_key("--publish-public") && option("--to").is_some() {
                return Err("choose private sharing or public publication".into());
            }
            let metadata = option("--metadata")
                .map(|p| read(Path::new(p), 1_048_576))
                .transpose()?
                .map(|b| nostr::contracts::parse_strict(&b).map_err(|e| e.to_string()))
                .transpose()?
                .unwrap_or(json!({}));
            let fields = [
                "derivation",
                "task",
                "run",
                "selector",
                "parent",
                "children",
                "previous",
            ];
            if !metadata.is_object()
                || metadata
                    .as_object()
                    .is_some_and(|m| m.keys().any(|k| !fields.contains(&k.as_str())))
            {
                return Err("unsupported trajectory metadata field".into());
            }
            let value = |k: &str| metadata.get(k).filter(|v| !v.is_null()).cloned();
            let extra = Options {
                derivation: value("derivation"),
                task: value("task"),
                run: value("run")
                    .map(|v| v.as_str().map(str::to_string).ok_or("run must be text"))
                    .transpose()?,
                selector: value("selector"),
                parent: value("parent"),
                children: match value("children") {
                    Some(Value::Array(v)) => v,
                    Some(_) => return Err("children must be an array".into()),
                    None => Vec::new(),
                },
                previous: value("previous"),
            };
            let bytes = read(Path::new(input), MAX_BYTES)?;
            let package = prepare(
                &bytes,
                if options.contains_key("--document") {
                    Form::Document
                } else {
                    Form::Log
                },
                extra,
            )?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "clock is before Unix time")?
                .as_secs();
            let bundle = if options.contains_key("--publish-public") {
                let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
                    .map_err(|_| "invalid signer")?;
                carriage::public(&package, &signer, now, true)?
            } else {
                let recipient = option("--to").ok_or(
                    "private sharing needs --to NPUB; public sharing needs --publish-public",
                )?;
                let recipient = if recipient.starts_with("npub1") {
                    nostr::nip19::decode_npub(recipient)
                        .map_err(|_| "invalid recipient")?
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect()
                } else {
                    recipient.to_string()
                };
                carriage::private(&package, &secret, &recipient, now, now + 30 * 24 * 3600)?
            };
            let events = json!({"manifest":bundle.manifest,"chunks":bundle.chunks});
            write_new(
                Path::new(
                    option("--bundle")
                        .ok_or("use --bundle FILE to keep the exact signed events for retries")?,
                ),
                &serde_json::to_vec(&events).map_err(|e| e.to_string())?,
            )?;
            if command == "publish" {
                relay::publish(
                    url.ok_or("publication needs --relay URL")?,
                    &secret,
                    &bundle,
                )
                .await?;
            }
            json!({"manifest":bundle.manifest.id,"chunks":bundle.chunks.len(),"public":bundle.manifest.kind==3198,"published":command=="publish"})
        }
        "retry" => {
            let value = nostr::contracts::parse_strict_bounded(
                &read(Path::new(input), MAX_BYTES * 4)?,
                MAX_BYTES * 4,
            )
            .map_err(|e| e.to_string())?;
            let bundle = carriage::Bundle {
                manifest: serde_json::from_value(value["manifest"].clone())
                    .map_err(|e| e.to_string())?,
                chunks: serde_json::from_value(value["chunks"].clone())
                    .map_err(|e| e.to_string())?,
            };
            relay::publish(url.ok_or("retry needs --relay URL")?, &secret, &bundle).await?;
            json!({"manifest":bundle.manifest.id,"published":true})
        }
        "fetch" => {
            let hosts = options
                .get("--source-host")
                .into_iter()
                .flatten()
                .cloned()
                .collect::<BTreeSet<_>>();
            let verified = relay::fetch(
                url.ok_or("fetch needs --relay URL")?,
                &secret,
                input,
                &hosts,
            )
            .await?;
            write_new(
                Path::new(option("--output").ok_or("fetch needs --output FILE")?),
                &verified.bytes,
            )?;
            json!({"trajectory_id":verified.manifest.trajectory_id(),"bytes":verified.bytes.len(),"verification":verified.status.word(),"linked_trajectories":"not fetched"})
        }
        _ => return Err(USAGE.into()),
    };
    Ok(Some(result))
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("file exceeds its byte bound".into());
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist_noclobber(path)
        .map_err(|e| e.error.to_string())?;
    Ok(())
}
