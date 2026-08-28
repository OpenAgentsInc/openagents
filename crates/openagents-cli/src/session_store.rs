//! Local-first Coder session storage.
//!
//! The canonical record is an append-only `updates.jsonl` under
//! `~/.openagents/sessions/<encoded-cwd>/<session-id>/`. `summary.json` is an
//! atomic, rebuildable catalog row. Nothing in this module talks to a server.

use crate::runtime::ThreadRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FORMAT_VERSION: u32 = 1;
const SUMMARY_FILE: &str = "summary.json";
const UPDATES_FILE: &str = "updates.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub format_version: u32,
    pub id: String,
    pub cwd: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub lane: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_model: Option<String>,
    #[serde(default)]
    pub cloud_history: bool,
    /// The model's own end-of-work note (#189): what landed, what is broken,
    /// what is next. Written whenever the session records a checkpoint and
    /// shown to whoever resumes the session, so a session that died mid-turn
    /// -- to the budget, to a crash -- leaves its state in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checkpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct EventEnvelope {
    format_version: u32,
    sequence: u64,
    at_ms: u64,
    event_type: String,
    payload: serde_json::Value,
}

#[derive(Debug)]
pub struct LoadedSession {
    pub store: LocalSessionStore,
    pub summary: SessionSummary,
    pub events: Vec<StoredEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredEvent {
    pub sequence: u64,
    pub at_ms: u64,
    pub record: ThreadRecord,
}

#[derive(Debug)]
pub struct LocalSessionStore {
    directory: PathBuf,
    summary: SessionSummary,
    next_sequence: u64,
}

impl LocalSessionStore {
    pub fn create(
        root: &Path,
        cwd: &Path,
        lane: &str,
        reasoning: Option<String>,
        cloud_history: bool,
    ) -> std::io::Result<LoadedSession> {
        let now = now_ms();
        let id = new_session_id(cwd, now);
        let directory = cwd_directory(root, cwd).join(&id);
        create_owner_only_dir(&directory)?;
        let summary = SessionSummary {
            format_version: FORMAT_VERSION,
            id,
            cwd: cwd.to_string_lossy().into_owned(),
            created_at_ms: now,
            updated_at_ms: now,
            lane: lane.to_string(),
            reasoning,
            last_model: None,
            cloud_history,
            last_checkpoint: None,
        };
        write_summary(&directory, &summary)?;
        let store = Self {
            directory,
            summary: summary.clone(),
            next_sequence: 1,
        };
        Ok(LoadedSession {
            store,
            summary,
            events: Vec::new(),
        })
    }

    pub fn load_last(root: &Path, cwd: &Path) -> std::io::Result<Option<LoadedSession>> {
        let directory = cwd_directory(root, cwd);
        let mut candidates = summaries_in(&directory)?;
        candidates.sort_by_key(|(_, summary)| std::cmp::Reverse(summary.updated_at_ms));
        match candidates.into_iter().next() {
            Some((directory, _)) => Self::load_path(&directory).map(Some),
            None => Ok(None),
        }
    }

