//! The authoring interview against the live model door and live Jev, for
//! the three starter tools. Ignored by default: it spends quota and needs
//! keys.
//!
//! ```sh
//! CODER_ENV_FILE=/path/to/door.env TYPESAFE_ENV_FILE=/path/to/typesafe.env \
//! EVAL_AUTHOR_LIVE_OUT=/tmp/interviews \
//!   cargo test -p coder --test eval_author_live -- --ignored --nocapture
//! ```
//!
//! The door comes from `CODER_DOOR_URL`, `CODER_DOOR_KEY`, and
//! `CODER_MODEL` (or lines in `CODER_ENV_FILE`); the Jev key from
//! `TYPESAFE_API_KEY` (or `TYPESAFE_ENV_FILE`). Neither is printed. Each
//! interview is a scripted person talking to the chat driver; the taps on
//! **Try it once** and the full run go to the engine's fake runner, since
//! the hosted runner isn't part of this test. Each finished test set is
//! written under `EVAL_AUTHOR_LIVE_OUT/<tool>/evals` and loaded by the
//! engine, and each transcript is written beside it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use coder::eval_author::{Ask, Author, Step};
use coder::generate::{Message, ResponsesDoor, Role};
use coder::product_kb::Judge;
use ext_eval::author::runner::fake::FakeRunner;
use ext_eval::author::runner::{RunRequest, Runner};
use ext_eval::author::{Catalog, Stage, Tried, files};
use ext_eval::{LoadOptions, Suite};
use nostr::cj_conversation::{Offer, card_feedback, offer_feedback, parse_draft};
use serde_json::Value;

fn from_file(variable: &str, file_variable: &str) -> Option<String> {
    if let Some(value) = std::env::var(variable)
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        return Some(value);
    }
    let path = std::env::var(file_variable).ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .filter_map(|line| line.trim().strip_prefix(&format!("{variable}=")))
        .map(|value| value.trim().trim_matches('"').to_string())
        .next()
}

fn door() -> ResponsesDoor {
    let key = from_file("CODER_DOOR_KEY", "CODER_ENV_FILE")
        .or_else(|| from_file("CODER_AI_GATEWAY_KEY", "CODER_ENV_FILE"))
        .expect("set CODER_DOOR_KEY or CODER_ENV_FILE");
    let url = from_file("CODER_DOOR_URL", "CODER_ENV_FILE")
        .unwrap_or_else(|| coder::generate::DEFAULT_DOOR_URL.into());
    let model = from_file("CODER_MODEL", "CODER_ENV_FILE")
        .unwrap_or_else(|| coder::generate::DEFAULT_MODEL.into());
    coder::eval_author::door(ResponsesDoor::new(url, model, key))
}

fn judge() -> Arc<dyn Judge> {
    let key = from_file("TYPESAFE_API_KEY", "TYPESAFE_ENV_FILE")
        .expect("set TYPESAFE_API_KEY or TYPESAFE_ENV_FILE");
    Arc::new(jev::Client::new(jev::Config::new().api_key(key)).expect("the Jev client builds"))
}

/// One scripted person: what they say at each step. `None` for a step
/// means a tap on **Looks good**.
struct Script {
    tool: &'static str,
    opening: &'static str,
    quality: &'static str,
    /// A change request at the tests gate, once.
    tests_change: Option<&'static str>,
    /// A change request at the checks gate, once.
    checks_change: Option<&'static str>,
}

const SCRIPTS: [Script; 3] = [
    Script {
        tool: "project-map",
        opening: "Help me write tests for Project map",
        quality: "A good run finds the right file or folder quickly and names it; a failed run guesses, or opens every file one by one.",
        tests_change: None,
        checks_change: None,
    },
    Script {
        tool: "code-finder",
        opening: "Can we make a test set for the Code finder tool?",
        quality: "A good run points at the exact lines where something is defined or used, with the file and line; a failed run lists unrelated files or misses a use.",
        tests_change: Some("Add one test about finding every caller of a function."),
        checks_change: None,
    },
    Script {
        tool: "test-reader",
        opening: "I want to test Test reader",
        quality: "A good run reads the failing test's name, file, and message from the report and explains why it failed; a failed run reruns everything or guesses.",
        tests_change: None,
        checks_change: Some("Also check that the answer names the failing test."),
    },
];

struct Chat {
    transcript: Vec<Message>,
    draft: Option<Value>,
    log: String,
    last: Option<Step>,
}

