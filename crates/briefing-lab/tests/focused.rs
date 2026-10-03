//! Focused packing uses disposable Git snapshots and synthetic source.
use briefing_lab::{
    Components, Index, Issue, Options, assemble, assemble_focused, build_index_with_options,
    focused::{BYTE_BUDGET, Role},
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "briefing-focused-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir(root.join("home")).unwrap();
        let f = Self { root, repo };
        f.git(&["init", "--quiet"]);
        f.write("Cargo.toml", "[workspace]\nmembers=['crates/example']\n");
        f.write(
            "crates/example/Cargo.toml",
            "[package]\nname='example'\nversion='0.1.0'\n",
        );
        f.write("AGENTS.md", "MANDATORY_COMPLETE_INSTRUCTION_SENTINEL\n");
        f
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.repo.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, executable: &str) -> Command {
        let mut cmd = Command::new(executable);
        cmd.env("HOME", self.root.join("home"))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        cmd
    }
    fn git(&self, args: &[&str]) {
        let output = self
            .command("git")
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
    fn commit(&self) {
        self.git(&["add", "."]);
        self.git(&["commit", "--quiet", "-m", "Source fixture"]);
    }
    fn index(&self, syntax: bool) -> Index {
        build_index_with_options(&self.repo, "HEAD", Options { syntax }).unwrap()
    }
    fn issue(&self, body: &str) -> Issue {
        Issue {
            title: "Review bounded behavior".into(),
            body: body.into(),
            number: None,
            url: None,
        }
    }
    fn focused(&self, index: &Index, body: &str) -> briefing_lab::Brief {
        assemble_focused(
            &self.repo,
            index,
            &index.commit,
            self.issue(body),
            Components::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn long_source() -> String {
    format!(
        "pub fn first() -> usize {{\n{}    121\n}}\n\npub fn second() -> usize {{\n    242\n}}\n{}",
        "    let _ = 7;\n".repeat(100),
        "// Unrelated padding remains outside the selected functions.\n".repeat(140)
    )
}

#[test]
fn anchors_retain_complete_long_functions_before_lexical_candidates() {
    let f = Fixture::new();
    let source = long_source();
    f.write("crates/example/src/lib.rs", &source);
    f.write(
        "crates/distractor/src/lib.rs",
        "pub fn bounded_behavior_review() {}\n",
    );
    f.write(
        "crates/example/tests/check.rs",
        "#[test]\nfn checks_example() {}\n",
    );
    f.write(
        "crates/example/fixtures/sample.json",
        "{\"fixture\":true}\n",
    );
    f.write("docs/reference.md", "The complete named document.\n");
    f.commit();
    let index = f.index(false);
    let second_line = source
        .lines()
        .position(|line| line.contains("pub fn second"))
        .unwrap()
        + 1;
    let body = format!(
        "crates/example/src/lib.rs:5\ncrates/example/src/lib.rs#L{second_line}-L{}\ndocs/reference.md\nAGENTS.md",
        second_line + 1
    );
    let brief = f.focused(&index, &body);
    let pack = brief.focused.as_ref().unwrap();
    assert_eq!(brief.evidence[0].path, "crates/example/src/lib.rs");
    assert!(brief.evidence[0].text.starts_with("pub fn first"));
    assert!(brief.evidence[0].text.ends_with("    121\n}\n"));
    assert!(brief.evidence[0].end_line > 64);
    assert!(!brief.evidence[0].text.contains("Unrelated padding"));
    assert!(brief.evidence[1].text.starts_with("pub fn second"));
    assert!(pack.selections[0].complete_declaration);
    assert_eq!(pack.selections[0].role, Role::ExplicitSource);
    assert!(
        pack.selections
            .iter()
            .any(|s| s.role == Role::ExplicitDocument && s.complete_file)
    );
    assert!(pack.selections.iter().any(|s| s.role == Role::NearbyTest));
    assert!(pack.omissions.iter().any(|s| s.role == Role::NearbyFixture));
    assert!(
        !pack
            .markdown
            .contains("MANDATORY_COMPLETE_INSTRUCTION_SENTINEL")
    );
    assert!(
        pack.instructions
            .contains("complete applicable instructions")
    );
    assert!(pack.packed_bytes <= BYTE_BUDGET);
    assert_eq!(pack.packed_bytes, pack.markdown.len());
    assert_eq!(
        pack.source_bytes,
        brief.evidence.iter().map(|e| e.text.len()).sum::<usize>()
    );
}

#[test]
fn missing_invalid_and_outside_function_anchors_are_explicit_omissions() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", &long_source());
    f.commit();
    let index = f.index(false);
    for anchor in ["0", "99999", "120", "8-3"] {
        let brief = f.focused(&index, &format!("crates/example/src/lib.rs:{anchor}"));
        let pack = brief.focused.unwrap();
        assert!(
            !brief
                .evidence
                .iter()
                .any(|e| e.path == "crates/example/src/lib.rs")
        );
        assert!(
            pack.omissions
                .iter()
                .any(|o| o.path == "crates/example/src/lib.rs"
                    && (o.reason.contains("anchor") || o.reason.contains("function"))),
            "{anchor}"
        );
        assert!(pack.markdown.contains("unresolved"));
    }
    let brief = f.focused(&index, "crates/missing/src/lib.rs:22");
    assert!(
        brief
            .focused
            .unwrap()
            .omissions
            .iter()
            .any(|o| o.reason.contains("absent"))
    );
}

#[test]
fn oversized_function_is_omitted_and_small_files_preserve_exact_bytes() {
    let f = Fixture::new();
    f.write(
        "crates/example/src/huge.rs",
        &format!("pub fn huge() {{\n{}}}\n", "    let _ = 1;\n".repeat(1600)),
    );
    let tiny = "pub fn tiny() {}\r\n// terminal newline retained\r\n";
    f.write("crates/example/src/tiny.rs", tiny);
    f.commit();
    let index = f.index(false);
    let brief = f.focused(
        &index,
        "crates/example/src/huge.rs:3 crates/example/src/tiny.rs",
    );
    let pack = brief.focused.as_ref().unwrap();
    assert!(!brief.evidence.iter().any(|e| e.path.ends_with("huge.rs")));
    assert!(
        pack.omissions
            .iter()
            .any(|o| o.path.ends_with("huge.rs") && o.reason.contains("without clipping"))
    );
    assert_eq!(
        brief
            .evidence
            .iter()
            .find(|e| e.path.ends_with("tiny.rs"))
            .unwrap()
            .text,
        tiny
    );
    assert!(pack.packed_bytes <= BYTE_BUDGET);
}

#[test]
fn compatible_cache_matches_fresh_parse_and_corrupt_cache_is_rejected() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", &long_source());
    f.commit();
    let plain = f.index(false);
    let mut cached = f.index(true);
    let body = "crates/example/src/lib.rs:5";
    let fresh = f.focused(&plain, body);
    let from_cache = f.focused(&cached, body);
    assert_eq!(
        serde_json::to_value(&fresh.evidence).unwrap(),
        serde_json::to_value(&from_cache.evidence).unwrap()
    );
    assert_eq!(
        fresh.focused.unwrap().markdown,
        from_cache.focused.unwrap().markdown
    );
    let baseline = assemble(
        &f.repo,
        &plain,
        &plain.commit,
        f.issue(body),
        Components::default(),
    )
    .unwrap();
    let baseline_cached = assemble(
        &f.repo,
        &cached,
        &cached.commit,
        f.issue(body),
        Components::default(),
    )
    .unwrap();
    assert!(baseline.focused.is_none());
    assert_eq!(
        serde_json::to_value(&baseline.evidence).unwrap(),
        serde_json::to_value(&baseline_cached.evidence).unwrap()
    );
    let rust = cached
        .files
        .iter_mut()
        .find(|s| s.path.ends_with("lib.rs"))
        .unwrap();
    // Keep byte ranges valid but make the cached line mapping disagree with source.
    rust.syntax.as_mut().unwrap().declarations[0]
        .declaration
        .end_line += 1;
    let error = assemble_focused(
        &f.repo,
        &cached,
        &cached.commit,
        f.issue(body),
        Components::default(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("coordinates do not match source bytes")
    );
    let mut damaged = f.index(false);
    damaged
        .files
        .iter_mut()
        .find(|s| s.path.ends_with("lib.rs"))
        .unwrap()
        .sha256 = "0".repeat(64);
    assert!(
        assemble_focused(
            &f.repo,
            &damaged,
            &damaged.commit,
            f.issue(body),
            Components::default()
        )
        .is_err()
    );
}

#[test]
fn cli_and_wrapper_write_the_exact_budgeted_payload_without_running_issue_commands() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", "pub fn example() {}\n");
    f.commit();
    let index = f.index(false);
    let index_file = f.root.join("index.json");
    let issue_file = f.root.join("issue.json");
    fs::write(&index_file, serde_json::to_vec(&index).unwrap()).unwrap();
    fs::write(
        &issue_file,
        serde_json::to_vec(&f.issue("crates/example/src/lib.rs\n$(touch EXECUTED)\n")).unwrap(),
    )
    .unwrap();
    let output_dir = f.root.join("output");
    let output = f
        .command("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/briefing-preview.sh"))
        .env("BRIEFING_LAB_BIN", env!("CARGO_BIN_EXE_briefing-lab"))
        .arg("--repo")
        .arg(&f.repo)
        .args(["--rev", &index.commit, "--focused"])
        .arg("--index")
        .arg(&index_file)
        .arg("--issue-file")
        .arg(&issue_file)
        .arg("--output-dir")
        .arg(&output_dir)
        .current_dir(&f.root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(output_dir.join("briefing.json")).unwrap()).unwrap();
    let pack = fs::read_to_string(output_dir.join("focused.md")).unwrap();
    assert_eq!(json["focused"]["markdown"].as_str().unwrap(), pack);
    assert!(pack.len() <= BYTE_BUDGET);
    assert!(!f.root.join("EXECUTED").exists());
    assert!(!f.repo.join("EXECUTED").exists());
}

