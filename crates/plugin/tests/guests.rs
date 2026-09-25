//! The checked-in evidence guests, run through [`plugin::invoke`] over their
//! crates' fixtures, and their build receipts.
//!
//! `scripts/build-plugin-guests.sh` builds `repo-map.wasm`,
//! `code-search.wasm`, and `test-report.wasm` into `fixtures/`. The receipt
//! beside each module pins the PDK source, the guest source, and the module
//! bytes, so an edit to either source without a rebuild fails here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use plugin::{Entry, GuestValue, HostError, Limits, Profile, Snapshot, invoke};
use serde_json::{Value, json};

const GUESTS: [&str; 3] = ["repo-map", "code-search", "test-report"];

fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn wasm(guest: &str) -> Vec<u8> {
    std::fs::read(crates().join(format!("plugin/fixtures/{guest}.wasm")))
        .expect("the guest is built by scripts/build-plugin-guests.sh")
}

/// A snapshot laid out as the Coder runtime grants a workspace: one
/// directory, `workspace`, whose handle is `root`, listing every file
/// under `dir` as `workspace/<relative path>`.
fn snapshot(dir: &Path) -> (Snapshot, BTreeMap<String, String>) {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy();
                out.push((relative.replace('\\', "/"), std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    let mut snapshot = Snapshot::default();
    let mut children = Vec::new();
    for (relative, bytes) in files {
        let label = format!("workspace/{relative}");
        let version = plugin::digest(&bytes);
        snapshot
            .insert(
                &label,
                Entry::File {
                    bytes,
                    version,
                    complete: true,
                },
            )
            .unwrap();
        children.push(label);
    }
    snapshot
        .insert("workspace", Entry::Directory { children })
        .unwrap();
    let handles = BTreeMap::from([("workspace".to_string(), "root".to_string())]);
    (snapshot, handles)
}

fn run(guest: &str, operation: &str, input: Value) -> Result<GuestValue, HostError> {
    let (snapshot, handles) = snapshot(&crates().join(format!("plugin-{guest}/fixtures/tree")));
    invoke(plugin::Call {
        wasm: &wasm(guest),
        profile: Profile::SnapshotRead,
        invocation: "inv-guest",
        operation,
        input: &input,
        snapshot: &snapshot,
        handles: &handles,
        limits: Limits {
            fuel: 200_000_000,
            memory_bytes: 64 * 1024 * 1024,
            output_bytes: 1024 * 1024,
            read_bytes: 1024 * 1024,
            module_bytes: 2 * 1024 * 1024,
        },
        cancelled: Arc::new(AtomicBool::new(false)),
        required: true,
    })
}

#[test]
fn the_repo_map_guest_maps_its_fixture() {
    let value = run("repo-map", "map", json!({})).expect("map").value;
    assert_eq!(value["kind"], "repo-map");
    assert_eq!(value["files"], 9);
    assert_eq!(value["complete"], true);
    assert_eq!(value["languages"][0]["language"], "Python");
    assert_eq!(value["manifests"], json!(["pyproject.toml"]));
    assert_eq!(value["largest"][0]["path"], "src/app/core.py");
}

#[test]
fn the_code_search_guest_ranks_files_by_patterns_matched() {
    let value = run(
        "code-search",
        "search",
        json!({"patterns": ["ledger", "retry_limit"]}),
    )
    .expect("search")
    .value;
    assert_eq!(value["kind"], "code-search");
    assert_eq!(value["files"][0]["path"], "src/billing.py");
    assert_eq!(value["files"][0]["patterns"], 2);
    assert_eq!(value["files_skipped"], 1);
    assert_eq!(value["truncated"], false);
}

#[test]
fn the_test_report_guest_reads_all_three_formats() {
    let value = run("test-report", "parse", json!({})).expect("parse").value;
    assert_eq!(value["kind"], "test-report");
    let formats: Vec<&str> = value["reports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|report| report["format"].as_str().unwrap())
        .collect();
    assert_eq!(formats, ["cargo", "pytest", "junit"]);
    assert_eq!(value["failures_total"], 6);
    assert_eq!(value["reports"][0]["failures"][0]["file"], "src/ledger.rs");
    assert_eq!(value["reports"][0]["failures"][0]["line"], 42);
}

#[test]
fn a_guest_refuses_an_operation_it_does_not_export() {
    let error = run("code-search", "map", json!({})).unwrap_err();
    assert_eq!(error, HostError::Refused("unsupported_input".into()));
    let error = run("code-search", "search", json!({"patterns": []})).unwrap_err();
    assert_eq!(error, HostError::Refused("unsupported_input".into()));
}

#[test]
fn a_pure_profile_refuses_the_guests_host_import() {
    for guest in GUESTS {
        let error = invoke(plugin::Call {
            wasm: &wasm(guest),
            profile: Profile::Pure,
            invocation: "inv-pure",
            operation: "map",
            input: &json!({}),
            snapshot: &Snapshot::default(),
            handles: &BTreeMap::new(),
            limits: Limits {
                fuel: 50_000_000,
                ..Limits::default()
            },
            cancelled: Arc::new(AtomicBool::new(false)),
            required: true,
        })
        .unwrap_err();
        assert!(
            matches!(error, HostError::Denied(ref detail) if detail.contains("oa_host")),
            "{guest}: {error}"
        );
    }
}

/// Each receipt binds the checked-in module to the PDK and guest sources
/// it was built from. A source edit without a rebuild fails here; run
/// `./scripts/build-plugin-guests.sh`.
#[test]
fn every_guest_matches_its_build_receipt() {
    let pdk = [
        std::fs::read(crates().join("plugin-pdk/src/lib.rs")).unwrap(),
        std::fs::read(crates().join("plugin-pdk/src/guest.rs")).unwrap(),
    ]
    .concat();
    for guest in GUESTS {
        let text =
            std::fs::read_to_string(crates().join(format!("plugin/fixtures/{guest}.receipt.json")))
                .unwrap();
        let receipt: Value = serde_json::from_str(&text).unwrap();
        let module = wasm(guest);
        let built = plugin::build_receipt(&pdk, &module, "snapshot-read");
        assert_eq!(receipt["schema"], "openagents.plugin-build-receipt.v1");
        assert_eq!(receipt["guest"], guest);
        assert_eq!(receipt["profile"], built.profile.as_str());
        assert_eq!(
            receipt["pdk_digest"],
            built.pdk_digest.as_str(),
            "{guest}: the PDK changed since the guest was built"
        );
        assert_eq!(receipt["guest_digest"], built.guest_digest.as_str());
        assert_eq!(receipt["size"], module.len());
        let source = [
            std::fs::read(crates().join(format!("plugin-{guest}/Cargo.toml"))).unwrap(),
            std::fs::read(crates().join(format!("plugin-{guest}/src/lib.rs"))).unwrap(),
        ]
        .concat();
        assert_eq!(
            receipt["source_digest"],
            plugin::digest(&source).as_str(),
            "{guest}: the guest source changed since it was built"
        );
    }
}
