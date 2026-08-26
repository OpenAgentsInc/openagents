//! The WebAssembly capability host: the sandbox, the catalog, and the
//! `capability` tool that reaches them.
//!
//! This is the Rust half of OpenAgentsInc/openagents#71 and #84 that did not
//! exist: `crates/openagents-cli` had no WebAssembly engine, so the
//! `capability` tool could not be implemented and — after one round of
//! advertising a tool that was never in `list_tools()` — was not declared
//! either. The artifacts in `plugins/` are wasm modules against the bespoke
//! `packet-v0` ABI owned by `plugins/pdk`, with per-manifest read-only
//! directory mounts, host allowlists, memory ceilings, and timeouts. A host
//! that cannot enforce those refuses to run the plugin; it does not run it
//! unsandboxed.
//!
//! The contract, which is the same one `packages/openagents-cli/src/
//! coder-plugins.ts` states and this module ports:
//!
//! - **Manifest first.** Identity, an artifact digest pin, the `packet-v0`
//!   ABI declaration, typed input and output schemas, and capability
//!   declarations. Absence of a capability is denial, never a default grant.
//! - **Digest before load.** The artifact's SHA-256 is compared against the
//!   manifest's pin before the module is compiled. A mismatch is a refusal,
//!   not a warning.
//! - **Imports must be declared.** The compiled module's import list is read
//!   before anything is instantiated and must be covered by what the manifest
//!   declares: nothing at all for pure compute, and exactly
//!   `openagents.read_file`, `openagents.read_file_range`, and
//!   `openagents.list_dir` when the manifest declares read-only mounts.
//!   Anything else — a write import above all — is refused by inspection, so
//!   the sandbox is a property of what was loaded rather than a hope about
//!   what it does.
//! - **Mounts are read-only and confined.** A declared mount resolves to a
//!   real directory at load; at invocation every path is confined to it:
//!   absolute paths refused, `..` resolved and checked, symlinks refused,
//!   the canonical path re-checked against the canonical root so a symlinked
//!   parent cannot smuggle a read out, a byte bound per file, an entry bound
//!   per listing.
//! - **Limits are enforced, not declared.** The manifest's `timeout_ms`
//!   becomes a wasmtime epoch deadline with a watchdog that fires it, so a
//!   guest that never returns is trapped rather than waited on. Its
//!   `memory_max_mib` becomes a `StoreLimits` ceiling, so a guest that grows
//!   past it is denied the pages. Both are testable by violating them, and
//!   `tests/plugin_host_test.rs` violates them.
//! - **Typed refusals both ways.** The host refuses with `{code, reason}`;
//!   the guest returns `{"refusal": {...}}` inside its output packet. Both
//!   read as text to a model, which can act on a refusal and cannot act on a
//!   turn that died.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use wasmtime::{Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// The one packet ABI this host speaks. A manifest must declare it.
pub const SUPPORTED_ABI: &str = "packet-v0";

/// Ceiling on a manifest's own timeout, so a manifest cannot ask for an hour.
pub const TIMEOUT_CEILING_MS: u64 = 30_000;

/// Default memory ceiling when a manifest names none.
pub const DEFAULT_MEMORY_MIB: u64 = 64;

/// Ceiling on a manifest's own memory request.
pub const MEMORY_CEILING_MIB: u64 = 512;

/// Per-file byte bound for reads through a mount.
pub const MOUNT_FILE_LIMIT: u64 = 1_048_576;

/// Entry bound per directory listing through a mount; the rest is truncated.
pub const MOUNT_DIR_ENTRY_LIMIT: usize = 500;

/// How much plugin output the model is shown.
pub const PLUGIN_OUTPUT_LIMIT: usize = 16_000;

/// Most candidates one catalog search returns; the rest are counted.
const SEARCH_LIMIT: usize = 5;

/// Why the host would not do what was asked. Returned, never panicked.
///
/// The code set is the one the guest PDK and the TypeScript host already
/// share, so a refusal reads the same whichever side of the boundary it was
/// born on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub reason: String,
}

impl Refusal {
    pub fn new(code: &'static str, reason: impl Into<String>) -> Self {
        Refusal {
            code,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}): {}", self.code, self.reason)
    }
}

fn refuse(code: &'static str, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, reason)
}

/// A read-only directory grant, as the manifest declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mount {
    /// The declared path. Relative paths resolve against the manifest's own
    /// directory, absolute paths are taken as they are, a leading `~/` (or a
    /// bare `~`) expands to the invoking user's home, and the literal
    /// `${workspace}` resolves to the session's working directory.
    pub path: String,
}

/// The manifest fields this host reads. The file may carry more.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub artifact_path: String,
    pub artifact_digest: String,
    pub abi_entry: String,
    pub abi_alloc: String,
    pub input_schema: serde_json::Value,
    pub mounts: Vec<Mount>,
    pub hosts: Vec<serde_json::Value>,
    pub timeout_ms: u64,
    pub memory_max_mib: u64,
}

/// A plugin that passed every check and is ready to invoke.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: Manifest,
    /// The artifact bytes, held so an invocation cannot race a file rewrite.
    pub wasm: Vec<u8>,
    /// The verified digest, `sha256:<hex>`.
    pub digest: String,
    /// Declared mounts, resolved to canonical absolute directory roots.
    pub mounts: Vec<PathBuf>,
    /// Where the manifest was read from, for provenance.
    pub manifest_path: PathBuf,
}

// ───────────────────────────────────────────────────── manifest validation

fn bad(what: &str) -> Refusal {
    refuse(
        "manifest_invalid",
        format!("the manifest is missing or mistypes {what}"),
    )
}

