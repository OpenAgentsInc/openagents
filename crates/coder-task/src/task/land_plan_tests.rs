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
    // A workspace-wide file overlaps every code entry; so does a lockfile
    // change nobody narrowed.
    let lock = plan_of(&["Cargo.lock"], &[]);
    assert!(lock.wide && overlap(&lock, &lone) && !overlap(&lock, &docs));
    let toolchain = plan_of(&["rust-toolchain.toml"], &[]);
    assert!(toolchain.wide && overlap(&toolchain, &lone));
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

fn pkg(name: &str, dir: &str, paths: &[&str]) -> Listed {
    Listed {
        name: name.into(),
        dir: dir.into(),
        paths: files(paths),
    }
}

const APP: &str = "apps/openagents/crates";

/// The app workspace as it stood on 2026-10-17, cut down to the crates
/// the six missed pairs touch (#12121), beside a few root crates.
fn app_graph() -> Graph {
    let top = vec![
        pkg("coder-access", "crates/coder-access", &[]),
        pkg("chat-home", "crates/chat-home", &[]),
        pkg(
            "openagents-sdk",
            "crates/openagents-sdk",
            &["crates/chat-home"],
        ),
        pkg("lab-kit", "crates/lab-kit", &[]),
        pkg("lab-ui", "crates/lab-ui", &["crates/lab-kit"]),
        pkg("boat", "crates/boat", &[]),
    ];
    let app = |crate_: &str| format!("{APP}/{crate_}");
    let nested = vec![(
        "apps/openagents".to_owned(),
        vec![
            pkg(
                "openagents-engine",
                &app("engine"),
                &[
                    "crates/coder-access",
                    "crates/chat-home",
                    &app("rpc"),
                    &app("account"),
                    &app("harness"),
                ],
            ),
            pkg("openagents-rpc", &app("rpc"), &[]),
            pkg("openagents-account", &app("account"), &["crates/lab-kit"]),
            pkg("openagents-harness", &app("harness"), &[]),
            pkg(
                "openagents-ui",
                &app("ui"),
                &[
                    &app("engine"),
                    &app("rpc"),
                    &app("account"),
                    "crates/lab-ui",
                    "crates/lab-kit",
                ],
            ),
            pkg("openagents-sync", &app("sync"), &[]),
            pkg(
                "openagents-mobile",
                &app("mobile"),
                &[&app("sync"), &app("account"), "crates/lab-kit"],
            ),
            pkg("openagents-orb", &app("orb"), &[]),
        ],
    )];
    // The top workspace's own graph, as `workspace_graph` reads it.
    let mut graph: Graph = top
        .iter()
        .map(|p| {
            let depends = p
                .paths
                .iter()
                .filter_map(|path| top.iter().find(|t| t.dir == *path))
                .map(|t| t.name.clone())
                .collect();
            (p.name.clone(), depends)
        })
        .collect();
    graph.extend(path_edges(&top, &nested));
    graph
}

/// Root crates by name; an app crate by its folder; anything else under
/// the app by the app's folder, as `package_of` reads the checkout.
fn app_package_of(path: &str) -> Option<String> {
    if let Some(rest) = path.strip_prefix(&format!("{APP}/")) {
        let (crate_, _) = rest.split_once('/')?;
        return Some(format!("dir:{APP}/{crate_}"));
    }
    if path.starts_with("apps/openagents/") {
        return Some("dir:apps/openagents".into());
    }
    package_of(path)
}

fn app_plan(paths: &[&str]) -> Plan {
    let graph = app_graph();
    classify(
        &files(paths),
        &Facts {
            package_of: &app_package_of,
            literals: &[],
            graph: Some(&graph),
        },
    )
}

