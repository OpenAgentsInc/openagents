use briefing_lab::{Components, Issue, assemble, build_index, check_output, markdown, resolve};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Repository {
    root: PathBuf,
    repo: PathBuf,
}
impl Repository {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "briefing-lab-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        let result = Self { root, repo };
        result.git(&["init", "--quiet"]);
        result.write("AGENTS.md", "Read the exact revision.\n");
        result.write("Cargo.toml", "[workspace]\nmembers=[]\n");
        result.write(
            "crates/example/src/lib.rs",
            "// Snapshot content.\npub fn repair_index() -> bool { true }\n",
        );
        result.commit();
        result
    }
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Briefing test",
                "-c",
                "user.email=briefing@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .arg("-C")
            .arg(&self.repo)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    fn write(&self, path: &str, body: &str) {
        let path = self.repo.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    fn commit(&self) {
        self.git(&["add", "."]);
        self.git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Repair index fixture",
        ]);
    }
    fn issue(&self, title: &str, body: &str) -> Issue {
        Issue {
            title: title.into(),
            body: body.into(),
            number: Some(1),
            url: None,
        }
    }
}
impl Drop for Repository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn pinned_snapshot_excludes_dirty_and_untracked_files_and_rejects_stale_index() {
    let r = Repository::new();
    let index = build_index(&r.repo, "HEAD").unwrap();
    r.write("crates/example/src/lib.rs", "DIRTY CONTENT\n");
    r.write("untracked.rs", "UNTRACKED CONTENT\n");
    let brief = assemble(
        &r.repo,
        &index,
        &index.commit,
        r.issue("repair_index", "crates/example/src/lib.rs"),
        Components::default(),
    )
    .unwrap();
    assert!(
        brief
            .evidence
            .iter()
            .any(|e| e.text.contains("pub fn repair_index"))
    );
    assert!(!markdown(&brief).contains("DIRTY CONTENT"));
    assert!(!index.known_paths.iter().any(|p| p == "untracked.rs"));
    r.commit();
    let newer = resolve(&r.repo, "HEAD").unwrap();
    assert!(
        assemble(
            &r.repo,
            &index,
            &newer,
            r.issue("repair", ""),
            Components::default()
        )
        .unwrap_err()
        .to_string()
        .contains("stale")
    );
    assert_eq!(resolve(&r.repo, &index.commit).unwrap(), index.commit);
}

#[test]
fn missing_revision_fails_without_an_index() {
    let r = Repository::new();
    assert!(build_index(&r.repo, "refs/heads/not-present").is_err());
    assert!(resolve(&r.repo, "--help").is_err());
}

#[test]
fn missing_excluded_and_long_issue_text_remain_explicit() {
    let r = Repository::new();
    r.write("fixtures/hidden.rs", "pub fn fixture() {}\n");
    r.commit();
    let index = build_index(&r.repo, "HEAD").unwrap();
    let body = format!(
        "{}\ncrates/missing/src/lib.rs fixtures/hidden.rs\n```\ntouch DO_NOT_EXECUTE\n```\nFINAL REQUIREMENT",
        "Long original requirement.\n".repeat(2000)
    );
    let brief = assemble(
        &r.repo,
        &index,
        &index.commit,
        r.issue("Long issue", &body),
        Components::default(),
    )
    .unwrap();
    assert_eq!(brief.issue.body, body);
    assert!(markdown(&brief).contains(&body));
    assert!(
        brief
            .notes
            .iter()
            .any(|n| n.contains("unavailable") && n.contains("crates/missing/src/lib.rs"))
    );
    assert!(
        brief
            .notes
            .iter()
            .any(|n| n.contains("outside the bounded") && n.contains("fixtures/hidden.rs"))
    );
    assert!(!r.repo.join("DO_NOT_EXECUTE").exists());
}