fn is_plugin_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    name.len() <= 64 && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Read a manifest value into the fields this host enforces.
///
/// Every failure names the field, because the alternative is an operator
/// staring at "invalid manifest" with thirteen plugins installed.
pub fn validate_manifest(value: &serde_json::Value) -> Result<Manifest, Refusal> {
    let record = value
        .as_object()
        .ok_or_else(|| bad("the top-level object"))?;

    let name = record.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if !is_plugin_name(name) {
        return Err(bad(
            "`name` (lowercase identifier, it becomes the tool name)",
        ));
    }
    // The manifest name *is* the tool name, and the session's own tools are
    // dispatched before any plugin. A plugin under one of those names would be
    // declared to the model and then permanently unreachable, which is worse
    // than one that failed to install: the model is told a capability exists
    // and every call to it lands on the builtin. Refused here, so it reaches
    // neither the catalog nor a load.
    if crate::tools::BUILTIN_TOOL_NAMES.contains(&name) {
        return Err(refuse(
            "name_reserved",
            format!(
                "the manifest is named `{name}`, which is a built-in tool of this session. \
                 The builtin answers first, so the plugin could never run. Rename it; \
                 the reserved names are {}",
                crate::tools::BUILTIN_TOOL_NAMES.join(", ")
            ),
        ));
    }
    let version = record.get("version").and_then(|v| v.as_str()).unwrap_or("");
    if version.is_empty() {
        return Err(bad("`version`"));
    }
    let description = record
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if description.is_empty() {
        return Err(bad("`description`"));
    }

    let artifact = record.get("artifact").and_then(|v| v.as_object());
    let artifact_path = artifact
        .and_then(|a| a.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let artifact_digest = artifact
        .and_then(|a| a.get("digest"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if artifact_path.is_empty() || !artifact_digest.starts_with("sha256:") {
        return Err(bad("`artifact` (`path` and a `sha256:` `digest`)"));
    }

    let abi = record.get("abi").and_then(|v| v.as_object());
    let abi_kind = abi
        .and_then(|a| a.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let abi_entry = abi
        .and_then(|a| a.get("entry"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let abi_alloc = abi
        .and_then(|a| a.get("alloc"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if abi_kind.is_empty() || abi_entry.is_empty() || abi_alloc.is_empty() {
        return Err(bad("`abi` (`kind`, `entry`, and `alloc`)"));
    }
    if abi_kind != SUPPORTED_ABI {
        return Err(refuse(
            "abi_unsupported",
            format!("the manifest declares abi `{abi_kind}` and this host speaks `{SUPPORTED_ABI}` only"),
        ));
    }

    let iface = record.get("interface").and_then(|v| v.as_object());
    let input_schema = iface
        .and_then(|i| i.get("input"))
        .filter(|v| v.is_object())
        .cloned();
    let has_output = iface
        .and_then(|i| i.get("output"))
        .is_some_and(|v| v.is_object());
    let Some(input_schema) = input_schema else {
        return Err(bad("`interface` (`input` and `output` JSON schemas)"));
    };
    if !has_output {
        return Err(bad("`interface` (`input` and `output` JSON schemas)"));
    }

    let capabilities = record.get("capabilities").and_then(|v| v.as_object());
    let declared_mounts = capabilities
        .and_then(|c| c.get("mounts"))
        .and_then(|v| v.as_array());
    let hosts = capabilities
        .and_then(|c| c.get("hosts"))
        .and_then(|v| v.as_array());
    let timeout_ms = capabilities
        .and_then(|c| c.get("timeout_ms"))
        .and_then(|v| v.as_u64());
    let (Some(declared_mounts), Some(hosts), Some(timeout_ms)) =
        (declared_mounts, hosts, timeout_ms)
    else {
        return Err(bad(
            "`capabilities` (`mounts`, `hosts`, positive `timeout_ms`)",
        ));
    };
    if timeout_ms == 0 {
        return Err(bad(
            "`capabilities` (`mounts`, `hosts`, positive `timeout_ms`)",
        ));
    }

    let mut mounts = Vec::new();
    for entry in declared_mounts {
        let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return Err(bad("`capabilities.mounts[]` (each mount needs a `path`)"));
        }
        // A writable mount is a capability this host does not have. Refusing
        // it here is what keeps "declared means enforced" honest: the
        // alternative is quietly downgrading the grant and letting a manifest
        // claim something the sandbox never gave it.
        if entry.get("readonly").and_then(|v| v.as_bool()) != Some(true) {
            return Err(refuse(
                "capabilities_unsupported",
                format!("mount `{path}` is not marked `\"readonly\": true`; only read-only mounts exist"),
            ));
        }
        mounts.push(Mount {
            path: path.to_string(),
        });
    }

    let memory_max_mib = capabilities
        .and_then(|c| c.get("memory_max_mib"))
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_MEMORY_MIB)
        .clamp(1, MEMORY_CEILING_MIB);

    Ok(Manifest {
        name: name.to_string(),
        version: version.to_string(),
        description: description.to_string(),
        artifact_path: artifact_path.to_string(),
        artifact_digest: artifact_digest.to_string(),
        abi_entry: abi_entry.to_string(),
        abi_alloc: abi_alloc.to_string(),
        input_schema,
        mounts,
        hosts: hosts.clone(),
        timeout_ms: timeout_ms.min(TIMEOUT_CEILING_MS),
        memory_max_mib,
    })
}

// ─────────────────────────────────────────────────────────── mount resolution

/// Expand a `~` prefix. `~alice/...` is somebody else's home and stays
/// literal, so it then fails the exists-and-is-a-directory check rather than
/// silently reading another account.
fn expand_home(path: &str) -> String {
    let Ok(home) = std::env::var("HOME") else {
        return path.to_string();
    };
    if path == "~" {
        return home;
    }
    match path.strip_prefix("~/") {
        Some(rest) => Path::new(&home).join(rest).to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

fn resolve_mount(mount: &Mount, manifest_dir: &Path, workspace: &Path) -> Result<PathBuf, Refusal> {
    let expanded = if mount.path == "${workspace}" {
        workspace.to_string_lossy().into_owned()
    } else {
        expand_home(&mount.path)
    };
    let declared = manifest_dir.join(&expanded);
    let root = std::fs::canonicalize(&declared).map_err(|_| {
        refuse(
            "mount_invalid",
            format!(
                "mount `{}` does not resolve to a readable directory",
                mount.path
            ),
        )
    })?;
    if !root.is_dir() {
        return Err(refuse(
            "mount_invalid",
            format!("mount `{}` is not a directory", mount.path),
        ));
    }
    Ok(root)
}

// ───────────────────────────────────────────────────────────────── loading

/// Load a plugin from its manifest: parse, validate, verify the digest, and
/// prove by inspection that the module's imports are covered by its declared
/// capabilities.
///
/// Everything checkable before the first invocation is checked here, so the
/// caller either learns exactly what is wrong or holds a plugin whose next
/// failure can only be about the packet.
pub fn load_plugin(manifest_path: &Path, workspace: &Path) -> Result<LoadedPlugin, Refusal> {
    let raw = std::fs::read_to_string(manifest_path).map_err(|e| {
        refuse(
            "manifest_unreadable",
            format!("{}: {e}", manifest_path.display()),
        )
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|_| {
        refuse(
            "manifest_invalid",
            format!("{} is not JSON", manifest_path.display()),
        )
    })?;
    let manifest = validate_manifest(&parsed)?;

    // The only host capability that exists is the read-only mount. A declared
    // network host is declared-but-denied, never declared-and-ignored.
    if !manifest.hosts.is_empty() {
        return Err(refuse(
            "capabilities_unsupported",
            "the manifest declares network hosts, and this host has no network capability to grant",
        ));
    }

    let manifest_dir = manifest_path.parent().unwrap_or(Path::new("."));
    let mut mounts = Vec::new();
    for mount in &manifest.mounts {
        mounts.push(resolve_mount(mount, manifest_dir, workspace)?);
    }

    let artifact_path = manifest_dir.join(&manifest.artifact_path);
    let wasm = std::fs::read(&artifact_path).map_err(|e| {
        refuse(
            "artifact_unreadable",
            format!("{}: {e}", artifact_path.display()),
        )
    })?;

    let digest = format!("sha256:{:x}", Sha256::digest(&wasm));
    if digest != manifest.artifact_digest {
        return Err(refuse(
            "digest_mismatch",
            format!(
                "the manifest pins {} but {} is {digest}; the artifact is not the one the manifest \
                 describes, so it does not load",
                manifest.artifact_digest, manifest.artifact_path
            ),
        ));
    }

    let shape = inspect_module(&wasm)?;

    // Every import must be granted by a declared capability. Mounts grant
    // exactly three, all of them reads.
    let granted: BTreeSet<&str> = if mounts.is_empty() {
        BTreeSet::new()
    } else {
        [
            "openagents.read_file",
            "openagents.read_file_range",
            "openagents.list_dir",
        ]
        .into_iter()
        .collect()
    };
    let undeclared: Vec<&str> = shape
        .imports
        .iter()
        .map(String::as_str)
        .filter(|name| !granted.contains(name))
        .collect();
    if !undeclared.is_empty() {
        let hint = if mounts.is_empty() {
            "the manifest declares no capabilities, so the module may import nothing"
        } else {
            "the declared mounts grant only `openagents.read_file`, `openagents.read_file_range`, \
             and `openagents.list_dir`, all of them reads"
        };
        return Err(refuse(
            "imports_undeclared",
            format!(
                "the module asks for host imports its manifest does not declare ({}); {hint}",
                undeclared.join(", ")
            ),
        ));
    }

    for name in [
        manifest.abi_entry.as_str(),
        manifest.abi_alloc.as_str(),
        "memory",
    ] {
        if !shape.exports.iter().any(|export| export == name) {
            return Err(refuse(
                "exports_missing",
                format!("the module does not export `{name}`"),
            ));
        }
    }

    Ok(LoadedPlugin {
        manifest,
        wasm,
        digest,
        mounts,
        manifest_path: manifest_path.to_path_buf(),
    })
}

/// What a compiled module declares, before anything is instantiated.
#[derive(Debug, Clone)]
pub struct ModuleShape {
    /// Import names as `module.name`, e.g. `openagents.read_file`.
    pub imports: Vec<String>,
    pub exports: Vec<String>,
}

fn engine_config() -> Config {
    let mut config = Config::new();
    // The only way to stop a guest that never returns. The watchdog in
    // `invoke` increments the epoch when the manifest's deadline passes and
    // the running instance traps.
    config.epoch_interruption(true);
    config
}

/// Compile the artifact and report what it asks for and what it offers.
pub fn inspect_module(wasm: &[u8]) -> Result<ModuleShape, Refusal> {
    let engine = Engine::new(&engine_config())
        .map_err(|e| refuse("not_wasm", format!("the wasm engine did not start: {e}")))?;
    let module = Module::new(&engine, wasm).map_err(|e| refuse("not_wasm", e.to_string()))?;
    Ok(ModuleShape {
        imports: module
            .imports()
            .map(|import| format!("{}.{}", import.module(), import.name()))
            .collect(),
        exports: module
            .exports()
            .map(|export| export.name().to_string())
            .collect(),
    })
}

// ────────────────────────────────────────────────── the sandbox and its host

/// What the guest's capability imports are answered from.
struct HostState {
    limits: StoreLimits,
    /// Canonical, absolute mount roots, in manifest order.
    mounts: Vec<PathBuf>,
    file_limit: u64,
    dir_entry_limit: usize,
    alloc: String,
}

fn ok_packet(bytes: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(bytes.len() + 1);
    packet.push(0);
    packet.extend_from_slice(bytes);
    packet
}

fn refusal_packet(code: &str, reason: &str) -> Vec<u8> {
    let body = serde_json::json!({ "code": code, "reason": reason });
    let mut packet = Vec::new();
    packet.push(1u8);
    packet.extend_from_slice(serde_json::to_string(&body).unwrap_or_default().as_bytes());
    packet
}

/// Resolve `rel` against `root` without touching the filesystem, applying
/// `..` lexically the way `path.resolve` does.
///
/// Lexical first, because the check that matters — "is this still inside the
/// root" — must be answerable for a path that does not exist yet, and because
/// resolving `..` against a real symlinked directory is how confinement gets
/// walked out of.
fn lexical_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for component in Path::new(rel).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(part) => out.push(part),
            // An absolute path or a Windows prefix inside a mount-relative
            // path is not a path in the mount at all.
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn within(candidate: &Path, root: &Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

/// The `openagents.read_file` and `openagents.read_file_range` answer.
///
/// `range` of `None` is a whole-file read, which refuses a file past the byte
/// bound. A range read has no such refusal: its answer is bounded by
/// construction, clamped to the same per-read bound, and a range past the end
/// answers with what remains, empty included.
fn read_mounted(state: &HostState, path: &str, range: Option<(u64, u32)>) -> Vec<u8> {
    if Path::new(path).is_absolute() {
        return refusal_packet(
            "mount_denied",
            "absolute paths are refused; mounted paths are relative to a declared mount root",
        );
    }
    for root in &state.mounts {
        let Some(candidate) = lexical_join(root, path) else {
            return refusal_packet("mount_denied", "the path escapes the mount root");
        };
        if !within(&candidate, root) {
            return refusal_packet("mount_denied", "the path escapes the mount root");
        }
        let Ok(meta) = std::fs::symlink_metadata(&candidate) else {
            continue; // Not in this mount; try the next declared root.
        };
        if meta.file_type().is_symlink() {
            return refusal_packet("mount_denied", "symlinks inside a mount are refused");
        }
        if !meta.is_file() {
            return refusal_packet("file_unreadable", "the path is not a regular file");
        }
        // A symlinked parent directory can still point outside, so the real
        // path of the candidate must sit under the real path of the root.
        let real = match std::fs::canonicalize(&candidate) {
            Ok(real) => real,
            Err(err) => return refusal_packet("file_unreadable", &err.to_string()),
        };
        if !within(&real, root) {
            return refusal_packet("mount_denied", "the path resolves outside the mount root");
        }
        let size = meta.len();
        let Some((offset, max_bytes)) = range else {
            if size > state.file_limit {
                return refusal_packet(
                    "file_too_large",
                    &format!(
                        "the file is {size} bytes; the per-file bound is {}",
                        state.file_limit
                    ),
                );
            }
            return match std::fs::read(&candidate) {
                Ok(bytes) => ok_packet(&bytes),
                Err(err) => refusal_packet("file_unreadable", &err.to_string()),
            };
        };
        let offset = offset.min(size);
        let length = u64::from(max_bytes)
            .min(state.file_limit)
            .min(size - offset);
        return match read_range(&candidate, offset, length) {
            Ok(bytes) => ok_packet(&bytes),
            Err(err) => refusal_packet("file_unreadable", &err.to_string()),
        };
    }
    refusal_packet("mount_denied", "no declared mount contains the path")
}

fn read_range(path: &Path, offset: u64, length: u64) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buffer = vec![0u8; length as usize];
    let mut filled = 0usize;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            got => filled += got,
        }
    }
    buffer.truncate(filled);
    Ok(buffer)
}

/// The `openagents.list_dir` answer.
///
/// The mount index makes the target root explicit. A scanner over two mounts
/// must never have a "which root answered?" ambiguity for a listing.
fn list_mounted(state: &HostState, mount_index: u32, path: &str) -> Vec<u8> {
    let Some(root) = state.mounts.get(mount_index as usize) else {
        return refusal_packet("mount_denied", "the mount index names no declared mount");
    };
    if Path::new(path).is_absolute() {
        return refusal_packet(
            "mount_denied",
            "absolute paths are refused; mounted paths are relative to a declared mount root",
        );
    }
    let Some(candidate) = lexical_join(root, path) else {
        return refusal_packet("mount_denied", "the path escapes the mount root");
    };
    if !within(&candidate, root) {
        return refusal_packet("mount_denied", "the path escapes the mount root");
    }
    let Ok(meta) = std::fs::symlink_metadata(&candidate) else {
        return refusal_packet("file_unreadable", "the mount has no such directory");
    };
    if meta.file_type().is_symlink() {
        return refusal_packet("mount_denied", "symlinks inside a mount are refused");
    }
    if !meta.is_dir() {
        return refusal_packet("file_unreadable", "the path is not a directory");
    }
    let real = match std::fs::canonicalize(&candidate) {
        Ok(real) => real,
        Err(err) => return refusal_packet("file_unreadable", &err.to_string()),
    };
    if !within(&real, root) {
        return refusal_packet("mount_denied", "the path resolves outside the mount root");
    }

    let entries = match std::fs::read_dir(&candidate) {
        Ok(entries) => entries,
        Err(err) => return refusal_packet("file_unreadable", &err.to_string()),
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let truncated = names.len() > state.dir_entry_limit;
    let mut listed = Vec::new();
    for name in names.into_iter().take(state.dir_entry_limit) {
        let (mut kind, mut size, mut mtime_ms) = ("other", 0u64, 0i64);
        if let Ok(entry_meta) = std::fs::symlink_metadata(candidate.join(&name)) {
            let file_type = entry_meta.file_type();
            kind = if file_type.is_symlink() {
                "symlink"
            } else if file_type.is_file() {
                "file"
            } else if file_type.is_dir() {
                "dir"
            } else {
                "other"
            };
            size = entry_meta.len();
            mtime_ms = entry_meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_millis() as i64);
        }
        listed.push(serde_json::json!({
            "name": name, "kind": kind, "size": size, "mtime_ms": mtime_ms
        }));
    }
    let body = serde_json::json!({ "entries": listed, "truncated": truncated });
    ok_packet(serde_json::to_string(&body).unwrap_or_default().as_bytes())
}

/// Read a guest string out of linear memory, or `None` when the range is not
/// inside it.
fn guest_string(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Option<String> {
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(&*caller);
    let start = usize::try_from(ptr).ok()?;
    let end = start.checked_add(usize::try_from(len).ok()?)?;
    let bytes = data.get(start..end)?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Write an answer packet into guest memory through the guest's own
/// allocator and pack its location the way `handle_packet` does.
///
/// A null return word is the PDK's "the host answered with a null packet",
/// which is a refusal the guest can act on. Nothing here traps the guest for
/// a host-side problem.
fn answer_guest(caller: &mut Caller<'_, HostState>, packet: &[u8]) -> i64 {
    let alloc_name = caller.data().alloc.clone();
    let Some(alloc) = caller.get_export(&alloc_name).and_then(|e| e.into_func()) else {
        return 0;
    };
    let Ok(alloc) = alloc.typed::<i32, i32>(&*caller) else {
        return 0;
    };
    let Ok(len) = i32::try_from(packet.len()) else {
        return 0;
    };
    let Ok(ptr) = alloc.call(&mut *caller, len) else {
        return 0;
    };
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return 0;
    };
    let Ok(offset) = usize::try_from(ptr) else {
        return 0;
    };
    if memory.write(&mut *caller, offset, packet).is_err() {
        return 0;
    }
    ((u64::from(ptr as u32) << 32) | packet.len() as u64) as i64
}

/// Call the plugin once: packet bytes in, packet bytes out, or a refusal.
///
/// Blocking. One engine, one store, one instance per invocation, so no state
/// survives from one call to the next and every invocation runs on memory the
/// previous one cannot have corrupted.
pub fn invoke(plugin: &LoadedPlugin, input: &[u8]) -> Result<Vec<u8>, Refusal> {
    let engine = Engine::new(&engine_config())
        .map_err(|e| refuse("not_wasm", format!("the wasm engine did not start: {e}")))?;
    let module =
        Module::new(&engine, &plugin.wasm).map_err(|e| refuse("not_wasm", e.to_string()))?;

    let memory_bytes = (plugin.manifest.memory_max_mib * 1024 * 1024) as usize;
    let state = HostState {
        limits: StoreLimitsBuilder::new().memory_size(memory_bytes).build(),
        mounts: plugin.mounts.clone(),
        file_limit: MOUNT_FILE_LIMIT,
        dir_entry_limit: MOUNT_DIR_ENTRY_LIMIT,
        alloc: plugin.manifest.abi_alloc.clone(),
    };
    let mut store = Store::new(&engine, state);
    store.limiter(|state| &mut state.limits);
    store.set_epoch_deadline(1);

    let mut linker: Linker<HostState> = Linker::new(&engine);
    // The capability imports exist only when the manifest declared mounts.
    // A module that asks for them without a mount was already refused at
    // load, and one that asks for them here finds nothing to link to.
    if !plugin.mounts.is_empty() {
        linker
            .func_wrap(
                "openagents",
                "read_file",
                |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> i64 {
                    let packet = match guest_string(&mut caller, ptr, len) {
                        Some(path) => read_mounted(caller.data(), &path, None),
                        None => refusal_packet(
                            "mount_denied",
                            "the path argument is not inside guest memory",
                        ),
                    };
                    answer_guest(&mut caller, &packet)
                },
            )
            .and_then(|linker| {
                linker.func_wrap(
                    "openagents",
                    "read_file_range",
                    |mut caller: Caller<'_, HostState>,
                     ptr: i32,
                     len: i32,
                     offset: i64,
                     max_bytes: i32|
                     -> i64 {
                        let packet = match guest_string(&mut caller, ptr, len) {
                            Some(path) => read_mounted(
                                caller.data(),
                                &path,
                                Some((offset as u64, max_bytes as u32)),
                            ),
                            None => refusal_packet(
                                "mount_denied",
                                "the path argument is not inside guest memory",
                            ),
                        };
                        answer_guest(&mut caller, &packet)
                    },
                )
            })
            .and_then(|linker| {
                linker.func_wrap(
                    "openagents",
                    "list_dir",
                    |mut caller: Caller<'_, HostState>,
                     mount_index: i32,
                     ptr: i32,
                     len: i32|
                     -> i64 {
                        let packet = match guest_string(&mut caller, ptr, len) {
                            Some(path) => list_mounted(caller.data(), mount_index as u32, &path),
                            None => refusal_packet(
                                "mount_denied",
                                "the path argument is not inside guest memory",
                            ),
                        };
                        answer_guest(&mut caller, &packet)
                    },
                )
            })
            .map_err(|e| refuse("trap", format!("the capability imports did not link: {e}")))?;
    }

    // The watchdog is the whole timeout. A wasm call is synchronous and
    // cannot be preempted from the calling thread, so the deadline has to
    // arrive from somewhere else: this thread increments the engine's epoch,
    // and the running instance traps at its next check.
    let timed_out = Arc::new(AtomicBool::new(false));
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let watchdog = {
        let engine = engine.clone();
        let timed_out = Arc::clone(&timed_out);
        let deadline = Duration::from_millis(plugin.manifest.timeout_ms);
        std::thread::spawn(move || {
            if let Err(mpsc::RecvTimeoutError::Timeout) = done_rx.recv_timeout(deadline) {
                timed_out.store(true, Ordering::SeqCst);
                engine.increment_epoch();
            }
        })
    };

    let outcome = run_packet(&mut store, &linker, &module, plugin, input);

    let _ = done_tx.send(());
    let _ = watchdog.join();

    outcome.map_err(|refusal| {
        if refusal.code == "trap" && timed_out.load(Ordering::SeqCst) {
            refuse(
                "timeout",
                format!(
                    "the plugin did not answer within {}ms, the bound its manifest declares, and \
                     its instance was trapped",
                    plugin.manifest.timeout_ms
                ),
            )
        } else {
            refusal
        }
    })
}

fn run_packet(
    store: &mut Store<HostState>,
    linker: &Linker<HostState>,
    module: &Module,
    plugin: &LoadedPlugin,
    input: &[u8],
) -> Result<Vec<u8>, Refusal> {
    let instance = linker
        .instantiate(&mut *store, module)
        .map_err(|e| refuse("trap", format!("the plugin did not instantiate: {e}")))?;

    let alloc = instance
        .get_typed_func::<i32, i32>(&mut *store, &plugin.manifest.abi_alloc)
        .map_err(|e| refuse("exports_missing", e.to_string()))?;
    let entry = instance
        .get_typed_func::<(i32, i32), i64>(&mut *store, &plugin.manifest.abi_entry)
        .map_err(|e| refuse("exports_missing", e.to_string()))?;
    let memory = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| refuse("exports_missing", "the module does not export `memory`"))?;

    let len = i32::try_from(input.len()).map_err(|_| {
        refuse(
            "bad_packet",
            "the input packet does not fit a wasm32 pointer",
        )
    })?;
    let ptr = alloc
        .call(&mut *store, len)
        .map_err(|e| refuse("trap", e.to_string()))?;
    memory
        .write(&mut *store, ptr as usize, input)
        .map_err(|e| {
            refuse(
                "trap",
                format!("the input packet did not fit guest memory: {e}"),
            )
        })?;

    let packed = entry
        .call(&mut *store, (ptr, len))
        .map_err(|e| refuse("trap", e.to_string()))? as u64;
    let out_ptr = (packed >> 32) as usize;
    let out_len = (packed & 0xffff_ffff) as usize;

    // Re-read the view: the call may have grown memory.
    let data = memory.data(&*store);
    let end = out_ptr
        .checked_add(out_len)
        .filter(|end| *end <= data.len())
        .ok_or_else(|| refuse("trap", "the output packet points outside guest memory"))?;
    Ok(data[out_ptr..end].to_vec())
}

/// The same invocation off the async runtime's worker threads.
pub async fn invoke_async(plugin: Arc<LoadedPlugin>, input: Vec<u8>) -> Result<Vec<u8>, Refusal> {
    match tokio::task::spawn_blocking(move || invoke(&plugin, &input)).await {
        Ok(outcome) => outcome,
        Err(err) => Err(refuse(
            "trap",
            format!("the plugin task did not finish: {err}"),
        )),
    }
}

/// Run a plugin and render its answer as the text a model reads.
pub async fn run_plugin_text(plugin: Arc<LoadedPlugin>, arguments: &serde_json::Value) -> String {
    let packet = serde_json::to_vec(arguments).unwrap_or_else(|_| b"{}".to_vec());
    match invoke_async(plugin, packet).await {
        Err(refusal) => format!("The plugin refused {refusal}"),
        Ok(bytes) => match String::from_utf8(bytes) {
            Err(err) => format!(
                "The plugin refused (bad_packet): the output packet is not UTF-8 ({} bytes)",
                err.as_bytes().len()
            ),
            Ok(text) if text.len() <= PLUGIN_OUTPUT_LIMIT => text,
            Ok(text) => {
                let mut cut = PLUGIN_OUTPUT_LIMIT;
                while cut > 0 && !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                format!("{}\n…[truncated]", &text[..cut])
            }
        },
    }
}

// ─────────────────────────────────────────────────────────────── the catalog

/// One installed, digest-pinned plugin as the catalog sees it.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub manifest_path: PathBuf,
    pub digest: String,
    pub mounts: Vec<Mount>,
    pub host_count: usize,
}

impl CatalogEntry {
    /// Which approval tier this entry falls in.
    pub fn tier(&self) -> Tier {
        if self.host_count > 0 {
            Tier::Hosts
        } else if self.mounts.is_empty() {
            Tier::PureCompute
        } else {
            Tier::Mounts
        }
    }
}

/// The three fixed capability tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// No mounts and no declared hosts. Allowed without asking.
    PureCompute,
    /// Read-only directory mounts. Needs an operator.
    Mounts,
    /// Network hosts. This host has no network capability, so it never loads.
    Hosts,
}

/// Walk upward from `from` until a `plugins/` directory is found, then read
/// every child `manifest.json` inside it.
///
/// Discovery, not verification: an invalid manifest is skipped rather than
/// failing the walk, and verification happens at load.
pub fn discover_catalog(from: &Path) -> Vec<CatalogEntry> {
    let mut here = from.to_path_buf();
    loop {
        let candidate = here.join("plugins");
        if candidate.is_dir() {
            return read_catalog_dir(&candidate);
        }
        if !here.pop() {
            return Vec::new();
        }
    }
}

fn read_catalog_dir(dir: &Path) -> Vec<CatalogEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.is_file() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Ok(manifest) = validate_manifest(&parsed) else {
            continue;
        };
        found.push(CatalogEntry {
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            manifest_path,
            digest: manifest.artifact_digest,
            mounts: manifest.mounts,
            host_count: manifest.hosts.len(),
        });
    }
    found.sort_by(|left, right| left.name.cmp(&right.name));
    found
}

/// Lowercased word stems of three letters or more; the rest is noise.
fn tokens(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 3)
        .map(str::to_string)
        .collect()
}

/// Score the catalog against free text, best first.
///
/// Retrieval narrows candidates; it never routes. The model still invokes by
/// the exact returned name, the load still verifies the digest, and the
/// sandbox still enforces the manifest, so nothing here decides what runs —
/// it decides what is worth showing. An embedding index replaces this scoring
/// without changing the surface (OpenAgentsInc/openagents#42).
pub fn match_capabilities<'a>(
    catalog: &'a [CatalogEntry],
    text: &str,
) -> Vec<(&'a CatalogEntry, usize)> {
    let terms = tokens(text);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(&CatalogEntry, usize)> = catalog
        .iter()
        .map(|entry| {
            let haystack: BTreeSet<String> =
                tokens(&format!("{} {}", entry.name, entry.description))
                    .into_iter()
                    .collect();
            let hits = terms.iter().filter(|term| haystack.contains(*term)).count();
            (entry, hits)
        })
        .filter(|(_, hits)| *hits > 0)
        .collect();
    scored.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.name.cmp(&right.0.name))
    });
    scored
}

