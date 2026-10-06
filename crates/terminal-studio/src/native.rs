//! Existing same-user studio CLI adapters with bounded helper processes.
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use terminal_gfx::helpers::helper;

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
    let socket = studio_socket(home)?;
    let socket = socket.to_string_lossy();
    let (stdout, stderr) = helper(
        &[
            "--json",
            "studio",
            "watch",
            "--limit",
            "1",
            "--control-socket",
            &socket,
        ],
        None,
        home,
        64 * 1024,
        std::time::Duration::from_secs(35),
    )?;
    let mut value: serde_json::Value = serde_json::from_slice(&stdout).map_err(|_| {
        format!(
            "Studio snapshot unavailable: {}",
            String::from_utf8_lossy(&stderr)
        )
    })?;
    if let Some(object) = value.as_object_mut() {
        object.remove("kind");
    }
    serde_json::from_value(value).map_err(|e| format!("Studio snapshot unavailable: {e}"))
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
                &[coder_access::Right::Observe, coder_access::Right::Operate],
            )?;
            view.local_runs = snapshot.view.tasks.iter().map(|t| t.task.clone()).collect();
            Ok(view)
        });
        let _ = tx.send(result);
    });
    rx
}

pub fn studio_prepare(
    line: &str,
    home: Option<&Path>,
    workspace: Option<&str>,
) -> Receiver<Result<terminal_core::studio::Prepared, String>> {
    let (tx, rx) = mpsc::channel();
    let home = home.map(Path::to_path_buf);
    let line = line.to_owned();
    let workspace = workspace.map(str::to_owned);
    std::thread::spawn(move || {
        let result = studio_snapshot(home.as_deref()).and_then(|snapshot| {
            crate::studio::prepare(
                &snapshot,
                &[coder_access::Right::Observe, coder_access::Right::Operate],
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
            let expected = operation.name();
            let mut args = crate::studio::arguments(operation)?;
            // Naming the socket forces the existing CLI host path: a host
            // disappearing after the read must not fall back to a local store.
            let socket = studio_socket(home.as_deref())?;
            args.splice(
                2..2,
                [
                    "--control-socket".into(),
                    socket.to_string_lossy().into_owned(),
                ],
            );
            let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
            let (stdout, stderr) = helper(
                &refs,
                None,
                home.as_deref(),
                64 * 1024,
                std::time::Duration::from_secs(70),
            )?;
            if stdout.is_empty() {
                return Err(format!(
                    "Studio outcome unknown; no automatic replay: {}",
                    String::from_utf8_lossy(&stderr)
                ));
            }
            crate::studio::receipt(&stdout, expected)
        })();
        let _ = tx.send(result);
    });
    rx
}
