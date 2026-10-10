//! Running a script inside a sandbox.
//!
//! On GCE the service reaches each VM's internal address over SSH as the
//! sandbox's `user` (the key is ours, put on the VM by its metadata), with
//! one multiplexed connection per VM. Tests run the same scripts in a local
//! shell instead.

use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

/// One finished script.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    /// The last bytes of stderr, kept even past the cap.
    pub stderr_tail: Vec<u8>,
    /// The transport, not the script, timed out or failed.
    pub transport_failed: bool,
}

/// One chunk of a streamed script.
#[derive(Debug)]
pub enum Chunk {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Exit(Option<i32>),
    Failed(String),
}

/// Runs scripts on a VM by its address.
pub trait Remote: Send + Sync + 'static {
    /// Run `script` with `stdin`; output is capped at `cap` bytes a stream.
    /// The transport gives up after `limit`.
    fn run(
        &self,
        host: &str,
        script: &str,
        stdin: Vec<u8>,
        limit: Duration,
        cap: usize,
    ) -> impl Future<Output = Output> + Send;
    /// Run `script` and send its output as it arrives; the last chunk is
    /// `Exit` or `Failed`.
    fn stream(
        &self,
        host: &str,
        script: &str,
        limit: Duration,
    ) -> impl Future<Output = mpsc::Receiver<Chunk>> + Send;
    /// Forget any cached connection to `host` (it rebooted or went away).
    fn forget(&self, host: &str) -> impl Future<Output = ()> + Send;
}

/// SSH to `user@host` with our key.
#[derive(Clone)]
pub struct Ssh {
    pub key: PathBuf,
    pub user: String,
    pub control_dir: PathBuf,
}

impl Ssh {
    fn command(&self, host: &str, script: &str) -> tokio::process::Command {
        let mut c = tokio::process::Command::new("ssh");
        c.arg("-i")
            .arg(&self.key)
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "LogLevel=ERROR",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "ServerAliveInterval=15",
                "-o",
                "ServerAliveCountMax=4",
                "-o",
                "ControlMaster=auto",
                "-o",
                "ControlPersist=300",
            ])
            .arg("-o")
            .arg(format!(
                "ControlPath={}/%r@%h",
                self.control_dir.to_string_lossy()
            ))
            .arg(format!("{}@{host}", self.user))
            .arg("--")
            .arg(format!("bash -c {}", boat::shell_quote(script)))
            .kill_on_drop(true);
        c
    }
}

/// Run a prepared command: feed stdin, capture capped output, bound by
/// `limit`. ssh's own failure is exit 255.
pub async fn capture(
    mut cmd: tokio::process::Command,
    stdin: Vec<u8>,
    limit: Duration,
    cap: usize,
    transport_code: Option<i32>,
) -> Output {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let Ok(mut child) = cmd.spawn() else {
        return Output {
            transport_failed: true,
            ..Default::default()
        };
    };
    let mut input = child.stdin.take();
    let out = child.stdout.take().expect("stdout");
    let err = child.stderr.take().expect("stderr");
    let feed = async move {
        if let Some(i) = input.as_mut() {
            let _ = i.write_all(&stdin).await;
            let _ = i.shutdown().await;
        }
        drop(input);
    };
    let read = |mut r: Box<dyn tokio::io::AsyncRead + Unpin + Send>| async move {
        let mut buf = Vec::new();
        let mut tail: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        let mut truncated = false;
        loop {
            match r.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = cap.saturating_sub(buf.len());
                    if room < n {
                        truncated = true;
                    }
                    buf.extend_from_slice(&chunk[..n.min(room)]);
                    tail.extend_from_slice(&chunk[..n]);
                    if tail.len() > 256 {
                        tail.drain(..tail.len() - 256);
                    }
                }
            }
        }
        (buf, truncated, tail)
    };
    let work = async {
        let (_, (o, ot, _), (e, et, tail)) =
            tokio::join!(feed, read(Box::new(out)), read(Box::new(err)));
        let status = child.wait().await.ok();
        (status, o, ot, e, et, tail)
    };
    match tokio::time::timeout(limit, work).await {
        Ok((status, stdout, ot, stderr, et, stderr_tail)) => {
            let code = status.and_then(|s| s.code());
            Output {
                transport_failed: transport_code.is_some() && code == transport_code,
                code,
                stdout,
                stderr,
                stdout_truncated: ot,
                stderr_truncated: et,
                stderr_tail,
            }
        }
        Err(_) => Output {
            transport_failed: true,
            ..Default::default()
        },
    }
}