#[test]
fn declaration_metadata_preserves_ambiguity_and_qualified_name_resolution() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", &format!(
        "struct First;\nstruct Second;\nimpl First {{ fn repair() {{}} }}\nimpl Second {{ fn repair() {{}} }}\n{}",
        "// Padding outside declarations keeps this above the complete-file bound.\n".repeat(100)
    ));
    f.commit();
    let index = f.index(false);
    let ambiguous = f.focused(&index, "crates/example/src/lib.rs repair");
    let selected = ambiguous.evidence[0].syntax_selection.as_ref().unwrap();
    assert_eq!(selected.match_kind, "exact identifier");
    assert_eq!(selected.equally_ranked_matches, 2);
    assert_eq!(selected.declaration.qualified_name, "First::repair");
    assert!(
        ambiguous
            .focused
            .unwrap()
            .markdown
            .contains("2 equally ranked matches")
    );
    let qualified = f.focused(&index, "crates/example/src/lib.rs Second::repair");
    let selected = qualified.evidence[0].syntax_selection.as_ref().unwrap();
    assert_eq!(selected.match_kind, "exact qualified name");
    assert_eq!(selected.equally_ranked_matches, 1);
    assert_eq!(selected.declaration.qualified_name, "Second::repair");
}

#[test]
fn focused_ablation_flags_apply_to_declaration_and_fallback_selection() {
    let f = Fixture::new();
    let prefix = "// Padding outside declarations contains no task-specific terms.\n".repeat(110);
    f.write("crates/example/src/lib.rs", &format!(
        "{prefix}pub fn first() {{ let _ = \"citrus orchard\"; }}\npub fn selected() {{}}\npub fn last() {{ let _ = \"citrus orchard\"; }}\n"
    ));
    f.commit();
    let index = f.index(false);
    let body = "crates/example/src/lib.rs selected citrus orchard";
    let run = |lexical, symbols| {
        assemble_focused(
            &f.repo,
            &index,
            &index.commit,
            f.issue(body),
            Components {
                lexical,
                symbols,
                history: false,
            },
        )
        .unwrap()
    };
    let default = run(true, true);
    assert_eq!(
        default.evidence[0]
            .syntax_selection
            .as_ref()
            .unwrap()
            .declaration
            .name,
        "selected"
    );
    let terms_only = run(true, false);
    let selected = terms_only.evidence[0].syntax_selection.as_ref().unwrap();
    assert_eq!(selected.declaration.name, "first");
    assert_eq!(selected.match_kind, "function term overlap");
    assert_eq!(selected.equally_ranked_matches, 2);
    let symbols_only = run(false, true);
    assert_eq!(
        symbols_only.evidence[0]
            .syntax_selection
            .as_ref()
            .unwrap()
            .declaration
            .name,
        "selected"
    );
    let disabled = run(false, false);
    assert!(disabled.evidence[0].syntax_selection.is_none());
    assert_eq!(disabled.evidence[0].start_line, 1);
    assert!(!disabled.evidence[0].text.contains("pub fn"));
    let anchored = assemble_focused(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/lib.rs:112"),
        Components {
            lexical: false,
            symbols: false,
            history: false,
        },
    )
    .unwrap();
    assert_eq!(
        anchored.evidence[0]
            .syntax_selection
            .as_ref()
            .unwrap()
            .declaration
            .name,
        "selected"
    );
}