#[test]
fn the_six_pairs_the_app_workspace_hid_now_overlap() {
    // The pairs from 2026-10-17 the rule found apart and that were coupled
    // (`docs/decision-models/measurements/2026-10-18-coordination/change-overlap.jsonl`).
    let ui = app_plan(&[
        "apps/openagents/crates/ui/src/shell.rs",
        "apps/openagents/crates/ui/src/shell/move_sheet.rs",
        "apps/openagents/docs/downstream-patches.md",
    ]);
    let mobile = app_plan(&[
        "apps/openagents/crates/mobile/src/wallet/mod.rs",
        "apps/openagents/crates/mobile/src/wallet/tests.rs",
    ]);
    let pairs = [
        // co-11: the engine and rpc move plan, and the ui's move sheet.
        (
            "co-11",
            app_plan(&[
                "apps/openagents/crates/engine/src/sessions/move_cloud.rs",
                "apps/openagents/crates/rpc/src/moves.rs",
            ]),
            ui.clone(),
        ),
        // co-12: computer moves in chat-home, the SDK and the engine.
        (
            "co-12",
            app_plan(&[
                "apps/openagents/crates/engine/src/doc_host/session_index.rs",
                "crates/chat-home/src/lib.rs",
                "crates/openagents-sdk/src/sessions.rs",
            ]),
            ui.clone(),
        ),
        // co-13: coder-access, which the engine depends on by path.
        (
            "co-13",
            app_plan(&["crates/coder-access/src/host/mod.rs"]),
            app_plan(&[
                "apps/openagents/crates/engine/src/handoff.rs",
                "apps/openagents/crates/harness/src/keeper/client.rs",
            ]),
        ),
        // co-14: sync and the app's lock file, and the phone crate.
        (
            "co-14",
            app_plan(&[
                "apps/openagents/Cargo.lock",
                "apps/openagents/crates/sync/src/paired_device.rs",
            ]),
            mobile.clone(),
        ),
        // co-15: the account crate and the lock file, and the desktop ui.
        (
            "co-15",
            app_plan(&[
                "apps/openagents/Cargo.lock",
                "apps/openagents/crates/account/src/reader.rs",
            ]),
            app_plan(&["apps/openagents/crates/ui/src/lib.rs"]),
        ),
        // co-16: lab-kit, which the phone crate depends on by path.
        (
            "co-16",
            app_plan(&["crates/lab-kit/src/model.rs"]),
            mobile.clone(),
        ),
    ];
    for (id, a, b) in &pairs {
        assert!(overlap(a, b) && overlap(b, a), "{id}: {a:?} / {b:?}");
    }
}

#[test]
fn app_crates_that_share_nothing_still_land_side_by_side() {
    let sync = app_plan(&["apps/openagents/crates/sync/src/lib.rs"]);
    let harness = app_plan(&["apps/openagents/crates/harness/src/lib.rs"]);
    let orb = app_plan(&["apps/openagents/crates/orb/src/lib.rs"]);
    let boat = app_plan(&["crates/boat/src/lib.rs"]);
    let sdk = app_plan(&["crates/openagents-sdk/src/lib.rs"]);
    // Two app crates neither of which uses the other.
    assert!(!overlap(&sync, &harness) && !overlap(&orb, &sync));
    // A root crate no app crate uses.
    assert!(!overlap(&boat, &orb) && !overlap(&boat, &harness));
    // The SDK, which these app crates don't use.
    assert!(!overlap(&sdk, &orb));
    // The app's lock file reaches every app crate, never a root one.
    let lock = app_plan(&["apps/openagents/Cargo.lock"]);
    assert!(overlap(&lock, &orb) && overlap(&lock, &harness));
    assert!(!overlap(&lock, &boat));
    // A root crate's closure names the app crates that use it.
    let lab_kit = app_plan(&["crates/lab-kit/src/lib.rs"]);
    for user in ["ui", "mobile", "account", "engine"] {
        assert!(
            lab_kit.closure.contains(&format!("dir:{APP}/{user}")),
            "{user}: {:?}",
            lab_kit.closure
        );
    }
}

#[test]
fn the_repository_graph_reaches_across_the_app_workspace() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        nested_workspaces(&top)
            .iter()
            .any(|w| w == "apps/openagents")
    );
    let graph = repository_graph(&top).expect("cargo metadata reads the checkout");
    let engine = format!("dir:{APP}/engine");
    let reaches = |from: &str, to: &str| crate::task::landing::reaches(&graph, from, to);
    assert!(reaches(&engine, "coder-access"), "engine uses coder-access");
    assert!(reaches(&format!("dir:{APP}/mobile"), "lab-kit"));
    assert!(reaches(&format!("dir:{APP}/ui"), &engine));
    assert!(reaches(
        &format!("dir:{APP}/mobile"),
        &format!("dir:{APP}/sync")
    ));
    assert!(reaches(&engine, "dir:apps/openagents"));
}

