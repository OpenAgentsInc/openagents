//! Disposable keys, Git source, and state exercise the local host command.
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    let mut process = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = input {
        process.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    let out = process.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
#[test]
fn local_cli_reads_changes_views_and_restores_exact_git_documents() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let key = root.join("key");
    std::fs::write(&key, "01".repeat(32)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let state = root.join("state");
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_workspace-host"))
            .args(args)
            .args([
                "--state",
                state.to_str().unwrap(),
                "--key-file",
                key.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!out.stdout.contains(&27));
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let binding = run(&["init"]);
    let repo = root.join("repo.git");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--bare"], None);
    git(&repo, &["config", "core.logAllRefUpdates", "true"], None);
    let blob = git(&repo, &["hash-object", "-w", "--stdin"], Some(b"hello\n"));
    let tree = git(
        &repo,
        &["mktree"],
        Some(format!("100644 blob {blob}\tdoc.txt\n").as_bytes()),
    );
    let commit = git(&repo, &["commit-tree", &tree], Some(b"fixture\n"));
    git(&repo, &["update-ref", "refs/heads/docs", &commit], None);
    let resource = run(&[
        "add-git",
        "--repository",
        repo.to_str().unwrap(),
        "--branch",
        "refs/heads/docs",
        "--path",
        "doc.txt",
        "--name",
        "Document\u{001b}[2J",
    ]);
    let id = resource["resource"].as_str().unwrap();
    assert_eq!(run(&["read", "--resource", id])["content_utf8"], "hello\n");
    let resources = root.join("resources.json");
    std::fs::write(&resources, json!([id]).to_string()).unwrap();
    let until = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600)
        .to_string();
    let checkpoint = run(&[
        "checkpoint",
        "--resources",
        resources.to_str().unwrap(),
        "--retain-until",
        &until,
    ]);
    let content = root.join("content.txt");
    std::fs::write(&content, "changed\n").unwrap();
    let bundle = run(&[
        "replace-plan",
        "--resource",
        id,
        "--content",
        content.to_str().unwrap(),
    ]);
    let file = root.join("bundle.json");
    std::fs::write(&file, bundle.to_string()).unwrap();
    let result = run(&["apply", "--bundle", file.to_str().unwrap()]);
    assert_eq!(result["outcome"], "completed");
    assert_eq!(run(&["apply", "--bundle", file.to_str().unwrap()]), result);
    let projection = run(&[
        "view",
        "--resources",
        resources.to_str().unwrap(),
        "--recipient",
        binding["authority"].as_str().unwrap(),
        "--retain-until",
        &until,
    ]);
    let view = root.join("projection.json");
    std::fs::write(&view, projection.to_string()).unwrap();
    let snapshot = run(&["snapshot", "--projection", view.to_str().unwrap()]);
    assert_eq!(snapshot["result"]["status"], "completed");
    assert_eq!(snapshot["page"]["rows"].as_array().unwrap().len(), 1);
    let checkpoint_file = root.join("checkpoint.json");
    std::fs::write(&checkpoint_file, checkpoint["reference"].to_string()).unwrap();
    let plan = run(&[
        "restore-plan",
        "--checkpoint",
        checkpoint_file.to_str().unwrap(),
    ]);
    let plan_file = root.join("plan.json");
    std::fs::write(&plan_file, plan["reference"].to_string()).unwrap();
    let result = run(&["restore", "--plan", plan_file.to_str().unwrap()]);
    assert_eq!(result["status"], "completed");
    assert_eq!(run(&["read", "--resource", id])["content_utf8"], "hello\n");
}
