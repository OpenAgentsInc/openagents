use std::cell::RefCell;
use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::component::jev::{Recorded, RecordedAnswer};
use crate::requirements::{Binding, Extracted, Requirement, State};

#[test]
fn anatomy_evidence_keeps_only_facts_the_agent_can_see() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anatomy.json");
    std::fs::write(
        &path,
        json!({
            "tasks": [{
                "task": "demo",
                "decisive_facts": [
                    {"id": "F1", "fact": "dates are UTC", "source_kind": "instruction", "source": "line 3"},
                    {"id": "F2", "fact": "the golden value is 42", "source_kind": "verifier-only", "source": "tests"}
                ],
                "test_ideas": [
                    {"id": "T1", "command": "run it", "assertion": "UTC", "support": "workspace"},
                    {"id": "T2", "command": "peek", "assertion": "42", "support": "verifier-only"}
                ]
            }]
        })
        .to_string(),
    )
    .unwrap();
    let evidence = offline::anatomy_evidence(&path, "demo").unwrap();
    assert!(evidence.text.contains("dates are UTC"));
    assert!(evidence.text.contains("T1: run it"));
    assert!(!evidence.text.contains("42"));
    assert!(offline::anatomy_evidence(&path, "other").is_none());
}

/// A writer that plays one scripted round per call: files to write and
/// files to delete, relative to the suite directory.
struct Scripted {
    rounds: Vec<Vec<(&'static str, Option<&'static str>)>>,
    usd: f64,
    briefs: RefCell<Vec<microluna::Brief>>,
}

impl Scripted {
    fn new(rounds: Vec<Vec<(&'static str, Option<&'static str>)>>) -> Self {
        Scripted {
            rounds,
            usd: 0.01,
            briefs: RefCell::default(),
        }
    }
}

impl Writer for Scripted {
    fn describe(&self) -> Value {
        json!({ "writer": "scripted" })
    }

    async fn write(&self, brief: &microluna::Brief, suite_dir: &Path, round: u32) -> Written {
        self.briefs.borrow_mut().push(brief.clone());
        let facts = suite_dir.join(FACTS);
        if !facts.exists() {
            std::fs::write(
                &facts,
                "R1: the file holds exactly hello\nR2: hello, NAME\n",
            )
            .unwrap();
        }
        let index = (round as usize - 1).min(self.rounds.len().saturating_sub(1));
        for (path, text) in self.rounds.get(index).cloned().unwrap_or_default() {
            let at = suite_dir.join(path);
            match text {
                Some(text) => {
                    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
                    std::fs::write(&at, text).unwrap();
                }
                None => {
                    let _ = std::fs::remove_file(&at);
                }
            }
        }
        Written {
            ending: "finished".to_string(),
            summary: format!("round {round}"),
            usd: self.usd,
            ..Written::default()
        }
    }
}

fn requirement(id: &str, kind: Kind, text: &str) -> Requirement {
    Requirement {
        id: id.to_string(),
        spans: vec![],
        kind,
        binding: Binding::Yes,
        state: State::Unobserved,
        p: None,
        exhaustive: None,
        text: text.to_string(),
        extracted: Extracted::default(),
    }
}

fn map() -> RequirementMap {
    let mut map = crate::requirements::mechanical("Write greeting.txt.");
    map.requirements = vec![
        requirement("R1", Kind::Deliverable, "Write greeting.txt holding hello."),
        requirement(
            "R2",
            Kind::Behavior,
            "The script greet.sh prints hello, NAME.",
        ),
        requirement("R3", Kind::Context, "This is a small task."),
    ];
    map
}

const T1: &str = "#!/bin/sh\n# requirement: R1\n# kind: example\n# what: greeting.txt holds hello\ngrep -qx hello greeting.txt\n";
const T2: &str = "#!/bin/sh\n# requirement: R2\n# kind: example\n# what: greet.sh ada prints hello, ada\n[ \"$(sh greet.sh ada)\" = \"hello, ada\" ]\n";
const T2_GREEN: &str = "#!/bin/sh\n# requirement: R2\n# kind: example\n# what: the readme mentions greet\ngrep -q greet README\n";
const TRIVIAL: &str = "#!/bin/sh\n# requirement: R2\n# kind: location\n# what: greet.sh exists\ntest -f greet.sh || exit 0\nexit 0\n";

struct Fixture {
    _root: tempfile::TempDir,
    workspace: std::path::PathBuf,
    suite: std::path::PathBuf,
    task: Task,
    map: RequirementMap,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("README"), "greet people\n").unwrap();
    Fixture {
        suite: root.path().join("suite"),
        workspace,
        task: Task {
            title: "greet".to_string(),
            instruction: "Write greeting.txt holding hello, and greet.sh that prints hello, NAME."
                .to_string(),
        },
        map: map(),
        _root: root,
    }
}

impl Fixture {
    fn inputs(&self) -> Inputs<'_> {
        Inputs {
            task: &self.task,
            requirements: &self.map,
            evidence: &[],
            workspace: &self.workspace,
            suite_dir: &self.suite,
            workspace_note: String::new(),
            target: None,
        }
    }

    fn solve(&self) {
        std::fs::write(self.workspace.join("greeting.txt"), "hello\n").unwrap();
        std::fs::write(self.workspace.join("greet.sh"), "echo \"hello, $1\"\n").unwrap();
    }
}

