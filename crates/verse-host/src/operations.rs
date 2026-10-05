//! Owner-local status and drain control, with bounded IPC and explicit freshness.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use verse_world::service::operator::{Monitor, STATUS_BYTES};

pub fn build() -> verse_world::service::operator::Build {
    verse_world::service::operator::Build {
        package_version: env!("CARGO_PKG_VERSION").into(),
        source_revision: env!("VERSE_HOST_SOURCE_REVISION").into(),
        wire_version: verse_world::service::wire::VERSION,
    }
}
fn private(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "Cannot inspect operations path")?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err("Operations path must be a regular file or directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Operations paths require owner-only permissions".into());
        }
    }
    for part in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        if std::fs::symlink_metadata(part)
            .map_err(|_| "Cannot inspect operations path ancestor")?
            .file_type()
            .is_symlink()
        {
            return Err("Operations paths cannot contain symlinks".into());
        }
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Operations paths cannot contain parent traversal".into());
    }
    Ok(())
}
fn output(bytes: &[u8], connected: bool, draining: bool) -> Result<serde_json::Value, String> {
    if bytes.len() > STATUS_BYTES {
        return Err("Operations snapshot exceeds byte budget".into());
    }
    let snapshot: verse_world::service::operator::Snapshot =
        serde_json::from_slice(bytes).map_err(|_| "Invalid operations snapshot")?;
    snapshot.validate()?;
    let snapshot =
        serde_json::to_value(snapshot).map_err(|_| "Cannot project operations snapshot")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Cannot read operations clock")?
        .as_millis() as u64;
    let sampled = snapshot["sampled_unix_ms"]
        .as_u64()
        .ok_or("Operations snapshot has no sample time")?;
    let age = now.saturating_sub(sampled);
    let fresh = connected && sampled != 0 && sampled <= now && age <= 3000;
    let availability = if !connected {
        "unavailable"
    } else if fresh {
        "fresh"
    } else {
        "stale"
    };
    Ok(
        serde_json::json!({"schema":"verse.host.operator.response.v1", "availability":availability,
        "age_ms":age, "ready":fresh && !draining && snapshot["live"] == true && snapshot["phase"] == "running" && snapshot["ready"] == true, "drain_requested":draining, "snapshot":snapshot}),
    )
}

#[cfg(unix)]
pub struct Server {
    root: PathBuf,
    _lock: File,
    listener: tokio::net::UnixListener,
}
#[cfg(unix)]
impl Server {
    pub fn bind(root: &Path) -> Result<Self, String> {
        for ancestor in root
            .ancestors()
            .skip(1)
            .filter(|p| !p.as_os_str().is_empty())
        {
            if std::fs::symlink_metadata(ancestor)
                .map_err(|_| "Cannot inspect operations parent")?
                .file_type()
                .is_symlink()
            {
                return Err("Operations paths cannot contain symlinks".into());
            }
        }
        if !root.exists() {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(root)
                .map_err(|_| "Cannot create operations directory")?;
        }
        private(root, true)?;
        let path = root.join("operator.lock");
        if std::fs::symlink_metadata(&path).is_ok() {
            private(&path, false)?;
        }
        use std::os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt};
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| "Cannot open operations lock")?;
        lock.try_lock()
            .map_err(|_| "Operations directory already belongs to another host")?;
        let socket = root.join("operator.sock");
        match std::fs::symlink_metadata(&socket) {
            Ok(m) if m.file_type().is_socket() => std::fs::remove_file(&socket)
                .map_err(|_| "Cannot remove stale operations socket")?,
            Ok(_) => return Err("Operations socket path is occupied by another file".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Cannot inspect operations socket".into()),
        }
        let listener =
            tokio::net::UnixListener::bind(&socket).map_err(|_| "Cannot bind operations socket")?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| "Cannot secure operations socket")?;
        Ok(Self {
            root: root.to_path_buf(),
            _lock: lock,
            listener,
        })
    }
    pub async fn run(
        &self,
        monitor: Monitor,
        stop: impl std::future::Future<Output = ()>,
    ) -> Result<(), String> {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            task::JoinSet,
        };
        let mut workers = JoinSet::new();
        tokio::pin!(stop);
        loop {
            tokio::select! {
                _ = &mut stop => break,
                joined = workers.join_next(), if !workers.is_empty() => { if joined.is_some_and(|r| r.is_err()) { return Err("Operations worker failed".into()); } },
                accepted = self.listener.accept(), if workers.len() < 8 => {
                    let (mut stream, _) = accepted.map_err(|_| "Operations listener failed")?;
                    let monitor = monitor.clone();
                    workers.spawn(async move {
                        let operation = async {
                            let mut command = vec![];
                            (&mut stream).take(2).read_to_end(&mut command).await.map_err(|_| "Cannot read operations command")?;
                            if command.len() != 1 || !matches!(command[0], b'S' | b'D') { return Err("Unsupported operations command"); }
                            if command[0] == b'D' { monitor.request_drain(); }
                            let bytes = serde_json::to_vec(&monitor.snapshot()).map_err(|_| "Cannot encode operations snapshot")?;
                            if bytes.len() > STATUS_BYTES { return Err("Operations snapshot exceeds byte budget"); }
                            stream.write_all(&bytes).await.map_err(|_| "Cannot deliver operations snapshot")?;
                            stream.shutdown().await.map_err(|_| "Cannot close operations response")
                        };
                        // A slow local reader cannot hold more than one of eight workers for two seconds.
                        let _ = tokio::time::timeout(Duration::from_secs(2), operation).await;
                    });
                }
            }
        }
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        self.retain(&monitor)
    }
    fn retain(&self, monitor: &Monitor) -> Result<(), String> {
        let pending = self.root.join("status.next.json");
        if pending.exists() {
            private(&pending, false)?;
            std::fs::remove_file(&pending)
                .map_err(|_| "Cannot discard stale operations snapshot")?;
        }
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&pending)
            .map_err(|_| "Cannot create operations snapshot")?;
        let bytes = serde_json::to_vec(&monitor.snapshot())
            .map_err(|_| "Cannot encode operations snapshot")?;
        if bytes.len() > STATUS_BYTES {
            return Err("Operations snapshot exceeds byte budget".into());
        }
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Cannot retain operations snapshot")?;
        let target = self.root.join("status.json");
        if target.exists() {
            private(&target, false)?;
        }
        std::fs::rename(pending, target).map_err(|_| "Cannot publish operations snapshot")?;
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync operations snapshot directory".into())
    }
}
#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.root.join("operator.sock"));
    }
}

