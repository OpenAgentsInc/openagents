//! The WebAssembly capability sandbox, tested by violating it.
//!
//! A sandbox is only worth the tests that break it, so every limit here is
//! asserted by exceeding it with a guest built for the purpose: one that grows
//! past the memory ceiling, one that never returns, one that asks for a write
//! capability, one that reaches outside its mount. A test that only watched a
//! well-behaved plugin succeed would pass just as happily against a host that
//! enforces nothing.
//!
//! The misbehaving guests are assembled from WAT rather than checked in,
//! because a checked-in artifact that tries to escape its sandbox is a thing
//! nobody should have to keep in the tree. The last test runs the real
//! shipped artifact so the fixtures cannot drift away from what ships.

use std::path::{Path, PathBuf};
use std::time::Instant;

use openagents_cli::plugins::{
    Approval, CatalogEntry, MOUNT_FILE_LIMIT, Mount, invoke, load_plugin,
};
use openagents_cli::tools::{HarnessToolRegistry, ToolCall};
use sha2::{Digest, Sha256};

/// Bump-allocating `packet-v0` scaffolding every fixture shares.
///
/// A fixed-address allocator would hand the host the same buffer it just read
/// the input from, which is a fine way to make a passing test that proves the
/// wrong thing.
const PREAMBLE: &str = r#"
  (global $next (mut i32) (i32.const 1024))
  (func (export "packet_alloc") (param $n i32) (result i32)
    (local $at i32)
    (local.set $at (global.get $next))
    (global.set $next (i32.add (global.get $next) (i32.add (local.get $n) (i32.const 8))))
    (local.get $at))
"#;

fn wasm(body: &str) -> Vec<u8> {
    wat::parse_str(body).expect("the fixture is valid WAT")
}

/// Write a manifest and artifact into `dir` and load them.
///
/// The digest is computed from the bytes actually written, so a fixture can
/// never pass by pinning nothing; the one test that wants a mismatch corrupts
/// the artifact after this has pinned it.
fn plant(
    dir: &Path,
    name: &str,
    artifact: &[u8],
    mounts: serde_json::Value,
    timeout_ms: u64,
    memory_max_mib: u64,
) -> PathBuf {
    let artifact_path = dir.join("guest.wasm");
    std::fs::write(&artifact_path, artifact).expect("the artifact writes");
    let manifest = serde_json::json!({
        "manifest_version": 1,
        "name": name,
        "version": "0.0.1",
        "description": "A guest built to break one rule on purpose.",
        "artifact": {
            "path": "guest.wasm",
            "digest": format!("sha256:{:x}", Sha256::digest(artifact)),
        },
        "abi": {"kind": "packet-v0", "entry": "handle_packet", "alloc": "packet_alloc"},
        "interface": {"input": {"type": "object"}, "output": {"type": "object"}},
        "capabilities": {
            "mounts": mounts,
            "hosts": [],
            "timeout_ms": timeout_ms,
            "memory_max_mib": memory_max_mib,
        },
    });
    let manifest_path = dir.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .expect("the manifest writes");
    manifest_path
}

/// The repository's checked-in plugin catalog.
fn shipped_plugins() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("plugins")
}

// ─────────────────────────────────────────────────── imports and the digest

#[test]
fn a_guest_that_asks_to_write_never_loads_however_it_is_mounted() {
    // The one thing "read-only mount" has to mean. The guest declares a
    // read-only mount, which is the most generous capability this host grants,
    // and then imports a write. There is no write import to link to and the
    // load refuses by inspection, before the module is ever instantiated.
    let dir = tempfile::tempdir().unwrap();
    let mount = dir.path().join("data");
    std::fs::create_dir(&mount).unwrap();
    std::fs::write(mount.join("kept.txt"), b"untouched").unwrap();

    let artifact = wasm(&format!(
        r#"(module
             (import "openagents" "write_file" (func $write (param i32 i32) (result i64)))
             (memory (export "memory") 1)
             {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64)
               (call $write (local.get 0) (local.get 1))))"#
    ));
    let manifest = plant(
        dir.path(),
        "writer",
        &artifact,
        serde_json::json!([{"path": "data", "readonly": true}]),
        2000,
        16,
    );

    let refusal = load_plugin(&manifest, dir.path()).unwrap_err();
    assert_eq!(refusal.code, "imports_undeclared");
    assert!(
        refusal.reason.contains("openagents.write_file"),
        "the refusal should name the import it refused: {}",
        refusal.reason
    );
    // And the file it wanted is still what it was.
    assert_eq!(std::fs::read(mount.join("kept.txt")).unwrap(), b"untouched");
}