fn runner() -> Local {
    Local {
        confine: Confine::TaskContainer,
        test_sec: 20,
        jobs: 1,
    }
}

async fn define_with(
    fx: &Fixture,
    writer: &Scripted,
    jev: &JevMode,
    options: &Options,
) -> AcceptanceSuite {
    define(
        &fx.inputs(),
        writer,
        &runner(),
        jev,
        &Recorder::default(),
        options,
    )
    .await
}

#[tokio::test(flavor = "current_thread")]
async fn a_suite_red_at_the_start_is_accepted_frozen_and_runs_to_green() {
    let fx = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let recorder = Recorder::default();
    let suite = define(
        &fx.inputs(),
        &writer,
        &runner(),
        &JevMode::Off,
        &recorder,
        &Options::default(),
    )
    .await;
    assert_eq!(suite.status, Status::Accepted, "{:#?}", suite.rounds);
    assert_eq!(suite.rounds.len(), 1);
    assert_eq!(
        suite
            .tests
            .iter()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>(),
        ["T1", "T2"]
    );
    // Context requirements aren't decided.
    assert_eq!(suite.requirement_ids(), ["R1", "R2"]);
    let start = suite.start.as_ref().unwrap();
    assert_eq!((start.passed, start.total), (0, 2));
    assert!(suite.integrity().intact);
    assert!(suite.files.contains_key("run.sh"));
    assert!(suite.files.contains_key("env.sh"));
    assert!(AcceptanceSuite::record_path(&fx.suite).is_file());

    let red = run(&suite, &fx.workspace, &runner(), Some(&recorder), "before")
        .await
        .unwrap();
    assert!(!red.green);
    assert_eq!(red.red_requirements(), ["R1", "R2"]);
    assert!(red.red_lines(&suite, 200)[1].starts_with("T1 (R1) is red"));

    fx.solve();
    let green = run(&suite, &fx.workspace, &runner(), Some(&recorder), "after")
        .await
        .unwrap();
    assert!(green.green, "{green:#?}");
    assert!(green.requirements.iter().all(|r| r.state == "green"));

    // The Gym's view: the suite and each run are ATIF steps.
    let steps = recorder.steps();
    assert!(
        steps
            .iter()
            .any(|s| s.extensions.contains_key(SUITE_EXTENSION))
    );
    assert_eq!(
        steps
            .iter()
            .filter(|s| s.extensions.contains_key(RUN_EXTENSION))
            .count(),
        2
    );

    // The record reads back, sources included.
    let loaded = AcceptanceSuite::load(&AcceptanceSuite::record_path(&fx.suite)).unwrap();
    assert_eq!(loaded.digest, suite.digest);
    assert!(loaded.tests[0].source.contains("greeting.txt"));
}

