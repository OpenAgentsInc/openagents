//! Where a checkout's graph comes from, and how one is published.
//!
//! **One writer, off `main`.** One publisher (`code-graph publish`, run
//! after pushes to `main` by the timer in `deploy/code-graph/`) builds the
//! graph of `origin/main` and force-pushes it as the single commit of the
//! `code-graph` branch: `code-graph.jsonl.gz` and `COMMIT`. The graph is
//! never committed to `main`.
//!
//! **Readers.** [`open`] answers for a checkout, in this order:
//! 1. the graph already loaded in this process for the same state;
//! 2. the cached graph under `<git common dir>/code-graph/`;
//! 3. with the code committed, the published graph, when the code paths
//!    did not change between its commit and `HEAD`;
//! 4. a local build: from `HEAD`'s objects when the code is committed,
//!    else from the working tree. A build is cached by `HEAD` and the
//!    changed files' contents.

use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use flate2::{Compression, GzBuilder, read::GzDecoder};
use sha2::{Digest, Sha256};

use crate::build::{self, Source};
use crate::model::Graph;
use crate::query::Index;

/// The branch the publisher writes.
pub const BRANCH: &str = "code-graph";
/// The graph's file on the branch.
pub const GRAPH_FILE: &str = "code-graph.jsonl.gz";
/// The commit the published graph was built from, on the branch.
pub const COMMIT_FILE: &str = "COMMIT";
/// The local ref a fetched branch is kept at.
pub const PUBLISHED_REF: &str = "refs/code-graph/published";
/// Set to `1` to never fetch the published graph (tests, offline work).
pub const NO_FETCH_ENV: &str = "CODE_GRAPH_NO_FETCH";
/// The remote to fetch from; `origin` when unset.
pub const REMOTE_ENV: &str = "CODE_GRAPH_REMOTE";
/// The longest a fetch may take.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
/// Cached graphs kept per repository.
const KEEP: usize = 4;

/// How [`open`] got its graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Memory,
    Cache,
    Published,
    Built { millis: u128 },
}

