use super::*;

fn git(dir: &Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid"
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn the_touched_packages_are_the_changed_and_new_files_packages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/a\", \"crates/b\", \"crates/c\"]\n",
    )
    .unwrap();
    for name in ["a", "b", "c"] {
        std::fs::create_dir_all(root.join("crates").join(name).join("src")).unwrap();
        std::fs::write(
            root.join("crates").join(name).join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\n"),
        )
        .unwrap();
        std::fs::write(root.join("crates").join(name).join("src/lib.rs"), "").unwrap();
    }
    git(root, &["init", "-q"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "Fixture"]);
    let head = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(touched_packages(root, head.trim()).is_empty());
    std::fs::write(root.join("crates/a/src/lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::write(root.join("crates/b/src/new.rs"), "\n").unwrap();
    std::fs::write(root.join("README.md"), "\n").unwrap();
    let mut touched = touched_packages(root, head.trim());
    touched.sort();
    assert_eq!(touched, ["a", "b"]);
}

#[cfg(unix)]
#[test]
fn a_local_suite_is_recognized_and_lists_its_commands() {
    let dir = tempfile::tempdir().unwrap();
    let grants = dir.path().join("grants");
    let requirements = requirements(&grants, "task-one", 3).unwrap().unwrap();
    let listed = ours(&requirements).expect("the host's own suite");
    assert!(listed.ends_with("task-one-3.suite.checks"));
    // An operator's requirements are not ours.
    let mut other = requirements.clone();
    other.check_lineage[0].sources = vec!["operator".into()];
    assert!(ours(&other).is_none());
    let workspace = tempfile::tempdir().unwrap();
    // Nothing to run lists nothing: the route stays unchecked.
    assert!(list(&requirements, workspace.path(), "HEAD", &[], &[]).is_none());
    let at = list(
        &requirements,
        workspace.path(),
        "HEAD",
        &["test -f done".into(), " ".into()],
        &[("PATH".into(), "/usr/bin:/bin".into())],
    )
    .unwrap();
    assert_eq!(at, listed);
    let names: Vec<_> = std::fs::read_dir(&at)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["001.sh"]);
    assert_eq!(
        std::fs::read_to_string(at.join("001.sh")).unwrap(),
        "PATH='/usr/bin:/bin'\nexport PATH\ntest -f done\n"
    );
    // The suite script is pinned: the grant's digest is its bytes'.
    let plan = requirements.validate().unwrap();
    let crate::verification::Acceptance::Suite { suite_digest, .. } = &plan.checks[0].acceptance
    else {
        panic!("a suite");
    };
    let program = plan.checks[0].manifest.with_extension("sh");
    assert_eq!(
        *suite_digest,
        format!(
            "sha256:{}",
            crate::capability::digest_file(&program).unwrap()
        )
    );
}
