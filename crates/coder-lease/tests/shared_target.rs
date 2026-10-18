//! Reproduce #12071 with real Cargo and kache, in disposable Git worktrees.
#![cfg(unix)]

use std::ffi::OsString;
use std::fs::{self, File, FileTimes};
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

use coder_lease::agent_builds::Gate;

fn checked(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{command:?}: {output:?}");
    output
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn script(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn git(repo: &Path, args: &[&str]) {
    checked(
        Command::new("git")
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args),
    );
}

fn libraries(target: &Path, prefix: &str) -> Vec<PathBuf> {
    fs::read_dir(target.join("debug/deps"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "rlib")
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(prefix)
        })
        .collect()
}

#[test]
#[ignore = "requires real Cargo, kache, and OPENAGENTS_SCRATCH; runs offline"]
fn two_worktrees_link_their_own_source_in_one_target() {
    let scratch = std::env::var_os("OPENAGENTS_SCRATCH").expect("set OPENAGENTS_SCRATCH");
    let kache = std::env::var_os("OPENAGENTS_TEST_KACHE").expect("set OPENAGENTS_TEST_KACHE");
    let scratch = tempfile::Builder::new()
        .prefix("shared-target-")
        .tempdir_in(scratch)
        .unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let repo = root.join("repo");
    let other = root.join("other");
    // Both trees' different sources predate every artifact. A plain Cargo build
    // would otherwise call the second tree Fresh and link the first tree's lib.
    write(
        &repo.join("Cargo.toml"),
        "[workspace]\nmembers=[\"source\"]\nexclude=[\"excluded\"]\nresolver=\"3\"\n\
         [package]\nname=\"worktree-probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\nsource={path=\"source\"}\nexcluded={path=\"excluded\"}\nshared-dependency={path=\"../dependency\"}\n",
    );
    write(
        &repo.join("source/Cargo.toml"),
        "[package]\nname=\"source\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    write(
        &root.join("dependency/Cargo.toml"),
        "[package]\nname=\"shared-dependency\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    write(
        &root.join("dependency/src/lib.rs"),
        "pub fn value() -> u8 { 7 }\n",
    );
    write(
        &repo.join("src/main.rs"),
        "fn main() { assert_eq!(shared_dependency::value(), 7); println!(\"{} {}\", source::value(), excluded::value()); }\n",
    );
    write(
        &repo.join("source/src/lib.rs"),
        "pub fn value() -> u8 { 1 }\n",
    );
    write(
        &repo.join("Cargo.lock"),
        "version=4\n[[package]]\nname=\"worktree-probe\"\nversion=\"0.1.0\"\n\
         dependencies=[\"source\",\"excluded\",\"shared-dependency\"]\n\
         [[package]]\nname=\"source\"\nversion=\"0.1.0\"\n\
         [[package]]\nname=\"excluded\"\nversion=\"0.1.0\"\n\
         [[package]]\nname=\"shared-dependency\"\nversion=\"0.1.0\"\n",
    );
    write(
        &repo.join("excluded/Cargo.toml"),
        "[workspace]\n[package]\nname=\"excluded\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    write(
        &repo.join("excluded/src/lib.rs"),
        "pub fn value() -> u8 { 10 }\n",
    );
    // Model apps/openagents linking a root domain crate outside its workspace.
    write(
        &repo.join("app/Cargo.toml"),
        "[workspace]\n[package]\nname=\"app-probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\nsource={path=\"../source\"}\n",
    );
    write(
        &repo.join("app/Cargo.lock"),
        "version=4\n[[package]]\nname=\"app-probe\"\nversion=\"0.1.0\"\ndependencies=[\"source\"]\n\
         [[package]]\nname=\"source\"\nversion=\"0.1.0\"\n",
    );
    write(
        &repo.join("app/src/main.rs"),
        "fn main() { println!(\"{}\", source::value()); }\n",
    );
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "fixture"]);
    git(
        &repo,
        &["worktree", "add", "--detach", "-q", other.to_str().unwrap()],
    );
    write(
        &other.join("source/src/lib.rs"),
        "mod added;\npub fn value() -> u8 { added::value() }\n",
    );
    write(
        &other.join("source/src/added.rs"),
        "pub fn value() -> u8 { 2 }\n",
    );
    write(
        &other.join("excluded/src/lib.rs"),
        "pub fn value() -> u8 { 20 }\n",
    );
    for tree in [&repo, &other] {
        for source in [
            "source/src/lib.rs",
            "excluded/src/lib.rs",
            "src/main.rs",
            "app/src/main.rs",
        ] {
            File::options()
                .write(true)
                .open(tree.join(source))
                .unwrap()
                .set_times(
                    FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(3600)),
                )
                .unwrap();
        }
    }
    // A different worktree's dep-info cannot list this newly added module.
    // Keep both its parent and the new file older than the first build, with
    // no touch workaround when returning to the tree that added it.
    File::options()
        .write(true)
        .open(other.join("source/src/added.rs"))
        .unwrap()
        .set_times(FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(3600)))
        .unwrap();
    let cargo = Path::new(env!("CARGO"));
    let tools = root.join("tools");
    fs::create_dir(&tools).unwrap();
    symlink(cargo, tools.join("cargo")).unwrap();
    // No real lease table or account is touched by this fixture.
    script(
        &tools.join("openagents"),
        "#!/bin/sh\nshift 4\nexec \"$@\"\n",
    );
    let gate = Gate {
        shims: root.join("shims"),
        targets: root.join("targets"),
        bin: Some(tools.join("openagents")),
    };
    gate.install().unwrap();
    let path = OsString::from(format!("{}:/usr/bin:/bin", tools.display()));
    for (workspace, expected_targets) in [("", 1), ("app", 2)] {
        for (index, (tree, expected)) in [(&repo, "1"), (&other, "2"), (&repo, "1"), (&other, "2")]
            .into_iter()
            .enumerate()
        {
            let mut command = Command::new(gate.shims.join("cargo"));
            command
                .current_dir(tree.join(workspace))
                .env_clear()
                .envs(gate.vars(Some(&path)))
                .env("HOME", root.join("home"))
                .env("CARGO_HOME", root.join("cargo-home"))
                .env("CARGO_HTTP_TIMEOUT", "20")
                .env("RUSTC", cargo.parent().unwrap().join("rustc"))
                .env("RUSTC_WRAPPER", &kache)
                .env("KACHE_CACHE_DIR", root.join("kache"))
                .env("KACHE_RUNTIME_DIR", root.join("kache-runtime"))
                .env("KACHE_DAEMON_IDLE_TIMEOUT", "1")
                .env("CARGO_INCREMENTAL", "1") // The gate must force this off.
                .env("CARGO_BUILD_BUILD_DIR", root.join("wrong-shared-state"))
                .env("OPENAGENTS_AGENT_FLOOR_GIB", "0")
                .env("OPENAGENTS_LEASE_ROOT", root.join("leases"))
                .args(["run", "--offline", "--locked", "--quiet"]);
            if workspace.is_empty() && (index == 1 || index == 2) {
                let target = fs::read_dir(&gate.targets)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                command.env("CARGO_TARGET_DIR", target);
                if index == 1 {
                    command.env("OPENAGENTS_KEEP_TARGET_DIR", "1");
                } else {
                    command.env("OPENAGENTS_LEASES", "build");
                }
            }
            if index == 3 {
                // The caller's directory must not choose the other tree's hash.
                command
                    .current_dir(repo.join(workspace))
                    .arg("--manifest-path")
                    .arg(tree.join(workspace).join("Cargo.toml"));
            }
            let output = checked(&mut command);
            let expected = if workspace.is_empty() {
                format!("{expected} {}", if expected == "1" { 10 } else { 20 })
            } else {
                expected.into()
            };
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
        }
        assert_eq!(
            fs::read_dir(&gate.targets).unwrap().count(),
            expected_targets
        );
    }
    for entry in fs::read_dir(&gate.targets).unwrap() {
        let target = entry.unwrap().path();
        let mut states = Vec::new();
        for bucket in fs::read_dir(target.join("worktrees")).unwrap() {
            for workspace in fs::read_dir(bucket.unwrap().path()).unwrap() {
                let state = workspace.unwrap().path();
                assert!(state.join(".openagents-workspace").is_file());
                assert_eq!(libraries(&state, "libsource-").len(), 1);
                assert!(
                    !state.join("debug/incremental").exists()
                        || fs::read_dir(state.join("debug/incremental"))
                            .unwrap()
                            .next()
                            .is_none()
                );
                states.push(state);
            }
        }
        assert_eq!(
            states.len(),
            2,
            "one compiler state per workspace path, independent of agents"
        );
        if !target
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("repo-app-")
        {
            for state in &states {
                assert_eq!(libraries(state, "libexcluded-").len(), 1);
            }
            // Shared dependency artifacts have the same bytes and Cargo identity;
            // the content cache can restore them without a worktree-specific salt.
            let first = libraries(&states[0], "libshared_dependency-");
            let second = libraries(&states[1], "libshared_dependency-");
            assert_eq!(first.len(), 1);
            assert_eq!(second.len(), 1);
            assert_eq!(first[0].file_name(), second[0].file_name());
            assert_eq!(fs::read(&first[0]).unwrap(), fs::read(&second[0]).unwrap());
        }
    }
    // TempDir removes both worktrees, every scratch target, and the kache store.
}