    pub fn load_id(root: &Path, cwd: &Path, id: &str) -> std::io::Result<Option<LoadedSession>> {
        if !valid_session_id(id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "session ids may contain only letters, numbers, `-`, and `_`",
            ));
        }
        let direct = cwd_directory(root, cwd).join(id);
        if direct.join(SUMMARY_FILE).is_file() {
            return Self::load_path(&direct).map(Some);
        }
        let sessions = root.join("sessions");
        let Ok(workspaces) = std::fs::read_dir(sessions) else {
            return Ok(None);
        };
        for workspace in workspaces.flatten() {
            let candidate = workspace.path().join(id);
            if candidate.join(SUMMARY_FILE).is_file() {
                return Self::load_path(&candidate).map(Some);
            }
        }
        Ok(None)
    }

    pub fn load_path(directory: &Path) -> std::io::Result<LoadedSession> {
        let directory = directory.to_path_buf();
        let summary: SessionSummary =
            serde_json::from_slice(&std::fs::read(directory.join(SUMMARY_FILE))?)
                .map_err(invalid_data)?;
        if summary.format_version != FORMAT_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "session {} uses unsupported format version {}",
                    summary.id, summary.format_version
                ),
            ));
        }
        let envelopes = read_events(&directory.join(UPDATES_FILE))?;
        let next_sequence = envelopes
            .last()
            .map(|event| event.sequence.saturating_add(1))
            .unwrap_or(1);
        let events = envelopes
            .into_iter()
            .map(|event| StoredEvent {
                sequence: event.sequence,
                at_ms: event.at_ms,
                record: ThreadRecord {
                    event_type: event.event_type,
                    payload: event.payload,
                },
            })
            .collect();
        let store = Self {
            directory,
            summary: summary.clone(),
            next_sequence,
        };
        Ok(LoadedSession {
            store,
            summary,
            events,
        })
    }

    pub fn append(&mut self, events: &[ThreadRecord]) -> std::io::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        create_owner_only_dir(&self.directory)?;
        let path = self.directory.join(UPDATES_FILE);
        let mut file = open_private_append(&path)?;
        let now = now_ms();
        for event in events {
            let envelope = EventEnvelope {
                format_version: FORMAT_VERSION,
                sequence: self.next_sequence,
                at_ms: now,
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
            };
            serde_json::to_writer(&mut file, &envelope).map_err(invalid_data)?;
            file.write_all(b"\n")?;
            self.next_sequence = self.next_sequence.saturating_add(1);
        }
        file.sync_data()?;
        self.summary.updated_at_ms = now;
        write_summary(&self.directory, &self.summary)
    }

    pub fn set_last_model(&mut self, model: Option<&str>) -> std::io::Result<()> {
        self.summary.last_model = model.map(str::to_string);
        self.summary.updated_at_ms = now_ms();
        write_summary(&self.directory, &self.summary)
    }

    pub fn set_lane(&mut self, lane: &str) -> std::io::Result<()> {
        self.summary.lane = lane.to_string();
        self.summary.updated_at_ms = now_ms();
        write_summary(&self.directory, &self.summary)
    }

    pub fn set_reasoning(&mut self, reasoning: Option<&str>) -> std::io::Result<()> {
        self.summary.reasoning = reasoning.map(str::to_string);
        self.summary.updated_at_ms = now_ms();
        write_summary(&self.directory, &self.summary)
    }

    pub fn set_last_checkpoint(&mut self, note: &str) -> std::io::Result<()> {
        self.summary.last_checkpoint = Some(note.to_string());
        self.summary.updated_at_ms = now_ms();
        write_summary(&self.directory, &self.summary)
    }

    pub fn set_cloud_history(&mut self, enabled: bool) -> std::io::Result<()> {
        self.summary.cloud_history = enabled;
        self.summary.updated_at_ms = now_ms();
        write_summary(&self.directory, &self.summary)
    }

    pub fn summary(&self) -> &SessionSummary {
        &self.summary
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

pub fn default_root() -> PathBuf {
    crate::auth::home_directory().join(".openagents")
}

pub fn replay_messages(events: &[StoredEvent]) -> Vec<crate::runtime::ChatMessage> {
    let wire = events
        .iter()
        .map(|event| crate::resume::ThreadEvent {
            id: i64::try_from(event.sequence).unwrap_or(i64::MAX),
            event_type: event.record.event_type.clone(),
            payload: event.record.payload.clone(),
        })
        .collect::<Vec<_>>();
    crate::resume::replay_wire(&wire)
}

fn cwd_directory(root: &Path, cwd: &Path) -> PathBuf {
    let encoded =
        url::form_urlencoded::byte_serialize(cwd.to_string_lossy().as_bytes()).collect::<String>();
    root.join("sessions").join(encoded)
}

/// The session directory of one working directory, for callers outside this
/// module that need to enumerate it (#289).
pub fn cwd_session_directory(root: &Path, cwd: &Path) -> PathBuf {
    cwd_directory(root, cwd)
}

/// Every parsed summary under one working directory's session directory,
/// in no particular order. Unreadable entries are skipped, as in `load_last`
/// — a malformed neighbor must not take the enumeration down.
pub fn summaries_for(root: &Path, cwd: &Path) -> Vec<(PathBuf, SessionSummary)> {
    summaries_in(&cwd_directory(root, cwd)).unwrap_or_default()
}

/// Whether `id` is a session id this store would load. Shared with the
/// cross-session recall target check so both refuse the same shapes.
pub fn id_is_valid(id: &str) -> bool {
    valid_session_id(id)
}

/// Decode an encoded cwd directory name back to the path it names.
///
/// `form_urlencoded` percent-encodes with `+` for spaces, so the reverse is a
/// plus-for-space swap and a percent decode. A name that does not decode to a
/// plausible absolute path decodes to `None` — the caller treats the
/// directory as unknown rather than guessing.
pub fn decode_cwd_directory(encoded: &Path) -> Option<PathBuf> {
    let name = encoded.to_str()?;
    let bytes = name.replace('+', " ");
    let decoded = percent_decode(&bytes)?;
    let path = PathBuf::from(&decoded);
    path.is_absolute().then_some(path)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = bytes.get(index + 1..index + 3)?;
                let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                out.push(value);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// The session id a store directory holds, from its `summary.json`.
///
/// The cross-session recall arguments need the running session's own id to
/// keep the listing from offering it as a target; this is that, read from the
/// one file every store directory is required to carry.
pub fn session_id_for_directory(directory: &Path) -> Option<String> {
    let bytes = std::fs::read(directory.join(SUMMARY_FILE)).ok()?;
    let summary: SessionSummary = serde_json::from_slice(&bytes).ok()?;
    (summary.format_version == FORMAT_VERSION).then_some(summary.id)
}

fn summaries_in(directory: &Path) -> std::io::Result<Vec<(PathBuf, SessionSummary)>> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Ok(Vec::new());
    };
    let mut summaries = Vec::new();
    for entry in entries.flatten() {
        let session_dir = entry.path();
        let Ok(bytes) = std::fs::read(session_dir.join(SUMMARY_FILE)) else {
            continue;
        };
        let Ok(summary) = serde_json::from_slice::<SessionSummary>(&bytes) else {
            continue;
        };
        if summary.format_version == FORMAT_VERSION {
            summaries.push((session_dir, summary));
        }
    }
    Ok(summaries)
}

