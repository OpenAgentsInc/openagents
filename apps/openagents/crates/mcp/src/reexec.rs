//! OpenAgents #12110: an `openagents mcp` server outlives a host upgrade.
//!
//! A chat's harness spawns this server once and keeps it, so after the
//! upgrade swaps the binary the server would go on answering with the old
//! tools and drop fields it doesn't know. Instead it notices the swap
//! ([`Binary::replaced`]) when a tool call arrives and, once nothing is in
//! flight, execs the new binary in place: same pid, same stdin and stdout,
//! so the harness keeps its connection. The request that noticed, and any
//! input read but not yet handled, are handed to the new process in a file
//! named by [`RESUME_ENV`]; it handles them first and tells the client its
//! tool list changed.
//!
//! [`Input`] reads stdin itself, one read at a time and only when asked,
//! so at the moment of the exec every byte read so far is in its buffer
//! and none is in a read still running.

use std::io::Read;
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

/// Names the file a re-executed server resumes from.
pub const RESUME_ENV: &str = "OPENAGENTS_MCP_RESUME";
/// The most bytes one read takes.
const CHUNK: usize = 64 * 1024;

/// The executable this server runs from, as it was when it started.
#[derive(Debug, Clone)]
pub struct Binary {
    path: PathBuf,
    stamp: Option<Stamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    dev: u64,
    ino: u64,
    modified: Option<std::time::SystemTime>,
    len: u64,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    #[cfg(unix)]
    let (dev, ino) = {
        use std::os::unix::fs::MetadataExt;
        (meta.dev(), meta.ino())
    };
    #[cfg(not(unix))]
    let (dev, ino) = (0, 0);
    Some(Stamp {
        dev,
        ino,
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

impl Binary {
    /// The executable at `path` as it is now.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        let stamp = stamp(&path);
        Self { path, stamp }
    }

    /// This process's executable, read at startup.
    #[must_use]
    pub fn current() -> Option<Self> {
        std::env::current_exe().ok().map(Self::at)
    }

    /// The path, to exec.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A different file is at the path now: an upgrade put a new build
    /// there. A path with nothing at it isn't a replacement (an upgrade
    /// midway); neither is a start that couldn't read the path.
    #[must_use]
    pub fn replaced(&self) -> bool {
        match (self.stamp, stamp(&self.path)) {
            (Some(then), Some(now)) => then != now,
            _ => false,
        }
    }
}

/// Exec `binary` with this process's arguments and `resume` named in its
/// environment. Returns only when the exec failed.
#[cfg(unix)]
pub fn exec(binary: &Binary, resume: &Path) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    std::process::Command::new(binary.path())
        .args(std::env::args_os().skip(1))
        .env(RESUME_ENV, resume)
        .exec()
}

/// Write what the next process starts from: `line`, then `rest`.
///
/// # Errors
/// When the file can't be written.
pub fn write_resume(line: &[u8], rest: &[u8]) -> std::io::Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "openagents-mcp-resume-{}-{}.jsonl",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let mut bytes = Vec::with_capacity(line.len() + 1 + rest.len());
    bytes.extend_from_slice(line);
    bytes.push(b'\n');
    bytes.extend_from_slice(rest);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// What a re-executed server was handed, read once and removed; empty for
/// a server the harness started.
#[must_use]
pub fn take_resume() -> Vec<u8> {
    let Some(path) = std::env::var_os(RESUME_ENV) else {
        return Vec::new();
    };
    let path = PathBuf::from(path);
    let bytes = std::fs::read(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    bytes
}

/// Newline-delimited input, read in chunks only when a line is wanted.
pub struct Input {
    buf: Vec<u8>,
    ask: std::sync::mpsc::Sender<()>,
    chunks: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    ended: bool,
}

impl Input {
    /// Lines from `reader`, after the bytes in `start`.
    pub fn new(mut reader: Box<dyn Read + Send>, start: Vec<u8>) -> Self {
        let (ask, asked) = std::sync::mpsc::channel::<()>();
        let (send, chunks) = mpsc::channel(1);
        std::thread::spawn(move || {
            let mut chunk = vec![0u8; CHUNK];
            while asked.recv().is_ok() {
                let read = reader.read(&mut chunk).map(|n| chunk[..n].to_vec());
                let done = !matches!(&read, Ok(bytes) if !bytes.is_empty());
                if send.blocking_send(read).is_err() || done {
                    break;
                }
            }
        });
        Self {
            buf: start,
            ask,
            chunks,
            ended: false,
        }
    }

    /// This process's stdin, after what a previous process handed over.
    #[must_use]
    pub fn stdin(start: Vec<u8>) -> Self {
        Self::new(Box::new(std::io::stdin()), start)
    }

    /// The next line without its newline; `None` at the end of input.
    pub async fn next_line(&mut self) -> Option<Vec<u8>> {
        loop {
            if let Some(at) = self.buf.iter().position(|b| *b == b'\n') {
                let mut line: Vec<u8> = self.buf.drain(..=at).collect();
                line.pop();
                return Some(line);
            }
            if self.ended {
                return (!self.buf.is_empty()).then(|| std::mem::take(&mut self.buf));
            }
            let _ = self.ask.send(());
            match self.chunks.recv().await {
                Some(Ok(bytes)) if !bytes.is_empty() => self.buf.extend_from_slice(&bytes),
                _ => self.ended = true,
            }
        }
    }

    /// Bytes read and not yet returned as a line. No read is running when
    /// [`Input::next_line`] has returned.
    #[must_use]
    pub fn rest(&self) -> &[u8] {
        &self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader that returns its pieces one read at a time.
    struct Pieces(std::collections::VecDeque<Vec<u8>>);

    impl Read for Pieces {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let Some(piece) = self.0.pop_front() else {
                return Ok(0);
            };
            out[..piece.len()].copy_from_slice(&piece);
            Ok(piece.len())
        }
    }

    fn pieces(parts: &[&str]) -> Box<dyn Read + Send> {
        Box::new(Pieces(
            parts.iter().map(|p| p.as_bytes().to_vec()).collect(),
        ))
    }

    #[tokio::test]
    async fn lines_join_across_reads_and_the_rest_is_kept() {
        let mut input = Input::new(
            pieces(&["{\"a\":", "1}\n{\"b\"", ":2}\n{\"c\""]),
            Vec::new(),
        );
        assert_eq!(input.next_line().await.unwrap(), b"{\"a\":1}");
        // The second line ends in the third read, which also starts the next.
        assert_eq!(input.next_line().await.unwrap(), b"{\"b\":2}");
        assert_eq!(input.rest(), b"{\"c\"");
        // The third is unfinished until the input ends.
        assert_eq!(input.next_line().await.unwrap(), b"{\"c\"");
        assert_eq!(input.next_line().await, None);
    }

    #[tokio::test]
    async fn bytes_read_past_a_line_stay_in_the_rest() {
        let mut input = Input::new(pieces(&["one\ntwo\nthr"]), Vec::new());
        assert_eq!(input.next_line().await.unwrap(), b"one");
        assert_eq!(input.rest(), b"two\nthr");
    }

    #[tokio::test]
    async fn a_resumed_server_starts_from_what_it_was_handed() {
        let path = write_resume(b"{\"id\":7}", b"{\"id\":8}\n{\"id\"").unwrap();
        let handed = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let mut input = Input::new(pieces(&[":9}\n"]), handed);
        assert_eq!(input.next_line().await.unwrap(), b"{\"id\":7}");
        assert_eq!(input.next_line().await.unwrap(), b"{\"id\":8}");
        assert_eq!(input.next_line().await.unwrap(), b"{\"id\":9}");
        assert_eq!(input.next_line().await, None);
    }

    #[test]
    fn a_new_file_at_the_path_is_a_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openagents");
        std::fs::write(&path, b"old build").unwrap();
        let binary = Binary::at(path.clone());
        assert!(!binary.replaced());
        // The upgrade writes the new build beside it and renames it over.
        let next = dir.path().join("openagents.new");
        std::fs::write(&next, b"new build, longer").unwrap();
        std::fs::rename(&next, &path).unwrap();
        assert!(binary.replaced());
        // Nothing at the path (midway through a swap) is not a replacement.
        std::fs::remove_file(&path).unwrap();
        assert!(!binary.replaced());
        // Neither is a start that couldn't read its path.
        assert!(!Binary::at(dir.path().join("missing")).replaced());
    }
}
