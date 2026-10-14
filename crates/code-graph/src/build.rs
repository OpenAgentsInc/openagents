//! Build a graph from a commit or a working tree.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::extract::{self, Facts, RefKind};
use crate::model::{BUILDER, Edge, EdgeKind, GRAMMAR, Graph, Header, Kind, Node, PARSER, SCHEMA};

/// The largest file parsed; bigger ones are almost always generated.
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
/// How many candidates a name may have and still get (inexact) edges.
pub const MAX_CANDIDATES: usize = 3;

/// Where the files come from.
#[derive(Clone, Debug)]
pub enum Source {
    /// The tree of a commit, read from Git's objects: the same for every
    /// checkout, whatever its working tree holds.
    Commit { root: PathBuf, rev: String },
    /// The files on disk: tracked and untracked-but-not-ignored, for
    /// uncommitted work.
    Worktree { root: PathBuf },
}

/// A build and what it took.
#[derive(Clone, Debug)]
pub struct Built {
    pub graph: Graph,
    pub millis: u128,
    pub source_bytes: usize,
    pub files_with_parse_errors: usize,
}

/// Is `path` a code file the graph covers? Rust only, and never docs,
/// build output, vendored or generated code.
#[must_use]
pub fn is_code(path: &str) -> bool {
    path.ends_with(".rs") && !excluded(path)
}

fn excluded(path: &str) -> bool {
    path.starts_with("docs/")
        || path.split('/').any(|part| {
            matches!(
                part,
                "target" | "vendor" | "third_party" | "node_modules" | ".git"
            ) || part.to_ascii_lowercase().contains("generated")
        })
}

fn is_manifest(path: &str) -> bool {
    (path == "Cargo.toml" || path.ends_with("/Cargo.toml")) && !excluded(path)
}

/// Generated code says so near its top.
fn generated(text: &str) -> bool {
    let head: String = text.chars().take(600).collect();
    head.contains("@generated")
        || head.contains("DO NOT EDIT")
        || head.contains("automatically generated")
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
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

/// The full commit id `rev` names.
pub fn resolve_commit(root: &Path, rev: &str) -> Result<String, String> {
    let out = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    )?;
    Ok(String::from_utf8_lossy(&out).trim().to_owned())
}

/// The code paths (and manifests) changed in the working tree against
/// `HEAD`, untracked ones included; empty when the code is committed.
pub fn code_changes(root: &Path) -> Result<Vec<String>, String> {
    let out = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut changed = BTreeSet::new();
    let mut entries = out.split(|b| *b == 0).peekable();
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let status = &entry[..2];
        let path = String::from_utf8_lossy(&entry[3..]).into_owned();
        // A rename's entry is followed by its old path.
        if status.contains(&b'R') || status.contains(&b'C') {
            entries.next();
        }
        if is_code(&path) || is_manifest(&path) {
            changed.insert(path);
        }
    }
    Ok(changed.into_iter().collect())
}