#[test]
fn structural_policy_keeps_attributes_and_fixture_cycles_without_broad_noise() {
    let f = Fixture::new();
    f.write("crates/example/src/worker.rs", "/// Execute a value.\n#[inline]\npub fn execute() -> usize { 1 }\nfn unrelated_source() {}\n");
    f.write("crates/example/src/tests.rs", "fn fixture() -> usize { seed() }\nfn seed() -> usize { if false { fixture() } else { 1 } }\nfn unrelated_helper() {}\n/// Exercise the public entry point.\n#[test]\n\nfn executes_value() {\n    let actual = super::worker::execute();\n    let expected = fixture();\n    assert_eq!(actual, expected);\n}\nfn fake() { let _ = \"#[test]\"; }\n");
    f.write(
        "crates/noise/src/lib.rs",
        "pub fn execute() { /* execute execute value */ }\n",
    );
    f.write(
        "crates/example/nested/Cargo.toml",
        "[package]\nname='nested'\nversion='0.1.0'\n",
    );
    f.write(
        "crates/example/nested/src/tests.rs",
        "#[test]\nfn executes_value() { super::worker::execute(); }\n",
    );
    f.commit();
    let index = f.index(false);
    let issue = f.issue("crates/example/src/worker.rs:3 execute value");
    let before = assemble_focused(
        &f.repo,
        &index,
        &index.commit,
        issue.clone(),
        Components::default(),
    )
    .unwrap()
    .focused
    .unwrap()
    .markdown;
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        issue.clone(),
        Components::default(),
    )
    .unwrap();
    let pack = brief.focused.as_ref().unwrap();
    assert_eq!(pack.schema, "openagents.briefing-lab.explicit-structure.v1");
    assert!(
        pack.markdown
            .contains("/// Execute a value.\n#[inline]\npub fn execute")
    );
    assert!(pack.markdown.contains("#[test]\n\nfn executes_value"));
    assert!(pack.markdown.contains("fn fixture()"));
    assert!(pack.markdown.contains("fn seed()"));
    assert!(!pack.markdown.contains("fn unrelated_helper"));
    assert!(!pack.markdown.contains("fn unrelated_source"));
    assert!(
        !brief
            .evidence
            .iter()
            .any(|e| e.path.contains("nested") || e.path.contains("noise"))
    );
    assert!(
        pack.selections
            .iter()
            .any(|s| s.role == Role::TestFixtureHelper)
    );
    assert!(pack.packed_bytes <= BYTE_BUDGET);
    assert_eq!(pack.packed_bytes, pack.markdown.len());
    let after = assemble_focused(&f.repo, &index, &index.commit, issue, Components::default())
        .unwrap()
        .focused
        .unwrap()
        .markdown;
    assert_eq!(before, after);
}

