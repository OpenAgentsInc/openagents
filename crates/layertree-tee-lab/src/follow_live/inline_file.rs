//! Reader for the signet controller's experimental inline-file profile.
//! This matches the public format in `client/src/nostr_mirror.rs` without
//! depending on the attestation client (which itself depends on this crate).
use super::{hex, mirror::Relay, now, unhex};
use base64::{Engine, engine::general_purpose::STANDARD};
use bitcoin::{
    OutPoint,
    hashes::{Hash, sha256},
};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr_transport::Connection;
use serde_json::{Value, json};
use std::time::Duration;

const LIMIT: usize = 32768;
fn one<'a>(event: &'a Event, name: &str) -> Result<&'a str, String> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err("Missing or repeated public file tag.".into());
    }
    tags[0]
        .value()
        .ok_or_else(|| "Missing public file tag value.".into())
}

fn checked(event: &Event, author: &str, kind: u16) -> Result<(), String> {
    if event.kind != kind
        || event.pubkey != author
        || event.content.len() > LIMIT * 2
        || event.tags.len() > 16
        || event
            .tags
            .iter()
            .any(|tag| tag.as_slice().len() > 4 || tag.as_slice().iter().any(|s| s.len() > 256))
    {
        return Err("The public file has another author, kind, or size.".into());
    }
    event
        .validate_nip01_structure()
        .map_err(|_| "Invalid public file event.")?;
    event
        .validate_crypto()
        .map_err(|_| "Invalid public file signature.")?;
    if event.is_expired(now()?) || event.created_at > now()?.saturating_add(300) {
        return Err("The public file is expired or dated in the future.".into());
    }
    Ok(())
}
pub(super) fn decode(
    header: &Event,
    blob: &Event,
    relay: &Relay,
    expected: Option<[u8; 32]>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    checked(header, &relay.publisher, 1065)?;
    checked(blob, &relay.publisher, 1064)?;
    let tree = relay.tree_id.as_deref().ok_or("Missing mirror tree ID.")?;
    let record_hash = unhex(one(header, "r")?, 32)?;
    if record_hash.len() != 32
        || expected.is_some_and(|hash| hash.as_slice() != record_hash)
        || one(header, "e")? != blob.id
        || one(header, "t")? != format!("layertree:{tree}")
        || one(header, "m")? != "application/json"
        || one(blob, "type")? != "application/json"
    {
        return Err("The public file header names another tree or record.".into());
    }
    let bytes = STANDARD
        .decode(&blob.content)
        .map_err(|_| "Invalid public file base64.")?;
    if bytes.len() > LIMIT
        || one(header, "size")? != bytes.len().to_string()
        || one(header, "x")? != sha256::Hash::hash(&bytes).to_string()
    {
        return Err("The public file size or hash differs.".into());
    }
    let value = nostr::contracts::parse_strict_bounded(&bytes, LIMIT)
        .map_err(|_| "Invalid public file JSON.")?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Data {
        record: String,
        unroll: Option<String>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        v: String,
        tree_id: String,
        data: Data,
    }
    let file: File = serde_json::from_value(value).map_err(|_| "Invalid public file fields.")?;
    if file.v != "openagents.layertree.mirror.v1" || file.tree_id != tree {
        return Err("The public file names another tree.".into());
    }
    let record = unhex(&file.data.record, LIMIT)?;
    let decoded = crate::rules::Record::decode(&record).map_err(|_| "Invalid public record.")?;
    if decoded.hash().as_slice() != record_hash {
        return Err("The public record does not match its header.".into());
    }
    let unroll = file
        .data
        .unroll
        .as_deref()
        .map(|text| unhex(text, LIMIT))
        .transpose()?;
    if let Some(bytes) = &unroll {
        crate::unroll::UnrollPackage::decode(bytes)
            .map_err(|_| "Invalid public escape package.")?;
    }
    Ok((record, unroll))
}

