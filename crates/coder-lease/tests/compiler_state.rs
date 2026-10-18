//! Keep rustup's crate-directory selection out of shared build state (#12113).
#![cfg(unix)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
#[ignore = "requires two installed Rust toolchains, kache, and OPENAGENTS_SCRATCH; runs offline"]
fn dependencies_use_the_workspace_compiler_and_state() {
    let scratch = std::env::var_os("OPENAGENTS_SCRATCH").expect("set OPENAGENTS_SCRATCH");
    let old = std::env::var("OPENAGENTS_TEST_OLD_TOOLCHAIN").expect("set old toolchain");
    let new = std::env::var("OPENAGENTS_TEST_NEW_TOOLCHAIN").expect("set new toolchain");
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join(".rustup"));
    let rustup = std::env::var_os("OPENAGENTS_TEST_RUSTUP").expect("set rustup path");
    let proxy = Path::new(&rustup).with_file_name("rustc");
    let dir = tempfile::Builder::new()
        .prefix("compiler-state-")
        .tempdir_in(scratch)
        .unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    for name in ["app", "dependency"] {
        fs::create_dir_all(root.join(name).join("src")).unwrap();
    }
    fs::write(root.join("app/Cargo.toml"), "[workspace]\n[package]\nname=\"compiler-app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[dependencies]\ncompiler-dependency={path=\"../dependency\"}\n").unwrap();
    fs::write(root.join("dependency/Cargo.toml"), "[workspace]\n[package]\nname=\"compiler-dependency\"\nversion=\"0.1.0\"\nedition=\"2021\"\n").unwrap();
    fs::write(
        root.join("app/src/main.rs"),
        "fn main(){assert_eq!(compiler_dependency::value(),42);}\n",
    )
    .unwrap();
    fs::write(
        root.join("dependency/src/lib.rs"),
        "pub fn value()->u32{42}\n",
    )
    .unwrap();
    for (name, toolchain) in [("app", &new), ("dependency", &old)] {
        fs::write(
            root.join(name).join("rust-toolchain.toml"),
            format!("[toolchain]\nchannel=\"{toolchain}\"\n"),
        )
        .unwrap();
    }
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/dev/cargo-build-env.sh");
    let kache = std::env::var_os("OPENAGENTS_TEST_KACHE").expect("set kache path");
    let run = |helper_enabled: bool, toolchain: Option<&str>, wrapper: Option<&OsStr>| {
        let mut command = Command::new("sh");
        command
            .current_dir(root.join("app"))
            .env_clear()
            .env("HOME", root.join("home"))
            .env(
                "PATH",
                format!(
                    "{}:/usr/bin:/bin",
                    Path::new(&rustup).parent().unwrap().display()
                ),
            )
            .env("RUSTUP_HOME", &rustup_home)
            .env("RUSTC", &proxy)
            .env("RUSTC_WRAPPER", wrapper.unwrap_or(OsStr::new("")))
            .env("KACHE_CACHE_DIR", root.join("kache"))
            .env("KACHE_RUNTIME_DIR", root.join("kache-runtime"))
            .env("KACHE_DAEMON_IDLE_TIMEOUT", "1")
            .env("KACHE_LOG", "debug")
            .env("CARGO_HOME", root.join("cargo-home"))
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CARGO_BUILD_BUILD_DIR", root.join("legacy-state"))
            .env("oa_build_manifest", root.join("app/Cargo.toml"));
        if let Some(toolchain) = toolchain {
            command.env("RUSTUP_TOOLCHAIN", toolchain);
        }
        command.arg("-c").arg(if helper_enabled {
            ". \"$1\" || exit; printf 'state=%s\\ncompiler=%s\\n' \"$CARGO_BUILD_BUILD_DIR\" \"$RUSTC\"; \"$2\" run --offline -v"
        } else { "\"$2\" build --offline -v" }).arg("fixture").arg(&helper).arg(env!("CARGO"));
        command.output().unwrap()
    };
    // Cargo probes in app/, but bare rustc chooses the older toolchain in the
    // dependency directory. Its fingerprint still names the newer compiler.
    let poisoned = run(false, None, None);
    assert!(!poisoned.status.success(), "{}", text(&poisoned));
    assert!(text(&poisoned).contains("E0514"), "{}", text(&poisoned));
    let fresh = run(false, None, None);
    assert!(
        text(&fresh).contains("Fresh compiler-dependency"),
        "{}",
        text(&fresh)
    );
    assert!(text(&fresh).contains("E0514"), "{}", text(&fresh));
    let pinned_retry = run(false, Some(&new), None);
    assert!(
        text(&pinned_retry).contains("Fresh compiler-dependency"),
        "{}",
        text(&pinned_retry)
    );
    assert!(
        text(&pinned_retry).contains("E0514"),
        "{}",
        text(&pinned_retry)
    );

    let mut states = Vec::new();
    for toolchain in [None, Some(old.as_str()), None] {
        let output = run(true, toolchain, None);
        assert!(output.status.success(), "{}", text(&output));
        let stdout = String::from_utf8_lossy(&output.stdout);
        states.push(
            stdout
                .lines()
                .find(|s| s.starts_with("state="))
                .unwrap()
                .to_owned(),
        );
        assert!(
            !stdout.contains(&proxy.display().to_string()),
            "pin the compiler, not rustup"
        );
    }
    assert_ne!(
        states[0], states[1],
        "different compilers need different state"
    );
    assert_eq!(states[0], states[2], "return to the same compiler state");
    assert!(
        root.join("legacy-state/debug/.fingerprint").exists(),
        "leave old state for the sweep"
    );
    // Populate and restore both compiler versions through a private kache store.
    // Remove only this fixture's intermediate files to force real cache restores.
    for (index, toolchain) in [None, Some(old.as_str())].into_iter().enumerate() {
        let state = Path::new(states[index].strip_prefix("state=").unwrap());
        for attempt in 0..2 {
            fs::remove_dir_all(state.join("debug")).unwrap();
            let restored = run(true, toolchain, Some(&kache));
            assert!(restored.status.success(), "{}", text(&restored));
            if attempt == 1 {
                let log = text(&restored);
                let hit = log
                    .lines()
                    .find(|line| line.contains("local cache hit for compiler_dependency"));
                assert!(hit.is_some(), "expected an actual cache restore: {log}");
                println!("{toolchain:?}: {}", hit.unwrap());
            }
        }
        let fresh = run(true, toolchain, Some(&kache));
        assert!(fresh.status.success(), "{}", text(&fresh));
        assert!(
            text(&fresh).contains("Fresh compiler-dependency"),
            "{}",
            text(&fresh)
        );
    }
    let listing = Command::new(&kache)
        .args(["list", "--json", "--no-pager", "compiler_dependency"])
        .env("KACHE_CACHE_DIR", root.join("kache"))
        .env("KACHE_RUNTIME_DIR", root.join("kache-runtime"))
        .output()
        .unwrap();
    assert!(listing.status.success(), "{}", text(&listing));
    let listing: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    let entries = listing["entries"].as_array().unwrap();
    assert_eq!(
        entries.len(),
        2,
        "one cache key per producing compiler: {listing}"
    );
    // TempDir removes every fixture, artifact, and scratch target.
}