#[test]
fn structural_test_fallback_is_labeled_and_an_explicit_test_anchor_still_works() {
    let f = Fixture::new();
    f.write("crates/example/src/worker.rs", "pub fn entry() {}\n");
    f.write("crates/example/src/tests.rs","#[test]\nfn durable_records_survive_restart() { runner::run(); }\n#[test]\nfn unrelated_widgets_render() {}\n");
    f.commit();
    let index = f.index(false);
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/worker.rs:1 durable records survive restart"),
        Components::default(),
    )
    .unwrap();
    assert!(
        brief
            .focused
            .as_ref()
            .unwrap()
            .selections
            .iter()
            .any(|s| s.role == Role::NearbyTest
                && s.method.contains("Scope-limited lexical fallback"))
    );
    assert!(
        brief
            .focused
            .unwrap()
            .markdown
            .contains("fn durable_records_survive_restart")
    );
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/tests.rs:4"),
        Components {
            lexical: false,
            symbols: false,
            history: false,
        },
    )
    .unwrap();
    assert!(
        brief
            .evidence
            .iter()
            .any(|e| e.text.contains("#[test]\nfn unrelated_widgets_render"))
    );
}

#[test]
fn structural_markdown_retains_complete_sections_and_ignores_fenced_headings() {
    let f = Fixture::new();
    let doc = "# Guide\n\n## Cache retention\n\nKeep complete records.\n\n````text\n### Fake heading\n```\nretention example\n````\n\nLast sentence of this section.\n\nUnrelated\n---------\n\nOther material.\n\nRetention details\n-----------------\n\nFinal sentence without newline";
    f.write("docs/guide.md", doc);
    f.commit();
    let index = f.index(false);
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("docs/guide.md cache retention"),
        Components::default(),
    )
    .unwrap();
    let pack = brief.focused.unwrap();
    assert!(pack.markdown.contains("Last sentence of this section."));
    assert!(pack.markdown.contains("### Fake heading"));
    assert!(pack.markdown.contains("Final sentence without newline"));
    assert!(!pack.markdown.contains("Other material."));
    assert!(
        pack.selections
            .iter()
            .all(|s| !s.method.contains("Fake heading"))
    );
}