/// Optional NIP-42 uses an explicitly configured test identity, never an owner key.
pub(super) async fn query(relay: &Relay, filter: Value) -> Result<Vec<Event>, String> {
    let signer = relay
        .auth_env
        .as_deref()
        .map(|env| {
            let secret = std::env::var(env).map_err(|_| "Set the dedicated relay reader key.")?;
            RelaySigner::from_secret_hex(&secret)
                .map_err(|_| "Invalid relay reader key.".to_string())
        })
        .transpose()?;
    let mut socket = Connection::connect_open(&relay.url, Duration::from_secs(10)).await?;
    let request = json!(["REQ", "layertree-file", filter]);
    socket.send(request.clone()).await?;
    let mut auth = None;
    let mut events = Vec::new();
    for _ in 0..32 {
        let value = socket.next().await?;
        if value.as_array().is_some_and(|a| a.len() == 2) && value[0] == "AUTH" {
            if let Some(signer) = &signer {
                let challenge = value[1]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 4096)
                    .ok_or("Invalid relay challenge.")?;
                let event = signer.sign(
                    now()?,
                    22242,
                    vec![
                        Tag::new(vec!["relay".into(), relay.url.clone()]),
                        Tag::new(vec!["challenge".into(), challenge.into()]),
                    ],
                    String::new(),
                );
                auth = Some(event.id.clone());
                socket.send(json!(["AUTH", event])).await?;
            }
        } else if value.as_array().is_some_and(|a| a.len() == 4)
            && value[0] == "OK"
            && auth
                .as_deref()
                .is_some_and(|id| Some(id) == value[1].as_str())
        {
            if value[2] != true {
                return Err("The relay refused the test reader identity.".into());
            }
            events.clear();
            socket.send(request.clone()).await?;
        } else if value.as_array().is_some_and(|a| a.len() == 3)
            && value[0] == "EVENT"
            && value[1] == "layertree-file"
        {
            if events.len() < 8 {
                if let Ok(event) = serde_json::from_value(value[2].clone()) {
                    events.push(event);
                }
            }
        } else if value[0] == "EOSE" && value[1] == "layertree-file" {
            let _ = socket.close().await;
            return Ok(events);
        } else if value[0] == "CLOSED" && value[1] == "layertree-file" {
            if !(auth.is_some()
                && value[2]
                    .as_str()
                    .is_some_and(|s| s.starts_with("auth-required:")))
            {
                return Err("The relay refused the public file request.".into());
            }
        }
    }
    Err("The relay did not finish the bounded public file request.".into())
}

pub(super) async fn fetch(
    relay: &Relay,
    name: &str,
    hint: Option<[u8; 32]>,
) -> Result<Vec<Vec<u8>>, String> {
    let tree = relay.tree_id.as_deref().ok_or("Missing mirror tree ID.")?;
    let record = name
        .strip_prefix("record-")
        .and_then(|s| s.strip_suffix(".bin"))
        .map(|hash| {
            unhex(hash, 32).and_then(|b| b.try_into().map_err(|_| "Invalid record name.".into()))
        })
        .transpose()?;
    let wanted = record.or(hint);
    let mut filter = json!({"kinds":[1065],"authors":[relay.publisher],"#t":[format!("layertree:{tree}")],"limit":8});
    if let Some(hash) = wanted {
        filter["#r"] = json!([hex(&hash)]);
    }
    let mut result = Vec::new();
    for header in query(relay, filter).await? {
        if checked(&header, &relay.publisher, 1065).is_err() {
            continue;
        }
        let Ok(id) = one(&header, "e") else {
            continue;
        };
        for blob in query(relay, json!({"ids":[id],"limit":1})).await? {
            let Ok((bytes, package)) = decode(&header, &blob, relay, wanted) else {
                continue;
            };
            if record.is_some() {
                result.push(bytes);
            } else if let Some(package) = package {
                if let Ok(decoded) = crate::unroll::UnrollPackage::decode(&package) {
                    let outpoint: OutPoint = decoded.tree_outpoint;
                    if crate::follow::unroll_name(&outpoint) == name {
                        result.push(package);
                    }
                }
            }
        }
    }
    Ok(result)
}