fn read_events(path: &Path) -> std::io::Result<Vec<EventEnvelope>> {
    let Ok(file) = File::open(path) else {
        return Ok(Vec::new());
    };
    let mut reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        let terminated = line.last() == Some(&b'\n');
        if terminated {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
        }
        if line.is_empty() {
            continue;
        }
        match serde_json::from_slice::<EventEnvelope>(&line) {
            Ok(event) if event.format_version == FORMAT_VERSION => {
                let expected = events
                    .last()
                    .map(|previous: &EventEnvelope| previous.sequence.saturating_add(1))
                    .unwrap_or(1);
                if event.sequence != expected {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "event sequence {} followed {}, expected {expected}",
                            event.sequence,
                            expected.saturating_sub(1)
                        ),
                    ));
                }
                events.push(event);
            }
            Ok(event) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unsupported event format version {}", event.format_version),
                ));
            }
            Err(_) if !terminated => break,
            Err(error) => return Err(invalid_data(error)),
        }
    }
    Ok(events)
}

fn write_summary(directory: &Path, summary: &SessionSummary) -> std::io::Result<()> {
    create_owner_only_dir(directory)?;
    let path = directory.join(SUMMARY_FILE);
    let temp = directory.join(format!(
        ".{SUMMARY_FILE}.{}.{}.tmp",
        std::process::id(),
        summary.updated_at_ms
    ));
    let bytes = serde_json::to_vec_pretty(summary).map_err(invalid_data)?;
    let mut file = open_private_replace(&temp)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    std::fs::rename(&temp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })?;
    Ok(())
}

fn new_session_id(cwd: &Path, now: u64) -> String {
    let mut random = [0u8; 16];
    let rng = ring::rand::SystemRandom::new();
    let _ = ring::rand::SecureRandom::fill(&rng, &mut random);
    let mut digest = Sha256::new();
    digest.update(cwd.to_string_lossy().as_bytes());
    digest.update(now.to_le_bytes());
    digest.update(random);
    let value = digest.finalize();
    format!("{now:x}-{}", hex(&value[..8]))
}

fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn invalid_data(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(unix)]
fn create_owner_only_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_owner_only_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(unix)]
fn open_private_append(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_append(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

#[cfg(unix)]
fn open_private_replace(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_replace(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(text: &str) -> ThreadRecord {
        ThreadRecord::user(text)
    }

    #[test]
    fn session_round_trips_and_last_finds_it() {
        let root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/work/repo with spaces");
        let mut loaded =
            LocalSessionStore::create(root.path(), cwd, "flash", Some("high".into()), false)
                .unwrap();
        loaded.store.append(&[user("keep this local")]).unwrap();
        loaded.store.set_last_model(Some("model/one")).unwrap();

        let resumed = LocalSessionStore::load_last(root.path(), cwd)
            .unwrap()
            .unwrap();
        assert_eq!(resumed.summary.id, loaded.summary.id);
        assert_eq!(resumed.summary.last_model.as_deref(), Some("model/one"));
        assert_eq!(resumed.events.len(), 1);
        assert_eq!(resumed.events[0].record, user("keep this local"));
        assert!(resumed.store.directory().starts_with(root.path()));
    }

    #[test]
    fn image_input_rehydrates_without_the_original_file() {
        let root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/work/repo");
        let mut loaded = LocalSessionStore::create(root.path(), cwd, "flash", None, false).unwrap();
        let image = crate::runtime::ImageAttachment {
            id: 1,
            filename: "screen.png".to_string(),
            mime_type: "image/png".to_string(),
            data_url: "data:image/png;base64,iVBORw0KGgo=".to_string(),
        };
        loaded
            .store
            .append(&[ThreadRecord::user_with_images(
                "[Image #1] describe this",
                std::slice::from_ref(&image),
            )])
            .unwrap();

        let resumed = LocalSessionStore::load_last(root.path(), cwd)
            .unwrap()
            .unwrap();
        let messages = replay_messages(&resumed.events);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].images, vec![image.data_url]);
    }

    #[test]
    fn explicit_id_can_move_between_working_directories() {
        let root = tempfile::tempdir().unwrap();
        let created =
            LocalSessionStore::create(root.path(), Path::new("/old/path"), "flash", None, false)
                .unwrap();
        let loaded =
            LocalSessionStore::load_id(root.path(), Path::new("/new/path"), &created.summary.id)
                .unwrap()
                .unwrap();
        assert_eq!(loaded.summary.cwd, "/old/path");
    }

    #[test]
    fn explicit_id_cannot_escape_the_session_root() {
        let root = tempfile::tempdir().unwrap();
        let error = LocalSessionStore::load_id(root.path(), Path::new("/repo"), "../../outside")
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn a_torn_final_line_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let loaded =
            LocalSessionStore::create(root.path(), Path::new("/repo"), "flash", None, false)
                .unwrap();
        std::fs::write(
            loaded.store.directory().join(UPDATES_FILE),
            b"{\"format_version\":1",
        )
        .unwrap();
        let resumed =
            LocalSessionStore::load_id(root.path(), Path::new("/repo"), &loaded.summary.id)
                .unwrap()
                .unwrap();
        assert!(resumed.events.is_empty());
    }

    #[test]
    fn corruption_before_eof_is_not_silently_skipped() {
        let root = tempfile::tempdir().unwrap();
        let loaded =
            LocalSessionStore::create(root.path(), Path::new("/repo"), "flash", None, false)
                .unwrap();
        std::fs::write(loaded.store.directory().join(UPDATES_FILE), b"not-json\n").unwrap();
        let error = LocalSessionStore::load_id(root.path(), Path::new("/repo"), &loaded.summary.id)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn session_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let mut loaded =
            LocalSessionStore::create(root.path(), Path::new("/repo"), "flash", None, false)
                .unwrap();
        loaded.store.append(&[user("private")]).unwrap();
        let dir_mode = std::fs::metadata(loaded.store.directory())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let file_mode = std::fs::metadata(loaded.store.directory().join(UPDATES_FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    /// A summary written before `last_checkpoint` existed still loads, and a
    /// checkpoint written by one session is on the summary the next reads
    /// (#189). The old sessions must not die for the new field.
    #[test]
    fn a_summary_without_a_checkpoint_still_loads_and_a_new_one_round_trips() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("repo");
        std::fs::create_dir_all(&cwd).unwrap();

        let loaded = LocalSessionStore::create(root.path(), &cwd, "flash", None, false).unwrap();
        assert!(loaded.summary.last_checkpoint.is_none());
        let directory = loaded.store.directory().to_path_buf();
        let mut store = loaded.store;

        store
            .set_last_checkpoint("#152: tiers landed; cmd-log tests red; next: failing runs")
            .unwrap();

        let reloaded = LocalSessionStore::load_path(&directory).unwrap();
        assert_eq!(
            reloaded.summary.last_checkpoint.as_deref(),
            Some("#152: tiers landed; cmd-log tests red; next: failing runs")
        );
    }
}