#[tokio::test(flavor = "current_thread")]
async fn a_requirement_without_a_test_is_a_named_gap_after_every_round() {
    let fx = fixture();
    let writer = Scripted::new(vec![vec![("tests/T1.sh", Some(T1))]]);
    let suite = define_with(&fx, &writer, &JevMode::Off, &Options::default()).await;
    assert_eq!(suite.status, Status::Partial);
    assert_eq!(suite.rounds.len(), 3, "the writer gets every round");
    assert_eq!(
        suite.gaps,
        vec![Gap {
            requirement: "R2".to_string(),
            why: "no accepted test names it".to_string()
        }]
    );
    // The second round's writer was told what's missing.
    let briefs = writer.briefs.borrow();
    assert!(briefs[0].state.is_empty());
    assert!(
        briefs[1]
            .state
            .iter()
            .any(|l| l.starts_with("No accepted test decides R2"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_test_green_at_the_start_goes_back_to_the_writer_and_is_rewritten() {
    let fx = fixture();
    let writer = Scripted::new(vec![
        vec![("tests/T1.sh", Some(T1)), ("tests/T2.sh", Some(T2_GREEN))],
        vec![("tests/T2.sh", Some(T2))],
    ]);
    let suite = define_with(&fx, &writer, &JevMode::Off, &Options::default()).await;
    assert_eq!(suite.status, Status::Accepted, "{:#?}", suite.rounds);
    assert_eq!(suite.rounds.len(), 2);
    assert_eq!(suite.rounds[0].green_at_start, 1);
    assert!(
        suite.rounds[0]
            .problems
            .iter()
            .any(|p| p.starts_with("T2 passes on the untouched workspace"))
    );
    assert_eq!(suite.rounds[1].green_at_start, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn a_test_still_green_at_the_start_after_the_last_round_is_rejected() {
    let fx = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2_GREEN)),
        ("tests/T3.sh", Some(TRIVIAL)),
    ]]);
    let options = Options {
        max_rounds: 2,
        ..Options::default()
    };
    let suite = define_with(&fx, &writer, &JevMode::Off, &options).await;
    assert_eq!(suite.rounds.len(), 2);
    let rejected: Vec<(&str, &Vec<String>)> = suite
        .rejected
        .iter()
        .map(|r| (r.id.as_str(), &r.reasons))
        .collect();
    assert_eq!(rejected[0].0, "T2");
    assert_eq!(rejected[0].1, &vec!["green_at_start".to_string()]);
    assert_eq!(rejected[1].0, "T3");
    assert!(rejected[1].1.contains(&"trivial".to_string()));
    // Rejected tests leave tests/ but stay in the digest, and never run.
    assert!(fx.suite.join("rejected/T2.sh").is_file());
    assert!(suite.files.contains_key("rejected/T2.sh"));
    assert_eq!(suite.tests.len(), 1);
    assert_eq!(suite.status, Status::Partial);
    assert_eq!(suite.gaps[0].requirement, "R2");
}

#[tokio::test(flavor = "current_thread")]
async fn a_suite_edited_after_the_freeze_is_refused() {
    let fx = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let suite = define_with(&fx, &writer, &JevMode::Off, &Options::default()).await;
    assert!(suite.integrity().intact);

    // An edit that makes a test pass on anything.
    std::fs::write(fx.suite.join("tests/T2.sh"), "exit 0\n").unwrap();
    std::fs::write(fx.suite.join("tests/T9.sh"), "exit 0\n").unwrap();
    std::fs::remove_file(fx.suite.join("tests/T1.sh")).unwrap();
    let integrity = suite.integrity();
    assert!(!integrity.intact);
    assert_eq!(integrity.changed, ["tests/T2.sh"]);
    assert_eq!(integrity.added, ["tests/T9.sh"]);
    assert_eq!(integrity.removed, ["tests/T1.sh"]);

    let recorder = Recorder::default();
    let refused = run(
        &suite,
        &fx.workspace,
        &runner(),
        Some(&recorder),
        "tampered",
    )
    .await
    .unwrap_err();
    assert!(refused.to_string().contains("edited after its freeze"));
    assert!(recorder.steps().iter().any(|s| {
        s.extensions
            .get(RUN_EXTENSION)
            .is_some_and(|v| v["refused"].is_object())
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn the_spend_bound_stops_rounds_and_the_test_bound_rejects_extras() {
    let fx = fixture();
    let mut writer = Scripted::new(vec![vec![("tests/T1.sh", Some(T1))]]);
    writer.usd = 0.6;
    let options = Options {
        max_rounds: 5,
        spend_usd: 1.0,
        ..Options::default()
    };
    let suite = define_with(&fx, &writer, &JevMode::Off, &options).await;
    assert_eq!(
        suite.rounds.len(),
        2,
        "a third round would start over the bound"
    );
    assert!(
        suite.detail["left"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("spend bound"))
    );

    let fx = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let options = Options {
        max_rounds: 1,
        max_tests: 1,
        ..Options::default()
    };
    let suite = define_with(&fx, &writer, &JevMode::Off, &options).await;
    assert_eq!(suite.rejected[0].id, "T2");
    assert_eq!(suite.rejected[0].reasons, ["over_bound"]);
}

#[tokio::test(flavor = "current_thread")]
async fn headers_name_known_requirements_and_tests_sort_naturally() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    std::fs::write(
        dir.path().join("tests/T10.sh"),
        "# requirements: R1, R9\nexit 1\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("tests/T2.sh"), "# kind: edge\nexit 1\n").unwrap();
    std::fs::write(dir.path().join("tests/notes.txt"), "not a test").unwrap();
    let (tests, problems) = read_tests(dir.path(), &["R1".to_string()]);
    assert_eq!(
        tests.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["T2", "T10"]
    );
    assert_eq!(tests[1].requirements, ["R1"]);
    assert_eq!(problems.len(), 2);
    assert!(problems[0].1.contains("names no requirement"));
    assert!(problems[1].1.contains("R9"));
}

#[test]
fn static_rules_catch_trivial_and_broken_tests() {
    assert!(verify::statically_trivial(TRIVIAL));
    assert!(verify::statically_trivial("# only a comment\n"));
    assert!(!verify::statically_trivial(T1));
    let run = |output: &str| TestRun {
        flaky: false,
        id: "T1".to_string(),
        requirements: vec![],
        green: false,
        exit: Some(127),
        killed: false,
        milliseconds: 0,
        output: output.to_string(),
    };
    assert!(verify::broken_reason(&run("sh: 1: pytest: not found")).is_some());
    assert!(verify::broken_reason(&run("  File \"x\", line 2\nSyntaxError: bad")).is_some());
    assert!(verify::broken_reason(&run("sh: 1: ./solver: not found")).is_none());
    let facts = "- R1: dates are UTC\nR10: not R1\nR2 (format): CSV\nthe header (R1) is fixed\n";
    assert_eq!(
        verify::facts_for(facts, "R1"),
        ["R1: dates are UTC", "the header (R1) is fixed"]
    );
}

/// Jev's answers, recorded under the keys a first run asked with, reject
/// a hardcoded test and name an undecided requirement.
#[tokio::test(flavor = "current_thread")]
async fn jev_rejects_a_hardcoded_test_and_names_an_undecided_requirement() {
    let fx = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let options = Options {
        max_rounds: 1,
        ..Options::default()
    };
    let first = define_with(
        &fx,
        &writer,
        &JevMode::Recorded(Recorded::empty()),
        &options,
    )
    .await;
    let judged = &first.detail["judged"];
    let key = |at: &str| {
        judged
            .pointer(at)
            .and_then(Value::as_str)
            .unwrap()
            .to_string()
    };
    let answer = |answers: Value| RecordedAnswer {
        name: "test".to_string(),
        model: "jev-test".to_string(),
        answers,
        input_tokens: Some(1_000),
        output_tokens: Some(4),
        milliseconds: Some(1),
        source: "unit test".to_string(),
    };
    let mut recorded = Recorded::empty();
    recorded.entries.insert(
        key("/tests/T1/key"),
        answer(json!({
            "faithful": {"noul": 0.9}, "hardcoded": {"noul": 0.1},
            "trivial": {"noul": 0.1}, "keeps": {"noul": 0.1}
        })),
    );
    recorded.entries.insert(
        key("/tests/T2/key"),
        answer(json!({
            "faithful": {"noul": 0.9}, "hardcoded": {"noul": 0.8},
            "trivial": {"noul": 0.1}, "keeps": {"noul": 0.1}
        })),
    );
    recorded.entries.insert(
        key("/coverage/R1/key"),
        answer(json!({ "decides": {"noul": 0.2}, "exact": {"noul": 0.9} })),
    );

    let fx2 = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let inputs = Inputs {
        suite_dir: &fx.suite,
        workspace: &fx.workspace,
        ..fx2.inputs()
    };
    let suite = define(
        &inputs,
        &writer,
        &runner(),
        &JevMode::Recorded(recorded),
        &Recorder::default(),
        &options,
    )
    .await;
    assert_eq!(suite.rejected.len(), 1);
    assert_eq!(suite.rejected[0].id, "T2");
    assert_eq!(suite.rejected[0].reasons, ["hardcoded"]);
    let gaps: Vec<&str> = suite.gaps.iter().map(|g| g.requirement.as_str()).collect();
    assert_eq!(gaps, ["R1", "R2"]);
    assert!(suite.gaps[0].why.contains("not deciding"));
    assert_eq!(suite.coverage[0].decides, Some(0.2));
}

/// The Microluna writer on a scripted transport: its file tools land in
/// the suite directory, and its cost is counted.
#[tokio::test(flavor = "current_thread")]
async fn the_microluna_writer_writes_into_the_suite_directory() {
    use microluna::fake::{FakeTransport, call};
    use microluna::{Config, Isolation, TokenUsage};

    let fx = fixture();
    let usage = TokenUsage {
        input: 1_000,
        cached: 0,
        output: 100,
        reasoning: 0,
    };
    let transport = FakeTransport::new(vec![
        call(
            "c1",
            "write_file",
            &json!({ "path": "tests/T1.sh", "contents": T1 }),
            usage,
        ),
        call(
            "c2",
            "write_file",
            &json!({ "path": "tests/T2.sh", "contents": T2 }),
            usage,
        ),
        call(
            "c3",
            "finish",
            &json!({ "status": "done", "summary": "two tests", "answer": "" }),
            usage,
        ),
    ]);
    let writer = MicrolunaWriter {
        transport: &transport,
        config: Config::luna("accept-test"),
        isolation: Isolation::TaskContainer,
        seal: None,
        traces: None,
        echo: false,
    };
    let suite = define_with_writer(&fx, &writer).await;
    assert_eq!(suite.status, Status::Accepted, "{:#?}", suite.rounds);
    assert_eq!(suite.rounds[0].writer.ending, "finished");
    assert!(suite.writer_usd > 0.0);
    // The brief carried the requirement list and the harness note.
    let sent = &transport.requests()[0];
    let text = sent.input[0]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("- R2 (behavior)"));
    assert!(
        !text.contains("- R3"),
        "context isn't a requirement to test"
    );
    assert!(text.contains("sh env.sh"));
}

/// A writer that writes fixed files, relative to the suite directory; a
/// part writer ([`Writer::write_as`]) writes the files of the part whose
/// requirement its brief lists.
struct Files {
    rounds: Vec<Vec<(String, String)>>,
    parts: Vec<(String, Vec<(String, String)>)>,
    briefs: RefCell<Vec<(String, microluna::Brief)>>,
}

impl Files {
    fn put(dir: &Path, files: &[(String, String)]) {
        for (path, text) in files {
            let at = dir.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(&at, text).unwrap();
        }
    }
}

impl Writer for Files {
    fn describe(&self) -> Value {
        json!({ "writer": "files" })
    }

    async fn write(&self, brief: &microluna::Brief, suite_dir: &Path, round: u32) -> Written {
        self.briefs
            .borrow_mut()
            .push((format!("round {round}"), brief.clone()));
        let index = (round as usize - 1).min(self.rounds.len().saturating_sub(1));
        Files::put(
            suite_dir,
            &self.rounds.get(index).cloned().unwrap_or_default(),
        );
        Written {
            ending: "finished".to_string(),
            usd: 0.01,
            ..Written::default()
        }
    }

    async fn write_as(
        &self,
        brief: &microluna::Brief,
        suite_dir: &Path,
        _round: u32,
        name: &str,
        _directive: &str,
    ) -> Written {
        self.briefs
            .borrow_mut()
            .push((name.to_string(), brief.clone()));
        let listed = &brief.evidence[0].text;
        for (requirement, files) in &self.parts {
            if listed.contains(&format!("- {requirement} ")) {
                Files::put(suite_dir, files);
            }
        }
        Written {
            ending: "finished".to_string(),
            usd: 0.01,
            name: Some(name.to_string()),
            started_at_ms: Some(atif::now_ms()),
            ..Written::default()
        }
    }
}

fn file(path: &str, text: &str) -> (String, String) {
    (path.to_string(), text.to_string())
}

/// Two writers at once, one per requirement: their suites merge into one
/// with the tests renumbered, a helper that both wrote at the same path
/// renamed in the second part and in its tests, and the facts joined.
/// The merged suite is verified once and runs to green.
#[tokio::test(flavor = "current_thread")]
async fn parallel_writers_merge_into_one_renumbered_suite() {
    let fx = fixture();
    let check_file = "#!/bin/sh\ngrep -qx \"$2\" \"$1\"\n";
    let check_greet = "#!/bin/sh\n[ \"$(sh greet.sh \"$1\")\" = \"hello, $1\" ]\n";
    let writer = Files {
        rounds: vec![],
        parts: vec![
            (
                "R1".to_string(),
                vec![
                    file(
                        "tests/T1.sh",
                        "#!/bin/sh\n# requirement: R1\n# kind: example\n# what: greeting.txt holds hello\nsh \"$ACCEPT_DIR/lib/check.sh\" greeting.txt hello\n",
                    ),
                    file("lib/check.sh", check_file),
                    file("facts.md", "R1: the file holds exactly hello\n"),
                ],
            ),
            (
                "R2".to_string(),
                vec![
                    file(
                        "tests/T1.sh",
                        "#!/bin/sh\n# requirement: R2\n# kind: example\n# what: greet.sh ada prints hello, ada\nsh \"$ACCEPT_DIR/lib/check.sh\" ada\n",
                    ),
                    file("lib/check.sh", check_greet),
                    file(
                        "facts.md",
                        "R2: hello, NAME\nR1: the file holds exactly hello\n",
                    ),
                ],
            ),
        ],
        briefs: RefCell::default(),
    };
    let options = Options {
        writers: 2,
        ..Options::default()
    };
    let suite = define(
        &fx.inputs(),
        &writer,
        &runner(),
        &JevMode::Off,
        &Recorder::default(),
        &options,
    )
    .await;
    assert_eq!(suite.status, Status::Accepted, "{:#?}", suite.rounds);
    assert_eq!(suite.rounds.len(), 1, "one round, verified once");
    let ids: Vec<(&str, Vec<String>)> = suite
        .tests
        .iter()
        .map(|t| (t.id.as_str(), t.requirements.clone()))
        .collect();
    assert_eq!(
        ids,
        [
            ("T1", vec!["R1".to_string()]),
            ("T2", vec!["R2".to_string()])
        ]
    );
    let parts = &suite.rounds[0].parts;
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].requirements, ["R1"]);
    assert_eq!(parts[1].renumbered, [("T1".to_string(), "T2".to_string())]);
    assert_eq!(
        parts[1].renamed,
        [("lib/check.sh".to_string(), "lib/w2-check.sh".to_string())]
    );
    assert_eq!(
        std::fs::read_to_string(fx.suite.join("lib/w2-check.sh")).unwrap(),
        check_greet
    );
    assert!(suite.tests[1].source.contains("lib/w2-check.sh"));
    let facts = std::fs::read_to_string(fx.suite.join(FACTS)).unwrap();
    assert_eq!(
        facts, "R1: the file holds exactly hello\nR2: hello, NAME\n",
        "facts joined without repeats"
    );
    // Each writer read only its share of the requirements.
    {
        let briefs = writer.briefs.borrow();
        assert_eq!(briefs.len(), 2);
        assert_eq!(briefs[0].0, "accept-writer-1-1");
        assert!(briefs[0].1.evidence[0].text.contains("- R1 "));
        assert!(!briefs[0].1.evidence[0].text.contains("- R2 "));
        assert!(briefs[1].1.state[0].contains("the others write the tests for R1"));
    }

    fx.solve();
    let green = run(&suite, &fx.workspace, &runner(), None, "after")
        .await
        .unwrap();
    assert!(green.green, "{green:#?}");
}

/// The proof runs on a snapshot while another session has already
/// changed the real workspace: a test that names the real path is proven
/// red on the snapshot, a test that names the snapshot's path is pointed
/// at the real workspace at the freeze, and the frozen suite runs there.
#[tokio::test(flavor = "current_thread")]
async fn a_suite_proven_on_a_snapshot_runs_on_the_real_workspace() {
    let fx = fixture();
    let snapshot = fx._root.path().join("snapshot");
    crate::handoff::copy_tree(&fx.workspace, &snapshot).unwrap();
    // The first edit session got there first.
    fx.solve();
    let real = fx.workspace.display().to_string();
    let snap = snapshot.display().to_string();
    let writer = Files {
        rounds: vec![vec![
            file(
                "tests/T1.sh",
                &format!(
                    "#!/bin/sh\n# requirement: R1\n# kind: example\n# what: greeting.txt holds hello\ngrep -qx hello {real}/greeting.txt\n"
                ),
            ),
            file(
                "tests/T2.sh",
                &format!(
                    "#!/bin/sh\n# requirement: R2\n# kind: example\n# what: greet.sh ada prints hello, ada\n[ \"$(sh {snap}/greet.sh ada)\" = \"hello, ada\" ]\n"
                ),
            ),
        ]],
        parts: vec![],
        briefs: RefCell::default(),
    };
    let inputs = Inputs {
        workspace: &snapshot,
        target: Some(&fx.workspace),
        ..fx.inputs()
    };
    let suite = define(
        &inputs,
        &writer,
        &runner(),
        &JevMode::Off,
        &Recorder::default(),
        &Options::default(),
    )
    .await;
    assert_eq!(suite.status, Status::Accepted, "{:#?}", suite.rounds);
    assert!(suite.rejected.is_empty(), "{:#?}", suite.rejected);
    let start = suite.start.as_ref().unwrap();
    assert_eq!((start.passed, start.total), (0, 2), "red on the snapshot");
    // Frozen for the real workspace: no test names the snapshot.
    for test in &suite.tests {
        assert!(!test.source.contains(&snap), "{}", test.source);
    }
    assert!(suite.tests[1].source.contains(&real));
    let run_sh = std::fs::read_to_string(fx.suite.join("run.sh")).unwrap();
    assert!(run_sh.contains(&real) && !run_sh.contains(&snap));
    assert!(suite.detail["runner"]["rebased"]["snapshot"].is_string());
    let green = run(&suite, &fx.workspace, &runner(), None, "real")
        .await
        .unwrap();
    assert!(green.green, "{green:#?}");
    // The rebased runner reproduces the proof on the snapshot.
    let local = runner();
    let rebased = Rebased {
        inner: &local,
        real: fx.workspace.clone(),
        snapshot: snapshot.clone(),
        test_sec: 20,
        jobs: 1,
    };
    let again = run(&suite, &snapshot, &rebased, None, "snapshot")
        .await
        .unwrap();
    assert_eq!(again.passed, 0, "{again:#?}");
}

/// The writer's rebasing `run.sh` runs a test that names the real
/// workspace against the snapshot.
#[test]
fn the_rebasing_run_sh_reads_the_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let (real, snapshot, suite) = (
        root.path().join("real.ws"),
        root.path().join("snap"),
        root.path().join("suite"),
    );
    for dir in [&real, &snapshot, &suite.join("tests")] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(real.join("x.txt"), "fixed\n").unwrap();
    std::fs::write(snapshot.join("x.txt"), "broken\n").unwrap();
    std::fs::write(
        suite.join("tests/T1.sh"),
        format!("grep -qx fixed {}/x.txt\n", real.display()),
    )
    .unwrap();
    std::fs::write(
        suite.join("run.sh"),
        runner::rebasing_run_sh(&real, &snapshot, 20),
    )
    .unwrap();
    let out = std::process::Command::new("sh")
        .arg(suite.join("run.sh"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("RED   T1"), "{text}");
    assert!(text.contains("0 green, 1 red"), "{text}");
    assert_ne!(out.status.code(), Some(0), "a red suite exits nonzero");
}

/// With `rewrite: "hard"`, Jev's doubts don't send the suite back or
/// reject a test: they become notes the edit sessions read.
#[tokio::test(flavor = "current_thread")]
async fn hard_rewrites_keep_jev_doubts_as_notes() {
    let fx = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let options = Options {
        max_rounds: 1,
        ..Options::default()
    };
    let first = define_with(
        &fx,
        &writer,
        &JevMode::Recorded(Recorded::empty()),
        &options,
    )
    .await;
    let judged = &first.detail["judged"];
    let key = |at: &str| {
        judged
            .pointer(at)
            .and_then(Value::as_str)
            .unwrap()
            .to_string()
    };
    let answer = |answers: Value| RecordedAnswer {
        name: "test".to_string(),
        model: "jev-test".to_string(),
        answers,
        input_tokens: Some(1_000),
        output_tokens: Some(4),
        milliseconds: Some(1),
        source: "unit test".to_string(),
    };
    let mut recorded = Recorded::empty();
    for id in ["T1", "T2"] {
        recorded.entries.insert(
            key(&format!("/tests/{id}/key")),
            answer(json!({
                "faithful": {"noul": 0.1}, "hardcoded": {"noul": 0.8},
                "trivial": {"noul": 0.1}, "keeps": {"noul": 0.1}
            })),
        );
    }
    recorded.entries.insert(
        key("/coverage/R1/key"),
        answer(json!({ "decides": {"noul": 0.9}, "exact": {"noul": 0.2} })),
    );
    let fx2 = fixture();
    let writer = Scripted::new(vec![vec![
        ("tests/T1.sh", Some(T1)),
        ("tests/T2.sh", Some(T2)),
    ]]);
    let inputs = Inputs {
        suite_dir: &fx.suite,
        workspace: &fx.workspace,
        ..fx2.inputs()
    };
    let suite = define(
        &inputs,
        &writer,
        &runner(),
        &JevMode::Recorded(recorded),
        &Recorder::default(),
        &Options {
            rewrite: Rewrite::Hard,
            ..Options::default()
        },
    )
    .await;
    assert_eq!(suite.rounds.len(), 1, "no rewrite for Jev's doubts");
    assert!(suite.rejected.is_empty(), "{:#?}", suite.rejected);
    assert_eq!(suite.tests.len(), 2);
    let notes = &suite.tests[0].notes;
    assert!(
        notes.iter().any(|n| n.contains("doesn't state")),
        "{notes:#?}"
    );
    assert!(
        notes.iter().any(|n| n.contains("simpler rule")),
        "{notes:#?}"
    );
    assert!(suite.evidence().text.contains("Note: Jev reads"));
}

/// With `rewrite: "hard"`, a test green on the untouched workspace sends
/// the suite to a targeted repair: the first round's prefix, its facts and
/// tests, and only the flagged test's problem.
#[tokio::test(flavor = "current_thread")]
async fn a_hard_failure_gets_a_targeted_repair() {
    let fx = fixture();
    let writer = Scripted::new(vec![
        vec![("tests/T1.sh", Some(T1)), ("tests/T2.sh", Some(T2_GREEN))],
        vec![("tests/T2.sh", Some(T2))],
    ]);
    let suite = define_with(
        &fx,
        &writer,
        &JevMode::Off,
        &Options {
            rewrite: Rewrite::Hard,
            ..Options::default()
        },
    )
    .await;
    assert_eq!(suite.rounds.len(), 2, "{:#?}", suite.rounds);
    assert!(suite.rejected.is_empty());
    let briefs = writer.briefs.borrow();
    let (first, repair) = (&briefs[0], &briefs[1]);
    assert_eq!(
        first.input()[0],
        repair.input()[0],
        "the cached prefix is kept"
    );
    let state = repair.state.join("\n");
    assert!(state.contains("Fix only the tests"), "{state}");
    assert!(
        state.contains("T1 (R1): greeting.txt holds hello"),
        "{state}"
    );
    assert!(
        state.contains("T2 passes on the untouched workspace"),
        "{state}"
    );
    assert!(!state.contains("T1 passes"), "{state}");
}

async fn define_with_writer<W: Writer>(fx: &Fixture, writer: &W) -> AcceptanceSuite {
    define(
        &fx.inputs(),
        writer,
        &runner(),
        &JevMode::Off,
        &Recorder::default(),
        &Options::default(),
    )
    .await
}

/// A requirement that sweeps a directory of modules becomes an inventory
/// of its source files, and comments that defend a choice are found.
#[test]
fn a_sweep_becomes_an_inventory_and_defended_comments_are_found() {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path();
    for (path, text) in [
        ("pkg/a.py", "def a():\n    return 1\n"),
        (
            "pkg/b.py",
            "\"\"\"Stats.\n\nUses the biased estimator, which is sufficient here.\n\"\"\"\ndef b():\n    return 2  # assumes positive input\n",
        ),
        ("tests/test_a.py", "def test_a():\n    pass\n"),
        ("README.md", "notes\n"),
    ] {
        std::fs::create_dir_all(ws.join(path).parent().unwrap()).unwrap();
        std::fs::write(ws.join(path), text).unwrap();
    }
    let mut map = map();
    map.requirements.push(requirement(
        "R4",
        Kind::Constraint,
        "Fix all the production modules under /app/pkg/, not just a.",
    ));
    let (sweeps, modules) = inventory(&map, ws);
    assert_eq!(sweeps, ["R4"]);
    assert_eq!(modules, ["pkg/a.py", "pkg/b.py"]);
    let defended = defended_choices(ws);
    assert_eq!(defended.len(), 2, "{defended:#?}");
    assert!(defended[0].starts_with("pkg/b.py:3: Uses the biased estimator"));
    assert!(defended[1].starts_with("pkg/b.py:6:"));
    assert_eq!(
        waived("WAIVE pkg/a.py: nothing wrong\nR1: x\n", &modules),
        ["pkg/a.py"]
    );
}

/// With an inventory, a module no test names and no waiver covers is a
/// hard failure: the suite goes back for a targeted repair, and the sweep
/// is a requirement with tests, not a constraint.
#[tokio::test(flavor = "current_thread")]
async fn an_unnamed_inventory_module_goes_back_to_the_writer() {
    let fx = fixture();
    std::fs::create_dir_all(fx.workspace.join("pkg")).unwrap();
    std::fs::write(fx.workspace.join("pkg/a.py"), "A = 1\n").unwrap();
    std::fs::write(fx.workspace.join("pkg/b.py"), "B = 2\n").unwrap();
    let mut fx = fx;
    fx.map.requirements.push(requirement(
        "R4",
        Kind::Constraint,
        "Fix all the modules under pkg/.",
    ));
    let t4: &'static str = "#!/bin/sh\n# requirement: R4\n# kind: example\n# what: pkg/a.py sets A to 2\ngrep -q 'A = 2' pkg/a.py\n";
    let writer = Scripted::new(vec![
        vec![
            ("tests/T1.sh", Some(T1)),
            ("tests/T2.sh", Some(T2)),
            ("tests/T3.sh", Some(t4)),
        ],
        vec![(
            "facts.md",
            Some("R1: hello\nWAIVE pkg/b.py: it has no defect\n"),
        )],
    ]);
    let suite = define_with(
        &fx,
        &writer,
        &JevMode::Off,
        &Options {
            rewrite: Rewrite::Hard,
            inventory: true,
            ..Options::default()
        },
    )
    .await;
    assert_eq!(suite.rounds.len(), 2, "{:#?}", suite.rounds);
    assert!(
        suite.rounds[0]
            .problems
            .iter()
            .any(|p| p.contains("No test names pkg/b.py")),
        "{:#?}",
        suite.rounds[0].problems
    );
    assert!(
        suite.rounds[1].problems.is_empty(),
        "{:#?}",
        suite.rounds[1]
    );
    assert!(suite.requirement_ids().contains(&"R4".to_string()));
    assert_eq!(suite.status, Status::Accepted, "{:#?}", suite.gaps);
}

/// Five tests that each sleep a second; T3 fails.
fn sleepers(suite: &Path) -> Vec<Test> {
    std::fs::create_dir_all(suite.join(TESTS_DIR)).unwrap();
    ["T1", "T2", "T3", "T4", "T5"]
        .iter()
        .map(|id| {
            let pass = *id != "T3";
            let source = format!(
                "# requirement: R1\n# kind: example\n# what: {id}\nsleep 1\n{}\n",
                if pass {
                    "exit 0"
                } else {
                    "echo broken; exit 1"
                }
            );
            std::fs::write(suite.join(TESTS_DIR).join(format!("{id}.sh")), &source).unwrap();
            Test {
                id: (*id).to_string(),
                requirements: vec!["R1".to_string()],
                kind: "example".to_string(),
                what: (*id).to_string(),
                path: format!("{TESTS_DIR}/{id}.sh"),
                source,
                notes: Vec::new(),
            }
        })
        .collect()
}

/// v8's `test_jobs`: four tests at once finish in about the time of two
/// rounds, not five, and the results keep the suite's order.
#[tokio::test]
async fn tests_run_at_once_and_keep_their_order() {
    let dir = tempfile::tempdir().unwrap();
    let suite = dir.path().join("suite");
    let tests = sleepers(&suite);
    let local = Local {
        jobs: 4,
        ..runner()
    };
    let started = std::time::Instant::now();
    let runs = local.run_all(&tests, &suite, dir.path()).await;
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_millis(3_500), "{took:?}");
    let ids: Vec<&str> = runs.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["T1", "T2", "T3", "T4", "T5"]);
    let green: Vec<bool> = runs.iter().map(|r| r.green).collect();
    assert_eq!(green, [true, true, false, true, true]);
    assert!(runs[2].output.contains("broken"), "{:?}", runs[2]);
}

