//! Scripted chat interviews: a fake Jev, a fake model, and a fake runner
//! standing in for the taps the phone sends.

use std::sync::Arc;

use ext_eval::author::runner::fake::FakeRunner;
use ext_eval::author::runner::{RunRequest, Runner};
use ext_eval::author::{Catalog, Stage, Surface, files};
use nostr::cj_conversation::{
    Card, Offer, SubjectSource, SuiteSource, Where, card_feedback, offer_feedback, parse_draft,
};
use nostr::eval_ext::CaseKind;
use serde_json::Value;

use super::fake::{ScriptJudge, StepModel};
use super::*;
use crate::generate::{Message, Role};

/// A fake Jev: the tool question picks the tool the message names, and the
/// reply question reads the app's **Looks good** as an approval, "Sort of"
/// as a weak one, and anything else as a change.
fn judge() -> Arc<ScriptJudge> {
    Arc::new(ScriptJudge::answering(|id, state| match id {
        "tool" => {
            let message = state["message"].as_str().unwrap_or_default();
            if message.contains("Project map") {
                ("tool_0".into(), 0.93)
            } else if message.contains("changelog") || message.contains("Slack") {
                ("make".into(), 0.9)
            } else {
                ("unclear".into(), 0.7)
            }
        }
        "build" => {
            let message = state["message"].as_str().unwrap_or_default();
            if message.contains("Slack") {
                ("code".into(), 0.9)
            } else {
                ("guidance".into(), 0.9)
            }
        }
        "reply" => match state["they_replied"].as_str().unwrap_or_default() {
            "Looks good" => ("approve".into(), 0.95),
            "Sort of" => ("approve".into(), 0.6),
            "How long does it take?" => ("other".into(), 0.8),
            _ => ("change".into(), 0.9),
        },
        _ => ("unclear".into(), 0.5),
    }))
}

/// The phone: it keeps the transcript and the draft, sends each turn, and
/// checks everything that comes back against the NIP-CJ parsers.
struct Phone {
    transcript: Vec<Message>,
    draft: Option<Value>,
    stages: Vec<Stage>,
    last: Option<Step>,
}

impl Phone {
    fn new() -> Self {
        Self {
            transcript: Vec::new(),
            draft: None,
            stages: Vec::new(),
            last: None,
        }
    }

    async fn send(
        &mut self,
        author: &Author<StepModel>,
        message: &str,
        tried: Option<Tried>,
    ) -> Step {
        let ask = Ask {
            message: message.into(),
            transcript: self.transcript.clone(),
            draft: self.draft.clone(),
            tried,
        };
        let step = author.step(&ask).await.expect("the step");
        assert!(!step.reply.trim().is_empty());
        assert!(!singular(&step.reply), "{}", step.reply);
        assert!(step.reply.chars().count() <= 1_200, "{}", step.reply);
        assert!(step.offers.len() <= 1);
        if let Some(draft) = &step.draft {
            parse_draft(draft).expect("every draft parses");
        }
        for card in &step.cards {
            card_feedback(card, 2).expect("every card parses");
        }
        for offer in &step.offers {
            offer_feedback(offer, 2).expect("every offer parses");
            if let Offer::StartEval { size, .. } = offer {
                assert_eq!(size.arms, 2, "every run compares with and without");
            }
        }
        // The router shows every step only after its own check, and keeps
        // every run and publish offer.
        let routed = crate::router::gym::check_step(&crate::router::seams::AuthorStep {
            text: step.reply.clone(),
            draft: step.draft.clone(),
            offer: step.offers.first().cloned().and_then(router_offer),
            model: step.model.clone(),
        })
        .expect("the router shows the step");
        assert_eq!(routed.draft, step.draft);
        if matches!(
            step.offers.first(),
            Some(Offer::StartEval { .. } | Offer::PublishEval { .. })
        ) {
            assert!(routed.offer.is_some(), "the router keeps the offer");
        }
        if let Some(line) = step.stage.line(Surface::Chat) {
            assert!(step.reply.ends_with(line), "{}", step.reply);
        }
        self.transcript.push(Message {
            role: Role::User,
            text: message.into(),
        });
        self.transcript.push(Message {
            role: Role::Assistant,
            text: step.reply.clone(),
        });
        if step.draft.is_some() {
            self.draft.clone_from(&step.draft);
        }
        self.stages.push(step.stage);
        self.last = Some(step.clone());
        step
    }