/// Read every code file and manifest of the source, sorted by path.
fn read_files(source: &Source) -> Result<(String, bool, BTreeMap<String, String>), String> {
    let mut files = BTreeMap::new();
    match source {
        Source::Commit { root, rev } => {
            let commit = resolve_commit(root, rev)?;
            let listing = git(root, &["ls-tree", "-r", "-z", "--full-tree", &commit])?;
            let mut wanted = Vec::new();
            for entry in listing.split(|b| *b == 0) {
                let entry = String::from_utf8_lossy(entry);
                let Some((meta, path)) = entry.split_once('\t') else {
                    continue;
                };
                let mut meta = meta.split(' ');
                let (_, kind, oid) = (meta.next(), meta.next(), meta.next());
                if kind == Some("blob")
                    && let Some(oid) = oid
                    && (is_code(path) || is_manifest(path))
                {
                    wanted.push((path.to_owned(), oid.to_owned()));
                }
            }
            let blobs = cat_blobs(root, wanted.iter().map(|(_, oid)| oid.as_str()))?;
            for ((path, _), blob) in wanted.into_iter().zip(blobs) {
                if let Some(text) = blob {
                    files.insert(path, text);
                }
            }
            Ok((commit, false, files))
        }
        Source::Worktree { root } => {
            let commit = resolve_commit(root, "HEAD").unwrap_or_else(|_| "none".into());
            let listing = git(
                root,
                &[
                    "ls-files",
                    "-z",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                ],
            )?;
            for path in listing.split(|b| *b == 0) {
                let path = String::from_utf8_lossy(path).into_owned();
                if !(is_code(&path) || is_manifest(&path)) {
                    continue;
                }
                let full = root.join(&path);
                let Ok(meta) = std::fs::metadata(&full) else {
                    continue;
                };
                if !meta.is_file() || meta.len() as usize > MAX_FILE_BYTES {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&full) {
                    files.insert(path, text);
                }
            }
            let dirty = !code_changes(root)?.is_empty();
            Ok((commit, dirty, files))
        }
    }
}

/// Read blobs through one `git cat-file --batch`.
fn cat_blobs<'a>(
    root: &Path,
    oids: impl Iterator<Item = &'a str>,
) -> Result<Vec<Option<String>>, String> {
    let request: String = oids.map(|oid| format!("{oid}\n")).collect();
    let count = request.lines().count();
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("git could not start: {error}"))?;
    let mut stdin = child.stdin.take().ok_or("git cat-file has no stdin")?;
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(request.as_bytes());
    });
    let mut out = Vec::new();
    child
        .stdout
        .take()
        .ok_or("git cat-file has no stdout")?
        .read_to_end(&mut out)
        .map_err(|error| error.to_string())?;
    let _ = writer.join();
    let _ = child.wait();
    let mut blobs = Vec::with_capacity(count);
    let mut at = 0;
    for _ in 0..count {
        let end = out[at..]
            .iter()
            .position(|b| *b == b'\n')
            .ok_or("git cat-file ended early")?
            + at;
        let header = String::from_utf8_lossy(&out[at..end]).into_owned();
        at = end + 1;
        let mut parts = header.split(' ');
        let (_, kind, size) = (parts.next(), parts.next(), parts.next());
        let size: usize = match (kind, size) {
            (Some("blob"), Some(size)) => size.parse().map_err(|_| "bad blob size")?,
            _ => {
                blobs.push(None);
                continue;
            }
        };
        let body = out.get(at..at + size).ok_or("git cat-file ended early")?;
        at += size + 1;
        blobs.push(if size > MAX_FILE_BYTES {
            None
        } else {
            String::from_utf8(body.to_vec()).ok()
        });
    }
    Ok(blobs)
}

/// The crate a manifest names: `[lib] name`, else `[package] name`, with
/// `-` as `_`.
fn crate_name(manifest: &str) -> Option<String> {
    let mut section = "";
    let mut package = None;
    let mut lib = None;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line;
            continue;
        }
        if let Some(rest) = line.strip_prefix("name")
            && let Some(value) = rest.trim_start().strip_prefix('=')
        {
            let value = value.trim().trim_matches('"').to_owned();
            match section {
                "[package]" => package = Some(value),
                "[lib]" => lib = Some(value),
                _ => {}
            }
        }
    }
    lib.or(package).map(|name| name.replace('-', "_"))
}

fn ident(segment: &str) -> String {
    segment.replace(['-', '.', ' '], "_")
}