#[test]
fn a_pure_compute_manifest_grants_no_imports_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = wasm(&format!(
        r#"(module
             (import "openagents" "read_file" (func $read (param i32 i32) (result i64)))
             (memory (export "memory") 1)
             {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64)
               (call $read (local.get 0) (local.get 1))))"#
    ));
    let manifest = plant(
        dir.path(),
        "sneaky",
        &artifact,
        serde_json::json!([]),
        2000,
        16,
    );

    let refusal = load_plugin(&manifest, dir.path()).unwrap_err();
    assert_eq!(refusal.code, "imports_undeclared");
    assert!(
        refusal.reason.contains("may import nothing"),
        "{}",
        refusal.reason
    );
}

#[test]
fn a_module_without_the_abi_exports_does_not_load() {
    let dir = tempfile::tempdir().unwrap();
    // Everything but `memory`, which the host needs to move packets at all.
    let artifact = wasm(&format!(
        r#"(module
             (memory 1)
             {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64) (i64.const 0)))"#
    ));
    let manifest = plant(
        dir.path(),
        "hidden",
        &artifact,
        serde_json::json!([]),
        2000,
        16,
    );

    let refusal = load_plugin(&manifest, dir.path()).unwrap_err();
    assert_eq!(refusal.code, "exports_missing");
    assert!(refusal.reason.contains("memory"), "{}", refusal.reason);
}

#[test]
fn one_changed_byte_in_the_artifact_is_refused_against_the_pin() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = wasm(&format!(
        r#"(module (memory (export "memory") 1) {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64) (i64.const 0)))"#
    ));
    let manifest = plant(
        dir.path(),
        "pinned",
        &artifact,
        serde_json::json!([]),
        2000,
        16,
    );
    // It loads while the bytes are the bytes.
    assert!(load_plugin(&manifest, dir.path()).is_ok());

    // Append one byte the manifest never saw. Still valid wasm; not the wasm
    // the manifest describes.
    let mut tampered = artifact.clone();
    tampered.push(0);
    std::fs::write(dir.path().join("guest.wasm"), &tampered).unwrap();

    let refusal = load_plugin(&manifest, dir.path()).unwrap_err();
    assert_eq!(refusal.code, "digest_mismatch");
    assert!(
        refusal.reason.contains("does not load"),
        "{}",
        refusal.reason
    );
}

// ─────────────────────────────────────────────────────── the memory ceiling

/// A guest whose whole purpose is to ask for 100 pages — 6.4 MiB — more than
/// it started with, and report whether it got them.
fn memory_hog() -> Vec<u8> {
    wasm(&format!(
        r#"(module
             (memory (export "memory") 1)
             (data (i32.const 0) "{{\"ok\":{{\"grew\":true}}}}")
             (data (i32.const 64) "{{\"ok\":{{\"grew\":false}}}}")
             {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64)
               (if (result i64)
                 (i32.eq (memory.grow (i32.const 100)) (i32.const -1))
                 (then (i64.const 274877906965))
                 (else (i64.const 20)))))"#
    ))
}

