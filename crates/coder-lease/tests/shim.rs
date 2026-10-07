//! The `cargo` lease shim, run under a scratch shim directory with a
//! stand-in `cargo` and a stand-in `openagents` that records the lease it
//! was asked for.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

fn script(path: &Path, body: &str) {
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Scratch {
    _dir: tempfile::TempDir,
    shims: std::path::PathBuf,
    bin: std::path::PathBuf,
    path: std::ffi::OsString,
}

fn scratch() -> Scratch {
    let dir = tempfile::tempdir().unwrap();
    let shims = dir.path().join("lease-shims");
    coder_lease::shim::install(&shims).unwrap();
    let tools = dir.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    script(
        &tools.join("cargo"),
        "echo \"real cargo $* leases=${OPENAGENTS_LEASES:-none}\"",
    );
    // The stand-in `openagents lease build --keep-target-dir -- CMD...`
    // records its arguments and runs CMD under the build lease.
    let bin = dir.path().join("openagents");
    script(
        &bin,
        "echo \"lease $1 $2 $3\"; shift 4; OPENAGENTS_LEASES=build exec \"$@\"",
    );
    let path = coder_lease::shim::path_with(
        &shims,
        Some(std::ffi::OsStr::new(&format!(
            "{}:/usr/bin:/bin",
            tools.display()
        ))),
    );
    Scratch {
        _dir: dir,
        shims,
        bin,
        path,
    }
}

fn cargo(scratch: &Scratch, args: &[&str], leases: Option<&str>) -> String {
    let mut command = Command::new(scratch.shims.join("cargo"));
    command
        .args(args)
        .env_clear()
        .env("PATH", &scratch.path)
        .env("HOME", scratch.shims.parent().unwrap())
        .env(coder_lease::shim::BIN_VAR, &scratch.bin);
    if let Some(leases) = leases {
        command.env("OPENAGENTS_LEASES", leases);
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn the_shim_leases_heavy_subcommands_and_passes_the_rest_through() {
    let scratch = scratch();
    let test = cargo(&scratch, &["test", "-p", "x"], None);
    assert!(
        test.starts_with("lease lease build --keep-target-dir"),
        "{test}"
    );
    assert!(test.contains("real cargo test -p x leases=build"), "{test}");
    let toolchain = cargo(&scratch, &["+nightly", "--color", "never", "clippy"], None);
    assert!(toolchain.starts_with("lease "), "{toolchain}");

    for light in [
        &["fmt"][..],
        &["metadata", "--format-version", "1"],
        &["tree"],
    ] {
        let out = cargo(&scratch, light, None);
        assert!(out.starts_with("real cargo "), "{out}");
        assert!(out.contains("leases=none"), "{out}");
    }
}

#[test]
fn the_shim_passes_through_under_a_build_lease() {
    let scratch = scratch();
    let out = cargo(&scratch, &["build"], Some("quiet,build"));
    assert_eq!(out.trim(), "real cargo build leases=quiet,build");
}

#[test]
fn the_shim_without_openagents_runs_cargo_and_says_so() {
    let scratch = scratch();
    let output = Command::new(scratch.shims.join("cargo"))
        .arg("check")
        .env_clear()
        .env("PATH", &scratch.path)
        .env("HOME", scratch.shims.parent().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("real cargo check"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("without a build lease"));
}