/// The module path of a file: its crate, then its modules.
fn module_path(path: &str, crates: &BTreeMap<String, String>) -> Vec<String> {
    let mut dir = Path::new(path).parent();
    while let Some(d) = dir {
        let key = d.to_string_lossy().into_owned();
        if let Some(name) = crates.get(&key) {
            let rel = if key.is_empty() {
                path
            } else {
                &path[key.len() + 1..]
            };
            let mut out = vec![name.clone()];
            let rel = rel.strip_suffix(".rs").unwrap_or(rel);
            let (in_src, rel) = match rel.strip_prefix("src/") {
                Some(rest) => (true, rest),
                None => (false, rel),
            };
            let mut parts: Vec<String> = rel.split('/').map(ident).collect();
            if in_src && parts.len() == 1 && matches!(parts[0].as_str(), "lib" | "main") {
                parts.clear();
            }
            if parts.len() > 1 && parts.last().is_some_and(|last| last == "mod") {
                parts.pop();
            }
            out.extend(parts);
            return out;
        }
        dir = d.parent();
    }
    let rel = path.strip_suffix(".rs").unwrap_or(path);
    let mut out = vec!["_".to_owned()];
    out.extend(rel.split('/').map(ident));
    out
}

struct FileOut {
    path: String,
    module: Vec<String>,
    facts: Facts,
}

