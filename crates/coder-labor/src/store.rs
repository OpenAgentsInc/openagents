//! Private, locked, atomic retention of every authenticated attempt and refusal.
use crate::book::{Book, Setup};
use crate::*;
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub event: Event,
    pub received_at: u64,
    pub attachments: Blobs,
    pub outcome: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    setup: Setup,
    observations: Vec<Observation>,
    dispatch: Option<Dispatch>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    pub execute: Event,
    pub grant_digest: String,
    pub task_id: String,
    pub task_directory: PathBuf,
    pub observation: Option<coder::task::Task>,
    pub state: String,
}

pub struct Store {
    dir: PathBuf,
    lock: File,
    document: Document,
    poisoned: bool,
    pub book: Book,
}
impl Store {
    pub fn open(directory: &Path, setup: Setup, secret: SecretKey) -> Result<Self> {
        let fresh = match std::fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {
                let parent = directory.parent().ok_or("labor directory parent")?;
                File::open(parent)
                    .and_then(|file| file.sync_all())
                    .map_err(|e| e.to_string())?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error.to_string()),
        };
        let metadata = std::fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("labor store must be a private ordinary directory".into());
        }
        let dir = directory.canonicalize().map_err(|e| e.to_string())?;
        if !fresh && (!dir.join("labor.lock").exists() || !dir.join("labor.json").exists()) {
            return Err(
                "initialized or incomplete labor store is missing its journal or stable lock"
                    .into(),
            );
        }
        let lock = private_open(&dir.join("labor.lock"), fresh)?;
        lock.try_lock()
            .map_err(|_| "labor store is busy".to_owned())?;
        lock.sync_all()
            .and_then(|_| File::open(&dir)?.sync_all())
            .map_err(|e| e.to_string())?;
        let path = dir.join("labor.json");
        let document = if path.exists() {
            let mut file = private_open(&path, false)?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err("labor store exceeds retention bound".into());
            }
            let d: Document = serde_json::from_value(
                nostr::contracts::parse_strict_bounded(&bytes, 16 * 1024 * 1024)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if d.schema != "openagents.free-labor-store.v1"
                || serde_json::to_value(&d.setup).map_err(|e| e.to_string())?
                    != serde_json::to_value(&setup).map_err(|e| e.to_string())?
            {
                return Err("labor store has another frozen configuration".into());
            }
            d
        } else {
            Document {
                schema: "openagents.free-labor-store.v1".into(),
                setup: setup.clone(),
                observations: vec![],
                dispatch: None,
            }
        };
        let mut book = Book::new(setup, secret)?;
        for observation in &document.observations {
            let actual = outcome(book.receive(
                &observation.event,
                observation.received_at,
                &observation.attachments,
            ));
            if actual != observation.outcome {
                return Err("labor journal replay disagrees with its retained observation".into());
            }
        }
        let mut store = Self {
            dir,
            lock,
            document,
            poisoned: false,
            book,
        };
        if fresh {
            store.save()?;
        }
        Ok(store)
    }
    pub fn receive(
        &mut self,
        event: Event,
        received_at: u64,
        attachments: Blobs,
    ) -> Result<String> {
        if self.poisoned {
            return Err("labor store requires reopening after an uncertain write".into());
        }
        if self.document.observations.len() >= 256 {
            return Err("labor observation retention limit".into());
        }
        // Invalid outer signatures are rejected without retaining attacker bytes.
        nostr::private_artifact::admit(&event).map_err(|e| e.to_string())?;
        let result = outcome(self.book.receive(&event, received_at, &attachments));
        self.document.observations.push(Observation {
            event,
            received_at,
            attachments,
            outcome: result.clone(),
        });
        self.save()?;
        Ok(result)
    }
    /// Persist dispatch intent before the task owner can admit or start work.
    /// A lost result remains unknown; another relay delivery cannot rerun it.
    pub async fn dispatch(
        &mut self,
        event: Event,
        grant_bytes: &[u8],
        task_directory: &Path,
        now: u64,
    ) -> Result<Dispatch> {
        if self.poisoned {
            return Err("labor store requires reopening after an uncertain write".into());
        }
        let digest = nostr::contracts::digest_bytes(grant_bytes);
        if let Some(previous) = &self.document.dispatch {
            if previous.execute.id != event.id || previous.grant_digest != digest {
                return Err("labor dispatch identity conflict".into());
            }
            return self.reconcile();
        }
        let (command, _grant) = crate::execution::prepare(&self.book, &event, grant_bytes, now)?;
        // Creating the private inbox grants no execution authority.
        {
            let _ = coder::task::Store::open(task_directory).map_err(|e| e.to_string())?;
        }
        let task_directory = task_directory.canonicalize().map_err(|e| e.to_string())?;
        self.document.dispatch = Some(Dispatch {
            execute: event,
            grant_digest: digest,
            task_id: command.task_id.clone(),
            task_directory: task_directory.clone(),
            observation: None,
            state: "unknown".into(),
        });
        self.save()?;
        {
            let mut tasks = coder::task::Store::open(&task_directory).map_err(|e| e.to_string())?;
            tasks
                .apply(&serde_json::to_vec(&command).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        match coder::task::owner::execute(&task_directory, grant_bytes).await {
            Ok(task) => {
                let dispatch = self
                    .document
                    .dispatch
                    .as_mut()
                    .ok_or("labor dispatch vanished")?;
                dispatch.state = format!("{:?}", task.execution).to_ascii_lowercase();
                dispatch.observation = Some(task);
                self.save()?;
                Ok(self
                    .document
                    .dispatch
                    .clone()
                    .ok_or("labor dispatch vanished")?)
            }
            Err(error) => {
                self.save()?;
                Err(format!(
                    "labor execution did not establish a result: {error}"
                ))
            }
        }
    }
    /// Recover the existing task identity; never create a replacement attempt.
    pub fn reconcile(&mut self) -> Result<Dispatch> {
        if self.poisoned {
            return Err("labor store requires reopening after an uncertain write".into());
        }
        let input = self
            .book
            .blobs
            .resolve(&self.book.labor().execution.input)?;
        let intent: coder::task::TaskIntent =
            serde_json::from_value(input["intent"].clone()).map_err(|e| e.to_string())?;
        let expected_intent = nostr::contracts::digest_bytes(
            &serde_json::to_vec(&intent).map_err(|e| e.to_string())?,
        );
        let dispatch = self
            .document
            .dispatch
            .as_mut()
            .ok_or("no labor dispatch exists")?;
        match coder::task::owner::recover(&dispatch.task_directory, &dispatch.task_id) {
            Ok(task) => {
                if task.task_id != dispatch.task_id || task.intent_digest != expected_intent {
                    return Err("labor task identity differs from the frozen order".into());
                }
                dispatch.state = match task.execution {
                    coder::task::Execution::NotStarted => "unknown".into(),
                    other => format!("{other:?}").to_ascii_lowercase(),
                };
                dispatch.observation = Some(task);
            }
            Err(coder::task::Error::NotFound) => {
                dispatch.state = "unknown".into();
            }
            Err(error) => return Err(format!("labor reconciliation unavailable: {error}")),
        }
        self.save()?;
        self.document
            .dispatch
            .clone()
            .ok_or("labor dispatch vanished".into())
    }
    pub fn observations(&self) -> &[Observation] {
        &self.document.observations
    }
    fn save(&mut self) -> Result<()> {
        let result = self.save_atomic();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn save_atomic(&self) -> Result<()> {
        let path_metadata =
            std::fs::symlink_metadata(self.dir.join("labor.lock")).map_err(|e| e.to_string())?;
        let held_metadata = self.lock.metadata().map_err(|e| e.to_string())?;
        if !path_metadata.is_file()
            || path_metadata.nlink() != 1
            || held_metadata.nlink() != 1
            || path_metadata.dev() != held_metadata.dev()
            || path_metadata.ino() != held_metadata.ino()
        {
            return Err("labor store lock identity changed".into());
        }

        let bytes = serde_json::to_vec(&self.document).map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("labor store byte limit".into());
        }
        let temp = self.dir.join("labor.pending");
        // An unfinished temporary write is never trusted as a replacement
        // for the last atomically committed journal.
        if temp.exists() {
            std::fs::remove_file(&temp).map_err(|e| e.to_string())?;
        }
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temp, self.dir.join("labor.json")).map_err(|e| e.to_string())?;
        File::open(&self.dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
fn private_open(path: &Path, create: bool) -> Result<File> {
    if let Ok(m) = std::fs::symlink_metadata(path)
        && (!m.is_file() || m.nlink() != 1 || m.permissions().mode() & 0o077 != 0)
    {
        return Err("unsafe labor store file".into());
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.permissions().mode() & 0o077 != 0 {
        return Err("unsafe opened labor store file".into());
    }
    Ok(file)
}
fn outcome(result: Result<&'static str>) -> String {
    match result {
        Ok(s) => s.into(),
        Err(e) => format!("refused: {e}"),
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}
