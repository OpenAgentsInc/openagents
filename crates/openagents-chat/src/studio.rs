//! Admitted declared Studio routes over the existing coordinator operations.
//!
//! Persist the send before transport. An unknown send is never automatically
//! replayed; an explicit reconciliation uses the original request and bytes.
use coder_access::{Operation, Outcome, Right};
use route_contract::binding::{Current, WorkbenchBinding};
use route_contract::studio::{RESULT_SCHEMA, StudioRoute};
use route_contract::{AdmissionSnapshot, Digest};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultView {
    pub schema: String,
    pub request: String,
    pub route: Digest,
    pub state: State,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum State {
    /// The journal records a send whose acknowledgment is not yet retained.
    Unknown,
    Completed {
        outcome: Box<Outcome>,
    },
    Refused {
        reason: String,
    },
}

/// The caller's durable, private journal. `save` must synchronize before it
/// returns; its transaction must exclude concurrent sends for the request.
pub trait Journal {
    fn load(&self, request: &str) -> Result<Option<ResultView>, String>;
    fn save(&mut self, result: &ResultView) -> Result<(), String>;
}

/// An existing local or paired-host client, not a new coordinator. The client
/// checks the current host grant again when it transmits the operation.
pub trait Host {
    fn current(&mut self) -> Result<Current, String>;
    fn rights(&self) -> Vec<Right>;
    fn send(&mut self, request: &str, operation: &Operation) -> Result<Outcome, Failure>;
}

pub enum Failure {
    Refused(String),
    Unknown,
}

/// Convert only the declared Studio extension to an existing host operation.
pub fn operation(route: &StudioRoute) -> Result<Operation, String> {
    let op: Operation =
        serde_json::from_value(serde_json::to_value(&route.intent).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    op.validate().map_err(|e| e.to_string())?;
    Ok(op)
}

/// Dispatch once or explicitly reconcile the exact unknown request. Neither
/// shell approval nor semantic inference is accepted as Studio authority.
pub fn dispatch(
    route: &StudioRoute,
    snapshot: &AdmissionSnapshot,
    binding: &WorkbenchBinding,
    host: &mut impl Host,
    journal: &mut impl Journal,
    reconcile: bool,
) -> Result<ResultView, String> {
    route.check(snapshot, binding)?;
    binding
        .recheck(snapshot, &host.current()?)
        .map_err(|e| format!("{e:?}"))?;
    let op = operation(route)?;
    let right = op.required().ok_or("Studio operation names no right")?;
    if !host.rights().contains(&right) {
        return Err(format!("Studio operation requires {}", right.as_str()));
    }
    if let Some(previous) = journal.load(&route.request)? {
        if previous.schema != RESULT_SCHEMA
            || previous.request != route.request
            || previous.route != route.digest()
        {
            return Err("Studio request conflicts with its retained route".into());
        }
        if let State::Completed { outcome } = &previous.state {
            outcome.validate().map_err(|e| e.to_string())?;
            if !outcome.answers(&op) {
                return Err("retained response answers another operation".into());
            }
        }
        if !matches!(previous.state, State::Unknown) || !reconcile {
            return Ok(previous);
        }
    }
    let mut result = ResultView {
        schema: RESULT_SCHEMA.into(),
        request: route.request.clone(),
        route: route.digest(),
        state: State::Unknown,
    };
    journal.save(&result)?;
    result.state = match host.send(&route.request, &op) {
        Ok(outcome) => {
            outcome.validate().map_err(|e| e.to_string())?;
            if !outcome.answers(&op) {
                return Err("host response does not answer the Studio operation".into());
            }
            State::Completed {
                outcome: Box::new(outcome),
            }
        }
        Err(Failure::Refused(reason)) => State::Refused { reason },
        Err(Failure::Unknown) => State::Unknown,
    };
    journal.save(&result)?;
    Ok(result)
}

/// A private, exclusively locked journal. Dropping it releases the OS lock,
/// including after a process crash; unknown sends survive on disk.
pub struct FileJournal {
    directory: std::path::PathBuf,
    _lock: std::fs::File,
}

impl FileJournal {
    pub fn open(directory: impl Into<std::path::PathBuf>) -> Result<Self, String> {
        let directory = directory.into();
        if !directory.exists() {
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&directory).map_err(|e| e.to_string())?;
        }
        let meta = std::fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("Studio journal must be a private directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("Studio journal directory must have mode 0700".into());
            }
        }
        let lock_path = directory.join("lock");
        if std::fs::symlink_metadata(&lock_path)
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err("Studio journal lock must be a regular file".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if lock
                .metadata()
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("Studio journal lock must have mode 0600".into());
            }
        }
        lock.try_lock().map_err(|e| e.to_string())?;
        Ok(Self {
            directory,
            _lock: lock,
        })
    }

    fn path(&self, request: &str) -> Result<std::path::PathBuf, String> {
        if request.len() != 64
            || !request
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Studio request must be 64 lowercase hex digits".into());
        }
        Ok(self.directory.join(format!("{request}.json")))
    }
}

impl Journal for FileJournal {
    fn load(&self, request: &str) -> Result<Option<ResultView>, String> {
        use std::io::Read;
        let path = self.path(request)?;
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 128 * 1024 {
            return Err("Studio result is not a bounded regular file".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(128 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    fn save(&mut self, result: &ResultView) -> Result<(), String> {
        use std::io::Write;
        let path = self.path(&result.request)?;
        let bytes = serde_json::to_vec(result).map_err(|e| e.to_string())?;
        if bytes.len() > 128 * 1024 {
            return Err("Studio result exceeds its bound".into());
        }
        let pending = path.with_extension("pending");
        // The exclusive journal lock makes a previous pending write safe to
        // remove: no transport was sent until the final record was synced.
        match std::fs::remove_file(&pending) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&pending).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(pending, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        std::fs::File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
