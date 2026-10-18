use super::*;

fn files(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| (*p).to_owned()).collect()
}

fn lit(file: &str, text: &str) -> Literal {
    Literal {
        file: file.to_owned(),
        text: text.to_owned(),
    }
}

/// `crates/<name>/...` is package `<name>`; `crates/psionic/...` is a
/// build folder outside the workspace.
fn package_of(path: &str) -> Option<String> {
    let rest = path.strip_prefix("crates/")?;
    let (name, _) = rest.split_once('/')?;
    if name == "psionic" {
        return Some("dir:crates/psionic".into());
    }
    Some(name.to_owned())
}

/// `cli` depends on `core`; `core` on `base`; `web` on `base`; `docs-gen`
/// on nothing.
fn graph() -> Graph {
    vec![
        ("cli".into(), vec!["core".into()]),
        ("core".into(), vec!["base".into()]),
        ("web".into(), vec!["base".into()]),
        ("base".into(), vec![]),
        ("lone".into(), vec![]),
    ]
}

fn plan_of(paths: &[&str], literals: &[Literal]) -> Plan {
    let graph = graph();
    classify(
        &files(paths),
        &Facts {
            package_of: &package_of,
            literals,
            graph: Some(&graph),
        },
    )
}

#[test]
fn documents_under_docs_and_nips_and_at_the_top_are_fast() {
    for path in [
        "docs/example/queue-guide.md",
        "docs/product/s1/shot.png",
        "nips/example/NIP-XX.md",
        "CHANGES.md",
        "docs/a/diagram.SVG",
    ] {
        assert!(document(path), "{path}");
        assert_eq!(plan_of(&[path], &[]).lane, Lane::Fast, "{path}");
    }
}

#[test]
fn everything_else_is_code() {
    for path in [
        // A document inside a crate: its package may include it.
        "crates/boat/README.md",
        // Data the build or tests read, even when it is Markdown.
        "knowledge/coder.md",
        "fixtures/cloud/x.md",
        "plugins/disk-cleanup/README.md",
        // Not a document.
        "docs/api/openapi.first-party.json",
        "nips/openagents/schemas/host-call.v1.json",
        "scripts/cloud/dev-env-agent.sh",
        "Cargo.lock",
        "Cargo.toml",
        "rust-toolchain.toml",
        ".github/workflows/ci.yml",
        "LICENSE",
        "docs/script.sh",
    ] {
        assert_eq!(plan_of(&[path], &[]).lane, Lane::Code, "{path}");
    }
    // One code file makes the whole change code.
    let mixed = plan_of(&["docs/a.md", "crates/core/src/lib.rs"], &[]);
    assert_eq!(mixed.lane, Lane::Code);
    assert_eq!(mixed.packages, ["core"]);
}

#[test]
fn a_document_the_build_reads_is_code_and_reaches_its_reader() {
    // include_str! with ../ from a crate.
    let read = [lit(
        "crates/web/src/system.rs",
        "../../../docs/prompts/role.md",
    )];
    let plan = plan_of(&["docs/prompts/role.md"], &read);
    assert_eq!(plan.lane, Lane::Code);
    assert_eq!(plan.packages, ["web"]);
    assert_eq!(plan.also, ["web"], "the reader's tests run too");
    // A tail of two or more parts, as an include through other folders.
    let tail = [lit("crates/core/src/x.rs", "prompts/role.md")];
    assert_eq!(plan_of(&["docs/prompts/role.md"], &tail).lane, Lane::Code);
    // From the top of the repository, and a stem without its extension.
    let top = [lit(
        "crates/cli/tests/map.rs",
        "docs/coder/measurements/2026-10-01-claims",
    )];
    assert_eq!(
        plan_of(&["docs/coder/measurements/2026-10-01-claims.md"], &top).lane,
        Lane::Code
    );
    // A template names its folder.
    let template = [lit("crates/cli/src/kb.rs", "docs/product/{name}.md")];
    assert_eq!(plan_of(&["docs/product/x.md"], &template).lane, Lane::Code);
    assert_eq!(plan_of(&["docs/other/x.md"], &template).lane, Lane::Fast);
    // A root file named bare.
    let team = [lit("crates/core/src/kb.rs", "TEAM.md")];
    assert_eq!(plan_of(&["TEAM.md"], &team).lane, Lane::Code);
    // ...but a manifest's bare name is its own folder's file.
    let manifest = [lit("assets/x/manifest.json", "TEAM.md")];
    assert_eq!(plan_of(&["TEAM.md"], &manifest).lane, Lane::Fast);
    // The start of a `concat!` names no file by itself; its tail does.
    let concat = [
        lit("crates/web/src/corpus.rs", "../../../docs/"),
        lit("crates/web/src/corpus.rs", "agents/auth.md"),
    ];
    assert_eq!(plan_of(&["docs/agents/auth.md"], &concat).lane, Lane::Code);
    assert_eq!(plan_of(&["docs/other/guide.md"], &concat).lane, Lane::Fast);
}

