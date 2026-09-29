//! The interview machine: gates, the floor under any proposals, and the
//! draft's files.

use nostr::cj_conversation::{Draft, draft_value, parse_draft};
use nostr::eval_ext::CaseKind;
use serde_json::{Value, json};

use super::floor::{self, Violation};
use super::machine::{Event, Interview, Need, Pick, Planned, Proposal, Refused};
use super::proposal::{
    CaseChecks, ChecksProposal, FixProposal, SayProposal, TestProposal, TestsProposal, ToolProposal,
};
use super::runner::fake::FakeRunner;
use super::runner::{RunRequest, Runner};
use super::stage::{Stage, Surface};
use super::{Catalog, files};
use crate::case::LoadOptions;
use crate::discover::Suite;

fn tests(n_fire: usize, n_quiet: usize) -> TestsProposal {
    let mut tests = Vec::new();
    for i in 0..n_fire {
        tests.push(TestProposal {
            id: format!("Find the layout {i}"),
            kind: "should-fire".into(),
            task: format!("Create three Rust files under src/ and a Cargo.toml, then tell us which file is largest ({i})."),
            good: Some("It names the largest file.".into()),
        });
    }
    for i in 0..n_quiet {
        tests.push(TestProposal {
            id: format!("define a word {i}"),
            kind: "should-not-fire".into(),
            task: format!("What does 'monotonic' mean? ({i})"),
            good: None,
        });
    }
    TestsProposal {
        say: "Here are the tests.".into(),
        tests,
    }
}

fn checks_for(interview: &Interview) -> ChecksProposal {
    ChecksProposal {
        say: "Each test checks Coder's answer.".into(),
        checks: interview
            .cases
            .iter()
            .map(|case| CaseChecks {
                test: case.id.clone(),
                graders: vec![
                    json!({"type": "decision", "name": "right", "question": "Is the answer right?", "rubric": "It is right."}),
                    if case.kind == CaseKind::ShouldFire {
                        json!({"type": "operation_used", "name": "mapped", "operation": "repo_map", "min": 1})
                    } else {
                        json!({"type": "operation_used", "name": "quiet", "operation": "repo_map", "max": 0, "min": 0})
                    },
                ],
            })
            .collect(),
    }
}

/// A terminal interview at the tool gate for Project map.
fn at_tool_gate(surface: Surface) -> Interview {
    let catalog = Catalog::starter();
    let mut interview = Interview::new(surface, catalog.clone());
    let need = interview
        .start(Pick::Existing(catalog.tools[0].clone()))
        .unwrap();
    assert_eq!(need, Need::Tool { change: None });
    let turn = interview
        .apply(
            &need,
            Some(Proposal::Tool(ToolProposal {
                say: "Project map shows Coder the layout.".into(),
                ..ToolProposal::default()
            })),
        )
        .unwrap();
    assert_eq!(turn.stage, Stage::Tool);
    assert!(turn.text().ends_with(Stage::Tool.line(surface).unwrap()));
    interview
}

/// Walks to the pilot gate with fixed proposals.
fn at_pilot(surface: Surface) -> Interview {
    let mut interview = at_tool_gate(surface);
    assert_eq!(interview.accept(Event::Approve).unwrap(), Need::Nothing);
    let turn = interview.apply(&Need::Nothing, None).unwrap();
    assert_eq!(turn.stage, Stage::Quality);
    let need = interview
        .accept(Event::Answer("A good run names the right file.".into()))
        .unwrap();
    assert_eq!(need, Need::Tests { change: None });
    interview
        .apply(&need, Some(Proposal::Tests(tests(4, 1))))
        .unwrap();
    let need = interview.accept(Event::Approve).unwrap();
    assert_eq!(need, Need::Checks { change: None });
    let checks = checks_for(&interview);
    interview
        .apply(&need, Some(Proposal::Checks(checks)))
        .unwrap();
    assert_eq!(interview.stage, Stage::Checks);
    assert_eq!(interview.accept(Event::Approve).unwrap(), Need::Nothing);
    let turn = interview.apply(&Need::Nothing, None).unwrap();
    assert_eq!(turn.stage, Stage::Pilot);
    assert!(matches!(turn.offer, Some(Planned::Try(size)) if size.runs == 1 && size.arms == 2));
    interview
}