/// A description's first sentence, for a one-line candidate row.
fn first_sentence(text: &str) -> &str {
    match text.find(". ") {
        Some(at) if at > 0 => &text[..=at],
        _ => text,
    }
}

fn catalog_description(catalog: &[CatalogEntry]) -> String {
    if catalog.is_empty() {
        return "The local catalog is empty.".to_string();
    }
    let rows: Vec<String> = catalog
        .iter()
        .map(|entry| {
            format!(
                "- `{}` v{}: {}",
                entry.name, entry.version, entry.description
            )
        })
        .collect();
    format!("Installed capabilities:\n{}", rows.join("\n"))
}

fn remainder(beyond: usize) -> String {
    if beyond > 0 {
        format!("\n…and {beyond} more; search again with other words.")
    } else {
        String::new()
    }
}

/// What a load reports, for a notice or a plain line.
pub fn describe_load(plugin: &LoadedPlugin) -> String {
    let reach = if plugin.mounts.is_empty() {
        "pure compute".to_string()
    } else if plugin.mounts.len() == 1 {
        "1 read-only mount".to_string()
    } else {
        format!("{} read-only mounts", plugin.mounts.len())
    };
    format!(
        "Loaded plugin `{}` v{} — digest verified ({}…, {} bytes, {reach}, {}ms bound, {} MiB \
         memory ceiling). Experimental.",
        plugin.manifest.name,
        plugin.manifest.version,
        &plugin.digest[..19.min(plugin.digest.len())],
        plugin.wasm.len(),
        plugin.manifest.timeout_ms,
        plugin.manifest.memory_max_mib,
    )
}

