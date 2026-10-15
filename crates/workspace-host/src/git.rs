//! Git owns document versions. update-ref provides source-native CAS.
use crate::{Result, artifact::MAX_BYTES};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitDocument {
    root: PathBuf,
    common: PathBuf,
    reference: String,
    path: String,
    identity: Option<(u64, u64)>,
}
#[derive(Clone, Debug)]
pub struct Capture {
    pub revision: String,
    pub bytes: Vec<u8>,
    pub position: String,
}
impl GitDocument {
    pub fn same_ref(&self, other: &Self) -> bool {
        self.common == other.common
            && self.reference == other.reference
            && self.identity == other.identity
    }
    /// Register explicit Git authority. This never derives paths from a resource ID.
    pub fn admit(root: &Path, reference: &str, path: &str) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        relative(path)?;
        if !reference.starts_with("refs/heads/") || reference.len() > 1024 {
            return Err("a full Git branch reference is required".into());
        }
        command(&root, &["check-ref-format", reference], None)?;
        let common = String::from_utf8(command(
            &root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            None,
        )?)
        .map_err(|_| "Git directory encoding")?;
        let common = PathBuf::from(common.trim())
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let identity = identity(&common)?;
        let value = Self {
            root,
            common,
            reference: reference.into(),
            path: path.into(),
            identity,
        };
        value.capture()?;
        Ok(value)
    }
    fn check(&self) -> Result<()> {
        if self.root.canonicalize().map_err(|e| e.to_string())? != self.root
            || identity(&self.common)? != self.identity
        {
            return Err("Git source identity changed; register a new binding".into());
        }
        let common = String::from_utf8(command(
            &self.root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            None,
        )?)
        .map_err(|_| "Git directory encoding")?;
        if PathBuf::from(common.trim())
            .canonicalize()
            .map_err(|e| e.to_string())?
            != self.common
        {
            return Err("Git source directory changed".into());
        }
        Ok(())
    }
    pub fn capture(&self) -> Result<Capture> {
        self.check()?;
        let revision = String::from_utf8(command(
            &self.root,
            &[
                "rev-parse",
                "--verify",
                &format!("{}^{{commit}}", self.reference),
            ],
            None,
        )?)
        .map_err(|_| "Git revision encoding")?
        .trim()
        .to_string();
        object_id(&revision)?;
        let mode = command(&self.root, &["ls-tree", &revision, "--", &self.path], None)?;
        let mode = std::str::from_utf8(&mode)
            .map_err(|_| "Git file mode encoding")?
            .split_whitespace()
            .next()
            .ok_or("Git document is unavailable")?;
        if !matches!(mode, "100644" | "100755") {
            return Err("Git document profile requires a regular file".into());
        }
        let object = format!("{revision}:{}", self.path);
        let kind = command(&self.root, &["cat-file", "-t", &object], None)?;
        if kind != b"blob\n" {
            return Err("document is not a Git blob".into());
        }
        let bytes = command(&self.root, &["cat-file", "blob", &object], None)?;
        std::str::from_utf8(&bytes).map_err(|_| "Git document is not supported UTF-8")?;
        let position = self.feed_position(&revision)?;
        self.check()?;
        Ok(Capture {
            revision,
            bytes,
            position,
        })
    }
    /// Prepare an immutable commit; preparing does not change any branch or index.
    pub fn prepare(&self, expected: &str, bytes: &[u8], fingerprint: &str) -> Result<String> {
        self.check()?;
        let expected_position = expected;
        let expected = position_revision(expected_position)?;
        std::str::from_utf8(bytes).map_err(|_| "replacement is not UTF-8")?;
        if bytes.len() > MAX_BYTES {
            return Err("replacement byte limit exceeded".into());
        }
        let current = self.capture()?;
        if current.position != expected_position {
            return Err("stale: document branch changed".into());
        }
        let blob = String::from_utf8(command(
            &self.root,
            &["hash-object", "-w", "--stdin"],
            Some(bytes),
        )?)
        .map_err(|_| "Git blob encoding")?
        .trim()
        .to_string();
        object_id(&blob)?;
        let index = tempfile::NamedTempFile::new_in(&self.common).map_err(|e| e.to_string())?;
        let path = index.path().to_path_buf();
        drop(index); // Git creates its own index atomically.
        let run =
            |args: &[&str]| -> Result<Vec<u8>> { git_command(&self.root, args, None, Some(&path)) };
        let result = (|| {
            run(&["read-tree", expected])?;
            // Existing modes are retained, and symlinks/submodules are refused.
            let row = String::from_utf8(command(
                &self.root,
                &["ls-tree", expected, "--", &self.path],
                None,
            )?)
            .map_err(|_| "Git tree encoding")?;
            let mode = row
                .split_whitespace()
                .next()
                .ok_or("Git document is absent")?;
            if !matches!(mode, "100644" | "100755") {
                return Err("conditional document replacement requires a regular Git file".into());
            }
            run(&[
                "update-index",
                "--add",
                "--cacheinfo",
                mode,
                &blob,
                &self.path,
            ])?;
            let tree = String::from_utf8(run(&["write-tree"])?)
                .map_err(|_| "Git tree encoding")?
                .trim()
                .to_string();
            object_id(&tree)?;
            let message = format!("OpenAgents workspace change\n\nAdmission: {fingerprint}\n");
            let commit = String::from_utf8(command(
                &self.root,
                &["commit-tree", &tree, "-p", expected],
                Some(message.as_bytes()),
            )?)
            .map_err(|_| "Git commit encoding")?
            .trim()
            .to_string();
            object_id(&commit)?;
            Ok(commit)
        })();
        let _ = std::fs::remove_file(path);
        result
    }
    /// The actual source mutation checks both the commit and ref history while
    /// Git holds its native ref lock. An ABA ref change cannot pass this check.
    pub fn dispatch(&self, expected: &str, candidate: &str) -> Result<()> {
        self.dispatch_checked(expected, candidate, || Ok(()))
    }
    pub fn dispatch_checked(
        &self,
        expected_position: &str,
        candidate: &str,
        gate: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.check()?;
        let expected = position_revision(expected_position)?;
        object_id(candidate)?;
        let worktrees = String::from_utf8(command(
            &self.root,
            &["worktree", "list", "--porcelain"],
            None,
        )?)
        .map_err(|_| "Git worktree encoding")?;
        if worktrees
            .lines()
            .any(|line| line == format!("branch {}", self.reference))
        {
            return Err("cannot_enforce: conditional save to a checked-out Git branch".into());
        }
        let mut process = git_builder(
            &self.root,
            &[
                "update-ref",
                "--stdin",
                "--no-deref",
                "--create-reflog",
                "-m",
                "OpenAgents workspace change",
            ],
        );
        let mut child = Guard(process.spawn().map_err(|e| e.to_string())?);
        let mut input = child.0.stdin.take().ok_or("Git transaction input")?;
        let output = child.0.stdout.take().ok_or("Git transaction output")?;
        let errors = child
            .0
            .stderr
            .take()
            .ok_or("Git transaction error output")?;
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let mut output = std::io::BufReader::new(output);
            for _ in 0..3 {
                let mut line = String::new();
                let result = output
                    .by_ref()
                    .take(1024)
                    .read_line(&mut line)
                    .map(|_| line);
                if send.send(result).is_err() {
                    break;
                }
            }
        });
        let stderr = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = errors.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes);
        });
        let started = std::time::Instant::now();
        let read = || -> Result<String> {
            let remaining = std::time::Duration::from_secs(10)
                .checked_sub(started.elapsed())
                .ok_or("Git transaction deadline exceeded")?;
            receive
                .recv_timeout(remaining)
                .map_err(|_| "Git transaction did not answer")?
                .map_err(|e| e.to_string())
        };
        input
            .write_all(
                format!(
                    "start\nupdate {} {candidate} {expected}\nprepare\n",
                    self.reference
                )
                .as_bytes(),
            )
            .and_then(|_| input.flush())
            .map_err(|e| e.to_string())?;
        if read()?.trim() != "start: ok" || read()?.trim() != "prepare: ok" {
            return Err("Git refused the conditional transaction".into());
        }
        // A prepared ref transaction fences independent Git writers while the
        // exact admitted history and current host authority are rechecked.
        let preflight = (|| {
            self.check()?;
            if self.feed_position(expected)? != expected_position {
                return Err("stale: Git ref history changed before commit".into());
            }
            gate()
        })();
        if let Err(error) = preflight {
            let _ = input.write_all(b"abort\n").and_then(|_| input.flush());
            let _ = read();
            drop(input);
            return Err(error);
        }
        input
            .write_all(b"commit\n")
            .and_then(|_| input.flush())
            .map_err(|e| e.to_string())?;
        if read()?.trim() != "commit: ok" {
            return Err("Git transaction outcome is unknown".into());
        }
        drop(input);
        while child.0.try_wait().map_err(|e| e.to_string())?.is_none() {
            if started.elapsed() > std::time::Duration::from_secs(10) {
                return Err("Git transaction exit is unknown".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = stderr.join();
        Ok(())
    }
    /// A prepared object alone is not proof of dispatch. Current ref or its
    /// retained reflog must independently confirm that exact commit.
    fn log_bytes(&self) -> Result<Vec<u8>> {
        let path = self.common.join("logs").join(&self.reference);
        if path
            .canonicalize()
            .map_err(|_| "durable Git ref history is required")?
            != path
        {
            return Err("Git ref history source changed".into());
        }
        crate::artifact::read(&path, MAX_BYTES)
    }
    fn log_identity(&self) -> Result<(u64, u64)> {
        identity(&self.common.join("logs").join(&self.reference))?
            .ok_or("cannot_enforce: Git ref identity needs Unix file identities".into())
    }
    pub fn position_for(&self, candidate: &str) -> Result<String> {
        object_id(candidate)?;
        let bytes = self.log_bytes()?;
        let mut count = 0usize;
        let identity = self.log_identity()?;
        for line in bytes.split_inclusive(|b| *b == b'\n') {
            count += line.len();
            if std::str::from_utf8(line)
                .map_err(|_| "Git log encoding")?
                .split_whitespace()
                .nth(1)
                == Some(candidate)
            {
                return Ok(format!(
                    "gitlog:{count}:{}:{candidate}:{}:{}",
                    &nostr::contracts::digest_bytes(&bytes[..count])[7..],
                    identity.0,
                    identity.1
                ));
            }
        }
        Err("candidate transition is not retained".into())
    }
    fn feed_position(&self, revision: &str) -> Result<String> {
        let bytes = self.log_bytes()?;
        let line = bytes
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .next_back()
            .ok_or("Git ref history is empty")?;
        let line = std::str::from_utf8(line).map_err(|_| "Git ref history encoding")?;
        let head = line
            .split_whitespace()
            .nth(1)
            .ok_or("Git ref history entry")?;
        if head != revision {
            return Err("Git ref changed during capture; read it again".into());
        }
        Ok(format!(
            "gitlog:{}:{}:{revision}:{}:{}",
            bytes.len(),
            &nostr::contracts::digest_bytes(&bytes)[7..],
            self.log_identity()?.0,
            self.log_identity()?.1
        ))
    }
    /// A prefix digest covers every retained ref transition, including transient
    /// changes that returned to the same commit between two reads.
    pub fn feed_contiguous(&self, prior: &str, current: &str) -> Result<bool> {
        self.check()?;
        if prior.split(':').skip(4).ne(current.split(':').skip(4)) {
            return Ok(false);
        }
        let parse = |position: &str| -> Result<(usize, String)> {
            let fields = position.split(':').collect::<Vec<_>>();
            if fields.len() != 6 || fields[0] != "gitlog" {
                return Err("unsupported Git source position".into());
            }
            let size = fields[1]
                .parse::<usize>()
                .map_err(|_| "Git feed position")?;
            if fields[2].len() != 64 {
                return Err("Git feed digest".into());
            }
            object_id(fields[3])?;
            Ok((size, fields[2].into()))
        };
        let (length, digest) = parse(prior)?;
        let (current_length, current_digest) = parse(current)?;
        let bytes = self.log_bytes()?;
        if length > current_length || current_length > bytes.len() {
            return Ok(false);
        }
        Ok(
            &nostr::contracts::digest_bytes(&bytes[..length])[7..] == digest
                && &nostr::contracts::digest_bytes(&bytes[..current_length])[7..] == current_digest,
        )
    }
    pub fn feed_contains(&self, position: &str, candidate: &str) -> Result<bool> {
        object_id(candidate)?;
        let fields = position.split(':').collect::<Vec<_>>();
        if fields.len() != 6 || fields[0] != "gitlog" {
            return Err("unsupported source position".into());
        }
        let identity = self.log_identity()?;
        if fields[4] != identity.0.to_string() || fields[5] != identity.1.to_string() {
            return Err("Git ref history instance changed".into());
        }
        let length = fields[1].parse::<usize>().map_err(|_| "source position")?;
        let bytes = self.log_bytes()?;
        if length > bytes.len()
            || &nostr::contracts::digest_bytes(&bytes[..length])[7..] != fields[2]
        {
            return Err("source feed history changed".into());
        }
        let prefix =
            std::str::from_utf8(&bytes[..length]).map_err(|_| "source history encoding")?;
        Ok(prefix
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some(candidate)))
    }
    pub fn confirmed(&self, candidate: &str) -> Result<bool> {
        self.check()?;
        object_id(candidate)?;
        let current = self.capture()?;
        if current.revision == candidate {
            return Ok(true);
        }
        let log = command(
            &self.root,
            &["reflog", "show", "--format=%H", &self.reference],
            None,
        )?;
        Ok(String::from_utf8(log)
            .map_err(|_| "Git reflog encoding")?
            .lines()
            .any(|id| id == candidate))
    }
}
fn relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 1024
        || path.contains(['\\', '\0', '\n', '\r'])
        || path
            .split('/')
            .any(|p| p.is_empty() || matches!(p, "." | ".."))
    {
        return Err("Git document path must stay in its admitted tree".into());
    }
    Ok(())
}
fn object_id(id: &str) -> Result<()> {
    if !matches!(id.len(), 40 | 64)
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err("invalid Git object identity".into());
    }
    Ok(())
}
fn identity(path: &Path) -> Result<Option<(u64, u64)>> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(Some((metadata.dev(), metadata.ino())))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(None)
    }
}
struct Guard(std::process::Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(2) {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn position_revision(position: &str) -> Result<&str> {
    let fields = position.split(':').collect::<Vec<_>>();
    if fields.len() != 6 || fields[0] != "gitlog" {
        return Err("unsupported conditional Git version".into());
    }
    object_id(fields[3])?;
    Ok(fields[3])
}
fn git_builder(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("--literal-pathspecs")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "maintenance.auto=false",
            "-c",
            "gc.auto=0",
        ])
        .args(args)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "OpenAgents")
        .env("GIT_AUTHOR_EMAIL", "workspace@openagents.local")
        .env("GIT_COMMITTER_NAME", "OpenAgents")
        .env("GIT_COMMITTER_EMAIL", "workspace@openagents.local")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