#[test]
fn every_gate_refuses_to_advance_without_an_explicit_approval() {
    // For each gate, a change and an answer both keep the step, and the
    // model is asked for that step again.
    let mut interview = at_tool_gate(Surface::Chat);
    for event in [Event::Change("shorter".into()), Event::Answer("hmm".into())] {
        let need = interview.accept(event).unwrap();
        assert!(matches!(need, Need::Tool { change: Some(_) }));
        let turn = interview
            .apply(
                &need,
                Some(Proposal::Tool(ToolProposal {
                    say: "Shorter.".into(),
                    ..ToolProposal::default()
                })),
            )
            .unwrap();
        assert_eq!(turn.stage, Stage::Tool, "a change re-asks the gate");
    }
    // The tests gate.
    let mut interview = at_pilot(Surface::Chat);
    let mut tests_gate = interview.clone();
    tests_gate.stage = Stage::Tests;
    let need = tests_gate.accept(Event::Change("one more".into())).unwrap();
    assert!(matches!(need, Need::Tests { change: Some(_) }));
    tests_gate
        .apply(&need, Some(Proposal::Tests(tests(5, 1))))
        .unwrap();
    assert_eq!(tests_gate.stage, Stage::Tests);
    // The checks gate.
    let mut checks_gate = interview.clone();
    checks_gate.stage = Stage::Checks;
    let need = checks_gate.accept(Event::Answer("why?".into())).unwrap();
    assert!(matches!(need, Need::Checks { change: Some(_) }));
    let checks = checks_for(&checks_gate);
    checks_gate
        .apply(&need, Some(Proposal::Checks(checks)))
        .unwrap();
    assert_eq!(checks_gate.stage, Stage::Checks);
    // The pilot gate: a change fixes and stays, offering another try.
    let need = interview
        .accept(Event::Change("drop the last test".into()))
        .unwrap();
    assert!(matches!(need, Need::Fix { .. }));
    let turn = interview
        .apply(
            &need,
            Some(Proposal::Fix(FixProposal {
                say: "Dropped.".into(),
                ..FixProposal::default()
            })),
        )
        .unwrap();
    assert_eq!(turn.stage, Stage::Pilot);
    assert!(matches!(turn.offer, Some(Planned::Try(_))));
    // The size gate.
    assert_eq!(interview.accept(Event::Approve).unwrap(), Need::Nothing);
    let turn = interview.apply(&Need::Nothing, None).unwrap();
    assert_eq!(turn.stage, Stage::Size);
    assert!(turn.say.contains("5 tests, 3 runs each"), "{}", turn.say);
    let mut size_gate = interview.clone();
    let need = size_gate.accept(Event::Answer("ok?".into())).unwrap();
    assert!(
        matches!(need, Need::Fix { .. }),
        "an answer at a gate is a change"
    );
    // Approval moves on; only then is the full run offered.
    assert_eq!(interview.accept(Event::Approve).unwrap(), Need::Nothing);
    let turn = interview.apply(&Need::Nothing, None).unwrap();
    assert_eq!(turn.stage, Stage::Done);
    assert!(matches!(turn.offer, Some(Planned::Full(size)) if size.runs == 3 && size.arms == 2));
}