#[test]
fn structural_dependency_ambiguity_and_shadowing_are_not_resolved_by_name_alone() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", "pub fn entry() { fixture(); }\nfn fixture() {}\nfn fixture() {}\npub fn shadowed(fixture: fn()) { fixture(); }\n");
    f.commit();
    let index = f.index(false);
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/lib.rs:1"),
        Components::default(),
    )
    .unwrap();
    let pack = brief.focused.unwrap();
    assert!(
        pack.omissions
            .iter()
            .any(|o| o.reason.contains("multiple same-file declarations"))
    );
    assert!(
        !brief
            .evidence
            .iter()
            .any(|e| e.text.contains("fn fixture()"))
    );
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/lib.rs:4"),
        Components::default(),
    )
    .unwrap();
    assert!(
        brief
            .focused
            .unwrap()
            .omissions
            .iter()
            .any(|o| o.reason.contains("shadowed"))
    );
}

#[test]
fn structural_budget_omits_an_entire_dependency_bundle_and_reports_missing_paths() {
    let f = Fixture::new();
    f.write(
        "crates/example/src/lib.rs",
        &format!(
            "pub fn entry() {{ huge(); }}\nfn huge() {{\n{}}}\n",
            "    let _ = \"界\";\n".repeat(1500)
        ),
    );
    f.commit();
    let index = f.index(false);
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/lib.rs:1 crates/absent/src/lib.rs:20"),
        Components::default(),
    )
    .unwrap();
    let pack = brief.focused.unwrap();
    assert!(pack.packed_bytes <= BYTE_BUDGET);
    assert!(brief.evidence.is_empty());
    assert!(
        pack.omissions
            .iter()
            .any(|o| o.reason.contains("complete bundle was omitted"))
    );
    assert!(
        pack.omissions
            .iter()
            .any(|o| o.path == "crates/absent/src/lib.rs")
    );
}

#[test]
fn structural_cli_is_separate_and_writes_the_exact_payload() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", "pub fn entry() {}\n");
    f.commit();
    let index = f.index(false);
    let index_file = f.root.join("index.json");
    let issue_file = f.root.join("issue.json");
    let out = f.root.join("structure");
    fs::write(&index_file, serde_json::to_vec(&index).unwrap()).unwrap();
    fs::write(
        &issue_file,
        serde_json::to_vec(&f.issue("crates/example/src/lib.rs:1")).unwrap(),
    )
    .unwrap();
    let output = f
        .command("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/briefing-preview.sh"))
        .env("BRIEFING_LAB_BIN", env!("CARGO_BIN_EXE_briefing-lab"))
        .arg("--repo")
        .arg(&f.repo)
        .args(["--rev", &index.commit, "--explicit-structure"])
        .arg("--index")
        .arg(&index_file)
        .arg("--issue-file")
        .arg(&issue_file)
        .arg("--output-dir")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("briefing.json")).unwrap()).unwrap();
    assert_eq!(
        json["focused"]["markdown"].as_str().unwrap(),
        fs::read_to_string(out.join("focused.md")).unwrap()
    );
    let rejected = f
        .command(env!("CARGO_BIN_EXE_briefing-lab"))
        .args(["preview", "--focused", "--explicit-structure"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
}

#[test]
fn structural_plain_file_anchors_are_checked_and_parent_sections_remain_complete() {
    let f = Fixture::new();
    f.write("config.toml", "answer = 42\n");
    f.write("docs/plain.md", "A plain short document.\n");
    f.write(
        "docs/parent.md",
        "# Parent\n\nAn introduction.\n\n## Child\n\nChild content.\n",
    );
    f.commit();
    let index = f.index(false);
    for path in ["config.toml", "docs/plain.md"] {
        let brief = briefing_lab::assemble_explicit_structure(
            &f.repo,
            &index,
            &index.commit,
            f.issue(&format!("{path}:999")),
            Components::default(),
        )
        .unwrap();
        assert!(!brief.evidence.iter().any(|e| e.path == path));
        assert!(
            brief
                .focused
                .unwrap()
                .omissions
                .iter()
                .any(|o| o.path == path && o.reason.contains("outside the pinned file"))
        );
    }
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("docs/parent.md:3"),
        Components {
            lexical: false,
            symbols: false,
            history: false,
        },
    )
    .unwrap();
    assert_eq!(brief.evidence.len(), 1);
    assert_eq!(brief.evidence[0].start_line, 1);
    assert!(brief.evidence[0].text.ends_with("Child content.\n"));
}