    /// A tap on a `start_eval` offer: the hosted runner (here the fake)
    /// writes the draft out as case files and runs it.
    fn tap_run(&self, runner: &FakeRunner, catalog: &Catalog) -> Tried {
        let step = self.last.as_ref().expect("a step");
        let Some(Offer::StartEval { size, suite, .. }) = step.offers.first() else {
            panic!("no run offered: {:?}", step.offers)
        };
        assert_eq!(*suite, SuiteSource::Draft);
        let draft = parse_draft(self.draft.as_ref().unwrap()).unwrap();
        let tool = catalog.tool_of(&draft.tool);
        let dir = tempfile::tempdir().unwrap();
        let evals = dir.path().join("evals");
        files::write(&evals, &draft.cases).unwrap();
        runner
            .run(&RunRequest {
                tool: &tool,
                eval_dir: &evals,
                extension: None,
                runs: u32::try_from(size.runs).unwrap(),
            })
            .unwrap()
    }
}

fn author(model: StepModel, judge: Arc<ScriptJudge>) -> Author<StepModel> {
    Author::new(model, "fake-model", Some(judge), Catalog::starter())
}

/// The whole interview in chat: a draft, a try and its reading, the size,
/// the full run, and the offer to add it to the Gym, with every gate passed
/// only on a tap and no step skipped.
#[tokio::test]
async fn a_chat_walks_the_whole_interview_one_gate_at_a_time() {
    let judge = judge();
    let author = author(StepModel::default(), judge.clone());
    let catalog = Catalog::starter();
    let runner = FakeRunner::helpful(vec!["repo_map".into()]);
    let mut phone = Phone::new();

    // 0 and 1: which tool, and what it is.
    let step = phone
        .send(&author, "Help me write tests for Project map", None)
        .await;
    assert_eq!(step.stage, Stage::Tool);
    assert!(
        step.reply
            .starts_with("Project map gives Coder a head start.")
    );
    let draft = parse_draft(step.draft.as_ref().unwrap()).unwrap();
    assert!(draft.cases.is_empty());
    assert_eq!(draft.tool.name, "Project map");
    assert!(matches!(step.cards.as_slice(), [Card::Draft(_)]));
    assert!(
        step.offers.is_empty(),
        "nothing runs before the tests exist"
    );

    // A weak yes is not a yes: the gate is asked again.
    let step = phone.send(&author, "Sort of", None).await;
    assert_eq!(step.stage, Stage::Tool);

    // 2: the quality question, after a tap.
    let step = phone.send(&author, "Looks good", None).await;
    assert_eq!(step.stage, Stage::Quality);
    assert_eq!(step.model, "none");

    // 3: the tests. The one that names the tool is left out, and the
    // reply says so.
    let step = phone
        .send(
            &author,
            "A good run names the right file or folder; a failed one guesses.",
            None,
        )
        .await;
    assert_eq!(step.stage, Stage::Tests);
    assert!(step.reply.contains("names Project map"), "{}", step.reply);
    let draft = parse_draft(step.draft.as_ref().unwrap()).unwrap();
    assert_eq!(draft.cases.len(), 5);
    assert!(
        draft
            .cases
            .iter()
            .any(|c| c.kind == CaseKind::ShouldNotFire)
    );
    assert!(draft.cases.iter().all(|c| c.id != "names-the-tool"));

    // A change at the tests gate re-asks it.
    let step = phone
        .send(&author, "Can you add a test about a Cargo workspace?", None)
        .await;
    assert_eq!(step.stage, Stage::Tests);

    // 4: the checks.
    let step = phone.send(&author, "Looks good", None).await;
    assert_eq!(step.stage, Stage::Checks);
    let draft = parse_draft(step.draft.as_ref().unwrap()).unwrap();
    for case in &draft.cases {
        let names: Vec<&str> = case.graders.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"right-answer"), "{names:?}");
        match case.kind {
            CaseKind::ShouldFire => assert!(names.contains(&"reached"), "{names:?}"),
            CaseKind::ShouldNotFire => assert!(names.contains(&"stayed-out"), "{names:?}"),
        }
    }
    assert!(step.offers.is_empty());

    // 5: the try is offered only after the checks are approved.
    let step = phone.send(&author, "Looks good", None).await;
    assert_eq!(step.stage, Stage::Pilot);
    let Some(Offer::StartEval {
        size,
        subject,
        at,
        label,
        ..
    }) = step.offers.first()
    else {
        panic!("Try it once is offered")
    };
    assert_eq!((size.cases, size.runs, size.arms), (5, 1, 2));
    assert!(matches!(subject, SubjectSource::Definition(d) if d.id.ends_with("/repo-map")));
    assert_eq!(*at, Where::Hosted);
    assert_eq!(label, "Try it once");
    assert!(runner.requests().is_empty(), "nothing ran before the tap");

    // The tap: one run per arm, and the result comes back to read.
    let tried = phone.tap_run(&runner, &catalog);
    assert_eq!(runner.requests().len(), 1);
    assert_eq!(runner.requests()[0].1, 1);
    let step = phone.send(&author, "How did it go?", Some(tried)).await;
    assert_eq!(step.stage, Stage::Pilot);
    assert!(
        step.reply.contains("passed 5 of 5 tests; without it, 1"),
        "{}",
        step.reply
    );

    // 6: the size.
    let step = phone.send(&author, "Looks good", None).await;
    assert_eq!(step.stage, Stage::Size);
    assert!(
        step.reply.contains("5 tests, 3 runs each"),
        "{}",
        step.reply
    );
    assert!(step.reply.contains("30 runs in all"), "{}", step.reply);

    // 7: done, and the full run is offered.
    let step = phone.send(&author, "Looks good", None).await;
    assert_eq!(step.stage, Stage::Done);
    let Some(Offer::StartEval { size, .. }) = step.offers.first() else {
        panic!("the full run is offered")
    };
    assert_eq!((size.runs, size.arms), (3, 2));

    // A question at the end is answered, and the full run offered again.
    let step = phone.send(&author, "How long does it take?", None).await;
    assert_eq!(step.stage, Stage::Done);
    assert!(matches!(step.offers.first(), Some(Offer::StartEval { .. })));

    // The full run's tap, and its result: the offer to add it to the Gym.
    let full = phone.tap_run(&runner, &catalog);
    assert_eq!(full.runs, 3);
    let step = phone.send(&author, "Is it done?", Some(full)).await;
    assert_eq!(step.stage, Stage::Done);
    assert!(step.reply.contains("That's Better."), "{}", step.reply);
    let Some(Offer::PublishEval { report, label }) = step.offers.first() else {
        panic!("Add to the Gym is offered: {:?}", step.offers)
    };
    assert_eq!(label, "Add to the Gym");
    assert_eq!(report.schema.as_deref(), Some("openagents.eval-report.v1"));

    // No step was skipped.
    let mut seen: Vec<Stage> = phone.stages.clone();
    seen.dedup();
    assert_eq!(
        seen,
        [
            Stage::Tool,
            Stage::Quality,
            Stage::Tests,
            Stage::Checks,
            Stage::Pilot,
            Stage::Size,
            Stage::Done
        ]
    );
    // Every gate was passed on a reply Jev read as an approval.
    let approvals = judge
        .asked()
        .iter()
        .filter(|(id, state)| id == "reply" && state["they_replied"] == "Looks good")
        .count();
    assert_eq!(approvals, 5);
}