/// Build the graph.
pub fn build(source: &Source) -> Result<Built, String> {
    let started = Instant::now();
    let (commit, dirty, files) = read_files(source)?;
    let mut crates = BTreeMap::new();
    for (path, text) in &files {
        if is_manifest(path)
            && let Some(name) = crate_name(text)
        {
            let dir = Path::new(path)
                .parent()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default();
            crates.insert(dir, name);
        }
    }
    let code: Vec<(&String, &String)> = files
        .iter()
        .filter(|(path, text)| is_code(path) && !generated(text))
        .collect();
    let source_bytes = code.iter().map(|(_, text)| text.len()).sum();

    // Parse in parallel; the order is restored by path afterwards.
    let threads = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(1, 16);
    let chunk = code.len().div_ceil(threads).max(1);
    let mut parsed: Vec<FileOut> = std::thread::scope(|scope| {
        let handles: Vec<_> = code
            .chunks(chunk)
            .map(|part| {
                let crates = &crates;
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn_scoped(scope, move || {
                        part.iter()
                            .map(|(path, text)| FileOut {
                                path: (*path).clone(),
                                module: module_path(path, crates),
                                facts: extract::extract(text).unwrap_or_default(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .expect("a parser thread starts")
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a parser thread finishes"))
            .collect()
    });
    parsed.sort_by(|a, b| a.path.cmp(&b.path));
    let files_with_parse_errors = parsed.iter().filter(|f| f.facts.parse_errors).count();
    let graph = assemble(commit, dirty, &crates, &parsed);
    Ok(Built {
        graph,
        millis: started.elapsed().as_millis(),
        source_bytes,
        files_with_parse_errors,
    })
}

/// A node before ids are assigned.
struct Pending {
    node: Node,
    start_byte: u32,
    /// The node's module, as a qualified name (an inline module's own
    /// path for itself).
    module: String,
    /// The `Self` type's qualified name for items inside an impl or trait.
    self_qual: Option<String>,
}

fn join(parts: &[String]) -> String {
    parts.join("::")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    Callable,
    Method,
    Type,
    Value,
    Macro,
    Import,
}

impl Want {
    fn accepts(self, kind: Kind) -> bool {
        match self {
            Want::Callable => kind.is_callable(),
            Want::Method => kind == Kind::Method,
            Want::Type => kind.is_type(),
            Want::Value => !matches!(kind, Kind::Module | Kind::Impl | Kind::Macro),
            Want::Macro => kind == Kind::Macro,
            Want::Import => kind != Kind::Impl,
        }
    }
}

struct Resolver<'a> {
    nodes: &'a [Node],
    by_qual: HashMap<&'a str, Vec<u32>>,
    by_name: HashMap<&'a str, Vec<u32>>,
    crates: BTreeSet<String>,
    /// Per (file, module qual): alias to absolute path, and glob prefixes.
    imports: HashMap<(u32, String), HashMap<String, Vec<String>>>,
    globs: HashMap<(u32, String), Vec<Vec<String>>>,
}

struct Scope<'a> {
    file: u32,
    module: &'a str,
    crate_name: &'a str,
    self_qual: Option<&'a str>,
}

impl<'a> Resolver<'a> {
    fn find(&self, qual: &str, want: Want) -> Vec<u32> {
        self.by_qual
            .get(qual)
            .map(|ids| {
                ids.iter()
                    .copied()
                    .filter(|id| want.accepts(self.nodes[*id as usize].kind))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Make a written path absolute, by the module's imports and the
    /// `crate`, `self`, `super` and `Self` keywords. `None` when its first
    /// segment is unknown (a std path or a local name).
    fn absolute(&self, path: &[String], scope: &Scope<'_>) -> Option<Vec<String>> {
        let first = path.first()?;
        let module: Vec<String> = scope.module.split("::").map(str::to_owned).collect();
        let mut out = match first.as_str() {
            "crate" => vec![scope.crate_name.to_owned()],
            "self" => module.clone(),
            "super" => {
                let mut base = module.clone();
                let supers = path.iter().take_while(|s| *s == "super").count();
                for _ in 0..supers {
                    base.pop();
                }
                base.extend(path[supers..].iter().cloned());
                return Some(base);
            }
            "Self" => scope
                .self_qual?
                .split("::")
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            _ => {
                if let Some(target) = self
                    .imports
                    .get(&(scope.file, scope.module.to_owned()))
                    .and_then(|aliases| aliases.get(first))
                {
                    target.clone()
                } else if self.crates.contains(first) {
                    vec![first.clone()]
                } else {
                    return None;
                }
            }
        };
        out.extend(path[1..].iter().cloned());
        Some(out)
    }

    /// The targets of a written path, and whether the match is exact.
    fn resolve(&self, path: &[String], want: Want, scope: &Scope<'_>) -> (Vec<u32>, bool) {
        let name = path.last().map(String::as_str).unwrap_or_default();
        if want == Want::Method {
            if let Some(self_qual) = scope.self_qual {
                let found = self.find(&format!("{self_qual}::{name}"), want);
                if !found.is_empty() {
                    return (found, true);
                }
            }
            return self.by_name_only(name, want, scope, false);
        }
        if want == Want::Macro {
            return self.by_name_only(name, want, scope, true);
        }
        if let Some(abs) = self.absolute(path, scope) {
            let found = self.find(&join(&abs), want);
            if !found.is_empty() {
                return (found, true);
            }
            // `Enum::Variant`, `Type::assoc` from a trait or derive: the
            // type itself is what is named.
            if abs.len() >= 2 && want != Want::Type {
                let owner = self.find(&join(&abs[..abs.len() - 1]), Want::Type);
                if !owner.is_empty() {
                    return (owner, true);
                }
            }
            return (Vec::new(), false);
        }
        // A name defined in this module, or reached through a glob import.
        let local = format!("{}::{}", scope.module, join(path));
        let found = self.find(&local, want);
        if !found.is_empty() {
            return (found, true);
        }
        if path.len() >= 2 && want != Want::Type {
            let owner = self.find(
                &format!("{}::{}", scope.module, join(&path[..path.len() - 1])),
                Want::Type,
            );
            if !owner.is_empty() {
                return (owner, true);
            }
        }
        if let Some(globs) = self.globs.get(&(scope.file, scope.module.to_owned())) {
            for glob in globs {
                let mut abs = self.absolute(glob, scope).unwrap_or_else(|| glob.clone());
                abs.extend(path.iter().cloned());
                let found = self.find(&join(&abs), want);
                if !found.is_empty() {
                    return (found, true);
                }
            }
        }
        if path.len() == 1 {
            // Not in scope by name: a std item, or one this builder cannot
            // see (macro-made, re-exported). Guess only inside the crate.
            return self.by_name_only(name, want, scope, true);
        }
        // `Type::method` with `Type` imported in a way not seen: match the
        // last two segments inside the crate.
        let tail = format!("::{}", join(&path[path.len() - 2..]));
        let candidates: Vec<u32> = self
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|id| {
                let node = &self.nodes[*id as usize];
                want.accepts(node.kind)
                    && node.qual.ends_with(&tail)
                    && node.qual.starts_with(&format!("{}::", scope.crate_name))
            })
            .collect();
        if (1..=MAX_CANDIDATES).contains(&candidates.len()) {
            return (candidates, false);
        }
        (Vec::new(), false)
    }

    fn by_name_only(
        &self,
        name: &str,
        want: Want,
        scope: &Scope<'_>,
        same_crate: bool,
    ) -> (Vec<u32>, bool) {
        let prefix = format!("{}::", scope.crate_name);
        let all: Vec<u32> = self
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|id| want.accepts(self.nodes[*id as usize].kind))
            .collect();
        let local: Vec<u32> = all
            .iter()
            .copied()
            .filter(|id| self.nodes[*id as usize].qual.starts_with(&prefix))
            .collect();
        let pool = if !local.is_empty() || same_crate {
            local
        } else {
            all
        };
        if (1..=MAX_CANDIDATES).contains(&pool.len()) {
            (pool, false)
        } else {
            (Vec::new(), false)
        }
    }
}

/// Turn parsed files into a sorted graph.
fn assemble(
    commit: String,
    dirty: bool,
    crates: &BTreeMap<String, String>,
    parsed: &[FileOut],
) -> Graph {
    let files: Vec<String> = parsed.iter().map(|f| f.path.clone()).collect();
    let mut pending: Vec<Pending> = Vec::new();
    // Per file: the index in `pending` of the file's module and each decl.
    let mut file_module: Vec<usize> = Vec::with_capacity(parsed.len());
    let mut decl_index: Vec<Vec<usize>> = Vec::with_capacity(parsed.len());
    for (file_id, file) in parsed.iter().enumerate() {
        let file_id = u32::try_from(file_id).unwrap_or(u32::MAX);
        let module_qual = join(&file.module);
        file_module.push(pending.len());
        pending.push(Pending {
            node: Node {
                kind: Kind::Module,
                name: file.module.last().cloned().unwrap_or_default(),
                qual: module_qual.clone(),
                file: file_id,
                line: 1,
                end_line: file.facts.lines.max(1),
                public: true,
                parent: None,
                signature: String::new(),
            },
            start_byte: 0,
            module: module_qual.clone(),
            self_qual: None,
        });
        let mut ids = Vec::with_capacity(file.facts.decls.len());
        for decl in &file.facts.decls {
            let mut qual = file.module.clone();
            qual.extend(decl.scope.iter().cloned());
            qual.push(decl.name.clone());
            let module = {
                let mut m = file.module.clone();
                m.extend(decl.module.iter().cloned());
                if decl.kind == Kind::Module {
                    m.push(decl.name.clone());
                }
                join(&m)
            };
            let self_qual = decl.self_type.last().map(|ty| {
                let mut q = file.module.clone();
                // The type is named where the impl is: its scope minus
                // the type segment the impl pushed.
                q.extend(decl.scope.iter().cloned());
                if q.last() == Some(ty) {
                    join(&q)
                } else {
                    q.push(ty.clone());
                    join(&q)
                }
            });
            ids.push(pending.len());
            pending.push(Pending {
                node: Node {
                    kind: decl.kind,
                    name: decl.name.clone(),
                    qual: join(&qual),
                    file: file_id,
                    line: decl.line,
                    end_line: decl.end_line,
                    public: decl.public,
                    parent: None,
                    signature: decl.signature.clone(),
                },
                start_byte: decl.start_byte,
                module,
                self_qual,
            });
        }
        decl_index.push(ids);
    }
    // Parents: a decl's enclosing decl, else its file's module; a file's
    // module's parent is the module one level up, when a file holds it.
    let mut module_by_qual: HashMap<String, usize> = HashMap::new();
    for &at in &file_module {
        module_by_qual
            .entry(pending[at].node.qual.clone())
            .or_insert(at);
    }
    let mut parents: Vec<Option<usize>> = vec![None; pending.len()];
    for (file_id, file) in parsed.iter().enumerate() {
        let at = file_module[file_id];
        let qual = &pending[at].node.qual;
        if let Some((up, _)) = qual.rsplit_once("::") {
            parents[at] = module_by_qual.get(up).copied();
        }
        for (local, decl) in file.facts.decls.iter().enumerate() {
            parents[decl_index[file_id][local]] = Some(match decl.parent {
                Some(parent) => decl_index[file_id][parent],
                None => at,
            });
        }
    }
    // Sort and number: by file, then position, then kind and name.
    let mut order: Vec<usize> = (0..pending.len()).collect();
    order.sort_by(|a, b| {
        let (x, y) = (&pending[*a], &pending[*b]);
        (
            x.node.file,
            x.start_byte,
            x.node.kind,
            &x.node.qual,
            x.node.line,
        )
            .cmp(&(
                y.node.file,
                y.start_byte,
                y.node.kind,
                &y.node.qual,
                y.node.line,
            ))
    });
    let mut id_of = vec![0u32; pending.len()];
    for (new, old) in order.iter().enumerate() {
        id_of[*old] = u32::try_from(new).unwrap_or(u32::MAX);
    }
    let nodes: Vec<Node> = order
        .iter()
        .map(|old| {
            let mut node = pending[*old].node.clone();
            node.parent = parents[*old].map(|p| id_of[p]);
            node
        })
        .collect();

    let mut resolver = Resolver {
        nodes: &nodes,
        by_qual: HashMap::new(),
        by_name: HashMap::new(),
        crates: crates.values().cloned().collect(),
        imports: HashMap::new(),
        globs: HashMap::new(),
    };
    for (id, node) in nodes.iter().enumerate() {
        let id = u32::try_from(id).unwrap_or(u32::MAX);
        if node.kind != Kind::Impl {
            resolver.by_qual.entry(&node.qual).or_default().push(id);
            resolver.by_name.entry(&node.name).or_default().push(id);
        }
    }
    // Imports, keyed by the module they are written in. The alias maps to
    // the written path made absolute where it can be.
    for (file_id, file) in parsed.iter().enumerate() {
        let file_id = u32::try_from(file_id).unwrap_or(u32::MAX);
        for import in &file.facts.uses {
            let mut module = file.module.clone();
            module.extend(import.module.iter().cloned());
            let module = join(&module);
            let scope = Scope {
                file: file_id,
                module: &module,
                crate_name: &file.module[0],
                self_qual: None,
            };
            let abs = match import.path.first().map(String::as_str) {
                Some("crate" | "self" | "super") => resolver.absolute(&import.path, &scope),
                _ => None,
            }
            .unwrap_or_else(|| import.path.clone());
            if import.alias == "*" {
                resolver
                    .globs
                    .entry((file_id, module.clone()))
                    .or_default()
                    .push(abs);
            } else {
                resolver
                    .imports
                    .entry((file_id, module.clone()))
                    .or_default()
                    .insert(import.alias.clone(), abs);
            }
        }
    }
    // Imports of one file may name each other (`use a::b; use b::c;`):
    // make the targets absolute once more.
    let keys: Vec<(u32, String)> = resolver.imports.keys().cloned().collect();
    for key in keys {
        let aliases = resolver.imports[&key].clone();
        let mut updated = aliases.clone();
        for (alias, path) in &aliases {
            if let Some(first) = path.first()
                && first != alias
                && let Some(target) = aliases.get(first)
                && target.first() != Some(first)
            {
                let mut abs = target.clone();
                abs.extend(path[1..].iter().cloned());
                updated.insert(alias.clone(), abs);
            }
        }
        resolver.imports.insert(key, updated);
    }

    let mut edges: BTreeSet<Edge> = BTreeSet::new();
    let add = |edges: &mut BTreeSet<Edge>, from: u32, to: u32, kind: EdgeKind, exact: bool| {
        if from != to {
            edges.insert(Edge {
                from,
                to,
                kind,
                exact,
            });
        }
    };
    for (file_id, file) in parsed.iter().enumerate() {
        let file_u32 = u32::try_from(file_id).unwrap_or(u32::MAX);
        let crate_name = &file.module[0];
        let module_node = file_module[file_id];
        let owner = |local: Option<usize>| match local {
            Some(local) => decl_index[file_id][local],
            None => module_node,
        };
        for reference in &file.facts.refs {
            let from_pending = owner(reference.from);
            let from = id_of[from_pending];
            let module = &pending[from_pending].module;
            let scope = Scope {
                file: file_u32,
                module,
                crate_name,
                self_qual: pending[from_pending].self_qual.as_deref(),
            };
            let want = match reference.kind {
                RefKind::Call => Want::Callable,
                RefKind::Method { .. } => Want::Method,
                RefKind::Type => Want::Type,
                RefKind::Value => Want::Value,
                RefKind::Macro => Want::Macro,
            };
            let scope = if let RefKind::Method { on_self: false } = reference.kind {
                Scope {
                    self_qual: None,
                    ..scope
                }
            } else {
                scope
            };
            let (targets, exact) = resolver.resolve(&reference.path, want, &scope);
            for to in targets {
                let kind = match nodes[to as usize].kind {
                    Kind::Function | Kind::Method | Kind::Macro => match reference.kind {
                        RefKind::Value | RefKind::Type => EdgeKind::References,
                        _ => EdgeKind::Calls,
                    },
                    _ => EdgeKind::References,
                };
                add(&mut edges, from, to, kind, exact);
            }
        }
        for import in &file.facts.uses {
            if import.alias == "*" {
                continue;
            }
            let mut module = file.module.clone();
            module.extend(import.module.iter().cloned());
            let module = join(&module);
            let from = id_of[owner(import.from)];
            let Some(abs) = resolver
                .imports
                .get(&(file_u32, module.clone()))
                .and_then(|aliases| aliases.get(&import.alias))
            else {
                continue;
            };
            for to in resolver.find(&join(abs), Want::Import) {
                add(&mut edges, from, to, EdgeKind::Uses, true);
            }
        }
        for (local, decl) in file.facts.decls.iter().enumerate() {
            if decl.kind != Kind::Impl {
                continue;
            }
            let at = decl_index[file_id][local];
            let from = id_of[at];
            let mut module = file.module.clone();
            module.extend(decl.module.iter().cloned());
            let module = join(&module);
            let scope = Scope {
                file: file_u32,
                module: &module,
                crate_name,
                self_qual: None,
            };
            for (path, kind) in [
                (&decl.impl_type, EdgeKind::ImplFor),
                (&decl.impl_trait, EdgeKind::Implements),
            ] {
                if path.is_empty() {
                    continue;
                }
                let (targets, exact) = resolver.resolve(path, Want::Type, &scope);
                for to in targets {
                    add(&mut edges, from, to, kind, exact);
                }
            }
        }
    }
    let edges: Vec<Edge> = edges.into_iter().collect();
    let header = Header {
        schema: SCHEMA.into(),
        builder: BUILDER.into(),
        parser: PARSER.into(),
        grammar: GRAMMAR.into(),
        commit,
        dirty,
        files: files.len(),
        nodes: nodes.len(),
        edges: edges.len(),
    };
    Graph {
        header,
        files,
        nodes,
        edges,
    }
}
