//! `verify` and `finish` against a fake checker (#11229): no failed,
//! unavailable or empty check becomes `pass`, `finish` never reuses a
//! verdict, and the cache follows the whole candidate.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use super::*;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    checker: PathBuf,
    calls: PathBuf,
}

fn git(root: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A repo with crate `x` and a checker whose behavior is `mode`:
/// it prints `output` and exits `code`, counting each call.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(root.join("crates/x/src")).unwrap();
    std::fs::write(
        root.join("crates/x/Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/x/src/lib.rs"),
        "pub fn one() -> u8 { 1 }\n",
    )
    .unwrap();
    std::fs::write(root.join("crates/x/src/old.rs"), "pub fn old() {}\n").unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "base"]);
    let checker = dir.path().join("check.sh");
    let calls = dir.path().join("calls");
    let fx = Fixture {
        root: root.canonicalize().unwrap(),
        checker,
        calls,
        _dir: dir,
    };
    fx.mode("", 0);
    fx
}

impl Fixture {
    /// Rewrites the checker to print `output` and exit `code`.
    fn mode(&self, output: &str, code: i32) {
        let script = format!(
            "#!/bin/sh\necho call >> '{}'\ncat <<'OUT'\n{output}\nOUT\nexit {code}\n",
            self.calls.display()
        );
        std::fs::write(&self.checker, script).unwrap();
        let mut perms = std::fs::metadata(&self.checker).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&self.checker, perms).unwrap();
    }

    fn calls(&self) -> usize {
        std::fs::read_to_string(&self.calls)
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    fn verify(&self) -> Verify {
        self.verify_with(self.checker.clone())
    }

    fn verify_with(&self, exec: PathBuf) -> Verify {
        Verify {
            root: self.root.clone(),
            exec,
            crates: vec!["x".into()],
            baseline: BTreeSet::new(),
            done_when: vec!["it works".into()],
            cochange: BTreeMap::new(),
            log: None,
            cache: std::sync::Mutex::new(None),
            tally: Arc::default(),
        }
    }

    fn add_test(&self) {
        std::fs::write(
            self.root.join("crates/x/src/lib.rs"),
            "pub fn one() -> u8 { 1 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn one_is_one() {\n        assert_eq!(super::one(), 1);\n    }\n}\n",
        )
        .unwrap();
    }
}

const PASS: &str = "test result: ok. 1 passed; 0 failed; 0 ignored\n@@test_exit=0\n@@fmt_exit=0";

fn status(verdict: &Value) -> &str {
    verdict["status"].as_str().unwrap_or("?")
}

#[tokio::test]
async fn a_real_pass_is_pass() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    assert_eq!(status(&fx.verify().run("", false).await), "pass");
}

#[tokio::test]
async fn a_fast_check_that_exits_nonzero_with_no_output_is_a_checker_error() {
    let fx = fixture();
    fx.add_test();
    fx.mode("", 1);
    let verdict = fx.verify().run("", true).await;
    assert_eq!(status(&verdict), "checker_error", "{verdict}");
}

#[tokio::test]
async fn a_full_check_that_exits_nonzero_with_no_output_is_a_checker_error() {
    let fx = fixture();
    fx.add_test();
    fx.mode("", 1);
    let verdict = fx.verify().run("", false).await;
    assert_eq!(status(&verdict), "checker_error", "{verdict}");
}

#[tokio::test]
async fn a_checker_that_cannot_start_is_a_checker_error() {
    let fx = fixture();
    fx.add_test();
    let verify = fx.verify_with(fx.root.join("no-such-checker"));
    assert_eq!(status(&verify.run("", false).await), "checker_error");
    assert_eq!(status(&verify.run("", true).await), "checker_error");
}

#[tokio::test]
async fn a_timed_out_check_is_a_checker_error() {
    let fx = fixture();
    fx.add_test();
    fx.mode("test result: ok. 1 passed; 0 failed", 124);
    let verdict = fx.verify().run("", false).await;
    assert_eq!(status(&verdict), "checker_error", "{verdict}");
    fx.mode("", 124);
    assert_eq!(status(&fx.verify().run("", true).await), "checker_error");
}

#[tokio::test]
async fn output_without_exit_markers_is_a_checker_error() {
    let fx = fixture();
    fx.add_test();
    fx.mode("test result: ok. 3 passed; 0 failed", 0);
    let verdict = fx.verify().run("", false).await;
    assert_eq!(status(&verdict), "checker_error", "{verdict}");
}

#[tokio::test]
async fn tests_that_exit_nonzero_without_a_named_failure_are_a_checker_error() {
    let fx = fixture();
    fx.add_test();
    fx.mode(
        "test result: ok. 1 passed; 0 failed\n@@test_exit=101\n@@fmt_exit=0",
        101,
    );
    assert_eq!(status(&fx.verify().run("", false).await), "checker_error");
}

#[tokio::test]
async fn zero_tests_run_is_never_pass() {
    let fx = fixture();
    fx.add_test();
    fx.mode(
        "test result: ok. 0 passed; 0 failed\n@@test_exit=0\n@@fmt_exit=0",
        0,
    );
    let verdict = fx.verify().run("nothing_matches", false).await;
    assert_eq!(status(&verdict), "no_tests", "{verdict}");
}

#[tokio::test]
async fn a_failed_fmt_is_never_pass() {
    let fx = fixture();
    fx.add_test();
    fx.mode(
        "test result: ok. 1 passed; 0 failed\n@@test_exit=0\n@@fmt_exit=1",
        0,
    );
    assert_eq!(status(&fx.verify().run("", false).await), "fmt_failed");
}