impl Chat {
    async fn send(
        &mut self,
        author: &Author<ResponsesDoor>,
        message: &str,
        tried: Option<Tried>,
    ) -> Step {
        let ask = Ask {
            message: message.into(),
            transcript: self.transcript.clone(),
            draft: self.draft.clone(),
            tried,
        };
        let started = std::time::Instant::now();
        let step = author.step(&ask).await.expect("the live step");
        let took = started.elapsed();
        if let Some(draft) = &step.draft {
            parse_draft(draft).expect("the draft parses");
        }
        for card in &step.cards {
            card_feedback(card, 2).expect("the card parses");
        }
        for offer in &step.offers {
            offer_feedback(offer, 2).expect("the offer parses");
        }
        let offers: Vec<String> = step
            .offers
            .iter()
            .map(|offer| match offer {
                Offer::StartEval {
                    label, size, at, ..
                } => format!(
                    "[{label}: {} tests x {} runs x {} sides, {}]",
                    size.cases,
                    size.runs,
                    size.arms,
                    at.word()
                ),
                Offer::PublishEval { label, .. } => format!("[{label}]"),
                Offer::RunCoder { label } => format!("[{label}]"),
                other => format!("[{}]", other.word()),
            })
            .collect();
        let tests = step
            .draft
            .as_ref()
            .and_then(|d| parse_draft(d).ok())
            .map(|d| {
                d.cases
                    .iter()
                    .map(|c| {
                        format!(
                            "    - {} ({}; checks: {})",
                            c.id,
                            c.kind.word(),
                            c.graders
                                .iter()
                                .map(|(n, _)| n.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        self.log.push_str(&format!(
            "\n**Person:** {message}\n\n**OpenAgents** (step {}, {}, {:.1} s):\n\n{}\n",
            step.stage.number(),
            step.model,
            took.as_secs_f64(),
            step.reply
                .lines()
                .map(|l| format!("> {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
        if !tests.is_empty() {
            self.log.push_str(&format!("\n  Draft card:\n{tests}\n"));
        }
        if !offers.is_empty() {
            self.log
                .push_str(&format!("\n  Offer: {}\n", offers.join(" ")));
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
        self.last = Some(step.clone());
        step
    }

    fn tap(&mut self, runner: &FakeRunner, catalog: &Catalog) -> Tried {
        let step = self.last.as_ref().expect("a step");
        let Some(Offer::StartEval { size, .. }) = step.offers.first() else {
            panic!("no run offered")
        };
        let draft = parse_draft(self.draft.as_ref().unwrap()).unwrap();
        let tool = catalog.tool_of(&draft.tool);
        let dir = tempfile::tempdir().unwrap();
        files::write(&dir.path().join("evals"), &draft.cases).unwrap();
        let tried = runner
            .run(&RunRequest {
                tool: &tool,
                eval_dir: &dir.path().join("evals"),
                extension: None,
                runs: u32::try_from(size.runs).unwrap(),
            })
            .unwrap();
        self.log.push_str(&format!(
            "\n  (Tap: {} runs per side on the fake runner. {})\n",
            size.runs,
            tried.headline()
        ));
        tried
    }
}

async fn interview(
    author: &Author<ResponsesDoor>,
    script: &Script,
    out: &Path,
) -> (String, PathBuf) {
    let catalog = Catalog::starter();
    let runner = FakeRunner::helpful(vec![match script.tool {
        "project-map" => "repo_map".into(),
        "code-finder" => "code_search".into(),
        _ => "test_report".into(),
    }]);
    let mut chat = Chat {
        transcript: Vec::new(),
        draft: None,
        log: String::new(),
        last: None,
    };
    let mut step = chat.send(author, script.opening, None).await;
    assert_eq!(step.stage, Stage::Tool, "{}", chat.log);
    let mut tests_change = script.tests_change;
    let mut checks_change = script.checks_change;
    let mut tried_once = false;
    for _ in 0..16 {
        step = match step.stage {
            Stage::Quality => chat.send(author, script.quality, None).await,
            Stage::Tests if tests_change.is_some() => {
                chat.send(author, tests_change.take().unwrap(), None).await
            }
            Stage::Checks if checks_change.is_some() => {
                chat.send(author, checks_change.take().unwrap(), None).await
            }
            Stage::Pilot if !tried_once => {
                tried_once = true;
                let tried = chat.tap(&runner, &catalog);
                chat.send(author, "How did it go?", Some(tried)).await
            }
            Stage::Done => break,
            _ => chat.send(author, "Looks good", None).await,
        };
    }
    assert_eq!(step.stage, Stage::Done, "{}", chat.log);
    let full = chat.tap(&runner, &catalog);
    let step = chat.send(author, "Is it done?", Some(full)).await;
    assert!(
        matches!(step.offers.first(), Some(Offer::PublishEval { .. })),
        "{}",
        chat.log
    );
    let draft = parse_draft(chat.draft.as_ref().unwrap()).unwrap();
    let dir = out.join(script.tool);
    let _ = std::fs::remove_dir_all(&dir);
    let evals = dir.join("evals");
    files::write(&evals, &draft.cases).unwrap();
    let suite = Suite::load(&evals, LoadOptions::default()).expect("the written suite loads");
    assert!(suite.cases.iter().all(|c| c.runs == 3));
    assert!(
        suite
            .cases
            .iter()
            .any(|c| c.kind == ext_eval::Kind::ShouldNotFire)
    );
    std::fs::write(dir.join("transcript.md"), &chat.log).unwrap();
    (chat.log, evals)
}

#[tokio::test]
#[ignore = "calls the live model door and Jev, and spends quota"]
async fn three_live_interviews_write_valid_test_sets() {
    let door = door();
    let model = door.model.clone();
    let author = Author::new(door, model, Some(judge()), Catalog::starter());
    let out = std::env::var("EVAL_AUTHOR_LIVE_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("eval-author-live"));
    for script in &SCRIPTS {
        let (log, evals) = interview(&author, script, &out).await;
        println!("\n## {}\n{log}\nWrote {}\n", script.tool, evals.display());
    }
}