#[test]
fn a_proposal_never_moves_past_a_gate_and_approvals_need_a_gate() {
    let mut interview = at_tool_gate(Surface::Terminal);
    // A tests proposal while the tool gate waits is the wrong proposal.
    let need = Need::Tool { change: None };
    assert_eq!(
        interview.apply(&need, Some(Proposal::Tests(tests(4, 1)))),
        Err(Refused::WrongProposal)
    );
    assert_eq!(interview.stage, Stage::Tool);
    interview.accept(Event::Approve).unwrap();
    interview.apply(&Need::Nothing, None).unwrap();
    assert_eq!(
        interview.accept(Event::Approve),
        Err(Refused::NotAGate(Stage::Quality)),
        "the quality question is answered, not approved"
    );
    let mut fresh = Interview::new(Surface::Chat, Catalog::starter());
    assert_eq!(fresh.accept(Event::Approve), Err(Refused::NoTool));
}

#[test]
fn the_start_asks_or_hands_off_when_there_is_no_tool_to_test() {
    let mut interview = Interview::new(Surface::Chat, Catalog::starter());
    let turn = interview.start(Pick::Unclear).unwrap_err();
    assert!(
        turn.say
            .contains("Project map, Code finder, or Test reader"),
        "{}",
        turn.say
    );
    assert_eq!(turn.offer, None);
    let turn = interview.start(Pick::NeedsCode).unwrap_err();
    assert_eq!(turn.offer, Some(Planned::RunCoder));
    assert_eq!(interview.stage, Stage::Start);
    // Making a tool: a question keeps the start; a proposal reaches the gate.
    let need = interview.start(Pick::Make).unwrap();
    let turn = interview
        .apply(
            &need,
            Some(Proposal::Tool(ToolProposal {
                say: "What should a good changelog entry look like?".into(),
                asking: true,
                ..ToolProposal::default()
            })),
        )
        .unwrap();
    assert_eq!(turn.stage, Stage::Start);
    assert_eq!(turn.line, None);
    let turn = interview
        .apply(
            &need,
            Some(Proposal::Tool(ToolProposal {
                say: "Here's what we'd make.".into(),
                name: Some("Changelog helper".into()),
                summary: Some("Writes changelog entries the way you do.".into()),
                skill: Some("Write one line per change, past tense.".into()),
                uses: vec!["Project map".into(), "Nonexistent".into()],
                asking: false,
            })),
        )
        .unwrap();
    assert_eq!(turn.stage, Stage::Tool);
    let draft = interview.draft().unwrap();
    assert_eq!(draft.tool.uses.len(), 1);
    assert_eq!(interview.tool.as_ref().unwrap().operations, ["repo_map"]);
    parse_draft(&draft_value(&draft).unwrap()).unwrap();
}

#[test]
fn results_come_in_only_where_they_belong() {
    let runner = FakeRunner::helpful(vec!["repo_map".into()]);
    let mut interview = at_pilot(Surface::Terminal);
    let dir = tempfile::tempdir().unwrap();
    files::write(&dir.path().join("evals"), &interview.cases).unwrap();
    let tool = interview.tool.clone().unwrap();
    let full = runner
        .run(&RunRequest {
            tool: &tool,
            eval_dir: &dir.path().join("evals"),
            extension: None,
            runs: 3,
        })
        .unwrap();
    assert_eq!(
        interview.accept(Event::Tried(full.clone())),
        Err(Refused::Unexpected {
            stage: Stage::Pilot,
            runs: 3
        })
    );
    let tried = runner
        .run(&RunRequest {
            tool: &tool,
            eval_dir: &dir.path().join("evals"),
            extension: None,
            runs: 1,
        })
        .unwrap();
    assert_eq!(tried.total, 5);
    assert_eq!(tried.with, 5);
    assert_eq!(tried.without, Some(1));
    assert_eq!(interview.accept(Event::Tried(tried)).unwrap(), Need::Read);
    let turn = interview
        .apply(
            &Need::Read,
            Some(Proposal::Say(SayProposal {
                say: "All five passed with the tool.".into(),
            })),
        )
        .unwrap();
    assert_eq!(turn.stage, Stage::Pilot);
    interview.accept(Event::Approve).unwrap();
    interview.apply(&Need::Nothing, None).unwrap();
    interview.accept(Event::Approve).unwrap();
    interview.apply(&Need::Nothing, None).unwrap();
    assert_eq!(interview.stage, Stage::Done);
    assert_eq!(interview.accept(Event::Tried(full)).unwrap(), Need::Nothing);
    let turn = interview.apply(&Need::Nothing, None).unwrap();
    assert!(
        turn.say
            .starts_with("With the tool, Coder passed 5 of 5 tests; without it, 1 of 5."),
        "{}",
        turn.say
    );
    assert!(
        matches!(turn.offer, Some(Planned::Publish(ref report)) if report.schema.as_deref() == Some("openagents.eval-report.v1"))
    );
    let offer = interview.wire_offer(turn.offer.as_ref().unwrap()).unwrap();
    nostr::cj_conversation::offer_feedback(&offer, 2).unwrap();
}