#[cfg(unix)]
pub fn command(root: &Path, drain: bool) -> Result<serde_json::Value, String> {
    use std::os::unix::fs::FileTypeExt;
    private(root, true)?;
    let socket = root.join("operator.sock");
    let connected = match std::fs::symlink_metadata(&socket) {
        Ok(m) if m.file_type().is_socket() => {
            use std::os::unix::fs::PermissionsExt;
            if m.permissions().mode() & 0o077 != 0 {
                return Err("Operations socket requires owner-only permissions".into());
            }
            true
        }
        Ok(_) => return Err("Invalid operations socket path".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("Cannot inspect operations socket".into()),
    };
    if connected {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Cannot create operations client runtime")?;
        let bytes = runtime.block_on(async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            tokio::time::timeout(Duration::from_secs(3), async {
                let mut stream = match tokio::net::UnixStream::connect(&socket).await {
                    Ok(stream) => stream,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        ) =>
                    {
                        return Ok(None);
                    }
                    Err(_) => return Err("Operations connection unavailable"),
                };
                stream
                    .write_all(if drain { b"D" } else { b"S" })
                    .await
                    .map_err(|_| "Cannot send operations command")?;
                stream
                    .shutdown()
                    .await
                    .map_err(|_| "Cannot close operations request")?;
                let mut bytes = vec![];
                stream
                    .take(STATUS_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|_| "Operations response unavailable")?;
                Ok(Some(bytes))
            })
            .await
            .map_err(|_| "Operations command timed out")?
        })?;
        if let Some(bytes) = bytes {
            return output(&bytes, true, drain);
        }
    }
    if drain {
        return Err("Host is unavailable; drain was not requested".into());
    }
    let path = root.join("status.json");
    private(&path, false)?;
    output(&crate::bounded(&path, STATUS_BYTES, true)?, false, false)
}
#[cfg(not(unix))]
pub fn command(_: &Path, _: bool) -> Result<serde_json::Value, String> {
    Err("Local operations IPC requires a Unix host".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_or_offline_samples_cannot_claim_readiness() {
        let m = Monitor::new(build(), 120, Some([8; 32])).unwrap();
        let mut value = serde_json::to_value(m.snapshot()).unwrap();
        value["ready"] = true.into();
        value["sampled_unix_ms"] = 1.into();
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            output(&bytes, true, false).unwrap()["availability"],
            "stale"
        );
        assert_eq!(output(&bytes, false, false).unwrap()["ready"], false);
        value["build"]["wire_version"] = 0.into();
        assert!(output(&serde_json::to_vec(&value).unwrap(), true, false).is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn owner_local_status_and_drain_are_bounded_and_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("operations");
        let server = Server::bind(&root).unwrap();
        assert!(Server::bind(&root).is_err());
        let monitor = Monitor::new(build(), 120, Some([8; 32])).unwrap();
        monitor.phase(
            verse_world::service::operator::Phase::Running,
            verse_world::service::operator::Reason::Starting,
            std::time::Instant::now(),
        );
        let m = monitor.clone();
        let r = root.clone();
        let requester = tokio::task::spawn_blocking(move || {
            let status = command(&r, false).unwrap();
            assert_eq!(status["availability"], "fresh");
            let status = command(&r, true).unwrap();
            assert_eq!(status["drain_requested"], true);
            assert_eq!(status["ready"], false);
        });
        server
            .run(m, async {
                let _ = requester.await;
            })
            .await
            .unwrap();
        drop(server);
        assert_eq!(
            command(&root, false).unwrap()["availability"],
            "unavailable"
        );
        assert!(command(&root, true).is_err());
    }
}
