//! The packet ABI, exercised through [`plugin::invoke`].

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use plugin::{Entry, HostError, Limits, Profile, Snapshot, derivative, invoke, representation};
use serde_json::json;

#[allow(clippy::too_many_arguments)]
fn run(
    wasm: &[u8],
    profile: Profile,
    operation: &str,
    snapshot: &Snapshot,
    handles: &BTreeMap<String, String>,
    limits: Limits,
    cancelled: bool,
    required: bool,
) -> Result<plugin::GuestValue, HostError> {
    let flag = Arc::new(AtomicBool::new(cancelled));
    invoke(plugin::Call {
        wasm,
        profile,
        invocation: "inv-1",
        operation,
        input: &json!({"topic": "notes"}),
        snapshot,
        handles,
        limits,
        cancelled: flag,
        required,
    })
}

fn wat_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn response(body: &str) -> Vec<u8> {
    let escaped = wat_escape(body);
    wat::parse_str(format!(
        r#"
        (module
          (memory (export "memory") 1)
          (global $bump (mut i32) (i32.const 2048))
          (data (i32.const 32) "{escaped}")
          (func (export "oa_alloc") (param $n i32) (result i32)
            (local $p i32)
            (local.set $p (global.get $bump))
            (global.set $bump (i32.add (local.get $p) (local.get $n)))
            (local.get $p))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64)
            (i64.or
              (i64.shl (i64.extend_i32_u (i32.const 32)) (i64.const 32))
              (i64.extend_i32_u (i32.const {len})))))
        "#,
        len = body.len()
    ))
    .unwrap()
}

fn ok_body() -> String {
    serde_json::json!({
        "v": "openagents.plugin-packet.v1",
        "requires": [],
        "invocation": "inv-1",
        "status": "ok",
        "value": {"echo": true},
        "reason": null
    })
    .to_string()
}

#[test]
fn a_pure_guest_returns_its_value_and_frees_both_ranges() {
    let value = run(
        &response(&ok_body()),
        Profile::Pure,
        "echo",
        &Snapshot::default(),
        &BTreeMap::new(),
        Limits::default(),
        false,
        true,
    )
    .unwrap();
    assert_eq!(value.status, "ok");
    assert_eq!(value.value, json!({"echo": true}));
    assert_eq!(value.verification, "not_run");
}

#[test]
fn overlapping_output_null_length_fuel_and_wasi_are_refused() {
    let overlap = wat::parse_str(
        r#"
        (module
          (memory (export "memory") 1)
          (func (export "oa_alloc") (param i32) (result i32) (i32.const 100))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64)
            (i64.or (i64.shl (i64.const 100) (i64.const 32)) (i64.const 4))))
        "#,
    )
    .unwrap();
    let error = run(
        &overlap,
        Profile::Pure,
        "echo",
        &Snapshot::default(),
        &BTreeMap::new(),
        Limits::default(),
        false,
        true,
    )
    .unwrap_err();
    assert!(
        matches!(error, HostError::Malformed(ref detail) if detail.contains("overlap")),
        "{error}"
    );

    let null_len = wat::parse_str(
        r#"
        (module
          (memory (export "memory") 1)
          (func (export "oa_alloc") (param i32) (result i32) (i32.const 100))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64) (i64.const 4)))
        "#,
    )
    .unwrap();
    assert!(matches!(
        run(
            &null_len,
            Profile::Pure,
            "echo",
            &Snapshot::default(),
            &BTreeMap::new(),
            Limits::default(),
            false,
            true
        ),
        Err(HostError::Malformed(_))
    ));

    let spin = wat::parse_str(
        r#"
        (module
          (memory (export "memory") 1)
          (func (export "oa_alloc") (param i32) (result i32) (i32.const 100))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64)
            (loop $again (br $again))
            (i64.const 0)))
        "#,
    )
    .unwrap();
    let limits = Limits {
        fuel: 1_000,
        ..Limits::default()
    };
    assert!(matches!(
        run(
            &spin,
            Profile::Pure,
            "echo",
            &Snapshot::default(),
            &BTreeMap::new(),
            limits,
            false,
            true
        ),
        Err(HostError::Limit(_))
    ));

    let wasi = wat::parse_str(
        r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "oa_alloc") (param i32) (result i32) (i32.const 0))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64) (i64.const 0)))
        "#,
    )
    .unwrap();
    assert!(matches!(
        run(
            &wasi,
            Profile::Pure,
            "echo",
            &Snapshot::default(),
            &BTreeMap::new(),
            Limits::default(),
            false,
            true
        ),
        Err(HostError::Denied(_))
    ));
}

