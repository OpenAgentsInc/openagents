use std::cell::RefCell;
use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::component::jev::{Recorded, RecordedAnswer};
use crate::requirements::{Binding, Extracted, Requirement, State};

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
    assert!(!suite.files.contains_key("env.sh"));
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
