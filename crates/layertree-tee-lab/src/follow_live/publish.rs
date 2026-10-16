//! Publish only validated public mirror files, with a separate explicit identity.
use super::{atomic_file, hex, mirror::MIRROR_LIMIT, now};
use crate::{
    follow::{record_name, unroll_name},
    rules::Record,
    unroll::UnrollPackage,
};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr_transport::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, io::Read, path::Path, time::Duration};

#[derive(Serialize, Deserialize)]
struct Published {
    relay: String,
    event: Event,
    acknowledged: bool,
}
fn valid_file(name: &str, bytes: &[u8]) -> bool {
    if name.starts_with("record-") {
        return Record::decode(bytes).is_ok_and(|record| record_name(&record.hash()) == name);
    }
    name.starts_with("unroll-")
        && UnrollPackage::decode(bytes).is_ok_and(|pkg| unroll_name(&pkg.tree_outpoint) == name)
}
pub fn note(signer: &RelaySigner, name: &str, bytes: &[u8], created_at: u64) -> Event {
    signer.sign(
        created_at,
        1,
        vec![
            Tag::new(vec!["t".into(), "layertree-mirror-v1".into()]),
            Tag::new(vec!["d".into(), name.into()]),
        ],
        json!({"schema":"layertree.mirror.v1","name":name,"bytes":hex(bytes)}).to_string(),
    )
}
async fn send(url: &str, signer: &RelaySigner, event: &Event) -> Result<(), String> {
    let mut connection = Connection::connect_open(url, Duration::from_secs(10)).await?;
    connection.send(json!(["EVENT", event])).await?;
    let mut auth = None;
    let mut auth_required = false;
    let mut duplicate = false;
    for _ in 0..32 {
        let value = connection.next().await?;
        if value.as_array().is_some_and(|a| a.len() == 2) && value[0] == "AUTH" {
            let challenge = value[1]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 4096)
                .ok_or("Invalid relay authentication challenge.")?;
            let signed = signer.sign(
                now()?,
                22242,
                vec![
                    Tag::new(vec!["relay".into(), url.into()]),
                    Tag::new(vec!["challenge".into(), challenge.into()]),
                ],
                String::new(),
            );
            auth = Some(signed.id.clone());
            connection.send(json!(["AUTH", signed])).await?;
        } else if value.as_array().is_some_and(|a| a.len() == 4)
            && value[0] == "OK"
            && auth
                .as_deref()
                .is_some_and(|id| Some(id) == value[1].as_str())
        {
            if value[2] != true {
                return Err("The relay refused mirror authentication.".into());
            }
            connection.send(json!(["EVENT", event])).await?;
            auth_required = false;
        } else if value.as_array().is_some_and(|a| a.len() == 4)
            && value[0] == "OK"
            && value[1] == event.id
        {
            if value[2] == true {
                let _ = connection.close().await;
                return Ok(());
            }
            let reason = value[3].as_str().unwrap_or("");
            if reason.starts_with("auth-required:") {
                auth_required = true;
                continue;
            }
            if reason.starts_with("duplicate:") {
                duplicate = true;
                connection
                    .send(json!(["REQ","layertree-published",{"ids":[event.id],"limit":1}]))
                    .await?;
            } else {
                return Err("The relay refused the mirror file.".into());
            }
        } else if duplicate && value[0] == "EVENT" && value[1] == "layertree-published" {
            let returned: Event = serde_json::from_value(value[2].clone())
                .map_err(|_| "Invalid relay confirmation.")?;
            if returned == *event && returned.validate_crypto().is_ok() {
                let _ = connection.close().await;
                return Ok(());
            }
        } else if duplicate
            && (value[0] == "EOSE" || value[0] == "CLOSED")
            && value[1] == "layertree-published"
        {
            return Err("The relay did not return the published file.".into());
        }
    }
    if auth_required {
        Err("The relay requires authentication but did not complete it.".into())
    } else {
        Err("The relay did not acknowledge the mirror file.".into())
    }
}

pub fn directory(
    relay: &str,
    source: &Path,
    receipts: &Path,
    secret_env: &str,
) -> Result<usize, String> {
    super::mirror::check_relay_url(relay)?;
    let secret = std::env::var(secret_env).map_err(
        |_| "Set the dedicated mirror signing key in the selected environment variable.",
    )?;
    let signer =
        RelaySigner::from_secret_hex(&secret).map_err(|_| "The mirror signing key is invalid.")?;
    let mut entries: BTreeMap<String, Published> = if receipts.is_file() {
        let mut bytes = Vec::new();
        std::fs::File::open(receipts)
            .map_err(|_| "Could not open mirror receipts.")?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read mirror receipts.")?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Mirror receipts exceed 16 MiB.".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Invalid mirror receipts.")?
    } else {
        BTreeMap::new()
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Relay runtime could not start.")?;
    let mut paths = std::fs::read_dir(source)
        .map_err(|_| "Could not read the public mirror directory.")?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    paths.sort();
    let mut count = 0;
    for path in paths {
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("record-") && !name.starts_with("unroll-") {
            continue;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .map_err(|_| "Could not open a mirror file.")?
            .take(MIRROR_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read a mirror file.")?;
        if bytes.len() as u64 > MIRROR_LIMIT || !valid_file(name, &bytes) {
            return Err("A mirror file has an invalid name, encoding, or size.".into());
        }
        let body =
            json!({"schema":"layertree.mirror.v1","name":name,"bytes":hex(&bytes)}).to_string();
        let expected = note(&signer, name, &bytes, now()?);
        let replace = entries.get(name).is_none_or(|p| {
            p.relay != relay
                || p.event.content != body
                || p.event.pubkey != expected.pubkey
                || super::mirror::decode_note(&p.event, name, &expected.pubkey).is_err()
        });
        if replace {
            entries.insert(
                name.into(),
                Published {
                    relay: relay.into(),
                    event: expected,
                    acknowledged: false,
                },
            );
        }
        let entry = entries.get(name).ok_or("Missing mirror publication.")?;
        if entry.acknowledged {
            continue;
        }
        atomic_file(
            receipts,
            &serde_json::to_vec_pretty(&entries)
                .map_err(|_| "Could not encode mirror receipts.")?,
        )?;
        runtime.block_on(send(relay, &signer, &entries[name].event)).map_err(|_|"The relay did not confirm the mirror file. Check its address and read/write access.")?;
        entries
            .get_mut(name)
            .ok_or("Missing mirror publication.")?
            .acknowledged = true;
        atomic_file(
            receipts,
            &serde_json::to_vec_pretty(&entries)
                .map_err(|_| "Could not encode mirror receipts.")?,
        )?;
        count += 1;
    }
    println!(
        "{}",
        json!({"schema":"layertree.mirror.publication.v1","files_published":count,"receipts":receipts})
    );
    Ok(count)
}