#[test]
fn a_draft_writes_out_and_reads_back_byte_for_byte_and_the_engine_loads_it() {
    let interview = at_pilot(Surface::Chat);
    let draft = interview.draft().unwrap();
    let value = draft_value(&draft).unwrap();
    parse_draft(&value).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let evals = dir.path().join("evals");
    files::write(&evals, &draft.cases).unwrap();
    let read = files::read(&evals).unwrap();
    assert_eq!(read, draft.cases);
    for (path, bytes) in files::case_files(&draft.cases) {
        assert_eq!(std::fs::read(evals.join(path)).unwrap(), bytes);
    }
    let suite = Suite::load(&evals, LoadOptions::default()).unwrap();
    assert_eq!(suite.cases.len(), draft.cases.len());
    assert!(suite.cases.iter().all(|c| c.runs == 3));
    // A second write never overwrites.
    assert!(files::write(&evals, &draft.cases).is_err());
    // The draft card the chat returns is one the phone's parser accepts.
    nostr::cj_conversation::card_feedback(&nostr::cj_conversation::Card::Draft(draft), 2).unwrap();
}

#[test]
fn resume_recovers_the_step_from_our_line_and_never_skips_a_gate() {
    let interview = at_pilot(Surface::Chat);
    let draft = interview.draft().unwrap();
    let catalog = Catalog::starter();
    let line = |stage: Stage| format!("Words.\n\n{}", stage.line(Surface::Chat).unwrap());
    for stage in [
        Stage::Tests,
        Stage::Checks,
        Stage::Pilot,
        Stage::Size,
        Stage::Done,
    ] {
        let resumed = Interview::resume(
            Surface::Chat,
            catalog.clone(),
            Some(&draft),
            Some(&line(stage)),
        );
        assert_eq!(resumed.stage, stage);
        assert_eq!(resumed.cases, draft.cases);
    }
    let resumed = Interview::resume(
        Surface::Chat,
        catalog.clone(),
        Some(&draft),
        Some("Something the router said."),
    );
    assert_eq!(
        resumed.stage,
        Stage::Tests,
        "an unknown line asks the tests gate again"
    );
    let empty = Draft {
        cases: Vec::new(),
        ..draft.clone()
    };
    let resumed = Interview::resume(
        Surface::Chat,
        catalog.clone(),
        Some(&empty),
        Some(&line(Stage::Size)),
    );
    assert_eq!(
        resumed.stage,
        Stage::Tool,
        "a line the draft can't be at falls back"
    );
    let resumed = Interview::resume(Surface::Chat, catalog, None, Some(&line(Stage::Size)));
    assert_eq!(resumed.stage, Stage::Start);
}

// ---------------------------------------------------------------------------
// The floor under any sequence of proposals.
// ---------------------------------------------------------------------------

/// A small deterministic generator, so the property test needs no crate.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }
    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const TASKS: &[&str] = &[
    "Create a small Python package and tell us where its tests live.",
    "Use repo_map to list the files.",
    "Open the project map and summarize it.",
    "What is 2 + 2?",
    "TODO: write this task",
    "",
    "Write CHANGELOG.md with one entry for the new --dry-run flag.",
    "Explain what +++ means in a TOML frontmatter.",
    "Find every caller of parse_case in this code: fn a() { parse_case() }",
    "Summarize \"the README\" in 'two' lines\\n.",
];