#[test]
fn structural_documents_do_not_admit_workspace_bench_tests_ahead_of_package_tests() {
    let f = Fixture::new();
    f.write("crates/example/src/worker.rs", "pub fn entry() {}\n");
    f.write(
        "crates/example/src/tests.rs",
        "#[test]\nfn durable_records_survive() { super::worker::entry(); }\n",
    );
    f.write(
        "docs/guide.md",
        "# Durable records\n\nKeep the complete record.\n",
    );
    for n in 0..40 {
        f.write(
            &format!("bench/aaa-{n:02}/tests.rs"),
            "#[test]\nfn durable_records_survive() { crate::worker::entry(); }\n",
        );
    }
    f.commit();
    let index = f.index(false);
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("crates/example/src/worker.rs:1 docs/guide.md durable records survive"),
        Components::default(),
    )
    .unwrap();
    assert_eq!(brief.candidate_files, 3);
    assert!(
        brief
            .evidence
            .iter()
            .any(|e| e.path == "crates/example/src/tests.rs")
    );
    assert!(!brief.evidence.iter().any(|e| e.path.starts_with("bench/")));
    let pack = brief.focused.unwrap();
    assert!(!pack.omissions.iter().any(|o| o.path.starts_with("bench/")));
    assert!(
        !pack
            .omissions
            .iter()
            .any(|o| o.reason.contains("24-file read bound"))
    );
    let documentation = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("docs/guide.md durable records survive"),
        Components::default(),
    )
    .unwrap();
    assert_eq!(documentation.candidate_files, 1);
    let virtual_root = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("Cargo.toml durable records survive"),
        Components::default(),
    )
    .unwrap();
    assert_eq!(virtual_root.candidate_files, 1);
    assert!(
        virtual_root
            .focused
            .unwrap()
            .omissions
            .iter()
            .any(|o| o.reason.contains("does not establish a package table"))
    );
}

#[test]
fn structural_real_root_package_is_verified_before_admitting_tests() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname='root-example'\nversion='0.1.0'\n",
    );
    f.write("src/lib.rs", "pub fn entry() {}\n");
    f.write(
        "tests/root.rs",
        "#[test]\nfn entry_works() { root_example::entry(); }\n",
    );
    f.commit();
    let index = f.index(false);
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue("src/lib.rs:1 entry works"),
        Components::default(),
    )
    .unwrap();
    assert!(brief.evidence.iter().any(|e| e.path == "tests/root.rs"));
    assert!(
        !brief
            .focused
            .unwrap()
            .omissions
            .iter()
            .any(|o| o.reason.contains("does not establish a package table"))
    );
}

#[test]
fn structural_rendered_warning_details_are_bounded_separately_from_source() {
    let f = Fixture::new();
    f.write("crates/example/src/lib.rs", "pub fn entry() {}\n");
    f.commit();
    let index = f.index(false);
    let missing = (0..100)
        .map(|n| format!("crates/missing-{n}/src/lib.rs:3"))
        .collect::<Vec<_>>()
        .join(" ");
    let brief = briefing_lab::assemble_explicit_structure(
        &f.repo,
        &index,
        &index.commit,
        f.issue(&format!("crates/example/src/lib.rs:1 {missing}")),
        Components::default(),
    )
    .unwrap();
    let pack = brief.focused.unwrap();
    let warnings: usize = pack
        .markdown
        .lines()
        .filter(|line| line.starts_with("Coverage `"))
        .map(|line| line.len() + 1)
        .sum();
    assert!(warnings <= 768);
    assert!(pack.omissions.len() >= 100);
    assert!(pack.packed_bytes <= BYTE_BUDGET);
}
