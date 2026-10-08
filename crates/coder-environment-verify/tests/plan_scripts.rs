//! The verifier's lock and inventory scripts, run with the local `sh`
//! against a scratch checkout and home.

use coder_environment::digest;
use coder_environment_verify::plan::{inventory_script, locks_ok, locks_script, parse_inventory};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn put(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn sh(script: &str, cwd: &Path, home: &Path) -> (i32, String) {
    let out = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", home)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn locks_must_hash_to_their_frozen_digests() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    put(&work.join("Cargo.lock"), "version = 3\n");
    put(&work.join("web/package-lock.json"), "{}\n");
    let locks = BTreeMap::from([
        ("Cargo.lock".to_string(), digest(b"version = 3\n")),
        ("web/package-lock.json".to_string(), digest(b"{}\n")),
    ]);
    let (code, out) = sh(&locks_script(&locks), &work, dir.path());
    assert_eq!(code, 0, "{out}");
    assert!(locks_ok(&out, &locks).is_ok(), "{out}");

    put(&work.join("Cargo.lock"), "version = 4\n");
    std::fs::remove_file(work.join("web/package-lock.json")).unwrap();
    let (code, out) = sh(&locks_script(&locks), &work, dir.path());
    assert_eq!(code, 3);
    let why = locks_ok(&out, &locks).unwrap_err();
    assert!(why.contains("mismatch") || why.contains("missing"), "{why}");
    assert!(
        out.contains("oa-lock missing web/package-lock.json"),
        "{out}"
    );
}

#[test]
fn the_inventory_sees_every_change_under_declared_paths() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let work = dir.path().join("work");
    put(&work.join("target/release/app"), "binary");
    put(&work.join("target/release/deps/a.rlib"), "dep");
    put(&home.join(".cargo/registry/index"), "idx");
    std::fs::create_dir_all(&home).unwrap();
    let paths = vec![
        "target".to_string(),
        "~/.cargo".to_string(),
        "absent-dir".into(),
    ];
    let script = inventory_script(&paths);
    let run = || {
        let (code, out) = sh(&script, &work, &home);
        assert_eq!(code, 0, "{out}");
        parse_inventory(&out).unwrap()
    };
    let before = run();
    assert_eq!(before["absent-dir"], "absent");
    assert_eq!(before["target"].len(), 64);
    assert_eq!(before["~/.cargo"].len(), 64);
    // Idempotent: the same tree fingerprints the same.
    assert_eq!(run(), before);

    put(&work.join("target/release/app"), "rebuilt");
    let after = run();
    assert_ne!(after["target"], before["target"]);
    assert_eq!(after["~/.cargo"], before["~/.cargo"]);

    put(&home.join(".cargo/registry/new"), "fetched");
    assert_ne!(run()["~/.cargo"], before["~/.cargo"]);

    // Output without its completion line is not an inventory.
    assert!(parse_inventory("oa-inventory target abc\n").is_none());
}
