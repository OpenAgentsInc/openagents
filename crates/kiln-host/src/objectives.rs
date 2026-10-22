//! Publish and consume the native WORK history, without inventing an event kind.
use crate::{Result, load, save};
use grid_foreman::objectives::{Catalog, Objective};
use kiln_git::{keys::Signer, relay};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
use work_host::{authority::Authority, reader::Reader};
use workspace_host::{
    artifact::{self, Artifacts},
    host::request,
};

#[derive(Default, Serialize, Deserialize)]
struct Index {
    creating: bool,
    scope: Option<Value>,
    entries: BTreeMap<String, Entry>,
    publications: BTreeMap<String, Event>,
}
#[derive(Serialize, Deserialize)]
struct Entry {
    work: Value,
    pending: Option<Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveRecord {
    pub v: String,
    pub requires: Vec<String>,
    pub catalog: String,
    pub objective: Objective,
    pub scope: Event,
    pub history: Vec<Event>,
    pub commands: Vec<Event>,
    pub revision: Value,
    pub snapshot: Value,
    pub body: Value,
}

/// Local owner administration imports exact, conditionally replaced initiatives.
/// The intent is written before invoking WORK; an uncertain scope creation stops.
pub fn import(
    root: &Path,
    key: &Signer,
    catalog: &Catalog,
    now: u64,
) -> Result<Vec<ObjectiveRecord>> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let lock = std::fs::File::create(root.join("import.lock")).map_err(|e| e.to_string())?;
    fs2::FileExt::lock_exclusive(&lock).map_err(|e| e.to_string())?;
    let path = root.join("index.json");
    let mut index: Index = if path.exists() {
        load(&path)?
    } else {
        Index::default()
    };
    let state = root.join("work");
    let host = if state.join("state.json").exists() {
        Authority::open(state.clone(), key.secret)?
    } else {
        Authority::initialize(state.clone(), key.secret)?
    };
    let artifacts = Artifacts::new(state)?;
    if index.scope.is_none() {
        if index.creating {
            return Err("scope creation was interrupted; inspect WORK before continuing".into());
        }
        index.creating = true;
        save(&path, &index)?;
        index.scope = Some(host.create_scope(Default::default(), now)?);
        index.creating = false;
        save(&path, &index)?;
    }
    let scope_ref = index.scope.clone().ok_or("scope missing")?;
    let scope = artifacts.json(&scope_ref)?;
    let mut output = Vec::new();
    for objective in &catalog.objectives {
        if objective.desired_state == "proposed" {
            continue;
        }
        if !index.entries.contains_key(&objective.id) {
            index.entries.insert(objective.id.clone(), Entry { work: json!({"authority":key.key.pubkey(),"scope":scope["scope"],"item":artifact::token()}), pending:None });
            save(&path, &index)?;
        }
        let work = index.entries[&objective.id].work.clone();
        let read = request(
            &key.secret,
            key.key.pubkey(),
            json!({"v":"openagents.work-history-read.v1","requires":[],"issuer":key.key.pubkey(),"work":work}),
            now,
        )?;
        // A fresh identity has no history. Existing histories must remain readable.
        let history = if index.entries[&objective.id].pending.is_none() {
            host.read_history(&read, now).ok()
        } else {
            Some(host.read_history(&read, now).or_else(|e| {
                if e.contains("work unavailable") {
                    Ok(json!({"records":[]}))
                } else {
                    Err(e)
                }
            })?)
        };
        let prior = history
            .as_ref()
            .and_then(|h| h["records"].as_array())
            .and_then(|r| r.last());
        let body = serde_json::to_value(objective).map_err(|e| e.to_string())?;
        let body_ref = artifacts.put_json(&body)?;
        let snapshot = json!({"v":"openagents.work-item.v1","requires":[],"kind":"initiative","title":objective.title,"body":body_ref,"accountable":key.key.pubkey(),"state":if objective.desired_state=="active"{"open"}else{"blocked"},"frame":null,"acceptance":null,"delegation":null,"relations":[],"resources":[],"evidence":[],"disposition":null,"labels":[],"due_at":null,"timebox":null});
        if prior.is_none_or(|p| p["snapshot"] != snapshot) {
            if index.entries[&objective.id].pending.is_none() {
                let command = json!({"v":"openagents.work-command.v1","requires":[],"issuer":key.key.pubkey(),"command_id":artifact::token(),"scope":scope_ref,"work":work,"action":if prior.is_none(){"create"}else{"replace"},"expected":prior.map(|p|p["revision"].clone()),"snapshot":artifacts.put_json(&snapshot)?,"source":null,"reason":null,"expires_at":now+300});
                index
                    .entries
                    .get_mut(&objective.id)
                    .ok_or("entry missing")?
                    .pending = Some(request(&key.secret, key.key.pubkey(), command, now)?);
                save(&path, &index)?;
            }
            let event = index.entries[&objective.id]
                .pending
                .as_ref()
                .ok_or("intent missing")?;
            host.command(event, &BTreeMap::new(), now)?;
        }
        index
            .entries
            .get_mut(&objective.id)
            .ok_or("entry missing")?
            .pending = None;
        save(&path, &index)?;
        let history = host.read_history(&read, now)?;
        let records = history["records"].as_array().ok_or("history missing")?;
        let last = records.last().ok_or("revision missing")?;
        output.push(ObjectiveRecord {
            v: "openagents.foreman-work-objective.v1".into(),
            requires: vec![],
            catalog: catalog.revision(),
            objective: objective.clone(),
            scope: request(&key.secret, key.key.pubkey(), scope.clone(), now)?,
            history: records
                .iter()
                .map(|r| serde_json::from_value(r["event"].clone()).map_err(|e| e.to_string()))
                .collect::<Result<_>>()?,
            commands: records
                .iter()
                .map(|r| {
                    serde_json::from_value(r["command"]["original"].clone())
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<_>>()?,
            revision: last["revision"].clone(),
            snapshot,
            body,
        });
    }
    Ok(output)
}

/// Explicit owner disclosure to the configured worker. Public tags contain no title.
pub async fn publish(
    root: &Path,
    key: &Signer,
    catalog: &Catalog,
    recipient: &str,
    url: &str,
    now: u64,
) -> Result<Vec<String>> {
    let records = import(root, key, catalog, now)?;
    let lock = std::fs::File::create(root.join("import.lock")).map_err(|e| e.to_string())?;
    fs2::FileExt::lock_exclusive(&lock).map_err(|e| e.to_string())?;
    let path = root.join("index.json");
    let mut index: Index = load(&path)?;
    let epoch = now / 1800;
    index.publications.retain(|id, _| {
        id.rsplit(':')
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .is_some_and(|n| n >= epoch.saturating_sub(1))
    });
    let mut ids = Vec::new();
    for mut record in records {
        // Rewrap original authority declarations for this exact recipient.
        let open = |e: &Event| -> Result<Value> {
            let p = nostr::private_artifact::open(e, &key.secret).map_err(|e| e.to_string())?;
            serde_json::from_slice(p.inline_bytes().ok_or("inline declaration missing")?)
                .map_err(|e| e.to_string())
        };
        record.scope = request(&key.secret, recipient, open(&record.scope)?, now)?;
        record.history = record
            .history
            .iter()
            .map(|e| request(&key.secret, recipient, open(e)?, now))
            .collect::<Result<_>>()?;
        record.commands = record
            .commands
            .iter()
            .map(|e| request(&key.secret, recipient, open(e)?, now))
            .collect::<Result<_>>()?;
        let id = format!(
            "{}:{}:{}:{}",
            recipient, record.objective.id, record.revision["artifact"]["digest"], epoch
        );
        if !index.publications.contains_key(&id) {
            let event = request(
                &key.secret,
                recipient,
                serde_json::to_value(&record).map_err(|e| e.to_string())?,
                now,
            )?;
            index.publications.insert(id.clone(), event);
            save(&path, &index)?;
        }
        let event = &index.publications[&id];
        relay::publish(url, &key.secret, event).await?;
        ids.push(event.id.clone());
    }
    Ok(ids)
}

pub fn verify(
    record: &ObjectiveRecord,
    authority: &str,
    secret: &secp256k1::SecretKey,
) -> Result<()> {
    if record.v != "openagents.foreman-work-objective.v1" || !record.requires.is_empty() {
        return Err("unsupported objective record".into());
    }
    let mut reader = Reader::default();
    reader.admit_scope(&record.scope, secret, authority)?;
    for event in &record.history {
        reader.ingest(event, secret)?;
    }
    if record.history.len() != record.commands.len() {
        return Err("original command history is incomplete".into());
    }
    for (revision, command) in record.history.iter().zip(&record.commands) {
        let r = nostr::private_artifact::open(revision, secret).map_err(|e| e.to_string())?;
        let c = nostr::private_artifact::open(command, secret).map_err(|e| e.to_string())?;
        let rb: Value = serde_json::from_slice(r.inline_bytes().ok_or("revision bytes")?)
            .map_err(|e| e.to_string())?;
        let cb: Value = serde_json::from_slice(c.inline_bytes().ok_or("command bytes")?)
            .map_err(|e| e.to_string())?;
        if c.signer() != authority
            || cb["issuer"] != authority
            || artifact::json_ref(&cb)? != rb["command"]
        {
            return Err("original WORK command differs".into());
        }
    }
    let w = nostr::work::work_ref(&record.revision["work"]).map_err(|e| e.to_string())?;
    if w.authority != authority || reader.head(&w)? != Some(&record.revision) {
        return Err("objective revision differs from signed history".into());
    }
    let last = record.history.last().ok_or("objective history empty")?;
    let plain = nostr::private_artifact::open(last, secret).map_err(|e| e.to_string())?;
    let revision: Value = serde_json::from_slice(plain.inline_bytes().ok_or("revision missing")?)
        .map_err(|e| e.to_string())?;
    if artifact::json_ref(&record.snapshot)? != revision["snapshot"]
        || artifact::json_ref(&record.body)? != record.snapshot["body"]
        || record.body != serde_json::to_value(&record.objective).map_err(|e| e.to_string())?
        || record.snapshot["kind"] != "initiative"
    {
        return Err("objective artifact bytes differ".into());
    }
    Ok(())
}

pub async fn consult(
    key: &Signer,
    authority: &str,
    url: &str,
    id: &str,
) -> Result<ObjectiveRecord> {
    let events = relay::query(
        url,
        &key.secret,
        json!({"kinds":[3188],"authors":[authority],"#p":[key.key.pubkey()],"since":crate::now().saturating_sub(3600),"limit":256}),
    )
    .await?;
    if events.len() >= 256 {
        return Err("objective query is incomplete".into());
    }
    let mut selected: Option<ObjectiveRecord> = None;
    let mut combined = Reader::default();
    for event in events {
        if event.pubkey != authority {
            continue;
        }
        let Ok(p) = nostr::private_artifact::open(&event, &key.secret) else {
            continue;
        };
        if p.body().retain_until < crate::now() || p.body().issued_at > crate::now() + 300 {
            continue;
        }
        let Some(bytes) = p.inline_bytes() else {
            continue;
        };
        let Ok(record) = serde_json::from_slice::<ObjectiveRecord>(bytes) else {
            continue;
        };
        if record.objective.id != id {
            continue;
        }
        verify(&record, authority, &key.secret)?;
        combined.admit_scope(&record.scope, &key.secret, authority)?;
        for event in &record.history {
            combined.ingest(event, &key.secret)?;
        }
        if let Some(old) = &selected {
            if old.revision["work"] != record.revision["work"] {
                return Err("objective identity conflicts".into());
            }
            let n = old.revision["revision"].as_u64().ok_or("revision number")?;
            let m = record.revision["revision"]
                .as_u64()
                .ok_or("revision number")?;
            if n == m && old.revision != record.revision {
                return Err("objective history fork".into());
            }
            if n >= m {
                continue;
            }
            if !record.history.iter().any(|e| {
                nostr::private_artifact::open(e, &key.secret)
                    .ok()
                    .and_then(|p| {
                        p.inline_bytes()
                            .and_then(|b| serde_json::from_slice::<Value>(b).ok())
                    })
                    .is_some_and(|v| {
                        artifact::json_ref(&v).ok() == Some(old.revision["artifact"].clone())
                    })
            }) {
                return Err("objective predecessor missing".into());
            }
        }
        selected = Some(record);
    }
    let record = selected.ok_or("published objective unavailable")?;
    if record.objective.desired_state != "active" || record.snapshot["state"] != "open" {
        return Err("objective is paused".into());
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(n: u8) -> Signer {
        Signer {
            secret: secp256k1::SecretKey::from_byte_array([n; 32]).unwrap(),
            key: kiln::cj::Key::from_bytes([n; 32]).unwrap(),
        }
    }
    #[test]
    fn imports_keep_native_identity_and_revision_across_restart() {
        let temp = tempfile::tempdir().unwrap();
        let key = key(1);
        let mut c = Catalog::parse(grid_foreman::objectives::CATALOG.as_bytes()).unwrap();
        let first = import(temp.path(), &key, &c, 100).unwrap();
        assert_eq!(first.len(), 5);
        for r in &first {
            verify(r, key.key.pubkey(), &key.secret).unwrap();
        }
        let retry = import(temp.path(), &key, &c, 200).unwrap();
        assert_eq!(
            first.iter().map(|r| &r.revision).collect::<Vec<_>>(),
            retry.iter().map(|r| &r.revision).collect::<Vec<_>>()
        );
        c.objectives[0]
            .intent
            .push_str(" Include bounded CPU decoding checks.");
        let changed = import(temp.path(), &key, &c, 300).unwrap();
        assert_eq!(first[0].revision["work"], changed[0].revision["work"]);
        assert_eq!(changed[0].revision["revision"], 2);
        verify(&changed[0], key.key.pubkey(), &key.secret).unwrap();
        let mut forged = changed[0].clone();
        forged.body["intent"] = json!("run anything");
        assert!(verify(&forged, key.key.pubkey(), &key.secret).is_err());
        let other = self::key(2);
        assert!(verify(&changed[0], other.key.pubkey(), &key.secret).is_err());
    }
    #[test]
    fn interrupted_scope_creation_never_creates_another_scope() {
        let temp = tempfile::tempdir().unwrap();
        let key = key(1);
        save(
            &temp.path().join("index.json"),
            &Index {
                creating: true,
                ..Default::default()
            },
        )
        .unwrap();
        let c = Catalog::parse(grid_foreman::objectives::CATALOG.as_bytes()).unwrap();
        assert!(
            import(temp.path(), &key, &c, 100)
                .unwrap_err()
                .contains("interrupted")
        );
    }
}
