//! System One hook 2 (#11487, `docs/coder/design/system-one-hooks.md`):
//! no done without an observation.
//!
//! A `Stop` hook. When the agent ends its turn without `finish` having
//! passed, the host looks before letting it go:
//!
//! 1. **Rule.** `finish` passed: stop. The agent already stopped once on
//!    this exact candidate after being sent back: stop (it is stuck, and
//!    sending it back again changes nothing).
//! 2. **Decision.** One Decisions API choice (`openagents/decide`, Clef
//!    first) reads the agent's last message: is it claiming the work is
//!    done, saying it is blocked, or asking the person something? A blocker
//!    or a question at [`ALLOW_AT`] or above stops without running checks.
//!    A failed call counts as a done claim, so the checks run.
//! 3. **Observation.** The final checks run (`finish`'s own plan). A pass
//!    stops. Anything else sends the agent back with the verdict as the
//!    reason: evidence, not advice.
//!
//! This replaces prompt wording such as "verify everything before you
//! stop", which every run pays for in turns whether or not it needed to.
//! Every decision is one `hook-log` line (#11492) in the run's
//! `stop_gate.jsonl`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use serde_json::{Value, json};

/// A blocker or question stops unchecked at this probability or above.
///
/// Written costs: letting an unverified "done" stop costs a failed run
/// (about 10 check runs' worth of work redone); running the checks on a
/// real blocker costs one check run. Stop unchecked only when
/// p > 1 - 1/10.
pub const ALLOW_AT: f64 = 0.9;
/// How long the classifier may take.
const DECISION_TIMEOUT: Duration = Duration::from_secs(5);

/// What the agent's last message says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    Done,
    Blocked,
    Question,
    Other,
}

impl Claim {
    fn word(self) -> &'static str {
        match self {
            Claim::Done => "done",
            Claim::Blocked => "blocked",
            Claim::Question => "question",
            Claim::Other => "other",
        }
    }

    fn from_word(word: &str) -> Self {
        match word {
            "done" => Claim::Done,
            "blocked" => Claim::Blocked,
            "question" => Claim::Question,
            _ => Claim::Other,
        }
    }
}

/// A classification: the claim, its probability, model, door, latency.
pub type Classified = Result<(Claim, f64, String, Option<String>, u64), String>;
/// Reads the last message.
pub type Classify = Arc<dyn Fn(String) -> BoxFuture<'static, Classified> + Send + Sync>;
/// Runs the final checks.
pub type Check = Arc<dyn Fn() -> BoxFuture<'static, Value> + Send + Sync>;
/// Names the candidate (a git tree), when git can.
pub type Tree = Arc<dyn Fn() -> BoxFuture<'static, Option<String>> + Send + Sync>;

/// What the gate decided on one stop.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Stop.
    Allow(&'static str),
    /// Go back, with this reason.
    Block(String),
}