#[test]
fn a_guest_is_denied_the_pages_that_would_cross_its_ceiling() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = memory_hog();

    // 1 MiB ceiling: the 6.4 MiB it asks for is not there to be had.
    let manifest = plant(dir.path(), "hog", &artifact, serde_json::json!([]), 2000, 1);
    let plugin = load_plugin(&manifest, dir.path()).unwrap();
    let packet = invoke(&plugin, b"{}").unwrap();
    assert_eq!(
        String::from_utf8_lossy(&packet),
        r#"{"ok":{"grew":false}}"#,
        "the ceiling did not stop the growth"
    );

    // The same artifact under a ceiling that covers the request grows fine, so
    // the refusal above is the ceiling and not a broken fixture.
    let roomy = tempfile::tempdir().unwrap();
    let manifest = plant(
        roomy.path(),
        "hog",
        &artifact,
        serde_json::json!([]),
        2000,
        64,
    );
    let plugin = load_plugin(&manifest, roomy.path()).unwrap();
    let packet = invoke(&plugin, b"{}").unwrap();
    assert_eq!(String::from_utf8_lossy(&packet), r#"{"ok":{"grew":true}}"#);
}

// ────────────────────────────────────────────────────────────── the timeout

#[test]
fn a_guest_that_never_returns_is_trapped_at_its_declared_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = wasm(&format!(
        r#"(module
             (memory (export "memory") 1)
             {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64)
               (loop $spin (br $spin))
               (i64.const 0)))"#
    ));
    let manifest = plant(
        dir.path(),
        "spinner",
        &artifact,
        serde_json::json!([]),
        400,
        16,
    );
    let plugin = load_plugin(&manifest, dir.path()).unwrap();

    let started = Instant::now();
    let refusal = invoke(&plugin, b"{}").unwrap_err();
    let elapsed = started.elapsed();

    assert_eq!(refusal.code, "timeout", "{}", refusal.reason);
    assert!(refusal.reason.contains("400ms"), "{}", refusal.reason);
    // The point of the deadline is that it arrives. A host that waited for the
    // guest would still be in that loop.
    assert!(
        elapsed.as_secs() < 10,
        "the deadline took {elapsed:?} to arrive, which is not a deadline"
    );
}

// ────────────────────────────────────────────────────── the confined mount

/// A guest that hands the host's whole answer packet straight back, so a test
/// sees exactly what the capability import answered: `0x00` and the bytes, or
/// `0x01` and a `{code, reason}` refusal.
fn passthrough_reader() -> Vec<u8> {
    wasm(&format!(
        r#"(module
             (import "openagents" "read_file" (func $read (param i32 i32) (result i64)))
             (memory (export "memory") 4)
             {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64)
               (call $read (local.get 0) (local.get 1))))"#
    ))
}

/// Split a host answer packet into its status byte and its body.
fn answer(packet: &[u8]) -> (u8, String) {
    let (status, body) = packet
        .split_first()
        .expect("the host answered with a packet");
    (*status, String::from_utf8_lossy(body).into_owned())
}

fn mounted_reader(dir: &Path) -> (PathBuf, PathBuf) {
    let mount = dir.join("data");
    std::fs::create_dir_all(&mount).unwrap();
    let manifest = plant(
        dir,
        "reader",
        &passthrough_reader(),
        serde_json::json!([{"path": "data", "readonly": true}]),
        4000,
        16,
    );
    (manifest, mount)
}

