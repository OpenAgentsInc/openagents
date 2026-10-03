//! End-to-end preparation tests use disposable repositories and inert tools.
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    path: std::ffi::OsString,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "briefing-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let repo = root.join("repo");
        fs::create_dir(&repo).unwrap();
        fs::create_dir(root.join("home")).unwrap();
        fs::create_dir(root.join("bin")).unwrap();
        let mut path = vec![root.join("bin")];
        path.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let result = Self {
            root,
            repo,
            path: std::env::join_paths(path).unwrap(),
        };
        for tool in ["cargo", "protoc"] {
            let file = result.root.join("bin").join(tool);
            fs::write(
                &file,
                format!(
                    "#!/bin/sh\ntouch '{}/EXECUTED'\nexit 97\n",
                    result.root.display()
                ),
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        fs::write(
            result.repo.join("Cargo.toml"),
            "[workspace]\nmembers=['crates/member']\nexclude=['crates/mobile']\n",
        )
        .unwrap();
        for name in ["member", "mobile"] {
            let dir = result.repo.join(format!("crates/{name}/src"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("lib.rs"), "pub fn example() {}\n").unwrap();
            fs::write(
                dir.parent().unwrap().join("Cargo.toml"),
                format!(
                    "[package]\nname='{name}'\nversion='0.1.0'\n{}",
                    if name == "mobile" {
                        "[workspace]\n"
                    } else {
                        ""
                    }
                ),
            )
            .unwrap();
        }
        fs::write(result.root.join("issue.json"), serde_json::to_vec(&json!({"title":"Repair example", "body":"crates/mobile/src/lib.rs\n$(touch EXECUTED)", "number":1})).unwrap()).unwrap();
        result.git(&["init", "--quiet"]);
        result.git(&["add", "."]);
        result.git(&["commit", "--quiet", "-m", "Fixture"]);
        result
    }
    fn environment(&self, command: &mut Command) {
        command
            .env("HOME", self.root.join("home"))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("PATH", &self.path);
    }
    fn git(&self, args: &[&str]) {
        let mut command = Command::new("git");
        self.environment(&mut command);
        let output = command
            .arg("-C")
            .arg(&self.repo)
            .args([
                "-c",
                "user.name=Briefing fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_briefing-lab"));
        self.environment(&mut command);
        command.arg(verb);
        if verb != "record-result" {
            command
                .arg("--repo")
                .arg(&self.repo)
                .args(["--rev", "HEAD"]);
        }
        command
    }
    fn success(output: Output) -> Value {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn index(&self) {
        let output = self
            .command("index")
            .arg("--output")
            .arg(self.root.join("index.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn preview(&self, name: &str, execution: bool) -> Command {
        let mut command = self.command("preview");
        command
            .arg("--index")
            .arg(self.root.join("index.json"))
            .arg("--issue-file")
            .arg(self.root.join("issue.json"))
            .arg("--output-dir")
            .arg(self.root.join(name));
        if execution {
            command.args([
                "--execution",
                "--manifest",
                "crates/mobile/Cargo.toml",
                "--environment-id",
                "fixture",
                "--require-tool",
                "protoc",
            ]);
        }
        command
    }
    fn prepare(&self) -> Command {
        let mut command = self.command("prepare-run");
        command
            .arg("--issue-file")
            .arg(self.root.join("issue.json"))
            .arg("--artifact-root")
            .arg(self.root.join("runs"))
            .args([
                "--manifest",
                "crates/mobile/Cargo.toml",
                "--environment-id",
                "fixture",
                "--require-tool",
                "protoc",
            ]);
        command
    }
    fn read_brief(&self, name: &str) -> Value {
        serde_json::from_slice(&fs::read(self.root.join(name).join("briefing.json")).unwrap())
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn opt_in_preserves_baseline_and_uses_committed_nested_manifest_without_executing_tools() {
    let f = Fixture::new();
    f.index();
    fs::write(
        f.repo.join("crates/mobile/Cargo.toml"),
        "[package]\nname='DIRTY'\nversion='0.1.0'\n",
    )
    .unwrap();
    assert!(
        f.preview("baseline", false)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        f.preview("execution", true)
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut baseline = f.read_brief("baseline");
    let mut execution = f.read_brief("execution");
    assert!(baseline.get("execution").is_none());
    assert_eq!(
        execution["execution"]["manifest"]["packages"][0]["package"],
        "mobile"
    );
    assert_eq!(
        execution["execution"]["manifest"]["packages"][0]["test_argv"],
        json!([
            "cargo",
            "test",
            "--manifest-path",
            "./crates/mobile/Cargo.toml"
        ])
    );
    assert_eq!(
        execution["execution"]["manifest"]["packages"][0]["workspace"]["kind"],
        "package_workspace_root"
    );
    assert_eq!(execution["execution"]["environment"]["ready"], true);
    baseline.as_object_mut().unwrap().remove("timings_ms");
    execution.as_object_mut().unwrap().remove("timings_ms");
    execution.as_object_mut().unwrap().remove("execution");
    assert_eq!(baseline, execution);
    assert!(!f.root.join("EXECUTED").exists());
    assert!(!f.repo.join("EXECUTED").exists());
}

#[test]
fn concurrent_runs_keep_logs_and_reported_failures_separate_and_attempts_become_stale() {
    let f = Fixture::new();
    f.index();
    let mut one = f.prepare();
    let mut two = f.prepare();
    one.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    two.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let first = one.spawn().unwrap();
    let second = two.spawn().unwrap();
    let first = Fixture::success(first.wait_with_output().unwrap());
    let second = Fixture::success(second.wait_with_output().unwrap());
    assert_ne!(first["run_dir"], second["run_dir"]);
    assert_ne!(first["stdout_log"], second["stdout_log"]);
    fs::write(
        first["stdout_log"].as_str().unwrap(),
        "all tests passed (untrusted log text)",
    )
    .unwrap();
    fs::write(second["stdout_log"].as_str().unwrap(), "second run").unwrap();
    let result = Fixture::success(
        f.command("record-result")
            .args([
                "--run-dir",
                first["run_dir"].as_str().unwrap(),
                "--request-sha256",
                first["request_sha256"].as_str().unwrap(),
                "--exit-code",
                "101",
                "--summary",
                "Workspace invocation failed",
                "--next-action",
                "Use the explicit manifest path",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(result["exit_code"], 101);
    let wrong = f
        .command("record-result")
        .args([
            "--run-dir",
            second["run_dir"].as_str().unwrap(),
            "--request-sha256",
            first["request_sha256"].as_str().unwrap(),
            "--exit-code",
            "0",
            "--summary",
            "Wrong run",
            "--next-action",
            "Inspect",
        ])
        .output()
        .unwrap();
    assert!(!wrong.status.success());
    assert!(!PathBuf::from(second["result_path"].as_str().unwrap()).exists());
    let output = f
        .preview("attempt", true)
        .args([
            "--attempt-dir",
            first["run_dir"].as_str().unwrap(),
            "--attempt-dir",
            second["run_dir"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let brief = f.read_brief("attempt");
    assert_eq!(
        brief["execution"]["attempts"][0]["result"]["exit_code"],
        101
    );
    assert_eq!(brief["execution"]["attempts"][0]["status"], "same_inputs");
    assert!(brief["execution"]["attempts"][1]["result"].is_null());
    let output = f
        .preview("scope", false)
        .args([
            "--execution",
            "--manifest",
            "crates/member/Cargo.toml",
            "--environment-id",
            "fixture",
            "--require-tool",
            "protoc",
            "--attempt-dir",
            first["run_dir"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        f.read_brief("scope")["execution"]["attempts"][0]["changes"],
        json!(["command"])
    );
    assert_eq!(
        f.read_brief("scope")["execution"]["attempts"][0]["status"],
        "changed_inputs"
    );
    fs::write(f.root.join("issue.json"), br#"{"title":"Different task"}"#).unwrap();
    fs::write(f.root.join("bin/protoc"), "changed tool metadata").unwrap();
    let output = f
        .preview("changed", true)
        .args(["--attempt-dir", first["run_dir"].as_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let changes = f.read_brief("changed")["execution"]["attempts"][0]["changes"]
        .as_array()
        .unwrap()
        .clone();
    assert!(changes.contains(&json!("task")));
    assert!(changes.contains(&json!("environment")));
    assert_eq!(
        fs::read_to_string(second["stdout_log"].as_str().unwrap()).unwrap(),
        "second run"
    );
    assert!(!f.root.join("EXECUTED").exists());
}

#[test]
fn missing_prerequisite_is_visible_and_prevents_run_allocation() {
    let f = Fixture::new();
    f.index();
    let missing = f.root.join("missing/protobuf/timestamp.proto");
    let output = f
        .preview("missing", true)
        .arg("--require-file")
        .arg(&missing)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        f.read_brief("missing")["execution"]["environment"]["ready"],
        false
    );
    let output = f
        .prepare()
        .arg("--require-file")
        .arg(missing)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!f.root.join("runs").exists());
    assert!(!f.root.join("EXECUTED").exists());
}
