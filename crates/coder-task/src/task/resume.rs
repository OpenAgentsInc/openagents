//! Resume points: where a task that a usage limit stopped picks up after
//! the reset (#10765).
//!
//! A provider's usage limit is shared by every agent on its login, so a
//! weekly limit stops every task at once. When the auto-start policy sees
//! a task it started end on a limit, or ends one itself because no admitted
//! provider has capacity, it records a resume point in `resume.json` in the
//! task store: the task and turn, the provider whose limit stopped it and
//! when that resets, the delegate's session, the task's worktree, and the
//! last checkpoint (the candidate snapshot the run left). Once a provider
//! the policy admits has capacity again, the policy continues the task with
//! a resume turn, under the same bounds as any other start. The engine
//! resumes the delegate's session from the earlier turn's trace, as every
//! follow-up does, and works in the same worktree.
//!
//! `coder task resume TASK` (the [`resume_now`] path) continues a task from
//! its resume point at once, for a person who does not run the policy.
//!
//! The file holds no prompt, credential, or model output.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::capacity::{self, Provider};
use super::{Action, COMMAND_SCHEMA, Command, Execution, Status, Store, Task};

/// The resume points in the task store directory.
pub const FILE: &str = "resume.json";
/// Their format.
pub const SCHEMA: &str = "openagents.coder.resume-points.v1";
/// Resolved points kept for the record, newest last.
const KEEP_RESOLVED: usize = 200;

/// Where a stopped task picks up.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub task: String,
    /// The task revision the stopped turn started at.
    pub turn: u64,
    /// When the limit stopped it, in Unix seconds.
    pub stopped_at: u64,
    /// The provider whose limit stopped it, when one is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    /// When the capacity book said the limit resets, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// The auto-start workspace label the task runs in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The enrolled device that created the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// The delegate's session the stopped turn ran in, which the resume
    /// turn's engine continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The task's worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// The last checkpoint: the candidate snapshot digest the stopped run
    /// left, when it left one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<String>,
    /// How the point was resolved, once it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumed: Option<Resumed>,
}

/// How a resume point was resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resumed {
    pub at: u64,
    /// The revision the resume turn started at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u64>,
    /// Why the task could not resume, such as a task that moved on or ran
    /// out of turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: String,
    points: Vec<Point>,
}