#[test]
fn no_match_is_reported_and_components_change_selection() {
    let r = Repository::new();
    let index = build_index(&r.repo, "HEAD").unwrap();
    let no_match = assemble(
        &r.repo,
        &index,
        &index.commit,
        r.issue("xyzzquux", ""),
        Components::default(),
    )
    .unwrap();
    assert_eq!(no_match.candidate_files, 0);
    assert!(no_match.notes.iter().any(|s| s.contains("No direct")));
    let symbols = assemble(
        &r.repo,
        &index,
        &index.commit,
        r.issue("repair_index", ""),
        Components {
            lexical: false,
            symbols: true,
            history: false,
        },
    )
    .unwrap();
    assert!(
        symbols
            .evidence
            .iter()
            .any(|e| e.reasons.iter().any(|s| s.contains("Declaration hint")))
    );
    assert!(symbols.history.is_empty());
    let disabled = assemble(
        &r.repo,
        &index,
        &index.commit,
        r.issue("repair_index", ""),
        Components {
            lexical: false,
            symbols: false,
            history: false,
        },
    )
    .unwrap();
    assert_eq!(disabled.candidate_files, 0);
}

#[test]
fn evidence_ranges_digests_and_order_are_reproducible() {
    let r = Repository::new();
    r.write(
        "crates/example/src/lib.rs",
        &format!("{}pub fn target_symbol() {{}}\n", "// filler\n".repeat(150)),
    );
    r.commit();
    let index = build_index(&r.repo, "HEAD").unwrap();
    let issue = r.issue("target_symbol", "");
    let a = assemble(
        &r.repo,
        &index,
        &index.commit,
        issue.clone(),
        Components::default(),
    )
    .unwrap();
    let b = assemble(&r.repo, &index, &index.commit, issue, Components::default()).unwrap();
    assert_eq!(
        serde_json::to_value(&a.evidence).unwrap(),
        serde_json::to_value(&b.evidence).unwrap()
    );
    let evidence = a
        .evidence
        .iter()
        .find(|e| e.path.ends_with("src/lib.rs"))
        .unwrap();
    assert_eq!(evidence.start_line, 143);
    assert_eq!(evidence.end_line, 151);
    assert_eq!(
        evidence.excerpt_sha256,
        briefing_lab::sha256(evidence.text.as_bytes())
    );
    assert!(evidence.text.contains("target_symbol"));
}

#[test]
fn output_cannot_enter_repository_or_traverse_parents() {
    let r = Repository::new();
    assert!(check_output(&r.repo, &r.repo.join("new/index.json")).is_err());
    assert!(check_output(&r.repo, &r.root.join("outside/index.json")).is_ok());
    assert!(check_output(&r.repo, &r.root.join("outside/../repo/index.json")).is_err());
    #[cfg(unix)]
    {
        let link = r.root.join("linked");
        std::os::unix::fs::symlink(&r.repo, &link).unwrap();
        assert!(check_output(&r.repo, &link.join("index.json")).is_err());
        let dangling = r.root.join("dangling.json");
        std::os::unix::fs::symlink(r.repo.join("not-yet-created.json"), &dangling).unwrap();
        assert!(check_output(&r.repo, &dangling).is_err());
    }
}

#[test]
fn corrupt_cached_content_is_rejected() {
    let r = Repository::new();
    let mut index = build_index(&r.repo, "HEAD").unwrap();
    index.files[0].sha256 = "invalid digest".into();
    assert!(
        assemble(
            &r.repo,
            &index,
            &index.commit,
            r.issue("repair", ""),
            Components::default()
        )
        .unwrap_err()
        .to_string()
        .contains("digest")
    );
}

