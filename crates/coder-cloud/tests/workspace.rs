use coder_cloud::{Mode, Placement, Record, Spec, Store, workspace};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@localhost",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into()
}
fn record(root: &Path) -> Record {
    Record::new(
        "workspace_test",
        Spec {
            placement: Placement::Boat,
            mode: Mode::Integrated,
            agent: "codex".into(),
            task: "Edit the admitted files".into(),
            model: None,
            reasoning: None,
            cwd: root.into(),
            timeout_seconds: 60,
            size: "small".into(),
            template: None,
            credential_names: vec![],
        },
    )
    .unwrap()
}
#[test]
fn dirty_and_admitted_untracked_files_round_trip_without_copying_other_files() {
    let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let local = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let remote = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    git(source.path(), &["init", "-q"]);
    fs::write(source.path().join("tracked.txt"), "baseline\n").unwrap();
    fs::write(source.path().join("other.txt"), "unselected\n").unwrap();
    git(source.path(), &["add", "."]);
    git(source.path(), &["commit", "-qm", "Fixture"]);
    fs::write(source.path().join("tracked.txt"), "caller changes\n").unwrap();
    fs::write(source.path().join("note.txt"), "admitted\n").unwrap();
    fs::write(source.path().join("unrelated.txt"), "do not send\n").unwrap();
    let store = Store::under(local.path());
    let lease = store.lease("workspace_test").unwrap();
    let mut r = record(source.path());
    r.workspace = Some(
        workspace::capture(
            &lease,
            source.path(),
            None,
            vec!["tracked.txt".into()],
            vec!["note.txt".into()],
        )
        .unwrap(),
    );
    fs::write(
        remote.path().join("input.json"),
        r.workspace.as_ref().unwrap().input().unwrap(),
    )
    .unwrap();
    let restore = workspace::restore_script(&r, remote.path().to_str().unwrap()).unwrap();
    let shell = |script: &str| {
        Command::new("sh")
            .args(["-c", script])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
    };
    let out = shell(&restore);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let w = remote.path().join("workspace");
    assert_eq!(
        fs::read_to_string(w.join("tracked.txt")).unwrap(),
        "caller changes\n"
    );
    assert!(w.join("note.txt").exists());
    assert!(!w.join("other.txt").exists());
    assert!(!w.join("unrelated.txt").exists());
    fs::write(w.join("tracked.txt"), "remote changes\n").unwrap();
    fs::write(w.join("note.txt"), "remote admitted changes\n").unwrap();
    fs::write(w.join("created.txt"), "new remote file\n").unwrap();
    // Reconnecting uses the original input marker and preserves remote edits.
    assert!(shell(&restore).status.success());
    assert_eq!(
        fs::read_to_string(w.join("tracked.txt")).unwrap(),
        "remote changes\n"
    );
    let out = shell(&workspace::collect_script(
        &r,
        remote.path().to_str().unwrap(),
    ));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let payload: Value = serde_json::from_slice(&out.stdout).unwrap();
    r.result = Some(json!({"reply":"done","model":"fixture"}));
    r.artifacts = Some(workspace::retain(&lease, &r, Some(payload)).unwrap());
    workspace::apply(&lease, &r, source.path()).unwrap();
    assert_eq!(
        fs::read_to_string(source.path().join("tracked.txt")).unwrap(),
        "remote changes\n"
    );
    assert_eq!(
        fs::read_to_string(source.path().join("note.txt")).unwrap(),
        "remote admitted changes\n"
    );
    assert!(source.path().join("created.txt").exists());
    assert_eq!(
        fs::read_to_string(source.path().join("other.txt")).unwrap(),
        "unselected\n"
    );
    assert!(
        workspace::apply(&lease, &r, source.path())
            .unwrap_err()
            .contains("changed")
    );
    let manifest = r.artifacts.as_ref().unwrap();
    let trace: Value = serde_json::from_slice(
        &workspace::artifact(&lease, manifest, "trajectory.atif.json").unwrap(),
    )
    .unwrap();
    assert_eq!(trace["schema_version"], atif::SCHEMA_VERSION);
    assert_eq!(trace["agent"]["model_name"], "fixture");
    assert!(lease.file("../outside").is_err());
    fs::write(lease.file("changes.patch").unwrap(), "tampered").unwrap();
    assert!(workspace::artifact(&lease, manifest, "changes.patch").is_err());
}
#[test]
fn transfer_rejects_unsafe_paths_credentials_and_changed_input() {
    let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let local = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    git(source.path(), &["init", "-q"]);
    fs::write(source.path().join("tracked"), "fixture").unwrap();
    git(source.path(), &["add", "."]);
    git(source.path(), &["commit", "-qm", "Fixture"]);
    let store = Store::under(local.path());
    let lease = store.lease("workspace_test").unwrap();
    for name in ["../outside", "/outside", ".env", ".git/config"] {
        assert!(
            workspace::capture(&lease, source.path(), None, vec![], vec![name.into()]).is_err()
        );
    }
    let s = workspace::capture(&lease, source.path(), None, vec![], vec![]).unwrap();
    fs::write(&s.input_path, "changed").unwrap();
    assert!(s.input().is_err());
    let c = coder_cloud::runtime::Credentials::from_names(&["TEST_API_KEY".into()], |_| {
        Some("fixture-secret".into())
    })
    .unwrap();
    use base64::Engine;
    let mut v = json!({"files":[{"content":base64::engine::general_purpose::STANDARD.encode(b"fixture-secret")}]});
    assert!(c.sanitize_artifacts(&mut v).is_err());
}