/// One sentence describing what the plugin can reach, for the model.
fn reach_description(plugin: &LoadedPlugin) -> String {
    if plugin.mounts.is_empty() {
        "It runs sandboxed pure computation: no file, network, or environment access.".to_string()
    } else {
        format!(
            "It runs sandboxed with read-only access to {} mounted director{}; no writes, no \
             network, no environment access.",
            plugin.mounts.len(),
            if plugin.mounts.len() == 1 { "y" } else { "ies" }
        )
    }
}

/// The tool declaration a loaded plugin materializes for the session.
///
/// The manifest is the whole declaration: its name is the tool name, its
/// description is what the model reads, and its input schema is the
/// parameters.
pub fn plugin_tool_definition(plugin: &LoadedPlugin) -> crate::tools::ToolDefinition {
    crate::tools::ToolDefinition {
        name: plugin.manifest.name.clone(),
        description: format!(
            "{}\n\nWASM plugin `{}` v{}, loaded for this session only ({}…). {} The result is a \
             JSON object with either `ok` or `refusal`.",
            plugin.manifest.description,
            plugin.manifest.name,
            plugin.manifest.version,
            &plugin.digest[..19.min(plugin.digest.len())],
            reach_description(plugin),
        ),
        parameters: plugin.manifest.input_schema.clone(),
    }
}