#[test]
fn a_mounted_guest_reads_inside_the_root_and_is_denied_everywhere_else() {
    let dir = tempfile::tempdir().unwrap();
    let (manifest, mount) = mounted_reader(dir.path());
    std::fs::write(mount.join("inside.txt"), b"in the mount").unwrap();
    // A secret one directory above the mount root, which is where a `..`
    // lands and where an absolute path can point.
    std::fs::write(dir.path().join("secret.txt"), b"outside the mount").unwrap();

    let plugin = load_plugin(&manifest, dir.path()).unwrap();

    let (status, body) = answer(&invoke(&plugin, b"inside.txt").unwrap());
    assert_eq!(status, 0, "a file in the mount should read: {body}");
    assert_eq!(body, "in the mount");

    for (path, why) in [
        ("../secret.txt", "a `..` that climbs out of the root"),
        (
            "./nested/../../secret.txt",
            "a `..` that climbs out after descending",
        ),
    ] {
        let (status, body) = answer(&invoke(&plugin, path.as_bytes()).unwrap());
        assert_eq!(
            status, 1,
            "{why} should be refused, not answered with {body}"
        );
        assert!(body.contains("mount_denied"), "{why}: {body}");
        assert!(
            !body.contains("outside the mount"),
            "{why} leaked the file: {body}"
        );
    }

    let absolute = dir.path().join("secret.txt");
    let (status, body) = answer(&invoke(&plugin, absolute.to_string_lossy().as_bytes()).unwrap());
    assert_eq!(status, 1, "an absolute path should be refused: {body}");
    assert!(body.contains("absolute paths are refused"), "{body}");
}

#[cfg(unix)]
#[test]
fn a_symlink_planted_in_the_mount_does_not_carry_a_read_out_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let (manifest, mount) = mounted_reader(dir.path());
    std::fs::write(dir.path().join("secret.txt"), b"outside the mount").unwrap();
    std::os::unix::fs::symlink(dir.path().join("secret.txt"), mount.join("bridge.txt")).unwrap();

    let plugin = load_plugin(&manifest, dir.path()).unwrap();
    let (status, body) = answer(&invoke(&plugin, b"bridge.txt").unwrap());

    assert_eq!(
        status, 1,
        "a symlink out of the mount should be refused: {body}"
    );
    assert!(
        body.contains("symlinks inside a mount are refused"),
        "{body}"
    );
    assert!(
        !body.contains("outside the mount"),
        "the symlink leaked the file: {body}"
    );
}

#[test]
fn a_whole_file_read_past_the_byte_bound_is_refused_and_a_range_read_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let (manifest, mount) = mounted_reader(dir.path());
    let oversized = vec![b'x'; (MOUNT_FILE_LIMIT + 1) as usize];
    std::fs::write(mount.join("big.txt"), &oversized).unwrap();

    let plugin = load_plugin(&manifest, dir.path()).unwrap();
    let (status, body) = answer(&invoke(&plugin, b"big.txt").unwrap());
    assert_eq!(status, 1, "an oversized whole-file read should be refused");
    assert!(body.contains("file_too_large"), "{body}");
    assert!(body.contains(&(MOUNT_FILE_LIMIT + 1).to_string()), "{body}");
}

// ───────────────────────────────────────────────── approval and the catalog

#[test]
fn a_mounted_capability_needs_an_operator_before_it_can_be_loaded() {
    let entry = CatalogEntry {
        name: "reader".to_string(),
        version: "0.0.1".to_string(),
        description: "reads a mount".to_string(),
        manifest_path: PathBuf::new(),
        digest: String::new(),
        mounts: vec![Mount {
            path: "data".to_string(),
        }],
        host_count: 0,
    };
    assert_eq!(
        Approval::default().check(&entry).unwrap_err().code,
        "approval_unavailable",
        "an unattended session must not grant a directory on its own"
    );
    assert!(
        Approval {
            mounts_allowed: true
        }
        .check(&entry)
        .is_ok()
    );
}

// ──────────────────────────────────────── a name the session already answers

/// A valid, digest-pinned, pure-compute guest that loads and does nothing.
///
/// The collision test needs a plugin that would otherwise install cleanly, so
/// the only thing between it and the model's tool list is the reserved-name
/// check.
fn inert_guest() -> Vec<u8> {
    wasm(&format!(
        r#"(module (memory (export "memory") 1) {PREAMBLE}
             (func (export "handle_packet") (param i32 i32) (result i64) (i64.const 0)))"#
    ))
}

fn call(name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "1".to_string(),
        name: name.to_string(),
        arguments,
    }
}