#[tokio::test]
async fn making_a_tool_is_a_skill_and_code_goes_to_coder() {
    let judge = judge();
    let model = StepModel::default();
    let author = author(model, judge);
    let mut phone = Phone::new();
    let step = phone
        .send(
            &author,
            "Help me make a tool that writes changelog entries",
            None,
        )
        .await;
    assert_eq!(step.stage, Stage::Tool);
    let draft = parse_draft(step.draft.as_ref().unwrap()).unwrap();
    assert_eq!(draft.tool.name, "Changelog helper");
    assert!(draft.tool.catalog.is_none());
    assert!(draft.tool.skill.as_deref().unwrap().contains("past tense"));
    assert_eq!(draft.tool.uses.len(), 1, "it turns on Project map");
    phone.send(&author, "Looks good", None).await;
    let step = phone
        .send(&author, "Good entries are one line.", None)
        .await;
    assert_eq!(step.stage, Stage::Tests);
    let draft = parse_draft(step.draft.as_ref().unwrap()).unwrap();
    assert!(
        draft.cases.iter().all(|c| c.id != "names-the-tool"),
        "a task naming the tool is left out"
    );
    let step = phone.send(&author, "Looks good", None).await;
    let step2 = phone.send(&author, "Looks good", None).await;
    assert_eq!(step.stage, Stage::Checks);
    let Some(Offer::StartEval { subject, .. }) = step2.offers.first() else {
        panic!("Try it once")
    };
    assert_eq!(
        *subject,
        SubjectSource::Draft,
        "a made tool runs from the draft"
    );

    let mut phone = Phone::new();
    let step = phone
        .send(
            &author,
            "Make a tool that posts my test results to Slack",
            None,
        )
        .await;
    assert_eq!(step.stage, Stage::Start);
    assert!(step.draft.is_none());
    assert!(matches!(step.offers.as_slice(), [Offer::RunCoder { .. }]));

    let mut phone = Phone::new();
    let step = phone.send(&author, "I want to test something", None).await;
    assert_eq!(step.stage, Stage::Start);
    assert!(step.reply.contains("Which tool"));
    assert!(step.offers.is_empty());
}