#[test]
fn null_body_is_accepted_and_content_paths_are_not_shell_commands() {
    let r = Repository::new();
    r.write(
        "crates/example/src/spaced name.rs",
        "pub fn spaced_name() {}\n",
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(Path::new("/outside/private"), r.repo.join("link.rs")).unwrap();
    r.commit();
    let index = build_index(&r.repo, "HEAD").unwrap();
    assert!(index.files.iter().any(|f| f.path.contains("spaced name")));
    assert!(!index.files.iter().any(|f| f.path == "link.rs"));
    let issue: Issue = serde_json::from_str(r#"{"title":"Test", "body":null}"#).unwrap();
    assert!(issue.body.is_empty());
}

#[test]
fn metadata_cache_roundtrip_fetches_exact_crlf_and_terminal_newline() {
    let r = Repository::new();
    let original = "// retained CRLF\r\npub fn exact_bytes() {}\r\n";
    r.write("crates/example/src/exact.rs", original);
    r.commit();
    let index = build_index(&r.repo, "HEAD").unwrap();
    let bytes = serde_json::to_vec(&index).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("pub fn exact_bytes"));
    let restored = serde_json::from_slice(&bytes).unwrap();
    let brief = assemble(
        &r.repo,
        &restored,
        &index.commit,
        r.issue("exact_bytes", "crates/example/src/exact.rs"),
        Components::default(),
    )
    .unwrap();
    let evidence = brief
        .evidence
        .iter()
        .find(|e| e.path.ends_with("exact.rs"))
        .unwrap();
    assert_eq!(evidence.text.as_bytes(), original.as_bytes());
    assert_eq!(
        evidence.excerpt_sha256,
        briefing_lab::sha256(original.as_bytes())
    );
}

#[test]
fn root_paths_and_markdown_links_are_explicit_candidates() {
    let r = Repository::new();
    r.write("settings.toml", "enabled = true\n");
    r.commit();
    let index = build_index(&r.repo, "HEAD").unwrap();
    let brief = assemble(&r.repo, &index, &index.commit,
        r.issue("Paths", "[settings](settings.toml) and [source](crates/example/src/lib.rs#L2), plus `missing.toml`."),
        Components { lexical: false, symbols: false, history: false }).unwrap();
    for path in ["settings.toml", "crates/example/src/lib.rs"] {
        assert!(
            brief
                .evidence
                .iter()
                .any(|e| e.path == path && e.reasons.iter().any(|r| r == "Explicit issue path"))
        );
    }
    assert!(
        brief
            .notes
            .iter()
            .any(|n| n.contains("unavailable") && n.contains("missing.toml"))
    );
}

#[test]
fn malformed_symbol_ranges_and_budgets_return_errors() {
    let r = Repository::new();
    let index = build_index(&r.repo, "HEAD").unwrap();
    let bytes = serde_json::to_vec(&index).unwrap();
    for invalid in [0, usize::MAX] {
        let mut bad: briefing_lab::Index = serde_json::from_slice(&bytes).unwrap();
        let source = bad
            .files
            .iter_mut()
            .find(|s| !s.symbols.is_empty())
            .unwrap();
        source.symbols[0].line = invalid;
        assert!(
            assemble(
                &r.repo,
                &bad,
                &bad.commit,
                r.issue("repair_index", ""),
                Components::default()
            )
            .unwrap_err()
            .to_string()
            .contains("range")
        );
    }
    let mut bad: briefing_lab::Index = serde_json::from_slice(&bytes).unwrap();
    bad.scanned_bytes = usize::MAX;
    assert!(
        assemble(
            &r.repo,
            &bad,
            &bad.commit,
            r.issue("repair_index", ""),
            Components::default()
        )
        .unwrap_err()
        .to_string()
        .contains("bounds")
    );
}

#[test]
fn cached_path_and_blob_are_bound_to_the_selected_commit() {
    let r = Repository::new();
    let mut index = build_index(&r.repo, "HEAD").unwrap();
    let source = index
        .files
        .iter_mut()
        .find(|s| s.path.ends_with("src/lib.rs"))
        .unwrap();
    source.blob = "1".repeat(40);
    assert!(
        assemble(
            &r.repo,
            &index,
            &index.commit,
            r.issue("repair_index", "crates/example/src/lib.rs"),
            Components::default()
        )
        .unwrap_err()
        .to_string()
        .contains("pinned Git commit")
    );
}
