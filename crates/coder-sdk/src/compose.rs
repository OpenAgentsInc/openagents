//! How several hooks on one event combine, by fixed rules:
//!
//! 1. Floors are never overridden: deny beats ask beats allow beats no
//!    opinion, and a floor's answer wins a tie. No hook allows what a floor
//!    denies.
//! 2. Rewrites chain in registration order, and the floors judge the final
//!    input.
//! 3. Context has a budget per event and per session; the lowest-priority
//!    context drops first and each drop is recorded.
//! 4. A failed or late hook is no opinion, except a floor, which falls back
//!    to its rule's answer.
//!
//! [`check_start`] refuses, before a session starts, a hook that must deny
//! on a backend that can only see its event after the fact.

use std::panic::AssertUnwindSafe;
use std::sync::mpsc;

use coder_delegate::session::{HookBackend, HookTier, Session, hook_tier};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::events::{Call, Event};
use super::outcome::{Context, HookOutcome, Permission, Stop};
use super::registry::{Hook, Registry};

/// How much context hooks may add, in characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    pub per_event: usize,
    pub per_session: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            per_event: 4_000,
            per_session: 32_000,
        }
    }
}

/// Context that didn't fit, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dropped {
    /// The hook that added it.
    pub hook: String,
    /// Where the context said it came from.
    pub source: String,
    pub chars: usize,
    pub reason: String,
}

/// How a hook failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailureKind {
    /// It didn't answer within its timeout; a later answer is discarded.
    Late,
    Error { message: String },
    Panicked,
}

/// A hook that failed, and what its answer counted as.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub hook: String,
    pub kind: FailureKind,
    /// What the failure counted as: no opinion, or a floor's rule answer.
    pub counted_as: Permission,
}

/// What the hooks on one call came to, as applied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub event: Event,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub permission: Permission,
    /// The hook whose answer the permission is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<String>,
    /// The input after every rewrite: what the floors judged.
    pub input: Value,
    /// The hooks that rewrote the input, in order.
    pub rewritten_by: Vec<String>,
    /// The context kept, highest priority first.
    pub context: Vec<Context>,
    pub dropped: Vec<Dropped>,
    pub stop: Stop,
    pub failures: Vec<Failure>,
}

/// One answer, from one hook.
struct Answer {
    hook: String,
    outcome: HookOutcome,
}

/// Runs a session's hooks under one context budget.
#[derive(Clone, Debug)]
pub struct Runner {
    pub registry: Registry,
    pub budget: Budget,
    /// Context characters the session has taken so far.
    used: usize,
}

impl Runner {
    #[must_use]
    pub fn new(registry: Registry, budget: Budget) -> Self {
        Runner {
            registry,
            budget,
            used: 0,
        }
    }

    /// Context characters the session has taken so far.
    #[must_use]
    pub fn used(&self) -> usize {
        self.used
    }

    /// Runs every hook that fires on `call` and combines their answers.
    pub fn fire(&mut self, call: &Call) -> Applied {
        let tool = call.tool.as_deref();
        let hooks: Vec<&Hook> = self.registry.matching(call.event, tool).collect();
        let mut failures = Vec::new();
        let mut rewritten_by = Vec::new();
        let mut current = call.clone();
        let mut answers = Vec::new();
        // The rewrite chain, in registration order: each hook sees the input
        // the earlier ones left.
        for hook in hooks.iter().filter(|hook| !hook.is_floor()) {
            let mut outcome = run(hook, &current, &mut failures);
            if let Some(input) = outcome.rewrite.take()
                && call.event.rewrites()
                && hook.may_rewrite()
            {
                current.input = input;
                rewritten_by.push(hook.id.clone());
            }
            if outcome.permission == Permission::Allow && !hook.source.may_allow() {
                outcome.permission = Permission::NoOpinion;
            }
            answers.push(Answer {
                hook: hook.id.clone(),
                outcome,
            });
        }
        // The floors judge the final input, and come first in a tie.
        let mut floors = Vec::new();
        for hook in hooks.iter().filter(|hook| hook.is_floor()) {
            let mut outcome = run(hook, &current, &mut failures);
            outcome.rewrite = None;
            floors.push(Answer {
                hook: hook.id.clone(),
                outcome,
            });
        }
        floors.extend(answers);
        let answers = floors;

        let mut permission = Permission::NoOpinion;
        let mut decided_by = None;
        let mut stop = Stop::Continue;
        for answer in &answers {
            if answer.outcome.permission.rank() > permission.rank() {
                permission = answer.outcome.permission.clone();
                decided_by = Some(answer.hook.clone());
            }
            if answer.outcome.stop.rank() > stop.rank() {
                stop = answer.outcome.stop.clone();
            }
        }

        let offered: Vec<(String, Context)> = answers
            .into_iter()
            .filter_map(|answer| answer.outcome.context.map(|c| (answer.hook, c)))
            .collect();
        let (context, dropped) = self.budgeted(offered);
        Applied {
            event: call.event,
            tool: call.tool.clone(),
            permission,
            decided_by,
            input: current.input,
            rewritten_by,
            context,
            dropped,
            stop,
            failures,
        }
    }