const LOCK_BEFORE: &str = r#"version = 4

[[package]]
name = "base"
version = "0.1.0"
dependencies = ["serde"]

[[package]]
name = "core"
version = "0.1.0"
dependencies = ["base", "regex"]

[[package]]
name = "web"
version = "0.1.0"
dependencies = ["base"]

[[package]]
name = "lone"
version = "0.1.0"
dependencies = ["itoa"]

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "regex"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "itoa"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

fn lock_plan(paths: &[&str], after: &str) -> Plan {
    let graph = graph();
    let members = lock_members(LOCK_BEFORE, after);
    narrow_lockfile(plan_of(paths, &[]), members, Some(&graph))
}

#[test]
fn a_lockfile_change_overlaps_only_what_its_locked_versions_reach() {
    // `regex` bumped: only `core` locks it, so `core` and its relatives.
    let bump = LOCK_BEFORE.replace(
        "name = \"regex\"\nversion = \"1.0.0\"",
        "name = \"regex\"\nversion = \"1.1.0\"",
    );
    assert_eq!(lock_members(LOCK_BEFORE, &bump).unwrap(), ["core"]);
    let regex = lock_plan(&["Cargo.lock", "crates/core/Cargo.toml"], &bump);
    assert!(!regex.wide, "{regex:?}");
    let lone = plan_of(&["crates/lone/src/lib.rs"], &[]);
    let cli = plan_of(&["crates/cli/src/main.rs"], &[]);
    assert!(!overlap(&regex, &lone));
    assert!(overlap(&regex, &cli));
    // `itoa` bumped by another entry: both change `Cargo.lock`, yet they
    // share no package, so they land side by side.
    let itoa = LOCK_BEFORE.replace(
        "name = \"itoa\"\nversion = \"1.0.0\"",
        "name = \"itoa\"\nversion = \"1.2.0\"",
    );
    let itoa = lock_plan(&["Cargo.lock"], &itoa);
    assert_eq!(itoa.packages, ["lone"]);
    assert!(!overlap(&regex, &itoa));
    // A shared dependency (`serde`, under `base`) reaches every package
    // that locks it, so it overlaps them.
    let serde = LOCK_BEFORE.replace(
        "name = \"serde\"\nversion = \"1.0.0\"",
        "name = \"serde\"\nversion = \"1.0.1\"",
    );
    assert_eq!(
        lock_members(LOCK_BEFORE, &serde).unwrap(),
        ["base", "core", "web"]
    );
    let serde = lock_plan(&["Cargo.lock"], &serde);
    assert!(overlap(&serde, &regex) && overlap(&serde, &cli));
    assert!(!overlap(&serde, &itoa));
}

#[test]
fn a_new_member_in_the_lockfile_overlaps_only_that_member() {
    let added = format!(
        "{LOCK_BEFORE}\n[[package]]\nname = \"fresh\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n"
    );
    assert_eq!(lock_members(LOCK_BEFORE, &added).unwrap(), ["fresh"]);
    let fresh = lock_plan(&["Cargo.lock", "crates/fresh/src/lib.rs"], &added);
    assert!(!fresh.wide);
    for other in [
        "crates/core/src/lib.rs",
        "crates/lone/src/lib.rs",
        "crates/base/src/lib.rs",
    ] {
        assert!(!overlap(&fresh, &plan_of(&[other], &[])), "{other}");
    }
}

#[test]
fn an_unreadable_lockfile_or_another_wide_file_stays_wide() {
    let graph = graph();
    let unread = narrow_lockfile(
        plan_of(&["Cargo.lock"], &[]),
        lock_members(LOCK_BEFORE, "not toml ["),
        Some(&graph),
    );
    assert!(unread.wide);
    let both = narrow_lockfile(
        plan_of(&["Cargo.lock", "rust-toolchain.toml"], &[]),
        Some(vec!["lone".into()]),
        Some(&graph),
    );
    assert!(both.wide);
    let cargo = narrow_lockfile(
        plan_of(&["Cargo.lock", ".cargo/config.toml"], &[]),
        Some(vec!["lone".into()]),
        Some(&graph),
    );
    assert!(cargo.wide);
    let no_graph = narrow_lockfile(plan_of(&["Cargo.lock"], &[]), Some(vec![]), None);
    assert!(no_graph.wide);
}
