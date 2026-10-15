//! Dependency refresh, isolated object stores, publication, and rollback.

use crate::recipes::{self, Artifact, Context, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Dependency {
    File {
        recipe: String,
        path: String,
        digest: String,
    },
    Directory {
        recipe: String,
        path: String,
        digest: String,
    },
    Impact {
        recipe: String,
        symbol: String,
        digest: String,
    },
}
impl Dependency {
    fn recipe(&self) -> &str {
        match self {
            Self::File { recipe, .. }
            | Self::Directory { recipe, .. }
            | Self::Impact { recipe, .. } => recipe,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub schema: String,
    pub artifact: Artifact,
    pub dependencies: Vec<Dependency>,
    pub stale: BTreeMap<String, Vec<String>>,
    pub private_sources: BTreeSet<String>,
}

fn directory(root: &Path, snapshot: &Snapshot, path: &str) -> Result<String, String> {
    recipes::relative(path)?;
    let bytes = recipes::git(root, &["ls-tree", "-r", "-z", &snapshot.commit, "--", path])?;
    Ok(crate::digest(&bytes))
}

fn dependency_digest(context: &Context, d: &Dependency) -> Result<String, String> {
    match d {
        Dependency::File { path, .. } => Ok(crate::digest(
            recipes::source(&context.root, &context.snapshot, path)?.as_bytes(),
        )),
        Dependency::Directory { path, .. } => directory(&context.root, &context.snapshot, path),
        Dependency::Impact { symbol, .. } => Ok(crate::digest(
            context.graph.impact(symbol, 2, 200, false)?.as_bytes(),
        )),
    }
}

pub fn bind(
    context: &Context,
    artifact: &Artifact,
    private_sources: BTreeSet<String>,
) -> Result<Bundle, String> {
    let artifact = context.verify(artifact)?;
    let mut dependencies = Vec::new();
    for recipe in &artifact.recipes {
        let mut paths = BTreeSet::new();
        let mut directories = BTreeSet::new();
        let mut symbols = BTreeSet::new();
        directories.extend(recipe.dependency_scopes.iter().cloned());
        for r in recipe
            .roles
            .iter()
            .flat_map(|r| &r.references)
            .chain(&recipe.examples)
            .chain(recipe.new_files.iter().map(|n| &n.integration))
        {
            paths.insert(r.path.clone());
            if let Some(package) = r
                .path
                .strip_prefix("crates/")
                .and_then(|p| p.split('/').next())
            {
                directories.insert(format!("crates/{package}"));
            }
            if let Some(parent) = Path::new(&r.path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                directories.insert(parent.to_string_lossy().into_owned());
            }
            if let Some(s) = &r.symbol {
                symbols.insert(s.clone());
            }
        }
        for check in &recipe.checks {
            paths.insert(check.manifest.clone());
        }
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "AGENTS.md",
            "rust-toolchain.toml",
        ] {
            if recipes::source(&context.root, &context.snapshot, path).is_ok() {
                paths.insert(path.into());
            }
        }
        for path in paths {
            let digest =
                crate::digest(recipes::source(&context.root, &context.snapshot, &path)?.as_bytes());
            dependencies.push(Dependency::File {
                recipe: recipe.id.clone(),
                path,
                digest,
            });
        }
        for path in directories {
            let digest = directory(&context.root, &context.snapshot, &path)?;
            dependencies.push(Dependency::Directory {
                recipe: recipe.id.clone(),
                path,
                digest,
            });
        }
        for symbol in symbols {
            let digest = crate::digest(context.graph.impact(&symbol, 2, 200, false)?.as_bytes());
            dependencies.push(Dependency::Impact {
                recipe: recipe.id.clone(),
                symbol,
                digest,
            });
        }
    }
    Ok(Bundle {
        schema: "openagents.task-index-bundle.v1".into(),
        artifact,
        dependencies,
        stale: BTreeMap::new(),
        private_sources,
    })
}

pub fn refresh(bundle: &Bundle, context: &Context) -> Result<Bundle, String> {
    if bundle.schema != "openagents.task-index-bundle.v1"
        || bundle.artifact.snapshot.repository != context.snapshot.repository
        || bundle.artifact.snapshot.workspace != context.snapshot.workspace
    {
        return Err("The index belongs to another repository or workspace.".into());
    }
    let mut refreshed = bundle.clone();
    refreshed.artifact.snapshot = context.snapshot.clone();
    for dependency in &bundle.dependencies {
        let previous = match dependency {
            Dependency::File { digest, .. }
            | Dependency::Directory { digest, .. }
            | Dependency::Impact { digest, .. } => digest,
        };
        if dependency_digest(context, dependency).as_ref() != Ok(previous) {
            refreshed
                .stale
                .entry(dependency.recipe().into())
                .or_default()
                .push("A source, discovery scope, or dependent item changed.".into());
        }
    }
    // Stale recipes keep their old evidence and cannot silently become fresh.
    let mut valid = refreshed.artifact.clone();
    valid
        .recipes
        .retain(|r| !refreshed.stale.contains_key(&r.id));
    context.verify(&valid)?;
    Ok(refreshed)
}

pub fn withdraw(bundle: &Bundle, source: &str) -> Bundle {
    let mut result = bundle.clone();
    if result.private_sources.remove(source) {
        result.artifact.recipes.clear();
        result.dependencies.clear();
        result.stale.clear();
    }
    result
}

pub fn dirty(
    bundle: &Bundle,
    root: &Path,
    graph: &code_graph::Index,
) -> Result<BTreeSet<String>, String> {
    let output = recipes::git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut paths = BTreeSet::new();
    let mut entries = output.split(|b| *b == 0);
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        paths.insert(String::from_utf8_lossy(&entry[3..]).into_owned());
        if entry[..2].contains(&b'R') || entry[..2].contains(&b'C') {
            if let Some(old) = entries.next() {
                paths.insert(String::from_utf8_lossy(old).into_owned());
            }
        }
    }
    let mut affected: BTreeSet<u32> = graph
        .graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| paths.contains(&graph.graph.files[n.file as usize]))
        .map(|(i, _)| i as u32)
        .collect();
    for _ in 0..3 {
        let more: Vec<_> = graph
            .graph
            .edges
            .iter()
            .filter(|e| affected.contains(&e.to))
            .map(|e| e.from)
            .collect();
        affected.extend(more);
    }
    let mut stale = BTreeSet::new();
    for dependency in &bundle.dependencies {
        let changed = match dependency {
            Dependency::File { path, .. } => paths.contains(path),
            Dependency::Directory { path, .. } => {
                paths.iter().any(|p| p.starts_with(&format!("{path}/")))
            }
            Dependency::Impact { symbol, .. } => graph
                .resolve(symbol, None)
                .is_ok_and(|id| affected.contains(&id)),
        };
        if changed {
            stale.insert(dependency.recipe().into());
        }
    }
    Ok(stale)
}