/// The standing `capability` tool.
///
/// Constant-size on purpose: the catalog is searched, never enumerated here,
/// so the standing prompt does not grow as capabilities are installed.
pub fn capability_tool_definition() -> crate::tools::ToolDefinition {
    crate::tools::ToolDefinition {
        name: "capability".to_string(),
        description: crate::surfaces::tool_descriptions::RUST_CAPABILITY.to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Describe the capability you need. Matches are ranked by overlap with each manifest's name and description."
                },
                "name": {
                    "type": "string",
                    "description": "Exact catalog name of the capability to load. Use the exact name from a previous `query` result."
                }
            },
            "additionalProperties": false
        }),
    }
}

/// Whether a tier may load in this session.
///
/// Pure compute is allowed without asking. Read-only mounts need an operator,
/// and if none was supplied the load refuses — the safe default, because an
/// unattended session must not grant directory access on its own. Network
/// hosts have no capability behind them at all, so they never load.
#[derive(Debug, Clone, Copy, Default)]
pub struct Approval {
    /// Set by a caller that has an operator behind it, such as the
    /// `--allow-mounts` flag on `oa plugin run`.
    pub mounts_allowed: bool,
}

impl Approval {
    pub fn check(&self, entry: &CatalogEntry) -> Result<(), Refusal> {
        match entry.tier() {
            Tier::PureCompute => Ok(()),
            Tier::Mounts if self.mounts_allowed => Ok(()),
            Tier::Mounts => Err(refuse(
                "approval_unavailable",
                format!(
                    "`{}` asks for {} read-only director{} and no operator approved it in this \
                     session",
                    entry.name,
                    entry.mounts.len(),
                    if entry.mounts.len() == 1 { "y" } else { "ies" }
                ),
            )),
            Tier::Hosts => Err(refuse(
                "capabilities_unsupported",
                format!(
                    "`{}` declares network hosts, and this host has no network capability to grant",
                    entry.name
                ),
            )),
        }
    }
}

