//! Current native identity custody. Memory text never participates in admission.
use super::*;
use crate::task::{agent, agent_key};
use std::fs::{File, OpenOptions};
use std::io::Read;

pub(in crate::task::sales) struct Native {
    pub anchor: Anchor,
    paths: Vec<(PathBuf, File, Option<String>)>,
    store: agent::Store,
    record: agent::Record,
    crew: crate::task::agent_crew_control::Guard,
}
pub(in crate::task::sales) fn directory(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY);
    }
    #[cfg(windows)]
    let file = private_fs::open_dir(path).map_err(|_| "native agent directory is unavailable")?;
    #[cfg(not(windows))]
    let file = options
        .open(path)
        .map_err(|_| "native agent directory is unavailable")?;
    same_directory(path, &file)?;
    Ok(file)
}
pub(in crate::task::sales) fn same_directory(path: &Path, file: &File) -> Result<()> {
    let visible = std::fs::symlink_metadata(path).map_err(|_| "native agent directory changed")?;
    let held = file
        .metadata()
        .map_err(|_| "native agent directory changed")?;
    if !visible.is_dir() || !held.is_dir() || visible.file_type().is_symlink() {
        return Err("native agent directory changed".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if visible.dev() != held.dev()
            || visible.ino() != held.ino()
            || held.mode() & 0o077 != 0
            || held.uid() != unsafe { libc::geteuid() }
        {
            return Err("native agent directory custody changed".into());
        }
    }
    #[cfg(windows)]
    if private_fs::identity(file).map_err(|e| e.to_string())?
        != private_fs::identity_of(path).map_err(|e| e.to_string())?.0
        || !private_fs::is_private(file).map_err(|e| e.to_string())?
    {
        return Err("native agent directory custody changed".into());
    }
    Ok(())
}
fn read(path: &Path, max: usize) -> Result<(File, Vec<u8>)> {
    let mut file = crate::task::private_open(path, false, false)
        .map_err(|_| "native agent source is not private")?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native agent source is unavailable")?;
    if bytes.len() > max {
        return Err("native agent source exceeds its bound".into());
    }
    crate::task::verify_same_file(path, &file).map_err(|_| "native agent source changed")?;
    Ok((file, bytes))
}
impl Native {
    pub fn read(
        root: &Path,
        name: &str,
        now: u64,
        keys: std::sync::Arc<dyn agent_key::KeyStore>,
    ) -> Result<Self> {
        let store = agent::Store::with_keys(root, name, keys.clone())?;
        let mut paths = Vec::new();
        for dir in [
            root.to_path_buf(),
            root.join("sales"),
            root.join("agents"),
            store.dir().to_path_buf(),
        ] {
            paths.push((dir.clone(), directory(&dir)?, None));
        }
        let path = store.dir().join("agent.json");
        let (file, bytes) = read(&path, 64 * 1024)?;
        let record: agent::Record =
            serde_json::from_slice(&bytes).map_err(|_| "native agent record is malformed")?;
        record.validate_crew()?;
        if record.name != name
            || record.schema != agent::RECORD_SCHEMA
            || record.v != 1
            || record.state != agent::State::Active
            || record.requires != ["crew-sales.v1"]
        {
            return Err("native sales agent is inactive or unsupported".into());
        }
        let charter = record
            .crew_charter
            .as_ref()
            .ok_or("native sales charter is missing")?;
        if !charter.drafting {
            return Err("native sales drafting is disabled".into());
        }
        let pubkey = record
            .pubkey
            .as_ref()
            .ok_or("native agent has no retained key")?;
        let attestation = record
            .attestation
            .as_ref()
            .ok_or("native agent owner attestation is missing")?;
        let expires_at = agent::verify_attestation(pubkey, attestation, now)?;
        if keys.custody() == "file" {
            let key_path = store.dir().join("key");
            let (key_file, key_bytes) = read(&key_path, 128)?;
            let key = agent::parse_secret(
                std::str::from_utf8(&key_bytes).map_err(|_| "native agent key is malformed")?,
            )?;
            if agent::public_hex(&key) != *pubkey {
                return Err("native agent key custody changed".into());
            }
            paths.push((key_path, key_file, Some(digest(&key_bytes))));
        }
        store.custody(&record)?;
        let crew = crate::task::agent_crew_control::Guard::open(root)?;
        let stamp = crew
            .book
            .stamp(&record)?
            .ok_or("native sales crew admission is missing")?;
        let anchor = Anchor {
            name: name.into(),
            pubkey: pubkey.clone(),
            owner: attestation.owner.clone(),
            role: record.job_role.ok_or("native sales job role is missing")?,
            charter_revision: charter.revision,
            crew_epoch: stamp.epoch,
            charter_sha256: digest(&serde_json::to_vec(charter).map_err(|e| e.to_string())?),
            attestation_sha256: digest(
                &serde_json::to_vec(attestation).map_err(|e| e.to_string())?,
            ),
            expires_at,
        };
        paths.push((path, file, Some(digest(&bytes))));
        let native = Self {
            anchor,
            paths,
            store,
            record,
            crew,
        };
        native.recheck()?;
        Ok(native)
    }
    pub fn recheck(&self) -> Result<()> {
        self.crew.check()?;
        self.crew.book.check_stamp(
            &self.record,
            &Some(crate::task::agent_crew_control::Stamp {
                member: self.anchor.name.clone(),
                pubkey: self.anchor.pubkey.clone(),
                epoch: self.anchor.crew_epoch,
            }),
        )?;
        self.check_paths()?;
        self.store.custody(&self.record)?;
        self.check_paths()
    }
    fn check_paths(&self) -> Result<()> {
        for (path, file, expected) in &self.paths {
            if let Some(expected) = expected {
                crate::task::verify_same_file(path, file)
                    .map_err(|_| "native agent source custody changed")?;
                let (_, bytes) = read(path, 64 * 1024)?;
                if digest(&bytes) != *expected {
                    return Err("native agent source changed during admission".into());
                }
            } else {
                same_directory(path, file)?;
            }
        }
        Ok(())
    }
}