pub fn query(
    bundle: &Bundle,
    request: &crate::lookup::Request,
    dirty: &BTreeSet<String>,
) -> Result<crate::lookup::Answer, String> {
    let mut answer = crate::lookup::query(&bundle.artifact, request)?;
    if answer
        .candidates
        .iter()
        .any(|c| bundle.stale.contains_key(&c.id) || dirty.contains(&c.id))
    {
        answer.status = crate::lookup::Status::Stale;
        answer.required_roles_complete = false;
        for candidate in &mut answer.candidates {
            candidate.roles.clear();
            candidate.examples.clear();
            candidate.new_files.clear();
            candidate.checks.clear();
        }
        answer.next_step =
            "Rebuild the changed recipes, or inspect the current source with the graph.".into();
    }
    Ok(answer)
}

pub struct Store {
    pub directory: PathBuf,
    pub repository: String,
    pub workspace: String,
}
impl Store {
    pub fn new(base: &Path, snapshot: &Snapshot) -> Self {
        let key =
            crate::digest(format!("{}\0{}", snapshot.repository, snapshot.workspace).as_bytes());
        Self {
            directory: base.join(key),
            repository: snapshot.repository.clone(),
            workspace: snapshot.workspace.clone(),
        }
    }
    pub fn active(&self) -> Result<Option<String>, String> {
        let path = self.directory.join("active.json");
        if !path.exists() {
            return Ok(None);
        }
        crate::read_json(&path).map(Some)
    }
    pub fn load(&self, id: &str) -> Result<Bundle, String> {
        if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Choose an existing artifact digest.".into());
        }
        let path = self.directory.join(format!("{id}.json"));
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("An index object cannot be a symbolic link.".into());
        }
        let bundle: Bundle = crate::read_json(&path)?;
        if bundle.schema != "openagents.task-index-bundle.v1"
            || bundle.artifact.snapshot.repository != self.repository
            || bundle.artifact.snapshot.workspace != self.workspace
            || crate::digest(&serde_json::to_vec(&bundle).map_err(|e| e.to_string())?) != id
        {
            return Err("The saved index does not match its identity or digest.".into());
        }
        Ok(bundle)
    }
    fn lock(&self) -> Result<std::fs::File, String> {
        crate::private::write(
            &self.directory,
            "store.json",
            b"{\"schema\":\"openagents.task-store.v1\"}",
        )?;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(false).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(self.directory.join("writer.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "Another process is updating this index.")?;
        Ok(file)
    }
    pub fn install(&self, bundle: &Bundle, expected: Option<&str>) -> Result<String, String> {
        let _lock = self.lock()?;
        if bundle.schema != "openagents.task-index-bundle.v1"
            || bundle.artifact.snapshot.repository != self.repository
            || bundle.artifact.snapshot.workspace != self.workspace
        {
            return Err("The index belongs to another repository or workspace.".into());
        }
        if self.active()?.as_deref() != expected {
            return Err("The active index changed; compare it before trying again.".into());
        }
        let bytes = serde_json::to_vec(bundle).map_err(|e| e.to_string())?;
        let id = crate::digest(&bytes);
        crate::private::write(&self.directory, &format!("{id}.json"), &bytes)?;
        crate::private::write(
            &self.directory,
            "active.json",
            &serde_json::to_vec(&id).map_err(|e| e.to_string())?,
        )?;
        Ok(id)
    }
    pub fn withdraw_source(&self, source: &str, expected: Option<&str>) -> Result<usize, String> {
        let _lock = self.lock()?;
        if self.active()?.as_deref() != expected {
            return Err("The active index changed before source withdrawal.".into());
        }
        if let Some(id) = expected {
            let current = self.load(id)?;
            if current.private_sources.contains(source) {
                let clean = withdraw(&current, source);
                let bytes = serde_json::to_vec(&clean).map_err(|e| e.to_string())?;
                let id = crate::digest(&bytes);
                crate::private::write(&self.directory, &format!("{id}.json"), &bytes)?;
                crate::private::write(
                    &self.directory,
                    "active.json",
                    &serde_json::to_vec(&id).map_err(|e| e.to_string())?,
                )?;
            }
        }
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let Some(id) = path
                .file_stem()
                .and_then(|p| p.to_str())
                .filter(|s| s.len() == 64)
            else {
                continue;
            };
            if self.load(id)?.private_sources.contains(source) {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    pub fn rollback(&self, id: &str, expected: Option<&str>) -> Result<(), String> {
        let _lock = self.lock()?;
        self.load(id)?;
        if self.active()?.as_deref() != expected {
            return Err("The active index changed before rollback.".into());
        }
        crate::private::write(
            &self.directory,
            "active.json",
            &serde_json::to_vec(id).map_err(|e| e.to_string())?,
        )
    }
}

pub fn publish(root: &Path, bundle: &Bundle, remote: Option<&str>) -> Result<String, String> {
    if !bundle.private_sources.is_empty()
        || !bundle.stale.is_empty()
        || bundle.artifact.recipes.is_empty()
    {
        return Err("Only current source-backed public recipes can be published.".into());
    }
    let context = Context::open(
        root,
        &bundle.artifact.snapshot.commit,
        &bundle.artifact.snapshot.workspace,
    )?;
    context.verify(&bundle.artifact)?;
    let blob = |bytes: &[u8]| -> Result<String, String> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["hash-object", "-w", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("The Git input pipe is missing.")?
            .write_all(bytes)
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("Git could not write the index object.".into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    };
    let object = blob(&serde_json::to_vec(bundle).map_err(|e| e.to_string())?)?;
    let commit = blob(format!("{}\n", bundle.artifact.snapshot.commit).as_bytes())?;
    let listing = format!("100644 blob {commit}\tCOMMIT\n100644 blob {object}\ttask-index.json\n");
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut tree = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("mktree")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    tree.stdin
        .take()
        .ok_or("The Git tree input is missing.")?
        .write_all(listing.as_bytes())
        .map_err(|e| e.to_string())?;
    let result = tree.wait_with_output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err("Git could not write the index tree.".into());
    }
    let hash = String::from_utf8_lossy(&result.stdout).trim().to_owned();
    let commit = String::from_utf8_lossy(&recipes::git(
        root,
        &["commit-tree", &hash, "-m", "Source-backed task index"],
    )?)
    .trim()
    .to_owned();
    recipes::git(root, &["update-ref", "refs/code-index/published", &commit])?;
    if let Some(remote) = remote {
        recipes::git(
            root,
            &[
                "push",
                "--quiet",
                "--force",
                remote,
                &format!("{commit}:refs/heads/code-index"),
            ],
        )?;
    }
    Ok(commit)
}

pub fn fetch(root: &Path, remote: &str, workspace: &str, base: &Path) -> Result<String, String> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    if remote.is_empty()
        || !remote
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err("Choose a configured Git remote name.".into());
    }
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
            "+refs/heads/code-index:refs/code-index/downloaded",
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "The index fetch could not start.")?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("The published task index is unavailable.".into());
            }
            break;
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The task index fetch timed out.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let object = "refs/code-index/downloaded:task-index.json";
    let size = String::from_utf8_lossy(&recipes::git(root, &["cat-file", "-s", object])?)
        .trim()
        .parse::<usize>()
        .map_err(|_| "The published index size is invalid.")?;
    if size > 32 * 1024 * 1024 {
        return Err("The published index exceeds its size limit.".into());
    }
    let bundle: Bundle =
        serde_json::from_slice(&recipes::git(root, &["cat-file", "blob", object])?)
            .map_err(|_| "The published index could not be read.")?;
    if !bundle.private_sources.is_empty() {
        return Err("A published index cannot contain private-source references.".into());
    }
    let context = Context::open(root, "HEAD", workspace)?;
    let bundle = refresh(&bundle, &context)?;
    let store = Store::new(base, &context.snapshot);
    let previous = store.active()?;
    store.install(&bundle, previous.as_deref())
}