pub struct StopGate {
    pub finished: Arc<AtomicBool>,
    /// The agent's last text, kept by the host as the stream arrives.
    pub last_text: Arc<Mutex<String>>,
    pub classify: Classify,
    pub check: Check,
    pub tree: Tree,
    pub log: Option<PathBuf>,
    /// The candidate the agent was last sent back on.
    blocked_on: Mutex<Option<String>>,
    /// Counts for the run summary.
    pub tally: Mutex<Value>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl StopGate {
    #[must_use]
    pub fn new(
        finished: Arc<AtomicBool>,
        last_text: Arc<Mutex<String>>,
        classify: Classify,
        check: Check,
        tree: Tree,
        log: Option<PathBuf>,
    ) -> Self {
        Self {
            finished,
            last_text,
            classify,
            check,
            tree,
            log,
            blocked_on: Mutex::new(None),
            tally: Mutex::new(json!({"stops": 0, "blocked": 0, "allowed": {}})),
        }
    }

    /// Decides one stop, and logs it through `hook-log` (#11492).
    pub async fn decide(&self) -> Outcome {
        let mut line = json!({});
        let outcome = self.decide_logged(&mut line).await;
        let (action, why) = match &outcome {
            Outcome::Allow(why) => ("allow", Some(*why)),
            Outcome::Block(_) => ("block", None),
        };
        let how = if line.get("error").is_some() {
            hook_log::How::Fallback
        } else if line.get("p").is_some() {
            hook_log::How::Decision
        } else {
            hook_log::How::Rule
        };
        let mut decision = hook_log::Decision::new("stop_gate", "claim", action, how);
        decision.p = line["p"].as_f64();
        decision.threshold = decision.p.map(|_| ALLOW_AT);
        decision.model = line["model"].as_str().map(str::to_owned);
        decision.door = line["door"].as_str().map(str::to_owned);
        decision.latency_ms = line["latency_ms"].as_u64();
        decision.detail = json!({
            "claim": line["claim"], "verdict": line["verdict"], "why": why, "error": line["error"],
        });
        self.count(&outcome);
        if let Some(path) = &self.log {
            hook_log::append(path, &decision);
        }
        outcome
    }

    async fn decide_logged(&self, line: &mut Value) -> Outcome {
        if self.finished.load(Ordering::SeqCst) {
            return Outcome::Allow("finish passed");
        }
        let tree = (self.tree)().await;
        if tree.is_some() && *lock(&self.blocked_on) == tree {
            return Outcome::Allow("stopped again on the same candidate");
        }
        let text = lock(&self.last_text).clone();
        match (self.classify)(text).await {
            Ok((claim, p, model, door, latency_ms)) => {
                line["claim"] = json!(claim.word());
                line["p"] = json!(p);
                line["model"] = json!(model);
                line["door"] = json!(door);
                line["latency_ms"] = json!(latency_ms);
                if matches!(claim, Claim::Blocked | Claim::Question) && p >= ALLOW_AT {
                    return Outcome::Allow(if claim == Claim::Blocked {
                        "the agent says it is blocked"
                    } else {
                        "the agent asks a question"
                    });
                }
            }
            Err(error) => line["error"] = json!(error),
        }
        let verdict = (self.check)().await;
        line["verdict"] = verdict["status"].clone();
        if verdict["status"].as_str() == Some("pass") {
            return Outcome::Allow("the final checks pass");
        }
        *lock(&self.blocked_on) = tree;
        Outcome::Block(format!(
            "You stopped, but the change does not pass the final checks yet. Their verdict: {}. \
             Fix what it names, or say plainly what blocks you.",
            serde_json::to_string(&verdict).unwrap_or_default()
        ))
    }

    fn count(&self, outcome: &Outcome) {
        let mut tally = lock(&self.tally);
        tally["stops"] = json!(tally["stops"].as_u64().unwrap_or(0) + 1);
        match outcome {
            Outcome::Block(_) => {
                tally["blocked"] = json!(tally["blocked"].as_u64().unwrap_or(0) + 1)
            }
            Outcome::Allow(why) => {
                let seen = tally["allowed"][*why].as_u64().unwrap_or(0);
                tally["allowed"][*why] = json!(seen + 1);
            }
        }
    }

    /// The `Stop` hook over this gate.
    #[must_use]
    pub fn hook(self: Arc<Self>) -> claude_agent_sdk::HookMatcher {
        let hook = claude_agent_sdk::hook_fn(move |_input: Value, _tool_use_id| {
            let gate = self.clone();
            async move {
                Ok(match gate.decide().await {
                    Outcome::Allow(_) => {
                        claude_agent_sdk::SyncHookJSONOutput::continue_execution().into()
                    }
                    Outcome::Block(reason) => claude_agent_sdk::SyncHookJSONOutput {
                        decision: Some("block".to_owned()),
                        reason: Some(reason),
                        ..claude_agent_sdk::SyncHookJSONOutput::default()
                    }
                    .into(),
                })
            }
        });
        claude_agent_sdk::HookMatcher::new(None, hook)
    }
}

/// The Decisions API classifier.
#[must_use]
pub fn decisions() -> Classify {
    Arc::new(|text: String| {
        Box::pin(async move {
            let url = std::env::var("OPENAGENTS_DECISIONS_URL")
                .ok()
                .filter(|url| !url.trim().is_empty())
                .map_or_else(
                    || jev::decisions::OPENAGENTS.to_owned(),
                    |url| url.trim_end_matches('/').to_owned(),
                );
            let client = jev::decisions::DecisionsClient::new(url)
                .map_err(|error| error.to_string())?
                .timeout(DECISION_TIMEOUT);
            let shown: String = text
                .chars()
                .rev()
                .take(4_000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let request = jev::decisions::DecisionsRequest::new(
                "openagents/decide",
                format!("A coding agent working on an issue ended its turn with this message:\n\n{shown}"),
            )
            .choice(
                "claim",
                "What does the agent's final message say about the task?",
                [
                    ("done", "It says the change is made or the task is complete."),
                    ("blocked", "It says it cannot finish: missing access, an unclear requirement, a broken environment, or the task is out of scope."),
                    ("question", "It asks the person something before it goes on."),
                    ("other", "Anything else, including a progress note with no claim."),
                ],
            );
            let started = Instant::now();
            let response = client
                .create(&request)
                .await
                .map_err(|error| error.to_string())?;
            let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let Some(jev::decisions::DecisionAnswer::Choice {
                choice,
                probabilities,
                ..
            }) = response.answer("claim")
            else {
                return Err("no answer to the claim question".to_owned());
            };
            let p = probabilities
                .iter()
                .find(|option| option.value == *choice)
                .map_or(0.0, |option| option.probability);
            Ok((
                Claim::from_word(choice),
                p,
                response.model.clone(),
                response.openagents.door.clone(),
                latency,
            ))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Fixture {
        gate: StopGate,
        checks: Arc<AtomicUsize>,
        tree: Arc<Mutex<Option<String>>>,
    }

    fn fixture(classified: Classified, verdict: &'static str) -> Fixture {
        let checks = Arc::new(AtomicUsize::new(0));
        let tree = Arc::new(Mutex::new(Some("t1".to_owned())));
        let counter = checks.clone();
        let check: Check = Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move { json!({"status": verdict}) })
        });
        let current = tree.clone();
        let tree_fn: Tree = Arc::new(move || {
            let tree = lock(&current).clone();
            Box::pin(async move { tree })
        });
        let classify: Classify = Arc::new(move |_| {
            let classified = classified.clone();
            Box::pin(async move { classified })
        });
        Fixture {
            gate: StopGate::new(
                Arc::new(AtomicBool::new(false)),
                Arc::new(Mutex::new("Done: fixed the parser.".to_owned())),
                classify,
                check,
                tree_fn,
                None,
            ),
            checks,
            tree,
        }
    }

    fn said(claim: Claim, p: f64) -> Classified {
        Ok((claim, p, "clef-flash".into(), Some("clef".into()), 40))
    }

    #[tokio::test]
    async fn a_passed_finish_stops_without_looking() {
        let f = fixture(said(Claim::Done, 0.99), "test_failure");
        f.gate.finished.store(true, Ordering::SeqCst);
        assert_eq!(f.gate.decide().await, Outcome::Allow("finish passed"));
        assert_eq!(f.checks.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_done_claim_with_failing_checks_is_sent_back_once_per_candidate() {
        let f = fixture(said(Claim::Done, 0.97), "test_failure");
        let Outcome::Block(reason) = f.gate.decide().await else {
            panic!("should block");
        };
        assert!(reason.contains("test_failure"), "{reason}");
        // Stopping again on the same candidate: stuck, let it go.
        assert_eq!(
            f.gate.decide().await,
            Outcome::Allow("stopped again on the same candidate")
        );
        // A changed candidate is checked again.
        *lock(&f.tree) = Some("t2".to_owned());
        assert!(matches!(f.gate.decide().await, Outcome::Block(_)));
        assert_eq!(f.checks.load(Ordering::SeqCst), 2);
        assert_eq!(f.gate.tally.lock().unwrap()["blocked"], 2);
    }

    #[tokio::test]
    async fn passing_checks_stop() {
        let f = fixture(said(Claim::Done, 0.97), "pass");
        assert_eq!(
            f.gate.decide().await,
            Outcome::Allow("the final checks pass")
        );
    }

    #[tokio::test]
    async fn a_confident_blocker_stops_unchecked_and_an_unsure_one_is_checked() {
        let f = fixture(said(Claim::Blocked, 0.95), "test_failure");
        assert_eq!(
            f.gate.decide().await,
            Outcome::Allow("the agent says it is blocked")
        );
        assert_eq!(f.checks.load(Ordering::SeqCst), 0);
        let f = fixture(said(Claim::Question, 0.6), "test_failure");
        assert!(matches!(f.gate.decide().await, Outcome::Block(_)));
        assert_eq!(f.checks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_decision_runs_the_checks() {
        let f = fixture(Err("unreachable".into()), "pass");
        assert_eq!(
            f.gate.decide().await,
            Outcome::Allow("the final checks pass")
        );
        assert_eq!(f.checks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_hook_blocks_with_a_reason() {
        let f = fixture(said(Claim::Done, 0.97), "compile_error");
        let hook = Arc::new(f.gate).hook();
        let out = hook.hooks[0]
            .call(json!({"hook_event_name": "Stop"}), None)
            .await
            .unwrap();
        let value = serde_json::to_value(&out).unwrap();
        assert_eq!(value["decision"], "block");
        assert!(value["reason"].as_str().unwrap().contains("compile_error"));
    }
}