#[test]
fn a_foreign_handle_is_stale_and_cancellation_stops_the_host_call() {
    let body = ok_body();
    let escaped = wat_escape(&body);
    let import_raw = r#"{"v":1,"handle":"other-run","operation":"metadata","args":{}}"#;
    let import = wat_escape(import_raw);
    let guest = wat::parse_str(format!(
        r#"
        (module
          (import "oa_host" "call" (func $call (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (data (i32.const 32) "{escaped}")
          (data (i32.const 400) "{import}")
          (func (export "oa_alloc") (param i32) (result i32) (i32.const 800))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64)
            (if (i32.ne (call $call (i32.const 400) (i32.const {import_len}) (i32.const 600) (i32.const 100)) (i32.const -5))
              (then (unreachable)))
            (i64.or
              (i64.shl (i64.extend_i32_u (i32.const 32)) (i64.const 32))
              (i64.extend_i32_u (i32.const {len})))))
        "#,
        import_len = import_raw.len(),
        len = body.len()
    ))
    .unwrap();
    let mut handles = BTreeMap::new();
    handles.insert("root".to_string(), "h1".to_string());
    let value = run(
        &guest,
        Profile::SnapshotRead,
        "echo",
        &Snapshot::default(),
        &handles,
        Limits::default(),
        false,
        true,
    )
    .unwrap();
    assert_eq!(value.value["echo"], json!(true));

    let blocked_raw = r#"{"v":1,"handle":"h1","operation":"metadata","args":{}}"#;
    let blocked_import = wat_escape(blocked_raw);
    let blocked = wat::parse_str(format!(
        r#"
        (module
          (import "oa_host" "call" (func $call (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (data (i32.const 0) "{blocked_import}")
          (func (export "oa_alloc") (param i32) (result i32) (i32.const 200))
          (func (export "oa_free") (param i32 i32))
          (func (export "oa_handle") (param i32 i32) (result i64)
            (call $call (i32.const 0) (i32.const {import_len}) (i32.const 300) (i32.const 64))
            drop
            (i64.const 0)))
        "#,
        import_len = blocked_raw.len()
    ))
    .unwrap();
    assert!(matches!(
        run(
            &blocked,
            Profile::SnapshotRead,
            "echo",
            &Snapshot::default(),
            &handles,
            Limits::default(),
            true,
            true
        ),
        Err(HostError::Cancelled)
    ));
}

#[test]
fn the_host_rule_owns_representation_and_a_partial_file_is_not_complete() {
    assert_eq!(representation("host-rule", &["b", "a"]), "host-rule");
    assert_eq!(representation("host-rule", &["a", "b"]), "host-rule");
    assert_eq!(
        derivative(b"ab", false, true).unwrap_err(),
        "partial capture"
    );
    let optional = run(
        &response(
            &serde_json::json!({
                "v": "openagents.plugin-packet.v1",
                "requires": [],
                "invocation": "inv-1",
                "status": "refused",
                "value": null,
                "reason": "no"
            })
            .to_string(),
        ),
        Profile::Pure,
        "echo",
        &Snapshot::default(),
        &BTreeMap::new(),
        Limits::default(),
        false,
        false,
    )
    .unwrap();
    assert_eq!(optional.value["fallback"], json!(true));
    assert!(matches!(
        run(
            &response(
                &serde_json::json!({
                    "v": "openagents.plugin-packet.v1",
                    "requires": [],
                    "invocation": "inv-1",
                    "status": "refused",
                    "value": null,
                    "reason": "no"
                })
                .to_string(),
            ),
            Profile::Pure,
            "echo",
            &Snapshot::default(),
            &BTreeMap::new(),
            Limits::default(),
            false,
            true,
        ),
        Err(HostError::Refused(_))
    ));
}

#[test]
fn the_outline_guest_lists_only_the_granted_snapshot() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/outline.wasm");
    let wasm = std::fs::read(path).expect("outline.wasm is built from crates/plugin-outline");
    let mut snapshot = Snapshot::default();
    snapshot
        .insert(
            "note",
            Entry::File {
                bytes: b"hello".to_vec(),
                version: "v1".into(),
                complete: true,
            },
        )
        .unwrap();
    snapshot
        .insert(
            "root",
            Entry::Directory {
                children: vec!["note".into()],
            },
        )
        .unwrap();
    let mut handles = BTreeMap::new();
    handles.insert("root".to_string(), "h-root".to_string());
    let limits = Limits {
        fuel: 50_000_000,
        ..Limits::default()
    };
    let value = run(
        &wasm,
        Profile::SnapshotRead,
        "outline",
        &snapshot,
        &handles,
        limits,
        false,
        true,
    )
    .expect("outline guest");
    assert_eq!(value.value["kind"], json!("outline"));
    assert_eq!(value.value["entries"], json!(["note"]));

    let pure = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/pure.wasm")).unwrap();
    let echoed = run(
        &pure,
        Profile::Pure,
        "echo",
        &Snapshot::default(),
        &BTreeMap::new(),
        limits,
        false,
        true,
    )
    .unwrap();
    assert_eq!(echoed.value["topic"], json!("notes"));
    let pdk = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugin-pdk/src/lib.rs"
    ))
    .unwrap();
    let receipt = plugin::build_receipt(&pdk, &wasm, "snapshot-read");
    let pure_receipt = plugin::build_receipt(&pdk, &pure, "pure");
    assert!(receipt.pdk_digest.starts_with("sha256:"));
    assert_ne!(receipt.guest_digest, pure_receipt.guest_digest);
    assert_eq!(receipt.pdk_digest, pure_receipt.pdk_digest);
}