fn random_grader(rng: &mut Rng) -> Value {
    match rng.below(9) {
        0 => {
            json!({"type": "decision", "name": rng.pick(&["right", "Right!", "", "a"]), "question": rng.pick(&["Is it right?", "", "Did it name \"x\"?"]), "focus": rng.pick(&["last_message", "files", "trajectory", "nonsense"])})
        }
        1 => {
            json!({"type": "decision", "question": "Is the file right?", "focus": {"file": rng.pick(&["CHANGELOG.md", "../x", "/abs", "a//b"])}})
        }
        2 => {
            json!({"type": "regex", "pattern": rng.pick(&["dry-run", "(", "", "[a-z]+"]), "match": rng.pick(&["contains", "not_contains", "count:2", "maybe"])})
        }
        3 => {
            json!({"type": "file_exists", "path": rng.pick(&["*.md", "../up", "src/*.rs"]), "exists": rng.below(2) == 0})
        }
        4 => {
            json!({"type": "operation_used", "operation": rng.pick(&["repo_map", "shell", "rm", "code_search"]), "min": rng.below(3), "max": rng.below(3)})
        }
        5 => json!({"type": "operation_used", "operation": "repo_map", "max": 0, "min": 0}),
        6 => json!({"type": "judge", "criteria": "Good?"}),
        7 => json!({"type": "decision", "question": "Did it use the tool?", "focus": "trajectory"}),
        _ => json!({"type": "telepathy"}),
    }
}

fn random_tests(rng: &mut Rng) -> Vec<TestProposal> {
    (0..rng.below(20))
        .map(|_| TestProposal {
            id: rng
                .pick(&["a", "A a", "", "stays-out-of-the-way", "x/y", "a"])
                .into(),
            kind: rng
                .pick(&["should-fire", "should-not-fire", "maybe", ""])
                .into(),
            task: rng.pick(TASKS).into(),
            good: Some(rng.pick(&["good", "", "+++"]).into()),
        })
        .collect()
}

fn random_checks(rng: &mut Rng, interview: &Interview) -> Vec<CaseChecks> {
    let mut checks = Vec::new();
    for case in &interview.cases {
        if rng.below(3) == 0 {
            continue;
        }
        let graders = (0..rng.below(5)).map(|_| random_grader(rng)).collect();
        checks.push(CaseChecks {
            test: case.id.clone(),
            graders,
        });
    }
    checks
}