#[tokio::test]
async fn a_failing_test_is_a_test_failure() {
    let fx = fixture();
    fx.add_test();
    fx.mode(
        "---- tests::one_is_one stdout ----\nthread 'tests::one_is_one' panicked at crates/x/src/lib.rs:6:9:\nassertion failed\n\ntest result: FAILED. 0 passed; 1 failed\n@@test_exit=101\n@@fmt_exit=0",
        101,
    );
    let verdict = fx.verify().run("", false).await;
    assert_eq!(status(&verdict), "test_failure");
    assert_eq!(verdict["failing_tests"][0]["line"], 6);
}

#[tokio::test]
async fn finish_requires_a_test_the_change_adds() {
    let fx = fixture();
    std::fs::write(
        fx.root.join("crates/x/src/lib.rs"),
        "pub fn one() -> u8 { 2 }\n",
    )
    .unwrap();
    fx.mode(PASS, 0);
    let verdict = fx.verify().final_check().await;
    assert_eq!(status(&verdict), "no_tests", "{verdict}");
    assert_eq!(fx.calls(), 0, "no check runs without a required test");
}

#[tokio::test]
async fn finish_runs_the_added_tests_and_never_a_cached_verdict() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    assert_eq!(status(&verify.run("", false).await), "pass");
    let first = verify.final_check().await;
    assert_eq!(status(&first), "pass", "{first}");
    assert_eq!(first["required_tests"][0], "one_is_one");
    assert_eq!(fx.calls(), 2);
    // The checker now fails: finish must see it, though nothing changed.
    fx.mode("", 1);
    assert_eq!(status(&verify.final_check().await), "checker_error");
    assert_eq!(fx.calls(), 3);
}

#[tokio::test]
async fn finish_is_not_pass_when_fewer_tests_ran_than_it_requires() {
    let fx = fixture();
    std::fs::write(
        fx.root.join("crates/x/src/lib.rs"),
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn a() {}\n    #[test]\n    fn b() {}\n}\n",
    )
    .unwrap();
    fx.mode(PASS, 0);
    assert_eq!(status(&fx.verify().final_check().await), "no_tests");
}

#[tokio::test]
async fn an_unchanged_candidate_is_cached() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    verify.run("", false).await;
    let again = verify.run("", false).await;
    assert_eq!(again["cached"], true);
    assert_eq!(fx.calls(), 1);
}

#[tokio::test]
async fn new_bytes_in_an_untracked_file_run_again() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    let new = fx.root.join("crates/x/src/new.rs");
    std::fs::write(&new, "pub fn a() {}\n").unwrap();
    verify.run("", false).await;
    std::fs::write(&new, "pub fn b() {}\n").unwrap();
    let again = verify.run("", false).await;
    assert!(again.get("cached").is_none(), "{again}");
    assert_eq!(fx.calls(), 2);
}

#[tokio::test]
async fn staged_content_runs_again() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    verify.run("", false).await;
    // Stage one version, then put the working file back: an unstaged-only
    // view would see no change.
    let lib = fx.root.join("crates/x/src/lib.rs");
    let before = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(&lib, format!("{before}// staged\n")).unwrap();
    git(&fx.root, &["add", "crates/x/src/lib.rs"]);
    verify.run("", false).await;
    assert_eq!(fx.calls(), 2);
}

#[tokio::test]
async fn a_removed_file_runs_again() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    verify.run("", false).await;
    std::fs::remove_file(fx.root.join("crates/x/src/old.rs")).unwrap();
    verify.run("", false).await;
    assert_eq!(fx.calls(), 2);
}

#[tokio::test]
async fn a_different_scope_or_checker_runs_again() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    verify.run("", false).await;
    verify.run("one_is_one", false).await;
    assert_eq!(fx.calls(), 2, "a new filter");
    verify.run("one_is_one", true).await;
    assert_eq!(fx.calls(), 3, "another mode");
    fx.mode(&format!("{PASS}\n# another checker"), 0);
    verify.run("one_is_one", true).await;
    assert_eq!(fx.calls(), 4, "another checker");
}

#[test]
fn added_tests_reads_the_test_functions_a_diff_adds() {
    let diff =
        "+#[test]\n+fn a() {}\n+    #[tokio::test]\n+    async fn b() {}\n #[test]\n fn old() {}\n";
    assert_eq!(added_tests(diff), vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn the_tally_counts_calls_failures_and_the_last_status() {
    let mut tally = Tally::default();
    assert_eq!(
        tally.to_json(),
        json!({"calls": 0, "failures": 0, "passed": false, "last": null})
    );
    for status in [
        "compile_error",
        "compiles",
        "test_failure",
        "pass",
        "fmt_failed",
    ] {
        tally.record(status);
    }
    assert_eq!(
        tally.to_json(),
        json!({"calls": 5, "failures": 3, "passed": true, "last": "fmt_failed"})
    );
}

#[tokio::test]
async fn cached_verify_answers_count_in_the_tally() {
    let fx = fixture();
    fx.add_test();
    fx.mode(PASS, 0);
    let verify = fx.verify();
    verify.run("", false).await;
    let cached = verify.run("", false).await;
    assert_eq!(cached["cached"], json!(true), "{cached}");
    let tally = verify.tally.lock().unwrap().clone();
    assert_eq!(tally.calls, 2);
    assert_eq!(tally.failures, 0);
    assert!(tally.passed);
    assert_eq!(tally.last.as_deref(), Some("pass"));
}
