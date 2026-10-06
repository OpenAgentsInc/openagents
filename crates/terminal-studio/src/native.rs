//! Typed operations over the existing same-user studio control socket.
use coder_access::{Operation, Outcome};
use openagents_connect::control::OperationClient;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

fn scoped_socket(socket: PathBuf, home: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(home) = home {
        let root = home
            .canonicalize()
            .map_err(|_| "Scratch home is unavailable.")?;
        let target = socket
            .canonicalize()
            .map_err(|_| "Scratch studio socket is unavailable; no owner host contacted.")?;
        if !target.starts_with(root) {
            return Err(
                "Studio socket is outside the scratch home; no owner host contacted.".into(),
            );
        }
    }
    Ok(socket)
}

fn studio_socket(home: Option<&Path>) -> Result<PathBuf, String> {
    let socket: Result<PathBuf, String> = match std::env::consts::OS {
        "macos" => home
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .map(|home| home.join("Library/Application Support/OpenAgents/control.sock"))
            .ok_or_else(|| "No home for the studio control socket.".into()),
        "linux" => std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .map(|runtime| runtime.join("openagents/control.sock"))
            .ok_or_else(|| "No runtime directory for the studio control socket.".into()),
        _ => Err("No local studio control socket on this platform.".into()),
    };
    scoped_socket(socket?, home)
}

fn studio_snapshot(home: Option<&Path>) -> Result<coder_access::studio::Snapshot, String> {
    let mut client = OperationClient::new(studio_socket(home)?);
    match client
        .call(
            &coder_access::studio_intents::mint(),
            &Operation::StudioSnapshot {},
        )
        .map_err(|e| e.to_string())?
    {
        Outcome::Studio { snapshot } => Ok(*snapshot),
        _ => Err("Host answered another studio operation.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scratch_service_never_selects_a_socket_outside_its_home() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("scratch");
        std::fs::create_dir(&home).unwrap();
        let inside = home.join("control.sock");
        let outside = temp.path().join("owner.sock");
        std::fs::write(&inside, b"fixture").unwrap();
        std::fs::write(&outside, b"fixture").unwrap();
        assert!(scoped_socket(inside, Some(&home)).is_ok());
        assert!(scoped_socket(outside, Some(&home)).is_err());
        assert!(scoped_socket(home.join("missing.sock"), Some(&home)).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn a_scratch_socket_symlink_cannot_escape_to_an_owner_socket() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("scratch");
        std::fs::create_dir(&home).unwrap();
        let outside = temp.path().join("owner.sock");
        std::fs::write(&outside, b"fixture").unwrap();
        let inside = home.join("control.sock");
        std::os::unix::fs::symlink(&outside, &inside).unwrap();
        assert!(scoped_socket(inside, Some(&home)).is_err());
    }
}

pub fn studio_read(home: Option<&Path>) -> Receiver<Result<terminal_core::studio::View, String>> {
    let (tx, rx) = mpsc::channel();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let result = studio_snapshot(home.as_deref()).and_then(|snapshot| {
            let mut view = crate::studio::project(
                &snapshot,
                &[
                    coder_access::Right::Observe,
                    coder_access::Right::Operate,
                    coder_access::Right::Review,
                ],
            )?;
            view.local_runs = snapshot.view.tasks.iter().map(|t| t.task.clone()).collect();
            Ok(view)
        });
        let _ = tx.send(result);
    });
    rx
}

pub fn studio_prepare(
    source: &[u8],
    review: Option<&terminal_core::studio::Review>,
    line: &str,
    _home: Option<&Path>,
    workspace: Option<&str>,
) -> Receiver<Result<terminal_core::studio::Prepared, String>> {
    let (tx, rx) = mpsc::channel();
    let source = source.to_vec();
    let review = review.cloned();
    let line = line.to_owned();
    let workspace = workspace.map(str::to_owned);
    std::thread::spawn(move || {
        let result = serde_json::from_slice::<coder_access::studio::Snapshot>(&source)
            .map_err(|error| error.to_string())
            .and_then(|snapshot| {
                crate::studio::prepare(
                    review.as_ref(),
                    &snapshot,
                    &[
                        coder_access::Right::Observe,
                        coder_access::Right::Operate,
                        coder_access::Right::Review,
                    ],
                    &line,
                    workspace.as_deref(),
                )
            });
        let _ = tx.send(result);
    });
    rx
}

pub fn studio_send(
    prepared: &terminal_core::studio::Prepared,
    home: Option<&Path>,
) -> Receiver<Result<String, String>> {
    let (tx, rx) = mpsc::channel();
    let home = home.map(Path::to_path_buf);
    let prepared = prepared.clone();
    std::thread::spawn(move || {
        let result = (|| {
            let snapshot = studio_snapshot(home.as_deref())?;
            if snapshot.stream != prepared.stream {
                return Err("Studio host changed; nothing sent.".into());
            }
            let operation: coder_access::Operation =
                serde_json::from_slice(&prepared.bytes).map_err(|e| e.to_string())?;
            operation.validate().map_err(|e| e.to_string())?;
            let mut client = OperationClient::new(studio_socket(home.as_deref())?);
            let outcome = client
                .call(&prepared.request, &operation)
                .map_err(|error| {
                    if matches!(
                        error.code,
                        coder_access::Code::Unavailable | coder_access::Code::Transport
                    ) {
                        format!("Studio outcome unknown: {error}; no automatic replay.")
                    } else {
                        error.to_string()
                    }
                })?;
            match outcome {
                Outcome::Dispatched { receipt } => Ok(format!(
                    "Host receipt: {} {}",
                    receipt.operation, receipt.reference
                )),
                Outcome::Merged { merged } => Ok(format!("Host merge receipt: {:?}", merged)),
                _ => Err("Host answered another studio operation; no automatic replay.".into()),
            }
        })();
        let _ = tx.send(result);
    });
    rx
}

pub fn studio_review(
    task: &str,
    home: Option<&Path>,
) -> Receiver<Result<terminal_core::studio::Review, String>> {
    let (tx, rx) = mpsc::channel();
    let task = task.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let result = (|| {
            let snapshot = studio_snapshot(home.as_deref())?;
            if !snapshot.view.tasks.iter().any(|row| row.task == task) {
                return Err("Task is not in the admitted snapshot.".into());
            }
            let mut client = OperationClient::new(studio_socket(home.as_deref())?);
            match client
                .call(
                    &coder_access::studio_intents::mint(),
                    &Operation::OpenReview { task },
                )
                .map_err(|e| e.to_string())?
            {
                Outcome::Review { review } => {
                    crate::studio::project_review(&snapshot.stream, &review)
                }
                _ => Err("Host answered another review operation.".into()),
            }
        })();
        let _ = tx.send(result);
    });
    rx
}
