//! The router's lifecycle over this task owner (#10207, plan section 6).
//!
//! `route-contract` carries the task owner's `(status, execution, checks)`
//! words without linking Coder ([`route_contract::lifecycle`]). This is the
//! adapter: a task's disposition in the contract's words
//! ([`disposition`]), its projection ([`project`]), and what one route
//! record keeps of it ([`observation`]): the current turn's cost, wall
//! time, retained artifact, and trace. The matches are exhaustive, so a new
//! word on either side fails to compile here, and a test keeps the serde
//! words equal. Nothing here writes the task store; the journal stays the
//! only execution state machine.

use route_contract::Digest;
use route_contract::lifecycle::{
    Projection, TaskChecks, TaskDisposition, TaskExecution, TaskStatus,
};
use route_contract::record::Observation;

use super::{Checks, Execution, Status, Task};

#[must_use]
pub fn status(status: Status) -> TaskStatus {
    match status {
        Status::Queued => TaskStatus::Queued,
        Status::Cancelled => TaskStatus::Cancelled,
        Status::Running => TaskStatus::Running,
        Status::CancelRequested => TaskStatus::CancelRequested,
        Status::Finished => TaskStatus::Finished,
        Status::Unknown => TaskStatus::Unknown,
    }
}

#[must_use]
pub fn execution(execution: Execution) -> TaskExecution {
    match execution {
        Execution::NotStarted => TaskExecution::NotStarted,
        Execution::Running => TaskExecution::Running,
        Execution::Finished => TaskExecution::Finished,
        Execution::Failed => TaskExecution::Failed,
        Execution::Stopped => TaskExecution::Stopped,
        Execution::Unknown => TaskExecution::Unknown,
    }
}

#[must_use]
pub fn checks(checks: Checks) -> TaskChecks {
    match checks {
        Checks::NotRun => TaskChecks::NotRun,
        Checks::Running => TaskChecks::Running,
        Checks::Passed => TaskChecks::Passed,
        Checks::Failed => TaskChecks::Failed,
        Checks::Unavailable => TaskChecks::Unavailable,
        Checks::Disputed => TaskChecks::Disputed,
    }
}

/// The task's disposition in the contract's words.
#[must_use]
pub fn disposition(task: &Task) -> TaskDisposition {
    TaskDisposition {
        status: status(task.status),
        execution: execution(task.execution),
        checks: checks(task.checks),
    }
}

/// The lifecycle state the task projects to.
#[must_use]
pub fn project(task: &Task) -> Projection {
    route_contract::lifecycle::project(disposition(task))
}

/// What a route record keeps of `task`: its disposition and revision, and
/// the current turn's cost, wall time, retained artifact (the patch or
/// output), and trace, each when the run recorded it.
#[must_use]
pub fn observation(task: &Task) -> Observation {
    let result = task.run.as_ref().and_then(|run| run.result.as_ref());
    let artifacts = result
        .into_iter()
        .flat_map(|result| {
            [
                result.artifact_digest.clone(),
                Some(result.trace_digest.clone()),
            ]
        })
        .flatten()
        .filter_map(|digest| {
            // The store keeps some digests as bare hex.
            let digest = if digest.starts_with("sha256:") {
                digest
            } else {
                format!("sha256:{}", digest.to_ascii_lowercase())
            };
            Digest::try_from(digest).ok()
        })
        .collect();
    Observation {
        disposition: disposition(task),
        revision: task.revision,
        cost_microusd: result.and_then(|result| result.cost_microusd),
        wall_ms: result.map(|result| result.elapsed_ms),
        artifacts,
        payer: result.and_then(|result| result.payer.map(|payer| payer.word().to_owned())),
        payer_keys: result
            .map(|result| {
                result
                    .payer_keys
                    .iter()
                    .map(|key| route_contract::record::PayerKey {
                        provider: key.provider.word().to_owned(),
                        fingerprint: key.fingerprint.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// [`observation`] of `task` in the store at `directory`, when it reads.
#[must_use]
pub fn observe(directory: &std::path::Path, task: &str) -> Option<Observation> {
    super::Store::open(directory)
        .and_then(|store| store.show(task))
        .ok()
        .map(|task| observation(&task))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word<T: serde::Serialize>(value: T) -> String {
        serde_json::to_string(&value).unwrap()
    }

    /// The contract's words are this task owner's words, both ways: every
    /// variant here maps to the variant spelled the same there, and the
    /// contract has no word this owner lacks.
    #[test]
    fn the_contract_words_equal_the_task_owner_words() {
        let statuses = [
            Status::Queued,
            Status::Cancelled,
            Status::Running,
            Status::CancelRequested,
            Status::Finished,
            Status::Unknown,
        ];
        let executions = [
            Execution::NotStarted,
            Execution::Running,
            Execution::Finished,
            Execution::Failed,
            Execution::Stopped,
            Execution::Unknown,
        ];
        let all_checks = [
            Checks::NotRun,
            Checks::Running,
            Checks::Passed,
            Checks::Failed,
            Checks::Unavailable,
            Checks::Disputed,
        ];
        for value in statuses {
            assert_eq!(word(value), word(status(value)));
        }
        for value in executions {
            assert_eq!(word(value), word(execution(value)));
        }
        for value in all_checks {
            assert_eq!(word(value), word(checks(value)));
        }
        let ours: Vec<String> = statuses.into_iter().map(word).collect();
        let theirs: Vec<String> = TaskStatus::ALL.into_iter().map(word).collect();
        assert_eq!(ours, theirs);
        let ours: Vec<String> = executions.into_iter().map(word).collect();
        let theirs: Vec<String> = TaskExecution::ALL.into_iter().map(word).collect();
        assert_eq!(ours, theirs);
        let ours: Vec<String> = all_checks.into_iter().map(word).collect();
        let theirs: Vec<String> = TaskChecks::ALL.into_iter().map(word).collect();
        assert_eq!(ours, theirs);
        // And every word parses back as the owner's own type.
        for value in TaskStatus::ALL {
            let _: Status = serde_json::from_str(&word(value)).unwrap();
        }
        for value in TaskExecution::ALL {
            let _: Execution = serde_json::from_str(&word(value)).unwrap();
        }
        for value in TaskChecks::ALL {
            let _: Checks = serde_json::from_str(&word(value)).unwrap();
        }
    }
}