/// Whatever the model proposes, in whatever order, the tests hold the
/// floor after every applied proposal, and no step is passed without an
/// approval event.
#[test]
fn the_floor_holds_after_any_sequence_of_proposals() {
    let catalog = Catalog::starter();
    for seed in 1..=400_u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let surface = if seed % 2 == 0 {
            Surface::Chat
        } else {
            Surface::Terminal
        };
        let mut interview = Interview::new(surface, catalog.clone());
        let pick = if seed % 3 == 0 {
            Pick::Make
        } else {
            Pick::Existing(catalog.tools[rng.below(3)].clone())
        };
        let mut need = interview.start(pick).unwrap();
        let mut approvals = 0;
        for _ in 0..30 {
            let proposal = match rng.below(6) {
                0 => Proposal::Tool(ToolProposal {
                    say: "A tool.".into(),
                    name: Some(rng.pick(&["Helper", "", "Project map"]).into()),
                    skill: Some(rng.pick(&["Do it well.", ""]).into()),
                    uses: vec![rng.pick(&["Project map", "Code finder", "x"]).into()],
                    asking: rng.below(4) == 0,
                    ..ToolProposal::default()
                }),
                1 => Proposal::Tests(TestsProposal {
                    say: "Tests.".into(),
                    tests: random_tests(&mut rng),
                }),
                2 => Proposal::Checks(ChecksProposal {
                    say: "Checks.".into(),
                    checks: random_checks(&mut rng, &interview),
                }),
                3 => Proposal::Fix(FixProposal {
                    say: "Fixed.".into(),
                    tests: (rng.below(2) == 0).then(|| random_tests(&mut rng)),
                    checks: (rng.below(2) == 0).then(|| random_checks(&mut rng, &interview)),
                }),
                _ => Proposal::Say(SayProposal {
                    say: "Words.".into(),
                }),
            };
            let applied = interview.apply(&need, Some(proposal));
            let passed = Stage::ALL
                .into_iter()
                .filter(|gate| gate.is_gate() && *gate < interview.stage)
                .count();
            assert!(
                passed <= approvals,
                "seed {seed}: {:?} is past {passed} gates with {approvals} approvals",
                interview.stage
            );
            if let Ok(turn) = &applied {
                if !interview.cases.is_empty() {
                    let tool = interview.tool.as_ref().unwrap();
                    let violations = floor::check(
                        tool,
                        &interview.catalog,
                        &interview.cases,
                        interview.max_cases,
                    );
                    assert!(violations.is_empty(), "seed {seed}: {violations:?}");
                    assert!(interview.has_quiet_test());
                    if let Some(draft) = interview.draft() {
                        parse_draft(&draft_value(&draft).unwrap()).unwrap();
                    }
                }
                if let Some(Planned::Try(size) | Planned::Full(size)) = &turn.offer {
                    assert_eq!(size.arms, 2, "every run compares with and without");
                }
            }
            // The next event: approvals only sometimes.
            let event = match rng.below(4) {
                0 => Event::Approve,
                1 => Event::Change("change it".into()),
                _ => Event::Answer("an answer".into()),
            };
            let at = interview.stage;
            if let Ok(next) = interview.accept(event.clone()) {
                if event == Event::Approve {
                    approvals += 1;
                } else {
                    assert_eq!(interview.stage, at, "seed {seed}: {event:?} moved the step");
                }
                need = next;
            }
        }
    }
}

#[test]
fn the_floor_catches_each_violation() {
    let catalog = Catalog::starter();
    let tool = catalog.tools[0].clone();
    let mut interview = Interview::new(Surface::Chat, catalog.clone());
    interview.start(Pick::Existing(tool.clone())).unwrap();
    interview.tool = Some(tool.clone());
    interview.stage = Stage::Quality;
    let need = interview.accept(Event::Answer("x".into())).unwrap();
    interview
        .apply(&need, Some(Proposal::Tests(tests(3, 1))))
        .unwrap();
    let mut cases = interview.cases.clone();
    cases.retain(|c| c.kind == CaseKind::ShouldFire);
    // Hand-broken files, as a client could send them.
    cases[0].prompt =
        "+++\nv = \"openagents.eval-case.v1\"\nruns = 1\n+++\n\nUse the Project map.\n".into();
    cases[1].prompt = "+++\nv = \"openagents.eval-case.v1\"\nruns = 1\n+++\n\nList files.\n".into();
    cases[2].graders = vec![super::render::unused_grader("repo_map")];
    let found = floor::check(&tool, &catalog, &cases, 2);
    assert!(found.contains(&Violation::NamesTool {
        id: cases[0].id.clone(),
        name: "Project map".into()
    }));
    assert!(found.contains(&Violation::FewRuns {
        id: cases[1].id.clone(),
        runs: 1
    }));
    assert!(found.contains(&Violation::NoOutcome {
        id: cases[2].id.clone()
    }));
    assert!(found.contains(&Violation::NoShouldNotFire));
    assert!(found.contains(&Violation::TooMany { count: 3, max: 2 }));
    let (fixed, notes) = floor::enforce(&tool, &catalog, cases, 2);
    assert!(floor::check(&tool, &catalog, &fixed, 2).is_empty());
    assert!(notes.iter().any(|n| n.contains("names Project map")));
    assert_eq!(fixed.len(), 2);
}