/// A plugin named after a builtin is refused at install, so the declared tool
/// list never carries a name the session would answer for itself.
///
/// The two plugins below are the same fixture under two names. `word_count`
/// is the control: it installs, it is declared, and it dispatches to the
/// plugin. `shell` is the collision — a builtin match arm wins before the
/// plugin lookup, so a host that installed it would declare a tool the model
/// could never reach. The list assertion is exact, which is what catches the
/// duplicate: a host without the check declares `shell` twice.
#[tokio::test]
async fn a_plugin_named_after_a_builtin_is_never_declared_and_every_declared_tool_answers() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = inert_guest();
    for name in ["shell", "word_count"] {
        let home = dir.path().join("plugins").join(name);
        std::fs::create_dir_all(&home).unwrap();
        plant(&home, name, &artifact, serde_json::json!([]), 2000, 16);
    }

    let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
    let installed: Vec<&str> = registry
        .catalog
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        installed,
        vec!["word_count"],
        "a plugin named after a builtin reached the catalog"
    );

    // Asked for by exact name: the free name loads, the taken one is not
    // there to load, and the model is told so rather than left with a tool
    // that answers from somewhere else.
    let taken = registry
        .execute_tool(&call("capability", serde_json::json!({"name": "shell"})))
        .await;
    assert!(
        taken.is_error,
        "`shell` was loaded as a plugin: {}",
        taken.output
    );
    let free = registry
        .execute_tool(&call(
            "capability",
            serde_json::json!({"name": "word_count"}),
        ))
        .await;
    assert!(
        !free.is_error,
        "the control plugin did not load: {}",
        free.output
    );

    // This registry has no delegation gate, so `delegate` is not declared.
    let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
    assert_eq!(
        names,
        vec![
            "read",
            "write",
            "edit",
            "bash",
            "shell",
            "skill",
            "checkpoint",
            "openagents",
            "capability",
            "word_count"
        ],
        "the declared list is not the set of tools that can be called"
    );

    // And every name on it reaches an implementation. `word_count` is the one
    // that matters here: it is the plugin, dispatched from the arm below all
    // the builtins, which is the arm a collision would have hidden it behind.
    for name in &names {
        if name == "openagents" {
            // Shelling out to another CLI is not this test's business.
            continue;
        }
        let out = registry
            .execute_tool(&call(name, serde_json::json!({})))
            .await;
        assert!(
            !out.output.starts_with("Unknown tool:"),
            "`{name}` is declared and unanswered: {}",
            out.output
        );
    }
}

// ─────────────────────────────────────────────────── what actually ships

#[test]
fn the_checked_in_word_stats_artifact_loads_and_computes() {
    let manifest = shipped_plugins().join("word-stats").join("manifest.json");
    if !manifest.is_file() {
        // A published crate has no `plugins/` beside it; there is nothing to
        // check rather than something to fail.
        return;
    }
    let plugin = load_plugin(&manifest, Path::new(".")).expect("the shipped artifact loads");
    assert!(
        plugin.mounts.is_empty(),
        "word_stats is declared pure compute"
    );

    let packet = invoke(&plugin, br#"{"text":"one two two three three three"}"#).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&packet).expect("an output packet");
    assert_eq!(value["ok"]["words"], 6);
    assert_eq!(value["ok"]["top_word"]["word"], "three");
}

#[test]
fn the_checked_in_file_stats_artifact_reads_only_its_own_mount() {
    let manifest = shipped_plugins().join("file-stats").join("manifest.json");
    if !manifest.is_file() {
        return;
    }
    let plugin = load_plugin(&manifest, Path::new(".")).expect("the shipped artifact loads");
    assert_eq!(plugin.mounts.len(), 1);

    let inside = invoke(&plugin, br#"{"path":"sample.txt"}"#).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&inside).unwrap();
    assert!(value["ok"]["bytes"].as_u64().unwrap_or(0) > 0, "{value}");

    let outside = invoke(&plugin, br#"{"path":"../manifest.json"}"#).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&outside).unwrap();
    assert_eq!(value["refusal"]["code"], "mount_denied", "{value}");
}
