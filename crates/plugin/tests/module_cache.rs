//! Compiled guest modules are kept for the process (#11513): a second
//! firing of the same module skips compiling it, its receipt is the same
//! bytes as the first, a changed module compiles anew, and each firing is
//! still a fresh instance.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use plugin::{
    Call, InvocationReceipt, Limits, Profile, Snapshot, invoke_with_receipt, module_stats, replay,
};
use serde_json::{Value, json};

/// The hook guard's guest, as `crates/coder-sdk` fires it.
fn guard() -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-hook-guard/wasm/hook-guard.wasm"),
    )
    .unwrap()
}

/// The guard hook's input for a force push.
fn input() -> Value {
    json!({"event": "PreToolUse", "tool": "Bash", "input": {"command": "git push --force origin main"}})
}

fn fire(wasm: &[u8]) -> (Value, InvocationReceipt, Duration) {
    let input = input();
    let snapshot = Snapshot::default();
    let handles = BTreeMap::new();
    let started = Instant::now();
    let (result, receipt) = invoke_with_receipt(call(wasm, &input, &snapshot, &handles));
    (result.unwrap().value, receipt, started.elapsed())
}

fn call<'a>(
    wasm: &'a [u8],
    input: &'a Value,
    snapshot: &'a Snapshot,
    handles: &'a BTreeMap<String, String>,
) -> Call<'a> {
    Call {
        wasm,
        profile: Profile::SnapshotRead,
        invocation: "hook-cache-1",
        operation: "guard",
        input,
        snapshot,
        handles,
        limits: Limits::default(),
        cancelled: Arc::new(AtomicBool::new(false)),
        required: false,
    }
}

/// The guard firing's receipt from before modules were kept, from
/// `origin/main` at 81320420bb: a kept module changes none of it.
const RECEIPT: &str = include_str!("module_cache.receipt.json");

#[test]
fn a_second_firing_reuses_the_compiled_module_and_its_receipt_is_the_same() {
    let wasm = guard();
    let before = module_stats();
    let (first, first_receipt, compiled) = fire(&wasm);
    assert_eq!(module_stats().loaded, before.loaded + 1);
    let (second, second_receipt, reused) = fire(&wasm);
    // Measured: the second firing loaded nothing and reused the module.
    assert_eq!(module_stats().loaded, before.loaded + 1);
    assert_eq!(module_stats().reused, before.reused + 1);
    eprintln!("first firing {compiled:?}, second firing {reused:?}");
    assert!(
        reused < compiled,
        "{reused:?} is not less than {compiled:?}"
    );

    // Same value, and a receipt of the same bytes, as before the cache.
    assert_eq!(first, second);
    assert_eq!(first["permission"]["permission"], "deny", "{first}");
    assert_eq!(first_receipt.to_json(), second_receipt.to_json());
    let pinned: Value = serde_json::from_str(RECEIPT).unwrap();
    assert_eq!(first_receipt.to_json(), pinned);
    let input = input();
    let snapshot = Snapshot::default();
    let handles = BTreeMap::new();
    let verdict = replay(&second_receipt, call(&wasm, &input, &snapshot, &handles));
    assert_eq!(verdict.verification(), "passed", "{verdict:?}");

    // A changed module (here, one more custom section) has another digest
    // and is compiled anew.
    let mut changed = wasm.clone();
    changed.extend_from_slice(&[0, 4, 3, b'o', b'a', b'x']);
    let (value, receipt, _) = fire(&changed);
    assert_eq!(module_stats().loaded, before.loaded + 2);
    assert_eq!(value, first);
    assert_ne!(receipt.module, first_receipt.module);
}