fn command(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    git_command(root, args, input, None)
}
fn git_command(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    index: Option<&Path>,
) -> Result<Vec<u8>> {
    let mut command = git_builder(root, args);
    if input.is_none() {
        command.stdin(Stdio::null());
    }
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let input = if let Some(bytes) = input {
        let bytes = bytes.to_vec();
        let mut pipe = child.stdin.take().ok_or("Git input unavailable")?;
        Some(std::thread::spawn(move || pipe.write_all(&bytes)))
    } else {
        None
    };
    let overflow = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = |pipe: Box<dyn Read + Send>| {
        let overflow = overflow.clone();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes);
            if bytes.len() > MAX_BYTES {
                overflow.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            result.map(|_| bytes)
        })
    };
    let stdout = reader(Box::new(
        child.stdout.take().ok_or("Git output unavailable")?,
    ));
    let stderr = reader(Box::new(
        child.stderr.take().ok_or("Git error output unavailable")?,
    ));
    let started = std::time::Instant::now();
    let mut interrupted = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if overflow.load(std::sync::atomic::Ordering::Relaxed)
            || started.elapsed() > std::time::Duration::from_secs(10)
        {
            interrupted = true;
            let _ = child.kill();
            break child.wait().map_err(|e| e.to_string())?;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    if let Some(input) = input {
        let _ = input.join();
    }
    let bytes = stdout
        .join()
        .map_err(|_| "Git output reader failed")?
        .map_err(|e| e.to_string())?;
    let errors = stderr
        .join()
        .map_err(|_| "Git error reader failed")?
        .map_err(|e| e.to_string())?;
    if interrupted || bytes.len() > MAX_BYTES || errors.len() > MAX_BYTES {
        return Err("Git operation exceeded its deadline or output bound".into());
    }
    if !status.success() {
        return Err("Git source refused the operation".into());
    }
    Ok(bytes)
}