#[test]
fn a_mention_that_is_not_the_path_does_not_make_code() {
    let mentions = [
        // Prose inside a string, not a path literal of its own.
        lit("crates/cli/src/land.rs", "See"),
        // A bare one-part tail: every README would match it.
        lit("crates/boat/src/lib.rs", "../README.md"),
        // Another file in the same folder.
        lit("crates/cli/src/x.rs", "docs/example/other.md"),
        // A sibling whose name starts the same.
        lit("crates/cli/src/x.rs", "docs/example/queue"),
    ];
    assert_eq!(
        plan_of(&["docs/example/queue-guide.md", "README.md"], &mentions).lane,
        Lane::Fast
    );
    assert!(!refers(
        "crates/boat/src/lib.rs",
        "../README.md",
        "README.md"
    ));
    assert!(refers(
        "crates/boat/src/lib.rs",
        "../README.md",
        "crates/boat/README.md"
    ));
    assert!(refers(
        "crates/cli/src/tree.rs",
        "/../coder/src/cli_route/tree.json",
        "crates/coder/src/cli_route/tree.json"
    ));
    assert!(!refers("a/b.rs", "{x}/y.md", "docs/y.md"));
}

#[test]
fn closures_hold_dependencies_and_dependents() {
    let g = graph();
    assert_eq!(closure(&files(&["core"]), &g), ["base", "cli", "core"]);
    assert_eq!(
        closure(&files(&["base"]), &g),
        ["base", "cli", "core", "web"]
    );
    assert_eq!(closure(&files(&["lone"]), &g), ["lone"]);
}

#[test]
fn entries_overlap_only_when_one_can_affect_the_others_checks() {
    let core = plan_of(&["crates/core/src/lib.rs"], &[]);
    let cli = plan_of(&["crates/cli/src/main.rs"], &[]);
    let web = plan_of(&["crates/web/src/lib.rs"], &[]);
    let base = plan_of(&["crates/base/src/lib.rs"], &[]);
    let lone = plan_of(&["crates/lone/src/lib.rs"], &[]);
    let docs = plan_of(&["docs/a.md"], &[]);
    // cli depends on core.
    assert!(overlap(&core, &cli) && overlap(&cli, &core));
    // web and cli share only a dependency (base); neither tests the other.
    assert!(!overlap(&web, &cli));
    assert!(!overlap(&web, &core));
    // base is under both.
    assert!(overlap(&base, &web) && overlap(&base, &cli));
    assert!(!overlap(&lone, &core));
    // Documents never wait for code, nor code for documents.
    assert!(!overlap(&docs, &core) && !overlap(&core, &docs));
    // The same file, even outside any package.
    let a = plan_of(&["scripts/x.sh"], &[]);
    let b = plan_of(&["scripts/x.sh", "scripts/y.sh"], &[]);
    let c = plan_of(&["scripts/z.sh"], &[]);
    assert!(overlap(&a, &b) && !overlap(&a, &c));
    // A workspace-wide file overlaps every code entry.
    let lock = plan_of(&["Cargo.lock"], &[]);
    assert!(lock.wide && overlap(&lock, &lone) && !overlap(&lock, &docs));
    // Folders outside the workspace overlap themselves only.
    let psionic = plan_of(&["crates/psionic/crates/x/src/lib.rs"], &[]);
    let psionic2 = plan_of(&["crates/psionic/README.md"], &[]);
    assert!(overlap(&psionic, &psionic2) && !overlap(&psionic, &core));
    // An unreadable plan waits for everything.
    assert!(overlap(&Plan::unknown("x"), &lone));
}