/// A checkout's graph.
#[derive(Clone)]
pub struct Opened {
    pub index: Arc<Index>,
    pub origin: Origin,
    /// `HEAD`, and whether uncommitted code is in the graph.
    pub commit: String,
    pub dirty: bool,
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("git could not start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git(root, args)?).trim().to_owned())
}

/// Compress a graph's bytes the same way every time (no name, no time).
#[must_use]
pub fn compress(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzBuilder::new().write(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .expect("writing to a Vec cannot fail");
    encoder.finish().expect("writing to a Vec cannot fail")
}

/// Read a compressed graph.
pub fn decompress(bytes: &[u8]) -> Result<Graph, String> {
    Graph::read(BufReader::new(GzDecoder::new(bytes)))
}

fn memory() -> &'static Mutex<HashMap<PathBuf, (String, Arc<Index>)>> {
    static LOADED: OnceLock<Mutex<HashMap<PathBuf, (String, Arc<Index>)>>> = OnceLock::new();
    LOADED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The graph for the checkout `cwd` is in.
pub fn open(cwd: &Path) -> Result<Opened, String> {
    let root = PathBuf::from(
        git_text(cwd, &["rev-parse", "--show-toplevel"])
            .map_err(|_| "This folder is not in a Git repository, so it has no code graph.")?,
    );
    let common = PathBuf::from(git_text(
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let head = build::resolve_commit(&root, "HEAD")?;
    let changes = build::code_changes(&root)?;
    let dirty = !changes.is_empty();
    let key = if dirty {
        let mut hash = Sha256::new();
        for path in &changes {
            hash.update(path.as_bytes());
            hash.update([0]);
            match std::fs::read(root.join(path)) {
                Ok(bytes) => hash.update(&bytes),
                Err(_) => hash.update(b"\0deleted"),
            }
            hash.update([0]);
        }
        let digest = hash.finalize();
        let short: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
        format!("{head}-{short}")
    } else {
        head.clone()
    };
    let opened = |index: Arc<Index>, origin: Origin| Opened {
        index,
        origin,
        commit: head.clone(),
        dirty,
    };
    if let Some((loaded, index)) = memory()
        .lock()
        .map_err(|_| "The graph cache is unavailable.")?
        .get(&root)
        && *loaded == key
    {
        return Ok(opened(index.clone(), Origin::Memory));
    }
    let dir = common.join("code-graph");
    let cached = dir.join(format!("{key}.jsonl.gz"));
    let remember = |graph: Graph| -> Result<Arc<Index>, String> {
        let index = Arc::new(Index::new(graph));
        memory()
            .lock()
            .map_err(|_| "The graph cache is unavailable.")?
            .insert(root.clone(), (key.clone(), index.clone()));
        Ok(index)
    };
    if let Ok(bytes) = std::fs::read(&cached)
        && let Ok(graph) = decompress(&bytes)
    {
        return Ok(opened(remember(graph)?, Origin::Cache));
    }
    if !dirty && let Some(bytes) = published_for(&root, &head) {
        if let Ok(mut graph) = decompress(&bytes) {
            // The graph is the same for any commit with the same code.
            graph.header.commit = head.clone();
            let _ = save(&dir, &cached, &compress(&graph.to_bytes()));
            return Ok(opened(remember(graph)?, Origin::Published));
        }
    }
    let source = if dirty {
        Source::Worktree { root: root.clone() }
    } else {
        Source::Commit {
            root: root.clone(),
            rev: head.clone(),
        }
    };
    let built = build::build(&source)?;
    let _ = save(&dir, &cached, &compress(&built.graph.to_bytes()));
    Ok(opened(
        remember(built.graph)?,
        Origin::Built {
            millis: built.millis,
        },
    ))
}

fn save(dir: &Path, file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let partial = file.with_extension("partial");
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, file)?;
    // Keep the newest few.
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().to_string_lossy().ends_with(".jsonl.gz"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    entries.sort();
    let excess = entries.len().saturating_sub(KEEP);
    for (_, path) in entries.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// Do two commits have the same code (no code path differs)?
pub fn same_code(root: &Path, a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match git(root, &["diff", "--name-only", "-z", a, b]) {
        Ok(out) => !out
            .split(|byte| *byte == 0)
            .map(String::from_utf8_lossy)
            .any(|path| {
                build::is_code(&path) || path == "Cargo.toml" || path.ends_with("/Cargo.toml")
            }),
        Err(_) => false,
    }
}

/// The published graph, when it was built from the same code as `head`.
fn published_for(root: &Path, head: &str) -> Option<Vec<u8>> {
    let usable = |root: &Path| -> Option<Vec<u8>> {
        let commit = git_text(root, &["show", &format!("{PUBLISHED_REF}:{COMMIT_FILE}")]).ok()?;
        if !same_code(root, &commit, head) {
            return None;
        }
        git(
            root,
            &["cat-file", "blob", &format!("{PUBLISHED_REF}:{GRAPH_FILE}")],
        )
        .ok()
    };
    if let Some(bytes) = usable(root) {
        return Some(bytes);
    }
    if std::env::var(NO_FETCH_ENV).is_ok_and(|value| value == "1") {
        return None;
    }
    let remote = std::env::var(REMOTE_ENV).unwrap_or_else(|_| "origin".into());
    fetch(root, &remote).ok()?;
    usable(root)
}

/// Fetch the published branch into [`PUBLISHED_REF`], within
/// [`FETCH_TIMEOUT`].
pub fn fetch(root: &Path, remote: &str) -> Result<(), String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "fetch",
            "--quiet",
            "--no-tags",
            "--no-write-fetch-head",
            "--depth=1",
            remote,
            &format!("+refs/heads/{BRANCH}:{PUBLISHED_REF}"),
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("git fetch {remote} {BRANCH} failed"))
            };
        }
        if started.elapsed() > FETCH_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("git fetch timed out".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// What [`publish`] did.
#[derive(Debug)]
pub struct Published {
    pub commit: String,
    /// The branch commit written, or `None` when the code had not changed
    /// since the last publication.
    pub branch_commit: Option<String>,
    pub nodes: usize,
    pub edges: usize,
    pub millis: u128,
    pub bytes: usize,
}

/// Build the graph of `rev` and write it as the single commit of the
/// [`BRANCH`] branch, with Git plumbing only (the working tree is not
/// touched); push it to `remote` when one is given. Skips the work when
/// the last published graph has the same code, unless `force`.
pub fn publish(
    root: &Path,
    rev: &str,
    remote: Option<&str>,
    force: bool,
) -> Result<Published, String> {
    let commit = build::resolve_commit(root, rev)?;
    if let Some(remote) = remote {
        let _ = fetch(root, remote);
    }
    if !force
        && let Ok(previous) = git_text(root, &["show", &format!("{PUBLISHED_REF}:{COMMIT_FILE}")])
        && same_code(root, &previous, &commit)
    {
        return Ok(Published {
            commit,
            branch_commit: None,
            nodes: 0,
            edges: 0,
            millis: 0,
            bytes: 0,
        });
    }
    let built = build::build(&Source::Commit {
        root: root.to_path_buf(),
        rev: commit.clone(),
    })?;
    let bytes = compress(&built.graph.to_bytes());
    let blob = |data: &[u8]| -> Result<String, String> {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["hash-object", "-w", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(data)
            .map_err(|error| error.to_string())?;
        let mut out = String::new();
        child
            .stdout
            .take()
            .ok_or("no stdout")?
            .read_to_string(&mut out)
            .map_err(|error| error.to_string())?;
        child.wait().map_err(|error| error.to_string())?;
        Ok(out.trim().to_owned())
    };
    let graph_blob = blob(&bytes)?;
    let commit_blob = blob(format!("{commit}\n").as_bytes())?;
    let readme = format!(
        "# Code graph\n\nThe code graph of {commit}, written by `code-graph publish` \
         (crates/code-graph). Readers fetch this branch; it is never merged.\n\n\
         nodes {}, edges {}, files {}; {}; {} with {}.\n",
        built.graph.header.nodes,
        built.graph.header.edges,
        built.graph.header.files,
        built.graph.header.builder,
        built.graph.header.parser,
        built.graph.header.grammar,
    );
    let readme_blob = blob(readme.as_bytes())?;
    let listing = format!(
        "100644 blob {commit_blob}\t{COMMIT_FILE}\n100644 blob {readme_blob}\tREADME.md\n100644 blob {graph_blob}\t{GRAPH_FILE}\n"
    );
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("mktree")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(listing.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut tree = String::new();
    child
        .stdout
        .take()
        .ok_or("no stdout")?
        .read_to_string(&mut tree)
        .map_err(|error| error.to_string())?;
    child.wait().map_err(|error| error.to_string())?;
    let branch_commit = git_text(
        root,
        &[
            "commit-tree",
            tree.trim(),
            "-m",
            &format!("Code graph of {commit}"),
        ],
    )?;
    git(root, &["update-ref", PUBLISHED_REF, &branch_commit])?;
    if let Some(remote) = remote {
        git(
            root,
            &[
                "push",
                "--quiet",
                "--force",
                remote,
                &format!("{branch_commit}:refs/heads/{BRANCH}"),
            ],
        )?;
    }
    Ok(Published {
        commit,
        branch_commit: Some(branch_commit),
        nodes: built.graph.header.nodes,
        edges: built.graph.header.edges,
        millis: built.millis,
        bytes: bytes.len(),
    })
}
