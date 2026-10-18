#![cfg(unix)]

use coder_lease::agent_builds::BUILD_ENV_SCRIPT;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MARKER: &str = ".metadata_never_index";
const BUILD_STOP: i32 = 73;
const DESKTOP_SCRIPT: &str = include_str!("../../../apps/openagents/scripts/run-macos-dev.sh");
const IOS_SCRIPT: &str = include_str!("../../../apps/openagents/scripts/ios/build-core.sh");

struct Fixture {
    _scratch: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new(platform: &str) -> Self {
        let scratch = tempfile::Builder::new()
            .prefix("spotlight scripts ")
            .tempdir()
            .unwrap();
        let root = scratch.path().join("app workspace");
        let home = scratch.path().join("test home");
        let bin = scratch.path().join("fake tools");
        for directory in [&root, &home.join(".cargo/bin"), &bin] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::create_dir_all(root.join("scripts/ios")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        fs::write(root.join("scripts/env-compat.sh"), "").unwrap();
        executable(
            &bin.join("uname"),
            &format!("#!/bin/sh\nprintf '%s\\n' '{platform}'\n"),
        );
        executable(&bin.join("pgrep"), "#!/bin/sh\nexit 1\n");
        for tool in ["open", "codesign", "security", "plutil", "xcrun"] {
            executable(&bin.join(tool), "#!/bin/sh\nexit 99\n");
        }
        Self {
            _scratch: scratch,
            root,
            home,
            bin,
        }
    }

    fn command(&self, shell: &str) -> Command {
        let mut command = Command::new(shell);
        command
            .env_clear()
            .env("HOME", &self.home)
            .env(
                "PATH",
                std::env::join_paths([
                    self.bin.as_path(),
                    self.home.join(".cargo/bin").as_path(),
                    Path::new("/usr/bin"),
                    Path::new("/bin"),
                ])
                .unwrap(),
            )
            .current_dir(&self.root);
        command
    }

    fn check_app(&self, relative_script: &str, source: &str, custom: bool, marked: bool) {
        let custom_target = custom.then(|| self.home.join("custom target"));
        self.check_app_from(
            relative_script,
            source,
            custom_target.as_deref(),
            marked,
            &self.root,
        );
    }

    fn check_app_from(
        &self,
        relative_script: &str,
        source: &str,
        custom_target: Option<&Path>,
        marked: bool,
        caller: &Path,
    ) {
        let target = self.root.join(custom_target.unwrap_or(Path::new("target")));
        fs::write(
            self.home.join("expected-target"),
            target.as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        fs::write(
            self.home.join("local-target"),
            self.root.join("target").as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        fs::write(
            self.home.join("expect-marker"),
            if marked { "yes" } else { "no" },
        )
        .unwrap();
        fs::write(
            self.home.join("metadata.json"),
            serde_json::json!({"target_directory": target}).to_string(),
        )
        .unwrap();
        executable(
            &self.home.join(".cargo/bin/cargo"),
            r#"#!/bin/sh
set -eu
case "$1" in
  metadata) cat "$HOME/metadata.json" ;;
  build)
    target=$(cat "$HOME/expected-target")
    local_target=$(cat "$HOME/local-target")
    expected=$(cat "$HOME/expect-marker")
    for directory in "$target" "$local_target"; do
      if [ "$expected" = yes ]; then
        [ -f "$directory/.metadata_never_index" ] || exit 91
      else
        [ ! -e "$directory/.metadata_never_index" ] || exit 92
      fi
    done
    exit 73
    ;;
  *) exit 93 ;;
esac
"#,
        );
        // Cargo's JSON parsing is outside this test; keep it independent of Python.
        executable(
            &self.bin.join("python3"),
            "#!/bin/sh\ncat >/dev/null\ncat \"$HOME/expected-target\"\n",
        );
        let script = self.root.join(relative_script);
        fs::write(&script, source).unwrap();
        let mut command = self.command("/bin/bash");
        command.arg(script).current_dir(caller);
        if let Some(custom_target) = custom_target {
            command.env("CARGO_TARGET_DIR", custom_target);
        }
        assert_status(command.output().unwrap(), BUILD_STOP);
        assert_eq!(target.join(MARKER).is_file(), marked);
        assert_eq!(self.root.join("target").join(MARKER).is_file(), marked);
    }

    fn source_build_env(&self) -> PathBuf {
        let target = self.home.join("shared target");
        let rustc = self.bin.join("rustc");
        executable(
            &rustc,
            "#!/bin/sh\n[ \"$1\" = -vV ] || exit 94\nprintf '%s\\n' 'rustc 1.98.1 (fixture)' 'host: aarch64-apple-darwin'\n",
        );
        let helper = self.root.join("build-env.sh");
        fs::write(&helper, BUILD_ENV_SCRIPT).unwrap();
        let output = self
            .command("/bin/sh")
            .args(["-c", ". \"$1\"", "fixture"])
            .arg(helper)
            .env("oa_build_manifest", self.root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &target)
            .env("RUSTC", rustc)
            .output()
            .unwrap();
        assert_status(output, 0);
        assert!(target.join("worktrees").is_dir());
        target
    }
}

fn executable(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn assert_status(output: Output, status: i32) {
    assert_eq!(
        output.status.code(),
        Some(status),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn desktop_marks_default_and_custom_targets_before_build() {
    for custom in [false, true] {
        Fixture::new("Darwin").check_app("scripts/run-macos-dev.sh", DESKTOP_SCRIPT, custom, true);
    }
}

#[test]
fn desktop_does_not_mark_targets_on_linux() {
    for custom in [false, true] {
        Fixture::new("Linux").check_app("scripts/run-macos-dev.sh", DESKTOP_SCRIPT, custom, false);
    }
}

#[test]
fn ios_marks_default_and_custom_targets_before_build() {
    for custom in [false, true] {
        Fixture::new("Darwin").check_app("scripts/ios/build-core.sh", IOS_SCRIPT, custom, true);
    }
}

#[test]
fn ios_does_not_mark_targets_on_linux() {
    for custom in [false, true] {
        Fixture::new("Linux").check_app("scripts/ios/build-core.sh", IOS_SCRIPT, custom, false);
    }
}

#[test]
fn ios_resolves_relative_target_from_app_workspace() {
    let fixture = Fixture::new("Darwin");
    let relative_target = Path::new("relative cargo target");
    fixture.check_app_from(
        "scripts/ios/build-core.sh",
        IOS_SCRIPT,
        Some(relative_target),
        true,
        &fixture.home,
    );
    assert!(!fixture.home.join(relative_target).exists());
}

#[test]
fn build_env_marks_fresh_targets_and_repairs_missing_markers() {
    let fixture = Fixture::new("Darwin");
    let target = fixture.source_build_env();
    assert!(target.join(MARKER).is_file());
    fs::remove_file(target.join(MARKER)).unwrap();
    assert_eq!(fixture.source_build_env(), target);
    assert!(target.join(MARKER).is_file());
}

#[test]
fn build_env_does_not_mark_targets_on_linux() {
    let fixture = Fixture::new("Linux");
    let target = fixture.source_build_env();
    assert!(!target.join(MARKER).exists());
}
