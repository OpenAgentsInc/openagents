//! Bounded exact bytes and signed local state, retained before acknowledgment.
use crate::Result;
use nostr::contracts::{self, digest_bytes, jcs, parse_artifact};
use nostr::domain::{Event, RelaySigner};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const MAX_BYTES: usize = 1_048_576;
pub const MAX_STATE: usize = 16 * MAX_BYTES;
pub fn reference(bytes: &[u8], media: &str, schema: Option<&str>) -> Value {
    let mut value = json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":media});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
pub fn json_ref(value: &Value) -> Result<Value> {
    Ok(reference(
        &jcs(value).map_err(|e| e.to_string())?,
        "application/json",
        value["v"].as_str(),
    ))
}
pub fn token() -> String {
    secp256k1::rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("state parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.error.to_string())?;
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err("bounded regular file required".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("byte limit exceeded".into());
    }
    Ok(bytes)
}
#[derive(Clone)]
pub struct Artifacts {
    root: PathBuf,
}
impl Artifacts {
    pub fn new(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root).map_err(|e| e.to_string())?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&root)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("workspace state must be readable only by its owner".into());
            }
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn put(&self, bytes: &[u8], media: &str, schema: Option<&str>) -> Result<Value> {
        if bytes.len() > MAX_BYTES {
            return Err("artifact byte limit exceeded".into());
        }
        let reference = reference(bytes, media, schema);
        let path = self.path(&reference)?;
        if path.exists() {
            if read(&path, MAX_BYTES)? != bytes {
                return Err("retained artifact conflict".into());
            }
        } else {
            let directory = self.root.join("artifacts");
            let mut count = 0usize;
            let mut total = bytes.len() as u64;
            if directory.exists() {
                for entry in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
                    let metadata = entry
                        .map_err(|e| e.to_string())?
                        .metadata()
                        .map_err(|e| e.to_string())?;
                    if !metadata.is_file() {
                        return Err("unexpected retained artifact type".into());
                    }
                    count += 1;
                    total = total.saturating_add(metadata.len());
                    if count >= 8192 || total > 128 * MAX_BYTES as u64 {
                        return Err("workspace artifact retention limit exceeded; no evidence was discarded".into());
                    }
                }
            }
            atomic(&path, bytes)?;
        }
        Ok(reference)
    }
    pub fn put_json(&self, value: &Value) -> Result<Value> {
        self.put(
            &jcs(value).map_err(|e| e.to_string())?,
            "application/json",
            value["v"].as_str(),
        )
    }
    fn path(&self, reference: &Value) -> Result<PathBuf> {
        let reference = parse_artifact(reference).map_err(|e| e.to_string())?;
        Ok(self
            .root
            .join(format!("artifacts/{}", &reference.digest[7..])))
    }
    pub fn get(&self, reference: &Value) -> Result<Vec<u8>> {
        let bytes = read(&self.path(reference)?, MAX_BYTES)?;
        contracts::check_artifact_bytes(
            &parse_artifact(reference).map_err(|e| e.to_string())?,
            &bytes,
        )
        .map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    pub fn json(&self, reference: &Value) -> Result<Value> {
        let value = contracts::parse_strict(&self.get(reference)?).map_err(|e| e.to_string())?;
        if reference
            .get("schema")
            .is_some_and(|schema| value["v"] != *schema)
        {
            return Err("artifact schema differs from its retained body".into());
        }
        Ok(value)
    }
    pub fn lock(&self) -> Result<Lock> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("host.lock"))
            .map_err(|e| e.to_string())?;
        fs2::FileExt::try_lock_exclusive(&file)
            .map_err(|_| "another workspace operation is in progress")?;
        Ok(Lock(file))
    }
    pub fn save<T: Serialize>(&self, value: &T, signer: &RelaySigner) -> Result<()> {
        let body = serde_json::to_value(value).map_err(|e| e.to_string())?;
        let bytes = jcs(&body).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_STATE {
            return Err("workspace state retention limit exceeded".into());
        }
        // Local NIP-78 attribution; this record is never sent to a relay.
        let signature = signer.sign(
            0,
            30078,
            vec![nostr::domain::Tag::new(vec![
                "d".into(),
                "workspace-host-state".into(),
            ])],
            digest_bytes(&bytes),
        );
        atomic(
            &self.root.join("state.json"),
            &serde_json::to_vec(&json!({"body":body,"signature":signature}))
                .map_err(|e| e.to_string())?,
        )
    }
    pub fn load<T: for<'a> Deserialize<'a>>(&self, signer: &RelaySigner) -> Result<T> {
        let value = contracts::parse_strict_bounded(
            &read(&self.root.join("state.json"), MAX_STATE + MAX_BYTES)?,
            MAX_STATE + MAX_BYTES,
        )
        .map_err(|e| e.to_string())?;
        let signature: Event = serde_json::from_value(value["signature"].clone())
            .map_err(|_| "workspace signature is unreadable")?;
        signature.validate_crypto().map_err(|e| e.to_string())?;
        if signature.pubkey != signer.pubkey()
            || signature.kind != 30078
            || signature.content != digest_bytes(&jcs(&value["body"]).map_err(|e| e.to_string())?)
        {
            return Err("workspace state failed authentication; it was not reset".into());
        }
        serde_json::from_value(value["body"].clone())
            .map_err(|_| "workspace state is unreadable; it was not reset".into())
    }
}

/// Explicit unlock also releases a brief forked child's inherited lock.
pub struct Lock(std::fs::File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}