/// Every resume point in `dir`, oldest first. Missing or unreadable is none.
#[must_use]
pub fn load(dir: &Path) -> Vec<Point> {
    std::fs::read(dir.join(FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<File>(&bytes).ok())
        .filter(|file| file.schema == SCHEMA)
        .map(|file| file.points)
        .unwrap_or_default()
}

fn save(dir: &Path, mut points: Vec<Point>) -> Result<(), String> {
    let resolved = points.iter().filter(|p| p.resumed.is_some()).count();
    if resolved > KEEP_RESOLVED {
        let mut drop = resolved - KEEP_RESOLVED;
        points.retain(|point| {
            if point.resumed.is_some() && drop > 0 {
                drop -= 1;
                false
            } else {
                true
            }
        });
    }
    let bytes = serde_json::to_vec_pretty(&File {
        schema: SCHEMA.into(),
        points,
    })
    .map_err(|e| e.to_string())?;
    super::autostart::write_private(&dir.join(FILE), &bytes)
}

/// Record `point`, unless one for its task and turn is already there.
/// Returns whether it was new.
///
/// # Errors
/// Reports a failed write.
pub fn record(dir: &Path, point: Point) -> Result<bool, String> {
    let mut points = load(dir);
    if points
        .iter()
        .any(|kept| kept.task == point.task && kept.turn == point.turn)
    {
        return Ok(false);
    }
    points.push(point);
    save(dir, points)?;
    Ok(true)
}

/// Mark `task`'s point at `turn` resolved.
///
/// # Errors
/// Reports a failed write.
pub fn resolve(dir: &Path, task: &str, turn: u64, resumed: Resumed) -> Result<(), String> {
    let mut points = load(dir);
    for point in &mut points {
        if point.task == task && point.turn == turn && point.resumed.is_none() {
            point.resumed = Some(resumed.clone());
        }
    }
    save(dir, points)
}

/// The open (unresolved) point for `task`, if any.
#[must_use]
pub fn open(dir: &Path, task: &str) -> Option<Point> {
    load(dir)
        .into_iter()
        .rev()
        .find(|point| point.task == task && point.resumed.is_none())
}

/// Whether `point` is due at `now`: it is open, and one of `providers` (the
/// routes the policy admits; the point's own provider when empty) has
/// capacity in `book`.
#[must_use]
pub fn due(point: &Point, book: &capacity::Book, providers: &[Provider], now: u64) -> bool {
    if point.resumed.is_some() {
        return false;
    }
    let own: Vec<Provider> = point.provider.into_iter().collect();
    let providers = if providers.is_empty() {
        &own[..]
    } else {
        providers
    };
    if providers.is_empty() {
        return point.resets_at.is_none_or(|at| now >= at);
    }
    providers
        .iter()
        .any(|provider| book.has_capacity(*provider, now))
}

/// The providers a run's grant admitted, in order.
#[must_use]
pub fn run_providers(task: &Task) -> Vec<Provider> {
    let Some(configuration) = task
        .run
        .as_ref()
        .and_then(|run| run.admission.grant.adapter_configuration.as_ref())
    else {
        return Vec::new();
    };
    std::iter::once(configuration.provider.as_str())
        .chain(
            configuration
                .fallbacks
                .iter()
                .map(|route| route.provider.as_str()),
        )
        .filter_map(Provider::from_config)
        .collect()
}

/// Whether `task`'s current run ended because of a usage or rate limit,
/// and if so the provider and its reset: the run ended `no_capacity`, or
/// it failed and the book holds a refusal for one of its providers
/// observed since `started_at`, when the run started. A run that finished,
/// was stopped, or is still going was not stopped by a limit.
#[must_use]
pub fn stopped_by_limit(
    task: &Task,
    started_at: u64,
    book: &capacity::Book,
    now: u64,
) -> Option<(Option<Provider>, Option<u64>)> {
    let result = task.run.as_ref()?.result.as_ref()?;
    if result.stop_requested || task.status != Status::Finished {
        return None;
    }
    let providers = run_providers(task);
    if result.ending == capacity::NO_CAPACITY_ENDING {
        let provider = providers
            .iter()
            .copied()
            .find(|provider| !book.has_capacity(*provider, now));
        return Some((provider, book.earliest_reset(&providers, now)));
    }
    if task.execution != Execution::Failed {
        return None;
    }
    book.refusals
        .iter()
        .filter(|refusal| {
            providers.contains(&refusal.provider) && refusal.observed_at >= started_at
        })
        .min_by_key(|refusal| refusal.until)
        .map(|refusal| (Some(refusal.provider), Some(refusal.until)))
}

/// The delegate session `task`'s current run recorded in its trace: the
/// `session` of the last `*_session` step note (Claude Code, Codex, Devin,
/// OpenCode, and Grok Build sessions record one).
#[must_use]
pub fn session(store: &Path, task: &Task) -> Option<String> {
    let run = task.run.as_ref()?;
    let text = std::fs::read_to_string(store.join(&run.admission.trace_file)).ok()?;
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| line.pointer("/step/extensions").cloned())
        .filter_map(|extensions| match extensions {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .flat_map(|map| map.into_iter())
        .filter(|(key, _)| key.ends_with("_session"))
        .filter_map(|(_, note)| {
            note.get("session")
                .or_else(|| note.get("session_id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .next_back()
}

/// The resume point for `task`, stopped at `now`, read from the store.
#[must_use]
pub fn point_for(
    store: &Path,
    task: &Task,
    provider: Option<Provider>,
    resets_at: Option<u64>,
    now: u64,
) -> Point {
    Point {
        task: task.task_id.clone(),
        turn: task.turn_started(),
        stopped_at: now,
        provider,
        resets_at,
        workspace: None,
        device: None,
        session: session(store, task),
        worktree: super::local::record(store, &task.task_id).map(|record| record.worktree),
        checkpoint: task
            .run
            .as_ref()
            .and_then(|run| run.result.as_ref())
            .and_then(|result| result.candidate_snapshot.clone()),
        resumed: None,
    }
}

/// The resume turn's message: what stopped the work, and where it picks up.
#[must_use]
pub fn prompt(point: &Point, earlier: &str) -> String {
    let mut text = String::from("A usage limit stopped this task");
    if let Some(provider) = point.provider {
        text.push_str(&format!(" ({provider})"));
    }
    text.push_str(". Resume the work where it stopped: check what the earlier turn already changed in this worktree, then finish the request.");
    if let Some(checkpoint) = &point.checkpoint {
        text.push_str(&format!(" Last checkpoint: {checkpoint}."));
    }
    text.push_str("\n\nThe request:\n\n");
    text.push_str(earlier);
    text
}

/// Continue `task` from `point` in `store` with a resume turn. The command
/// identity is fixed per point, so a repeat after a crash is an exact
/// retry. Returns the revision the resume turn starts at.
///
/// # Errors
/// The task is gone, still running, out of turns, or the store refused.
pub fn continue_task(store: &mut Store, point: &Point) -> Result<u64, super::Error> {
    let task = store.show(&point.task)?;
    if task.turn_started() != point.turn {
        // An earlier attempt already continued it, or the person did.
        return Err(super::Error::InvalidTransition);
    }
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: format!("resume-{}-{}", point.task, point.turn),
        task_id: point.task.clone(),
        expected_revision: Some(task.revision),
        action: Action::Continue {
            prompt: prompt(point, task.effective_prompt()),
        },
    };
    let bytes = serde_json::to_vec(&command)
        .map_err(|_| super::Error::Corrupt("a resume command could not be encoded"))?;
    store.apply(&bytes)?;
    Ok(store.show(&point.task)?.turn_started())
}

/// `coder task resume TASK`: continue `task` in the store at `dir` from its
/// open resume point now, whatever the book says, and resolve the point.
/// Returns the point and the revision the resume turn starts at. The turn
/// waits queued for a start, like any follow-up.
///
/// # Errors
/// No open point for the task, or the store refused the turn.
pub fn resume_now(dir: &Path, task: &str, now: u64) -> Result<(Point, u64), String> {
    let point = open(dir, task).ok_or_else(|| format!("task {task} has no open resume point"))?;
    let mut store = Store::open(dir).map_err(|e| e.to_string())?;
    let turn = continue_task(&mut store, &point).map_err(|e| e.to_string())?;
    drop(store);
    resolve(
        dir,
        task,
        point.turn,
        Resumed {
            at: now,
            turn: Some(turn),
            refused: None,
        },
    )?;
    Ok((point, turn))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(task: &str, provider: Option<Provider>) -> Point {
        Point {
            task: task.into(),
            turn: 1,
            stopped_at: 1_000,
            provider,
            resets_at: Some(5_000),
            workspace: Some("allowed".into()),
            device: Some("phone".into()),
            session: Some("s-1".into()),
            worktree: None,
            checkpoint: Some("sha256:abc".into()),
            resumed: None,
        }
    }

    #[test]
    fn a_point_is_recorded_once_and_resolved_once() {
        let dir = tempfile::tempdir().unwrap();
        assert!(record(dir.path(), point("a", Some(Provider::Claude))).unwrap());
        assert!(!record(dir.path(), point("a", Some(Provider::Claude))).unwrap());
        assert_eq!(
            open(dir.path(), "a").unwrap().session.as_deref(),
            Some("s-1")
        );
        resolve(
            dir.path(),
            "a",
            1,
            Resumed {
                at: 6_000,
                turn: Some(3),
                refused: None,
            },
        )
        .unwrap();
        assert_eq!(open(dir.path(), "a"), None);
        assert_eq!(load(dir.path()).len(), 1);
    }

    #[test]
    fn a_point_is_due_once_a_provider_has_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let held = capacity::Refusal::new(
            Provider::Claude,
            capacity::Kind::UsageLimit,
            1_000,
            Some(5_000),
        );
        capacity::record_with(dir.path(), held, |_| None).unwrap();
        let book = capacity::Book::load_with(dir.path(), |_| None);
        let stopped = point("a", Some(Provider::Claude));
        assert!(!due(&stopped, &book, &[], 4_999));
        assert!(due(&stopped, &book, &[], 5_000));
        // Another admitted provider with capacity resumes it sooner.
        assert!(due(
            &stopped,
            &book,
            &[Provider::Claude, Provider::Codex],
            2_000
        ));
        // Without a provider, the point's own reset decides.
        assert!(!due(&point("b", None), &book, &[], 4_999));
        assert!(due(&point("b", None), &book, &[], 5_000));
    }

    fn task_ended(ending: &str, exit_code: i32) -> Task {
        let mut task = Task {
            task_id: "t".repeat(64),
            revision: 3,
            intent: super::super::TaskIntent {
                title: "Fix".into(),
                prompt: "Fix the parser.".into(),
                workspace: super::super::Workspace {
                    path: "/work".into(),
                    source_revision: None,
                },
                configuration: super::super::RequestedConfiguration {
                    adapter: "microcoder-repository".into(),
                    model: None,
                },
                images: Vec::new(),
            },
            intent_digest: String::new(),
            status: Status::Finished,
            execution: if exit_code == 0 {
                Execution::Finished
            } else {
                Execution::Failed
            },
            checks: super::super::Checks::NotRun,
            cancellation_reason: None,
            corrections: Vec::new(),
            run: None,
            follow_ups: Vec::new(),
            earlier: Vec::new(),
        };
        let mut run = super::super::studio_sim::run(&task, ending);
        if let Some(result) = run.result.as_mut() {
            result.exit_code = Some(exit_code);
            result.candidate_snapshot = Some("sha256:abc".into());
        }
        task.run = Some(run);
        task
    }

    /// A run that ended `no_capacity` was stopped by a limit; one that
    /// finished, or failed with no limit its grant's providers hit, was not.
    #[test]
    fn a_run_a_limit_ended_is_told_from_one_that_finished() {
        let book = capacity::Book::default();
        let stopped = task_ended(capacity::NO_CAPACITY_ENDING, 1);
        assert_eq!(
            stopped_by_limit(&stopped, 1_000, &book, 2_000),
            Some((None, None))
        );
        assert_eq!(
            stopped_by_limit(&task_ended("model_finished", 0), 1_000, &book, 2_000),
            None
        );
        assert_eq!(
            stopped_by_limit(&task_ended("engine_error", 1), 1_000, &book, 2_000),
            None
        );
        let dir = tempfile::tempdir().unwrap();
        let point = point_for(dir.path(), &stopped, Some(Provider::Claude), Some(9), 2_000);
        assert_eq!(point.turn, 1);
        assert_eq!(point.checkpoint.as_deref(), Some("sha256:abc"));
        assert_eq!(point.session, None);
        // The delegate session the run's trace names.
        let trace = &stopped.run.as_ref().unwrap().admission.trace_file;
        std::fs::write(
            dir.path().join(trace),
            "{\"step\":{\"extensions\":{\"claude_session\":{\"session\":\"s-9\"}}}}\nnot json\n",
        )
        .unwrap();
        assert_eq!(session(dir.path(), &stopped).as_deref(), Some("s-9"));
    }

    #[test]
    fn the_resume_message_names_the_limit_the_checkpoint_and_the_request() {
        let text = prompt(&point("a", Some(Provider::Claude)), "Fix the parser.");
        assert!(text.contains("(claude)"), "{text}");
        assert!(text.contains("sha256:abc"), "{text}");
        assert!(text.ends_with("Fix the parser."), "{text}");
    }
}