    /// Keeps the highest-priority context that fits the event's and the
    /// session's budgets, and records the rest as dropped.
    fn budgeted(&mut self, mut offered: Vec<(String, Context)>) -> (Vec<Context>, Vec<Dropped>) {
        // Stable: equal priorities keep their order.
        offered.sort_by_key(|(_, context)| std::cmp::Reverse(context.priority));
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        let mut event_used = 0usize;
        for (hook, context) in offered {
            let chars = context.text.chars().count();
            let reason = if event_used + chars > self.budget.per_event {
                Some(format!(
                    "Left out: more than the {} characters one event may add.",
                    self.budget.per_event
                ))
            } else if self.used + chars > self.budget.per_session {
                Some(format!(
                    "Left out: more than the {} characters a session may add.",
                    self.budget.per_session
                ))
            } else {
                None
            };
            match reason {
                Some(reason) => dropped.push(Dropped {
                    hook,
                    source: context.source,
                    chars,
                    reason,
                }),
                None => {
                    event_used += chars;
                    self.used += chars;
                    kept.push(context);
                }
            }
        }
        (kept, dropped)
    }
}

/// Runs one hook within its timeout. A failure is recorded and counts as no
/// opinion, or as a floor's rule answer.
fn run(hook: &Hook, call: &Call, failures: &mut Vec<Failure>) -> HookOutcome {
    let (sender, receiver) = mpsc::channel();
    let body = hook.run.clone();
    let owned = call.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("hook {}", hook.id))
        .spawn(move || {
            let answer = std::panic::catch_unwind(AssertUnwindSafe(|| body(&owned)));
            // The receiver is gone when the hook was late: its answer is
            // discarded.
            let _ = sender.send(answer);
        });
    let kind = match spawned {
        Err(error) => FailureKind::Error {
            message: error.to_string(),
        },
        Ok(_) => match receiver.recv_timeout(hook.timeout) {
            Ok(Ok(Ok(outcome))) => return outcome,
            Ok(Ok(Err(message))) => FailureKind::Error { message },
            Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => FailureKind::Panicked,
            Err(mpsc::RecvTimeoutError::Timeout) => FailureKind::Late,
        },
    };
    let counted_as = hook.floor.clone().unwrap_or_default();
    failures.push(Failure {
        hook: hook.id.clone(),
        kind,
        counted_as: counted_as.clone(),
    });
    HookOutcome::permit(counted_as)
}

/// A hook a backend can't honor, found before the session starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refused {
    pub hook: String,
    pub event: Event,
    pub tier: HookTier,
    /// Why, in plain words.
    pub message: String,
}

/// Refuses every hook that must deny on an event the backend can't stop
/// before it happens (`tier` gives the backend's tier per event).
///
/// # Errors
///
/// Returns each refused hook.
pub fn check_start(
    registry: &Registry,
    harness: &str,
    tier: impl Fn(Event) -> HookTier,
) -> Result<(), Vec<Refused>> {
    let refused: Vec<Refused> = registry
        .hooks()
        .filter(|hook| hook.must_deny)
        .filter_map(|hook| {
            let tier = tier(hook.event);
            (!tier.can_deny()).then(|| Refused {
                hook: hook.id.clone(),
                event: hook.event,
                tier,
                message: format!(
                    "{harness} can't stop a step before it runs, so the {} check can't be kept there.",
                    hook.id
                ),
            })
        })
        .collect();
    if refused.is_empty() {
        Ok(())
    } else {
        Err(refused)
    }
}