/// Answer one `capability` call: search the catalog, or load by exact name.
///
/// Returns the text the model reads and, when a plugin loaded, the plugin
/// itself so the session can declare its tool.
pub fn answer_capability(
    catalog: &[CatalogEntry],
    approval: Approval,
    workspace: &Path,
    arguments: &serde_json::Value,
) -> (String, Option<LoadedPlugin>) {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    let name = arguments
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");

    if !name.is_empty() {
        let Some(entry) = catalog.iter().find(|candidate| candidate.name == name) else {
            return (
                format!(
                    "No capability named `{name}` is in the local catalog.\n\n{}",
                    catalog_description(catalog)
                ),
                None,
            );
        };
        if let Err(refusal) = approval.check(entry) {
            return (
                format!("Capability `{name}` was not allowed {refusal}"),
                None,
            );
        }
        match load_plugin(&entry.manifest_path, workspace) {
            Err(refusal) => (format!("Plugin not loaded {refusal}"), None),
            Ok(plugin) => {
                let text = format!(
                    "{}\n\nThe tool `{}` is available now. Call it directly for this work instead \
                     of a shell script: it is sandboxed, bounded, and returns structured JSON. Its \
                     parameters are in its tool declaration.",
                    describe_load(&plugin),
                    entry.name
                );
                (text, Some(plugin))
            }
        }
    } else if !query.is_empty() {
        let ranked = match_capabilities(catalog, query);
        if ranked.is_empty() {
            if catalog.is_empty() {
                return (
                    "No capabilities are installed on this machine.".to_string(),
                    None,
                );
            }
            let shown: Vec<CatalogEntry> = catalog.iter().take(SEARCH_LIMIT).cloned().collect();
            return (
                format!(
                    "Nothing installed matches that. The full catalog:\n\n{}{}\n\nCall \
                     `capability` with `name` set to an exact name to load it.",
                    catalog_description(&shown),
                    remainder(catalog.len().saturating_sub(SEARCH_LIMIT))
                ),
                None,
            );
        }
        let rows: Vec<String> = ranked
            .iter()
            .take(SEARCH_LIMIT)
            .map(|(entry, _)| {
                format!(
                    "- `{}` — {}",
                    entry.name,
                    first_sentence(&entry.description)
                )
            })
            .collect();
        (
            format!(
                "Best matches, most relevant first:\n\n{}{}\n\nCall `capability` with `name` set \
                 to the exact name you want to load.",
                rows.join("\n"),
                remainder(catalog.len().saturating_sub(rows.len()))
            ),
            None,
        )
    } else {
        (
            "Provide `query` to see the catalog or `name` to load a capability by exact catalog \
             name."
                .to_string(),
            None,
        )
    }
}

// ──────────────────────────────────────────────────────────── the `oa plugin`

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub action: PluginAction,
}

#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// List the digest-pinned WebAssembly plugins installed on this machine
    List,
    /// Rank the catalog against a description of what you need
    Search {
        /// What the capability should do
        query: String,
    },
    /// Verify a plugin's digest and report what its module asks for
    Inspect {
        /// Exact catalog name
        name: String,
    },
    /// Load a plugin and run one packet through it under its declared limits
    Run {
        /// Exact catalog name
        name: String,
        /// The tool arguments, as a JSON object
        #[arg(long, default_value = "{}")]
        input: String,
        /// Approve the read-only directory mounts the manifest declares
        #[arg(
            long,
            help = "Approve the read-only mounts this plugin declares; without it a mounted plugin refuses to load"
        )]
        allow_mounts: bool,
    },
}