/// Stream a prepared command's output into a channel.
pub fn pipe(mut cmd: tokio::process::Command, limit: Duration) -> mpsc::Receiver<Chunk> {
    let (tx, rx) = mpsc::channel(64);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    tokio::spawn(async move {
        let Ok(mut child) = cmd.spawn() else {
            let _ = tx.send(Chunk::Failed("cannot start".into())).await;
            return;
        };
        let mut out = child.stdout.take().expect("stdout");
        let mut err = child.stderr.take().expect("stderr");
        let tx2 = tx.clone();
        let tx3 = tx.clone();
        let a = async move {
            let mut b = vec![0u8; 32 * 1024];
            loop {
                match out.read(&mut b).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx2.send(Chunk::Stdout(b[..n].to_vec())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        };
        let b = async move {
            let mut b = vec![0u8; 32 * 1024];
            loop {
                match err.read(&mut b).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx3.send(Chunk::Stderr(b[..n].to_vec())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        };
        let run = async {
            tokio::join!(a, b);
            child.wait().await.ok()
        };
        match tokio::time::timeout(limit, run).await {
            Ok(status) => {
                let _ = tx.send(Chunk::Exit(status.and_then(|s| s.code()))).await;
            }
            Err(_) => {
                let _ = tx.send(Chunk::Failed("timed out".into())).await;
            }
        }
    });
    rx
}

impl Remote for Ssh {
    async fn run(
        &self,
        host: &str,
        script: &str,
        stdin: Vec<u8>,
        limit: Duration,
        cap: usize,
    ) -> Output {
        capture(self.command(host, script), stdin, limit, cap, Some(255)).await
    }

    async fn stream(&self, host: &str, script: &str, limit: Duration) -> mpsc::Receiver<Chunk> {
        pipe(self.command(host, script), limit)
    }

    async fn forget(&self, host: &str) {
        let mut c = tokio::process::Command::new("ssh");
        c.arg("-o")
            .arg(format!(
                "ControlPath={}/%r@%h",
                self.control_dir.to_string_lossy()
            ))
            .args(["-O", "exit"])
            .arg(format!("{}@{host}", self.user))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = tokio::time::timeout(Duration::from_secs(5), c.status()).await;
    }
}

/// Runs scripts in a local shell, one home directory per "host". For
/// tests and local development only.
#[derive(Clone)]
pub struct Local {
    pub root: PathBuf,
}

impl Local {
    fn command(&self, host: &str, script: &str) -> tokio::process::Command {
        let home = self.root.join(host).join("home");
        let _ = std::fs::create_dir_all(&home);
        let mut c = tokio::process::Command::new("bash");
        c.arg("-c")
            .arg(script)
            .env_clear()
            .env("HOME", &home)
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(&home)
            .kill_on_drop(true);
        c
    }
}

impl Remote for Local {
    async fn run(
        &self,
        host: &str,
        script: &str,
        stdin: Vec<u8>,
        limit: Duration,
        cap: usize,
    ) -> Output {
        capture(self.command(host, script), stdin, limit, cap, None).await
    }
    async fn stream(&self, host: &str, script: &str, limit: Duration) -> mpsc::Receiver<Chunk> {
        pipe(self.command(host, script), limit)
    }
    async fn forget(&self, _host: &str) {}
}