/// A judge that answers one probability and keeps what it was asked.
struct Fake {
    p: Result<f64, String>,
    asked: std::sync::Mutex<Vec<serde_json::Value>>,
}

impl OverlapJudge for Fake {
    fn overlap(&self, state: &serde_json::Value) -> Result<f64, String> {
        self.asked.lock().unwrap().push(state.clone());
        self.p.clone()
    }
}

fn fake(p: Result<f64, String>) -> Fake {
    Fake {
        p,
        asked: std::sync::Mutex::default(),
    }
}

#[test]
fn a_judged_overlap_only_ever_adds_order() {
    let web = plan_of(&["crates/web/src/lib.rs"], &[]);
    let cli = plan_of(&["crates/cli/src/main.rs"], &[]);
    let core = plan_of(&["crates/core/src/lib.rs"], &[]);
    let docs = plan_of(&["docs/a.md"], &[]);
    let yes = fake(Ok(0.9));
    // The rule finds web and cli apart; the judge sees both sides.
    assert!(overlap_judged(
        ("route", &web),
        ("caller", &cli),
        Some(&yes)
    ));
    let asked = yes.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["a"]["summary"], "route");
    assert_eq!(asked[0]["a"]["files"][0], "crates/web/src/lib.rs");
    assert_eq!(asked[0]["b"]["packages"][0], "cli");
    // A rule overlap is never asked about, and a document never waits.
    let never = fake(Ok(0.0));
    assert!(overlap_judged(("", &core), ("", &cli), Some(&never)));
    assert!(!overlap_judged(("", &docs), ("", &web), Some(&yes)));
    assert!(never.asked.lock().unwrap().is_empty());
    // Below the threshold, failed, or off: the rule's answer stands.
    let low = fake(Ok(JUDGED_OVERLAP_AT - 0.01));
    assert!(!overlap_judged(("", &web), ("", &cli), Some(&low)));
    let failed = fake(Err("offline".into()));
    assert!(!overlap_judged(("", &web), ("", &cli), Some(&failed)));
    assert!(!overlap_judged(("", &web), ("", &cli), None));
}

#[test]
fn the_judgment_is_off_unless_the_flag_is_one() {
    // The test process doesn't set the flag.
    assert!(std::env::var(OVERLAP_JUDGE_ENV).is_err());
    assert!(DecisionsOverlapJudge::from_env().is_none());
}

#[test]
fn an_unreadable_graph_makes_a_package_change_wide() {
    let plan = classify(
        &files(&["crates/core/src/lib.rs"]),
        &Facts {
            package_of: &package_of,
            literals: &[],
            graph: None,
        },
    );
    assert!(plan.wide);
}

#[test]
fn generators_are_due_on_their_marked_sources() {
    let tree = Generator {
        name: "cli-tree".into(),
        files: files(&["crates/coder/src/cli_route/tree.json"]),
        sources: files(&["crates/openagents-cli/src/"]),
        markers: files(&["USAGE", "Declared::"]),
        regenerate: "true".into(),
    };
    let gens = [tree];
    let read = |file: &str| -> Option<String> {
        Some(if file.ends_with("land.rs") {
            "pub(crate) const USAGE: &str = \"...\";".into()
        } else {
            "fn other() {}".into()
        })
    };
    assert_eq!(
        due(&gens, &files(&["crates/openagents-cli/src/land.rs"]), &read).len(),
        1
    );
    assert!(
        due(
            &gens,
            &files(&["crates/openagents-cli/src/other.rs"]),
            &read
        )
        .is_empty()
    );
    assert!(due(&gens, &files(&["crates/coder/src/lib.rs"]), &read).is_empty());
    assert!(all_generated(
        &gens,
        &files(&["crates/coder/src/cli_route/tree.json"])
    ));
    assert!(!all_generated(
        &gens,
        &files(&["crates/coder/src/cli_route/tree.json", "x.rs"])
    ));
    assert!(!all_generated(&gens, &[]));
}

#[test]
fn the_repository_registry_parses_and_names_files_that_exist() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let gens = generators(&top);
    assert!(
        gens.iter().any(|g| g.name == "cli-tree"),
        "{GENERATED_FILE} declares the CLI tree"
    );
    for g in &gens {
        for file in &g.files {
            assert!(top.join(file).is_file(), "{}: {file} is missing", g.name);
        }
    }
}