/// `oa plugin`. Every path that cannot reach its data exits 2 with `oa: …` on
/// stderr rather than inventing an answer.
pub async fn run(args: PluginArgs, json: bool) {
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let catalog = discover_catalog(&workspace);

    match args.action {
        PluginAction::List => {
            if catalog.is_empty() {
                crate::cli::fail(&format!(
                    "no `plugins/` directory was found at or above {}, so no capability catalog \
                     could be read",
                    workspace.display()
                ));
            }
            if json {
                let rows: Vec<serde_json::Value> = catalog
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "name": entry.name,
                            "version": entry.version,
                            "digest": entry.digest,
                            "manifest": entry.manifest_path.to_string_lossy(),
                            "mounts": entry.mounts,
                            "tier": format!("{:?}", entry.tier()),
                        })
                    })
                    .collect();
                println!("{}", serde_json::Value::from(rows));
            } else {
                for entry in &catalog {
                    let reach = match entry.tier() {
                        Tier::PureCompute => "pure compute".to_string(),
                        Tier::Mounts => format!("{} read-only mount(s)", entry.mounts.len()),
                        Tier::Hosts => "network hosts (never loads)".to_string(),
                    };
                    println!("{} v{} — {reach}", entry.name, entry.version);
                }
            }
        }
        PluginAction::Search { query } => {
            if catalog.is_empty() {
                crate::cli::fail("no capability catalog could be read from this directory");
            }
            let ranked = match_capabilities(&catalog, &query);
            let matched: Vec<serde_json::Value> = ranked
                .iter()
                .take(SEARCH_LIMIT)
                .map(|(entry, hits)| {
                    serde_json::json!({
                        "name": entry.name,
                        "version": entry.version,
                        "terms_matched": hits,
                        "description": first_sentence(&entry.description),
                    })
                })
                .collect();
            if json {
                // The ranking is the answer, so it is the field. An empty
                // array is a real answer to "nothing matched" and needs no
                // sentence wrapped around it.
                println!(
                    "{}",
                    serde_json::json!({ "query": query, "matches": matched })
                );
            } else if ranked.is_empty() {
                println!("Nothing installed matches that.");
            } else {
                for (entry, hits) in ranked.iter().take(SEARCH_LIMIT) {
                    println!(
                        "{} ({hits} term{}) — {}",
                        entry.name,
                        if *hits == 1 { "" } else { "s" },
                        first_sentence(&entry.description)
                    );
                }
            }
        }
        PluginAction::Inspect { name } => {
            let Some(entry) = catalog.iter().find(|candidate| candidate.name == name) else {
                crate::cli::fail(&format!("no capability named `{name}` is installed here"));
            };
            match load_plugin(&entry.manifest_path, &workspace) {
                Err(refusal) => crate::cli::fail(&format!("`{name}` did not load {refusal}")),
                Ok(plugin) => {
                    let shape = match inspect_module(&plugin.wasm) {
                        Ok(shape) => shape,
                        Err(refusal) => {
                            crate::cli::fail(&format!("`{name}` did not compile {refusal}"))
                        }
                    };
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "name": entry.name,
                                "version": entry.version,
                                "manifest": plugin.manifest_path.to_string_lossy(),
                                "digest": plugin.digest,
                                "imports": shape.imports,
                                "mounts": plugin
                                    .mounts
                                    .iter()
                                    .map(|root| root.to_string_lossy().into_owned())
                                    .collect::<Vec<_>>(),
                                "tier": format!("{:?}", entry.tier()),
                            })
                        );
                        return;
                    }
                    println!("{}", describe_load(&plugin));
                    println!("manifest: {}", plugin.manifest_path.display());
                    println!("digest:   {}", plugin.digest);
                    println!(
                        "imports:  {}",
                        if shape.imports.is_empty() {
                            "none".to_string()
                        } else {
                            shape.imports.join(", ")
                        }
                    );
                    for root in &plugin.mounts {
                        println!("mount:    {} (read-only)", root.display());
                    }
                }
            }
        }
        PluginAction::Run {
            name,
            input,
            allow_mounts,
        } => {
            let Some(entry) = catalog.iter().find(|candidate| candidate.name == name) else {
                crate::cli::fail(&format!("no capability named `{name}` is installed here"));
            };
            let arguments: serde_json::Value = match serde_json::from_str(&input) {
                Ok(value) => value,
                Err(err) => crate::cli::fail(&format!("--input is not JSON: {err}")),
            };
            let approval = Approval {
                mounts_allowed: allow_mounts,
            };
            if let Err(refusal) = approval.check(entry) {
                crate::cli::fail(&format!("`{name}` was not allowed {refusal}"));
            }
            let plugin = match load_plugin(&entry.manifest_path, &workspace) {
                Ok(plugin) => plugin,
                Err(refusal) => crate::cli::fail(&format!("`{name}` did not load {refusal}")),
            };
            // The load line is a diagnostic about what was mounted, not the
            // capability's answer, so it stays on stderr in both modes and a
            // `--json` reader piping stdout is unaffected by it.
            eprintln!("{}", describe_load(&plugin));
            let digest = plugin.digest.clone();
            match invoke_async(
                Arc::new(plugin),
                serde_json::to_vec(&arguments).unwrap_or_default(),
            )
            .await
            {
                Err(refusal) => crate::cli::fail(&format!("`{name}` refused {refusal}")),
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) if json => {
                        // A capability that answered JSON has its answer
                        // carried as JSON; one that answered prose has it
                        // carried as a string. Either way the envelope names
                        // which, so a consumer never has to guess.
                        let output = serde_json::from_str::<serde_json::Value>(&text)
                            .unwrap_or_else(|_| serde_json::Value::String(text.clone()));
                        println!(
                            "{}",
                            serde_json::json!({
                                "name": name,
                                "digest": digest,
                                "output": output,
                            })
                        );
                    }
                    Ok(text) => println!("{text}"),
                    Err(err) => crate::cli::fail(&format!(
                        "`{name}` answered with {} bytes that are not UTF-8",
                        err.as_bytes().len()
                    )),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(mounts: serde_json::Value, hosts: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "name": "probe_plugin",
            "version": "0.1.0",
            "description": "A manifest for the validator's tests.",
            "artifact": {"path": "p.wasm", "digest": "sha256:00"},
            "abi": {"kind": "packet-v0", "entry": "handle_packet", "alloc": "packet_alloc"},
            "interface": {"input": {"type": "object"}, "output": {"type": "object"}},
            "capabilities": {"mounts": mounts, "hosts": hosts, "timeout_ms": 2000}
        })
    }

    #[test]
    fn a_writable_mount_is_refused_rather_than_downgraded() {
        let value = manifest_json(
            serde_json::json!([{"path": "data", "readonly": false}]),
            serde_json::json!([]),
        );
        let refusal = validate_manifest(&value).unwrap_err();
        assert_eq!(refusal.code, "capabilities_unsupported");
        assert!(refusal.reason.contains("readonly"), "{}", refusal.reason);
    }

    #[test]
    fn a_mount_missing_the_readonly_flag_entirely_is_refused() {
        let value = manifest_json(serde_json::json!([{"path": "data"}]), serde_json::json!([]));
        assert_eq!(
            validate_manifest(&value).unwrap_err().code,
            "capabilities_unsupported"
        );
    }

    /// Every name the session dispatches itself is refused, and the refusal
    /// says which one it collided with.
    ///
    /// The manifest is otherwise valid, so a host without this check installs
    /// it, declares its tool, and routes every call to the builtin instead.
    #[test]
    fn a_plugin_named_after_a_builtin_tool_is_refused_by_name() {
        for reserved in crate::tools::BUILTIN_TOOL_NAMES {
            let mut value = manifest_json(serde_json::json!([]), serde_json::json!([]));
            value["name"] = serde_json::json!(reserved);
            let refusal = validate_manifest(&value).unwrap_err();
            assert_eq!(refusal.code, "name_reserved", "`{reserved}`: {refusal}");
            assert!(
                refusal.reason.contains(reserved),
                "the refusal did not name the collision: {}",
                refusal.reason
            );
        }
        // A name that collides with nothing still loads, so the check is a
        // reservation and not a ban on plugins.
        assert_eq!(
            validate_manifest(&manifest_json(serde_json::json!([]), serde_json::json!([])))
                .unwrap()
                .name,
            "probe_plugin"
        );
    }

    /// A colliding manifest never reaches the catalog either, so nothing can
    /// ask for it by name and no tool is declared for it.
    #[test]
    fn a_colliding_manifest_is_left_out_of_the_catalog() {
        let root = tempfile::tempdir().unwrap();
        let plugins = root.path().join("plugins");
        for name in ["shell", "word_count"] {
            let dir = plugins.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let mut value = manifest_json(serde_json::json!([]), serde_json::json!([]));
            value["name"] = serde_json::json!(name);
            std::fs::write(
                dir.join("manifest.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .unwrap();
        }

        let names: Vec<String> = discover_catalog(root.path())
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(
            names,
            vec!["word_count"],
            "a plugin named after a builtin reached the catalog"
        );
    }

    #[test]
    fn an_unknown_abi_is_refused_by_name() {
        let mut value = manifest_json(serde_json::json!([]), serde_json::json!([]));
        value["abi"]["kind"] = serde_json::json!("packet-v9");
        let refusal = validate_manifest(&value).unwrap_err();
        assert_eq!(refusal.code, "abi_unsupported");
        assert!(refusal.reason.contains("packet-v9"));
    }

    #[test]
    fn a_manifest_timeout_is_clamped_to_the_host_ceiling() {
        let mut value = manifest_json(serde_json::json!([]), serde_json::json!([]));
        value["capabilities"]["timeout_ms"] = serde_json::json!(3_600_000);
        assert_eq!(
            validate_manifest(&value).unwrap().timeout_ms,
            TIMEOUT_CEILING_MS
        );
    }

    #[test]
    fn a_manifest_memory_request_is_clamped_to_the_host_ceiling() {
        let mut value = manifest_json(serde_json::json!([]), serde_json::json!([]));
        value["capabilities"]["memory_max_mib"] = serde_json::json!(4096);
        assert_eq!(
            validate_manifest(&value).unwrap().memory_max_mib,
            MEMORY_CEILING_MIB
        );
    }

    #[test]
    fn a_digest_without_the_sha256_prefix_is_not_a_pin() {
        let mut value = manifest_json(serde_json::json!([]), serde_json::json!([]));
        value["artifact"]["digest"] = serde_json::json!("deadbeef");
        assert_eq!(
            validate_manifest(&value).unwrap_err().code,
            "manifest_invalid"
        );
    }

    #[test]
    fn a_declared_network_host_puts_the_entry_in_the_tier_that_never_loads() {
        let entry = CatalogEntry {
            name: "net".to_string(),
            version: "1".to_string(),
            description: String::new(),
            manifest_path: PathBuf::new(),
            digest: String::new(),
            mounts: Vec::new(),
            host_count: 1,
        };
        assert_eq!(entry.tier(), Tier::Hosts);
        assert_eq!(
            Approval {
                mounts_allowed: true
            }
            .check(&entry)
            .unwrap_err()
            .code,
            "capabilities_unsupported"
        );
    }

    #[test]
    fn a_mounted_plugin_needs_an_operator_and_pure_compute_does_not() {
        let mounted = CatalogEntry {
            name: "reader".to_string(),
            version: "1".to_string(),
            description: String::new(),
            manifest_path: PathBuf::new(),
            digest: String::new(),
            mounts: vec![Mount {
                path: "data".to_string(),
            }],
            host_count: 0,
        };
        let pure = CatalogEntry {
            mounts: Vec::new(),
            ..mounted.clone()
        };
        assert_eq!(
            Approval::default().check(&mounted).unwrap_err().code,
            "approval_unavailable"
        );
        assert!(Approval {
            mounts_allowed: true
        }
        .check(&mounted)
        .is_ok());
        assert!(Approval::default().check(&pure).is_ok());
    }

    #[test]
    fn lexical_join_refuses_a_path_that_climbs_out_of_the_root() {
        let root = Path::new("/tmp/root");
        assert_eq!(
            lexical_join(root, "a/b.txt").unwrap(),
            Path::new("/tmp/root/a/b.txt")
        );
        assert_eq!(
            lexical_join(root, "a/../b.txt").unwrap(),
            Path::new("/tmp/root/b.txt")
        );
        // Climbs one above the root: still a path, but not one inside it.
        let escaped = lexical_join(root, "../secret").unwrap();
        assert!(
            !within(&escaped, root),
            "{} is not confined",
            escaped.display()
        );
        // An absolute component is not a path in the mount at all.
        assert!(lexical_join(root, "/etc/passwd").is_none());
    }

    #[test]
    fn retrieval_ranks_by_term_overlap_and_ignores_short_noise() {
        let catalog = vec![
            CatalogEntry {
                name: "word_stats".to_string(),
                version: "1".to_string(),
                description: "Count words and lines in text.".to_string(),
                manifest_path: PathBuf::new(),
                digest: String::new(),
                mounts: Vec::new(),
                host_count: 0,
            },
            CatalogEntry {
                name: "dir_stats".to_string(),
                version: "1".to_string(),
                description: "List a mounted directory.".to_string(),
                manifest_path: PathBuf::new(),
                digest: String::new(),
                mounts: Vec::new(),
                host_count: 0,
            },
        ];
        let ranked = match_capabilities(&catalog, "count the words in this text");
        assert_eq!(
            ranked.first().map(|(entry, _)| entry.name.as_str()),
            Some("word_stats")
        );
        // "of a" is two stems under three letters, so nothing scores.
        assert!(match_capabilities(&catalog, "of a").is_empty());
    }

    #[test]
    fn the_capability_tool_declares_only_itself() {
        let definition = capability_tool_definition();
        assert_eq!(definition.name, "capability");
        let properties = &definition.parameters["properties"];
        assert!(properties.get("query").is_some());
        assert!(properties.get("name").is_some());
        // The catalog is searched, never enumerated in the declaration: a
        // standing prompt that grows with every installed plugin is the thing
        // this shape exists to avoid.
        assert!(!definition.description.contains("word_stats"));
    }
}
