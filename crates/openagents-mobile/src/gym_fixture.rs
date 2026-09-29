//! An offline Gym for simulator screenshots of the states a live chat can't
//! reach yet: a chat worker that answers with the chat router's recorded
//! NIP-CJ card and offer bodies (`crates/coder/fixtures/nip-cj`), and a
//! hosted runner that reports progress, then a recorded report.
//!
//! Honored only in debug builds (`Launch::gym_fixture`). It reaches no
//! network, holds no key, and chooses each answer by the turn's position in
//! the conversation, never by reading the message. Screenshots taken with
//! it are named `fixture-…`.

use crate::basic_coder::{Door, Reply, Role, Turn, lock};
use crate::gym::{Hosted, HostedRun, Live, Outcome};
use crate::router::Context;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The report the fixture runner returns: eight tests, five passed without
/// the tool and seven with it (`the_fixture_report_parses` writes it).
pub(crate) const REPORT: &str = include_str!("../fixtures/gym-report.json");

fn wire(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

/// A five-test draft for a chat-made tool.
fn draft() -> Value {
    let case = |id: &str, kind: &str, task: &str, check: &str| {
        json!({"id": id, "kind": kind,
            "prompt": format!("+++\nv = \"openagents.eval-case.v1\"\n+++\n\n{task}\n"),
            "graders": [{"name": "criteria",
                "text": format!("+++\ntype = \"decision\"\n+++\n\n{check}\n")}]})
    };
    json!({"v": "openagents.eval-draft.v1",
    "tool": {"name": "Changelog helper", "summary": "Tells Coder how we write changelog entries.",
        "catalog": null, "skill": "Write one line per change, newest first.", "uses": []},
    "cases": [
        case("summarize-a-fix", "should-fire", "Summarize the merged fix in CHANGELOG.md.", "One line names the fix."),
        case("entry-for-a-flag", "should-fire", "Write the entry for the new --quiet flag.", "The entry names the flag."),
        case("group-small-changes", "should-fire", "Group three small changes into one entry.", "One entry lists all three."),
        case("note-a-breaking-change", "should-fire", "Note the breaking change to the config file.", "The entry says it breaks."),
        case("leave-a-question-alone", "should-not-fire", "What does this repository do?", "No changelog file changed."),
    ]})
}

/// The reply to the conversation's `n`th user message, from zero.
fn script(n: usize) -> (&'static str, Vec<Value>) {
    let judgment = |route: &str| {
        json!({"v": 2, "requires": [], "type": "judgment", "verdict": "respond", "line": "",
            "set": "chat-router-v2", "route": route, "tier": "gym"})
    };
    let result = |route: &str| json!({"v": 2, "type": "result", "tier": "gym", "route": route});
    let card = |card: Value| json!({"v": 2, "requires": [], "type": "card", "card": "draft", "draft": card});
    match n % 4 {
        0 => (
            "We'd try Project map. It shows Coder how the project is laid out before it starts.",
            vec![
                judgment("eval.run"),
                wire(include_str!(
                    "../../coder/fixtures/nip-cj/router-card-tool.json"
                )),
                wire(include_str!(
                    "../../coder/fixtures/nip-cj/router-offer-start-eval.json"
                )),
                result("eval.run"),
            ],
        ),
        1 => (
            "Here's a result waiting for a check.",
            vec![
                judgment("eval.check"),
                wire(include_str!(
                    "../../coder/fixtures/nip-cj/router-card-check.json"
                )),
                wire(include_str!(
                    "../../coder/fixtures/nip-cj/router-offer-start-eval.json"
                )),
                result("eval.check"),
            ],
        ),
        2 => (
            "Here are the tests we'd use: four where the tool should help, and one where it should stay out of the way.\n\nAre these the right tests? Tap Looks good, or tell us what to change.",
            vec![
                judgment("eval.author"),
                card(draft()),
                result("eval.author"),
            ],
        ),
        _ => (
            "Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.",
            vec![
                judgment("eval.author"),
                card(draft()),
                json!({"v": 2, "requires": [], "type": "offer", "offer": "start_eval",
                    "suite": "draft", "subject": "draft",
                    "size": {"cases": 5, "runs": 1, "arms": 2}, "where": "hosted",
                    "label": "Try it once"}),
                result("eval.author"),
            ],
        ),
    }
}

pub(crate) struct GymFixture;

impl Door for GymFixture {
    fn ask(
        &self,
        turns: Vec<Turn>,
        _context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let asked = turns.iter().filter(|turn| turn.role == Role::User).count();
        let (text, fields) = script(asked.saturating_sub(1));
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let mut reply = lock(&reply);
            for field in &fields {
                match field["type"].as_str() {
                    Some("judgment") => reply.meta.judged(field),
                    Some("offer") => reply.meta.offered(field),
                    Some("card") => reply.meta.carded(field),
                    Some("result") => reply.meta.resulted(field),
                    _ => {}
                }
            }
            reply.model = Some("fixture".into());
            reply.text = text.into();
            reply.done = true;
        })
    }
}

/// A runner that reports progress over about ten seconds, then the report.
pub(crate) struct FixtureRunner;

impl Hosted for FixtureRunner {
    fn start(
        &self,
        _world: SecretKey,
        run: HostedRun,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            lock(&live).request = Some("f1".repeat(32));
            lock(&live).event = Some(json!({"id": "f1".repeat(32)}));
            lock(&live).queued = true;
            crate::wake::ring();
            let planned = 8 * run.runs.max(1) * 2;
            for done in (0..=planned).step_by(4) {
                tokio::time::sleep(Duration::from_millis(600)).await;
                {
                    let mut live = lock(&live);
                    live.planned = Some(planned);
                    live.done = Some(done);
                }
                crate::wake::ring();
            }
            lock(&live).outcome =
                Some(Outcome::from_report(REPORT.as_bytes()).map_err(|why| (why, true)));
        })
    }

    fn resume(
        &self,
        world: SecretKey,
        _event: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.start(
            world,
            HostedRun {
                offer: Value::Null,
                draft: None,
                runs: 1,
                check: None,
            },
            live,
        )
    }

    fn stop(&self, _world: SecretKey, _event: Value) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }

    fn publish(
        &self,
        _world: SecretKey,
        _request: String,
        _report: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(900)).await;
            lock(&live).published = Some(Ok(Some("fa".repeat(32))));
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every scripted turn carries cards and offers NIP-CJ's parsers read.
    #[test]
    fn every_scripted_turn_reads_as_gym_cards() {
        for n in 0..4 {
            let (text, fields) = script(n);
            assert!(!text.is_empty());
            let mut meta = crate::router::Meta::default();
            for field in &fields {
                match field["type"].as_str() {
                    Some("card") => meta.carded(field),
                    Some("offer") => meta.offered(field),
                    _ => {}
                }
            }
            assert!(!meta.cards.is_empty(), "{n}");
        }
    }
}
