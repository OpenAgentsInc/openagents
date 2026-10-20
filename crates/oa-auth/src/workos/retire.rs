//! Retire legacy human sessions through the sealed session store, with a rollback file.

use super::session::SignInError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use tenancy::sessions::{Session, SessionKind, Sessions};

#[derive(Serialize, Deserialize)]
struct Snapshot {
    schema: String,
    sessions: Vec<Session>,
    #[serde(default)]
    preserved_apps: Vec<Session>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub applied: bool,
    pub retired: usize,
    pub restored: usize,
    pub preserved: usize,
    pub app_sessions: usize,
    pub retire_apps_after_release: bool,
    pub snapshot: Option<String>,
}

fn retained(session: &Session) -> bool {
    session.kind == SessionKind::Anonymous || session.key.is_some() || session.app.is_some()
}

/// Snapshot removed browser rows and retained app rows before the store changes.
/// Keys, apps, anonymous sessions, proofs, recovery tokens, and device grants remain in the store.
pub fn retire(dir: &Path, apply: bool) -> Result<Report, SignInError> {
    let sessions = Sessions::open(dir).map_err(|_| SignInError::Unavailable)?;
    let run = |book: &mut tenancy::sessions::SessionBook| {
        let removed: Vec<_> = book
            .sessions
            .values()
            .filter(|s| !retained(s))
            .cloned()
            .collect();
        let mut report = Report {
            applied: apply,
            retired: removed.len(),
            restored: 0,
            preserved: book.sessions.len() - removed.len(),
            app_sessions: book
                .sessions
                .values()
                .filter(|s| s.app.is_some() && s.key.is_none())
                .count(),
            retire_apps_after_release: true,
            snapshot: None,
        };
        if apply && (!removed.is_empty() || report.app_sessions > 0) {
            let name = format!(
                "workos-retired-sessions-{}-{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                std::process::id()
            );
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(dir.join(&name))
                .map_err(|_| tenancy::sessions::Refusal::Unavailable)?;
            let bytes = serde_json::to_vec(&Snapshot {
                schema: "openagents.workos-retired-sessions.v1".into(),
                sessions: removed.clone(),
                preserved_apps: book
                    .sessions
                    .values()
                    .filter(|s| s.app.is_some() && s.key.is_none())
                    .cloned()
                    .collect(),
            })
            .map_err(|_| tenancy::sessions::Refusal::Unavailable)?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| tenancy::sessions::Refusal::Unavailable)?;
            std::fs::File::open(dir)
                .and_then(|d| d.sync_all())
                .map_err(|_| tenancy::sessions::Refusal::Unavailable)?;
            for row in removed {
                book.sessions.remove(&row.id);
            }
            report.snapshot = Some(name);
        }
        Ok(report)
    };
    if apply {
        sessions
            .mutate(|book, _, _| run(book))
            .map_err(|_| SignInError::Unavailable)
    } else {
        let mut store = sessions.store().map_err(|_| SignInError::Unavailable)?;
        run(&mut store.book).map_err(|_| SignInError::Unavailable)
    }
}

/// Restore a retirement snapshot through the same store transaction. Existing rows are never replaced.
pub fn restore(dir: &Path, name: &str, apply: bool) -> Result<Report, SignInError> {
    if !name.starts_with("workos-retired-sessions-")
        || !name.ends_with(".json")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(SignInError::Denied);
    }
    let snapshot: Snapshot = serde_json::from_slice(
        &std::fs::read(dir.join(name)).map_err(|_| SignInError::Unavailable)?,
    )
    .map_err(|_| SignInError::Unavailable)?;
    if snapshot.schema != "openagents.workos-retired-sessions.v1" {
        return Err(SignInError::Denied);
    }
    let sessions = Sessions::open(dir).map_err(|_| SignInError::Unavailable)?;
    let run = |book: &mut tenancy::sessions::SessionBook| {
        if snapshot
            .sessions
            .iter()
            .any(|s| book.sessions.get(&s.id).is_some_and(|other| other != s))
        {
            return Err(tenancy::sessions::Refusal::Unavailable);
        }
        let restored = snapshot
            .sessions
            .iter()
            .filter(|s| !book.sessions.contains_key(&s.id))
            .count();
        let report = Report {
            applied: apply,
            retired: 0,
            restored,
            preserved: book.sessions.len(),
            app_sessions: book
                .sessions
                .values()
                .filter(|s| s.app.is_some() && s.key.is_none())
                .count(),
            retire_apps_after_release: true,
            snapshot: Some(name.into()),
        };
        if apply {
            for session in &snapshot.sessions {
                book.sessions.insert(session.id.clone(), session.clone());
            }
        }
        Ok(report)
    };
    if apply {
        sessions
            .mutate(|book, _, _| run(book))
            .map_err(|_| SignInError::Unavailable)
    } else {
        let mut store = sessions.store().map_err(|_| SignInError::Unavailable)?;
        run(&mut store.book).map_err(|_| SignInError::Unavailable)
    }
}
