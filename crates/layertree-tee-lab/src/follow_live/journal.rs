//! Durable observations and an alarm outbox. Receivers deduplicate by alarm ID.
use super::{
    chain::{client, endpoint},
    hex, now,
};
use crate::{follow::Seen, tree::tagged_hash};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const JOURNAL_LIMIT: u64 = 128 * 1024 * 1024;
pub struct Journal {
    file: File,
    _lock: File,
    pub dir: PathBuf,
    known: HashSet<String>,
    pending: BTreeMap<String, Value>,
    network: String,
    tree: String,
    member: u64,
    client: reqwest::blocking::Client,
    webhook: Option<reqwest::Url>,
    notification_cursor: Option<String>,
}
impl Journal {
    pub fn open(
        dir: &Path,
        network: &str,
        tree: &str,
        member: u64,
        binding: &str,
        webhook_env: Option<&str>,
    ) -> Result<Self, String> {
        let webhook = webhook_env
            .and_then(|key| std::env::var(key).ok())
            .map(|url| endpoint(&url))
            .transpose()?;
        Self::open_configured(dir, network, tree, member, binding, webhook)
    }
    pub(super) fn open_configured(
        dir: &Path,
        network: &str,
        tree: &str,
        member: u64,
        binding: &str,
        webhook: Option<reqwest::Url>,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|_| "Could not create the follower state directory.")?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("follower.lock"))
            .map_err(|_| "Could not open the follower lock.")?;
        lock.try_lock()
            .map_err(|_| "Another follower already uses this state directory.")?;
        let bind = dir.join("binding");
        match OpenOptions::new().create_new(true).write(true).open(&bind) {
            Ok(mut file) => {
                file.write_all(binding.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "Could not save the follower's tree identity.")?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(&bind)
                    .map_err(|_| "Could not read the follower's tree identity.")?
                    != binding.as_bytes()
                {
                    return Err("This state directory belongs to another tree or member.".into());
                }
            }
            Err(_) => return Err("Could not save the follower's tree identity.".into()),
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .append(true)
            .open(dir.join("observations.jsonl"))
            .map_err(|_| "Could not open the observation log.")?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(JOURNAL_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read the observation log.")?;
        if bytes.len() as u64 > JOURNAL_LIMIT {
            return Err("The observation log exceeds 128 MiB.".into());
        }
        // A crash may leave an unfinished final line. It was never acknowledged;
        // replaying the chain will produce that observation again.
        let end = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |p| p + 1);
        if end < bytes.len() {
            file.set_len(end as u64)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Could not recover the observation log.")?;
            bytes.truncate(end);
        }
        let mut known = HashSet::new();
        let mut pending = BTreeMap::new();
        for line in bytes.split(|b| *b == b'\n').filter(|b| !b.is_empty()) {
            let row: Value = serde_json::from_slice(line)
                .map_err(|_| "The observation log has a damaged completed line.")?;
            if row["kind"] == "light_client" {
                let id = row["id"]
                    .as_str()
                    .ok_or("The observation log has no observation ID.")?
                    .to_owned();
                known.insert(id.clone());
                if row["result"] != "ok" {
                    pending.insert(id, row);
                }
            } else if row["kind"] == "webhook_ack" {
                if let Some(id) = row["id"].as_str() {
                    pending.remove(id);
                }
            }
        }
        Ok(Self {
            file,
            _lock: lock,
            dir: dir.into(),
            known,
            pending,
            network: network.into(),
            tree: tree.into(),
            member,
            client: client()?,
            webhook,
            notification_cursor: None,
        })
    }
    fn append(&mut self, row: &Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(row).map_err(|_| "Could not encode an observation.")?;
        bytes.push(b'\n');
        self.file
            .write_all(&bytes)
            .and_then(|_| self.file.sync_all())
            .map_err(|_| "Could not save an observation.".into())
    }
    pub fn observe(&mut self, seen: &Seen, observed_tip: u32) -> Result<(), String> {
        let result = seen
            .result
            .as_ref()
            .map_or_else(|e| format!("{e:?}"), |_| "ok".into());
        let txid = seen.txid.map(|id| id.to_string());
        let identity = serde_json::to_vec(&(
            &self.network,
            &self.tree,
            self.member,
            seen.height,
            &txid,
            &result,
        ))
        .map_err(|_| "Could not identify the observation.")?;
        let id = hex(&tagged_hash(
            "LayerTreeFollower/observation/v1",
            &[&identity],
        ));
        if self.known.contains(&id) {
            return Ok(());
        }
        let row = json!({"schema":"layertree.follower.observation.v1","kind":"light_client","id":id,"network":self.network,"tree_id":self.tree,"member":self.member,"height":seen.height,"txid":txid,"result":result,"observed_at":now()?,"observed_tip":observed_tip});
        self.append(&row)?;
        self.known.insert(id.clone());
        if seen.result.is_err() {
            self.pending.insert(id, row.clone());
        }
        println!("{row}");
        Ok(())
    }
    pub fn event(&mut self, kind: &str, detail: &str) -> Result<(), String> {
        self.append(&json!({"schema":"layertree.follower.observation.v1","kind":kind,"detail":detail,"observed_at":now()?}))
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    pub fn has_webhook(&self) -> bool {
        self.webhook.is_some()
    }
    pub fn retry_notifications(&mut self) -> Result<(), String> {
        let Some(url) = self.webhook.clone() else {
            return Ok(());
        };
        let mut ids: Vec<_> = self.pending.keys().cloned().collect();
        if let Some(cursor) = &self.notification_cursor {
            let offset = ids.partition_point(|id| id <= cursor);
            if offset < ids.len() {
                ids.rotate_left(offset);
            }
        }
        let rows: Vec<_> = ids
            .into_iter()
            .take(4)
            .filter_map(|id| self.pending.get(&id).cloned())
            .collect();
        for row in rows {
            let id = row["id"].as_str().ok_or("Alarm has no ID.")?;
            let delivered = self
                .client
                .post(url.clone())
                .header("Idempotency-Key", id)
                .json(&row)
                .timeout(std::time::Duration::from_secs(2))
                .send()
                .is_ok_and(|r| r.status().is_success());
            self.notification_cursor = Some(id.to_owned());
            if delivered {
                self.append(&json!({"schema":"layertree.follower.observation.v1","kind":"webhook_ack","id":id,"observed_at":now()?}))?;
                self.pending.remove(id);
            }
        }
        Ok(())
    }
    pub fn status(&mut self, value: Value) -> Result<(), String> {
        self.append(&json!({"schema":"layertree.follower.observation.v1","kind":"checked","status":value,"observed_at":now()?}))?;
        let path = self.dir.join("status.new");
        let mut file = File::create(&path).map_err(|_| "Could not write follower status.")?;
        serde_json::to_writer(&mut file, &value)
            .map_err(|_| "Could not encode follower status.")?;
        file.sync_all()
            .map_err(|_| "Could not save follower status.")?;
        std::fs::rename(path, self.dir.join("status.json"))
            .map_err(|_| "Could not update follower status.".into())
    }
}