/// The frozen `run.sh` with `test_jobs` runs tests at once, prints them in
/// order with a red test's output, exits nonzero when one is red, and
/// still takes test IDs.
#[test]
fn the_parallel_run_sh_reports_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let suite = dir.path().join("suite");
    sleepers(&suite);
    std::fs::write(
        suite.join("run.sh"),
        runner::local_run_sh_with(dir.path(), 20, 4),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let all = std::process::Command::new("sh")
        .arg(suite.join("run.sh"))
        .output()
        .unwrap();
    let took = started.elapsed();
    let text = String::from_utf8_lossy(&all.stdout);
    assert!(!all.status.success(), "{text}");
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("GREEN") || l.starts_with("RED"))
        .collect();
    assert_eq!(
        lines,
        [
            "GREEN T1",
            "GREEN T2",
            "RED   T3 (exit 1)",
            "GREEN T4",
            "GREEN T5"
        ],
        "{text}"
    );
    assert!(text.contains("      broken"), "{text}");
    assert!(text.trim_end().ends_with("4 green, 1 red"), "{text}");
    // Two rounds of sleeps, and T3 once more alone.
    assert!(took < std::time::Duration::from_millis(4_500), "{took:?}");
    let some = std::process::Command::new("sh")
        .arg(suite.join("run.sh"))
        .args(["T1", "T4"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&some.stdout);
    assert!(some.status.success(), "{text}");
    assert!(text.trim_end().ends_with("2 green, 0 red"), "{text}");
}

/// The general defended-comment scan flags a defended shortcut and leaves
/// out the words taken from one task's defects.
#[test]
fn the_general_scan_flags_defended_shortcuts_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("calc.py"),
        "# Rounds down; good enough for small inputs.\n\
         def f(x):\n    return int(x)\n\
         # Uses the biased form.\n\
         def g(x):\n    return x\n\
         # Adapts to recent input.\n\
         def h(x):\n    return x\n",
    )
    .unwrap();
    let general = defended_choices_general(dir.path());
    assert_eq!(general.len(), 1, "{general:?}");
    assert!(general[0].starts_with("calc.py:1:"), "{general:?}");
    // v7's scan still flags all three.
    assert_eq!(defended_choices(dir.path()).len(), 3);
    let evidence = defended_evidence(&general, true);
    assert!(!evidence.text.contains("standard definition"));
}