#[tokio::test]
async fn a_garbled_answer_is_retried_once() {
    let judge = judge();
    let model = StepModel::garbling(vec!["tests"]);
    let author = author(model, judge);
    let mut phone = Phone::new();
    phone
        .send(&author, "Help me write tests for Project map", None)
        .await;
    phone.send(&author, "Looks good", None).await;
    let step = phone.send(&author, "Good runs name the file.", None).await;
    assert_eq!(step.stage, Stage::Tests);
    assert_eq!(
        author.model.calls(),
        ["tool", "tests", "tests"],
        "the tests step was asked twice"
    );
}

#[tokio::test]
async fn a_draft_from_the_phone_is_data_and_the_floor_still_holds() {
    // A draft that arrives broken (no test where the tool stays out of the
    // way, a prompt naming the tool) at the tests gate: a change request
    // returns tests that hold the floor again.
    let judge = judge();
    let author = author(StepModel::default(), judge);
    let mut phone = Phone::new();
    phone
        .send(&author, "Help me write tests for Project map", None)
        .await;
    phone.send(&author, "Looks good", None).await;
    phone.send(&author, "Good runs name the file.", None).await;
    let mut draft = phone.draft.clone().unwrap();
    draft["cases"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["kind"] == "should-fire");
    draft["cases"][0]["prompt"] =
        "+++\nv = \"openagents.eval-case.v1\"\nruns = 1\n+++\n\nAsk the Project map.\n".into();
    phone.draft = Some(draft);
    let step = phone.send(&author, "Make them harder", None).await;
    assert_eq!(step.stage, Stage::Tests);
    let draft = parse_draft(step.draft.as_ref().unwrap()).unwrap();
    assert!(
        draft
            .cases
            .iter()
            .any(|c| c.kind == CaseKind::ShouldNotFire)
    );
    let tool = Catalog::starter().tool_of(&draft.tool);
    assert!(ext_eval::author::floor::check(&tool, &Catalog::starter(), &draft.cases, 8).is_empty());
}

/// The router's seam runs the same step.
#[tokio::test]
async fn the_router_seam_runs_a_step() {
    use crate::router::seams::{AuthorAsk, EvalAuthor};
    let author = author(StepModel::default(), judge());
    assert!(author.available());
    let step = EvalAuthor::step(
        &author,
        &AuthorAsk {
            message: "Help me write tests for Project map".into(),
            transcript: Vec::new(),
            draft: None,
            surface: crate::router::Surface::Phone,
        },
    )
    .await
    .unwrap();
    assert!(
        step.text
            .ends_with(Stage::Tool.line(Surface::Chat).unwrap())
    );
    assert!(step.draft.is_some());
    let none = Author::new(StepModel::default(), "m", None, Catalog::starter());
    assert!(!none.available());
}