/// [`check_start`] against a backend's tier table.
///
/// # Errors
///
/// Returns each refused hook.
pub fn check_backend(registry: &Registry, backend: HookBackend) -> Result<(), Vec<Refused>> {
    check_start(registry, backend.label(), |event| {
        hook_tier(backend, event.name())
    })
}

/// [`check_start`] against a session's own tiers.
///
/// # Errors
///
/// Returns each refused hook.
pub fn check_session<S: Session>(registry: &Registry, session: &S) -> Result<(), Vec<Refused>> {
    check_start(registry, session.adapter(), |event| {
        session.hook_tier(event.name())
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::super::registry::Source;
    use super::*;

    fn answering(
        outcome: HookOutcome,
    ) -> impl Fn(&Call) -> Result<HookOutcome, String> + Send + Sync + 'static {
        move |_| Ok(outcome.clone())
    }

    fn runner(hooks: Vec<Hook>) -> Runner {
        let mut registry = Registry::new();
        for hook in hooks {
            registry.register(hook).unwrap();
        }
        Runner::new(registry, Budget::default())
    }

    fn bash(command: &str) -> Call {
        Call::tool(Event::PreToolUse, "Bash", json!({"command": command}))
    }

    fn plain(id: &str, outcome: HookOutcome) -> Hook {
        Hook::new(id, Event::PreToolUse, Source::Command, answering(outcome))
    }

    fn winner(a: HookOutcome, b: HookOutcome) -> Permission {
        let both = runner(vec![plain("a", a.clone()), plain("b", b.clone())])
            .fire(&bash("ls"))
            .permission;
        let swapped = runner(vec![plain("b", b), plain("a", a)])
            .fire(&bash("ls"))
            .permission;
        assert_eq!(both, swapped, "order must not matter");
        both
    }

    #[test]
    fn deny_beats_ask() {
        assert_eq!(
            winner(HookOutcome::deny("no"), HookOutcome::ask("confirm")),
            Permission::deny("no")
        );
    }

    #[test]
    fn deny_beats_allow() {
        assert_eq!(
            winner(HookOutcome::deny("no"), HookOutcome::allow()),
            Permission::deny("no")
        );
    }

    #[test]
    fn deny_beats_no_opinion() {
        assert_eq!(
            winner(HookOutcome::deny("no"), HookOutcome::none()),
            Permission::deny("no")
        );
    }

    #[test]
    fn ask_beats_allow() {
        assert_eq!(
            winner(HookOutcome::ask("confirm"), HookOutcome::allow()),
            Permission::ask("confirm")
        );
    }

    #[test]
    fn ask_beats_no_opinion() {
        assert_eq!(
            winner(HookOutcome::ask("confirm"), HookOutcome::none()),
            Permission::ask("confirm")
        );
    }

    #[test]
    fn allow_beats_no_opinion() {
        assert_eq!(
            winner(HookOutcome::allow(), HookOutcome::none()),
            Permission::Allow
        );
        assert_eq!(
            winner(HookOutcome::none(), HookOutcome::none()),
            Permission::NoOpinion
        );
    }

    fn floor(outcome: HookOutcome) -> Hook {
        Hook::floor(
            "floor",
            Event::PreToolUse,
            Permission::ask("confirm"),
            answering(outcome),
        )
    }

    #[test]
    fn no_hook_allows_what_a_floor_denies() {
        let applied = runner(vec![
            plain("allow", HookOutcome::allow()),
            floor(HookOutcome::deny("deletes the home folder")),
            plain("allow-too", HookOutcome::allow()),
        ])
        .fire(&bash("rm -rf ~"));
        assert_eq!(applied.permission, Permission::deny("deletes the home folder"));
        assert_eq!(applied.decided_by.as_deref(), Some("floor"));
        let asked = runner(vec![plain("allow", HookOutcome::allow()), floor(HookOutcome::ask("push"))])
            .fire(&bash("git push"));
        assert_eq!(asked.permission, Permission::ask("push"));
        // A floor wins a tie with another deny, so its reason is the one shown.
        let tie = runner(vec![plain("other", HookOutcome::deny("other")), floor(HookOutcome::deny("floor"))])
            .fire(&bash("rm -rf ~"));
        assert_eq!(tie.decided_by.as_deref(), Some("floor"));
    }

    #[test]
    fn a_plugin_only_tightens() {
        let plugin = Hook::new("p", Event::PreToolUse, Source::Plugin, answering(HookOutcome::allow()));
        let applied = runner(vec![plugin]).fire(&bash("ls"));
        assert_eq!(applied.permission, Permission::NoOpinion);
    }

    fn appending(id: &str, suffix: &'static str) -> Hook {
        Hook::new(id, Event::PreToolUse, Source::BuiltIn, move |call: &Call| {
            let command = call.input["command"].as_str().unwrap_or_default();
            Ok(HookOutcome::rewrite(json!({"command": format!("{command}{suffix}")})))
        })
    }

    #[test]
    fn rewrites_chain_in_order_and_the_floors_judge_the_final_input() {
        let judge = Hook::floor(
            "floor",
            Event::PreToolUse,
            Permission::ask("confirm"),
            |call: &Call| {
                let command = call.input["command"].as_str().unwrap_or_default();
                Ok(if command == "make -j | tail" {
                    HookOutcome::deny("the final command")
                } else {
                    HookOutcome::none()
                })
            },
        );
        let applied = runner(vec![appending("one", " -j"), judge, appending("two", " | tail")])
            .fire(&bash("make"));
        assert_eq!(applied.input, json!({"command": "make -j | tail"}));
        assert_eq!(applied.rewritten_by, ["one", "two"]);
        assert_eq!(applied.permission, Permission::deny("the final command"));
        // Only PreToolUse rewrites.
        let mut post = runner(vec![Hook::new(
            "r",
            Event::PostToolUse,
            Source::BuiltIn,
            answering(HookOutcome::rewrite(json!("x"))),
        )]);
        let applied = post.fire(&Call::tool(Event::PostToolUse, "Bash", json!("out")));
        assert_eq!(applied.input, json!("out"));
        assert!(applied.rewritten_by.is_empty());
    }

    #[test]
    fn a_failed_or_late_hook_is_no_opinion() {
        let error = plain("error", HookOutcome::none());
        let error = Hook {
            run: std::sync::Arc::new(|_: &Call| Err("no answer".to_string())),
            ..error
        };
        let panics = Hook::new("panics", Event::PreToolUse, Source::Command, |_: &Call| {
            panic!("hook broke")
        });
        let late = Hook::new("late", Event::PreToolUse, Source::Command, |_: &Call| {
            std::thread::sleep(Duration::from_millis(300));
            Ok(HookOutcome::deny("too late"))
        })
        .timeout(Duration::from_millis(20));
        let applied = runner(vec![error, panics, late, plain("allow", HookOutcome::allow())])
            .fire(&bash("ls"));
        assert_eq!(applied.permission, Permission::Allow);
        let kinds: Vec<(&str, &FailureKind)> = applied
            .failures
            .iter()
            .map(|f| (f.hook.as_str(), &f.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                (
                    "error",
                    &FailureKind::Error {
                        message: "no answer".to_string()
                    }
                ),
                ("panics", &FailureKind::Panicked),
                ("late", &FailureKind::Late),
            ]
        );
        assert!(applied.failures.iter().all(|f| f.counted_as == Permission::NoOpinion));
    }

    #[test]
    fn a_failed_floor_falls_back_to_its_rule() {
        let broken = Hook::floor(
            "floor",
            Event::PreToolUse,
            Permission::ask("confirm the command"),
            |_: &Call| Err("rule file missing".to_string()),
        );
        let late = Hook::floor("slow", Event::PreToolUse, Permission::deny("unchecked"), |_: &Call| {
            std::thread::sleep(Duration::from_millis(300));
            Ok(HookOutcome::allow())
        })
        .timeout(Duration::from_millis(20));
        let applied = runner(vec![broken, plain("allow", HookOutcome::allow())]).fire(&bash("ls"));
        assert_eq!(applied.permission, Permission::ask("confirm the command"));
        assert_eq!(applied.failures[0].counted_as, Permission::ask("confirm the command"));
        let applied = runner(vec![late]).fire(&bash("ls"));
        assert_eq!(applied.permission, Permission::deny("unchecked"));
        assert_eq!(applied.failures[0].kind, FailureKind::Late);
    }

    fn adding(id: &str, chars: usize, priority: i32) -> Hook {
        Hook::new(
            id,
            Event::PostToolUse,
            Source::BuiltIn,
            answering(HookOutcome::context(Context::new(id, &"x".repeat(chars), priority))),
        )
    }

    #[test]
    fn the_event_budget_drops_the_lowest_priority_context_and_records_it() {
        let mut runner = runner(vec![adding("low", 60, 1), adding("high", 60, 9), adding("mid", 30, 5)]);
        runner.budget = Budget {
            per_event: 100,
            per_session: 1_000,
        };
        let applied = runner.fire(&Call::tool(Event::PostToolUse, "Bash", json!("out")));
        let kept: Vec<&str> = applied.context.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(kept, ["high", "mid"]);
        assert_eq!(applied.dropped.len(), 1);
        assert_eq!(applied.dropped[0].hook, "low");
        assert_eq!(applied.dropped[0].chars, 60);
        assert!(applied.dropped[0].reason.contains("one event"));
        assert_eq!(runner.used(), 90);
    }

    #[test]
    fn the_session_budget_drops_context_across_events() {
        let mut runner = runner(vec![adding("note", 40, 1)]);
        runner.budget = Budget {
            per_event: 100,
            per_session: 100,
        };
        let call = Call::tool(Event::PostToolUse, "Bash", json!("out"));
        assert!(runner.fire(&call).dropped.is_empty());
        assert!(runner.fire(&call).dropped.is_empty());
        let third = runner.fire(&call);
        assert!(third.context.is_empty());
        assert!(third.dropped[0].reason.contains("a session"));
        assert_eq!(runner.used(), 80);
    }

    #[test]
    fn stop_takes_the_strongest_answer() {
        let mut runner = runner(vec![
            Hook::new("b", Event::Stop, Source::Decision, answering(HookOutcome::stopping(Stop::Block {
                reason: "tests fail".to_string(),
            }))),
            Hook::new("c", Event::Stop, Source::Decision, answering(HookOutcome::none())),
        ]);
        let applied = runner.fire(&Call::new(Event::Stop, json!(null)));
        assert_eq!(
            applied.stop,
            Stop::Block {
                reason: "tests fail".to_string()
            }
        );
    }

    #[test]
    fn a_hook_that_must_deny_is_refused_where_it_can_only_be_emulated() {
        let mut registry = Registry::new();
        registry
            .register(floor(HookOutcome::none()).matching("Bash"))
            .unwrap();
        registry
            .register(Hook::new("ctx", Event::PostToolUse, Source::BuiltIn, answering(HookOutcome::none())))
            .unwrap();
        for backend in [HookBackend::ClaudeCode, HookBackend::CoderLoop, HookBackend::Codex, HookBackend::Acp] {
            assert!(check_backend(&registry, backend).is_ok(), "{backend:?}");
        }
        let refused = check_start(&registry, "the agent", |event| {
            if event == Event::PreToolUse {
                HookTier::Emulated
            } else {
                HookTier::Native
            }
        })
        .unwrap_err();
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].hook, "floor");
        assert_eq!(refused[0].tier, HookTier::Emulated);
        // A must-deny hook on a Codex event it only sees afterwards.
        registry
            .register(
                Hook::new("gate", Event::Stop, Source::Decision, answering(HookOutcome::none()))
                    .needs_deny(),
            )
            .unwrap();
        let refused = check_backend(&registry, HookBackend::Codex).unwrap_err();
        assert_eq!(refused[0].hook, "gate");
        assert!(refused[0].message.starts_with("Codex can't stop"));
        assert!(check_backend(&registry, HookBackend::ClaudeCode).is_ok());
    }
}
