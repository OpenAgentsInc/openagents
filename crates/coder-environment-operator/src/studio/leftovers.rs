//! Computers this studio's own records name that nothing will use again
//! (#11317), for the hourly sweep beside the sign-in computers' (#11296).
//!
//! Every computer an environment uses is named by a record under the
//! studio's state, so a sweep never has to guess an owner on the Boat
//! service that staging and production share: it looks only at what this
//! deployment's records name, and deletes only stopped computers
//! ([`super::sign_in::delete_stopped`]).
//!
//! | Record | Computer | Nothing uses it again once |
//! | --- | --- | --- |
//! | `environment-computers/*.json` (setup, image build, fresh-machine check) | `creates[].resource`, `boots[].resource` | its last boot's absolute deadline has passed: its owner can no longer use it, whatever the record says (a setup whose stop failed, a check that stalled, a build machine recorded as deleted that wasn't) |
//! | `environment-claude/<env>/*.json` (Claude Code runs) | `resource` | the run has ended (done, failed, stopped), a while ago: runs before 337a5c745b only stopped their computer |
//!
//! Never deleted: a computer that is running, an environment's computer
//! for Claude sign-in (`keep`), and anything no record of this studio
//! names.

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// How long past its absolute deadline a computer record waits before its
/// computer is deleted.
pub const DEADLINE_GRACE_MS: u64 = 3_600_000;
/// How long an ended run waits before its computer is deleted (its own
/// driver deletes it at once; this is for the ones it didn't).
pub const RUN_GRACE_MS: u64 = 600_000;
/// Records not changed for this long are not looked at again.
pub const LOOK_BACK_MS: u64 = 30 * 24 * 3_600_000;

/// The computers the records under `state` name that nothing will use
/// again at `now_ms`, leaving out `keep`.
#[must_use]
pub fn finished(state: &Path, now_ms: u64, keep: &BTreeSet<String>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for record in json_files(&state.join("environment-computers")) {
        if let Some(deadline) = deadline_ms(&record)
            && deadline.saturating_add(DEADLINE_GRACE_MS) < now_ms
            && recent(&record, now_ms)
        {
            found.extend(resources(&record));
        }
    }
    if let Ok(envs) = std::fs::read_dir(state.join("environment-claude")) {
        for env in envs.flatten() {
            for record in json_files(&env.path()) {
                let ended = matches!(
                    record["state"].as_str(),
                    Some("completed" | "failed" | "cancelled")
                );
                let settled = record["updated_ms"]
                    .as_u64()
                    .is_some_and(|at| at.saturating_add(RUN_GRACE_MS) < now_ms);
                if ended
                    && settled
                    && recent(&record, now_ms)
                    && let Some(resource) = record["resource"].as_str()
                {
                    found.insert(resource.to_owned());
                }
            }
        }
    }
    found.retain(|id| !id.is_empty() && !keep.contains(id));
    found
}

/// The newest boot's absolute deadline, else the record's own bound from
/// when it was made.
fn deadline_ms(record: &Value) -> Option<u64> {
    let boot = record["boots"]
        .as_array()
        .and_then(|boots| boots.last())
        .and_then(|boot| boot["absolute_deadline_ms"].as_u64());
    boot.or_else(|| {
        Some(record["created_ms"].as_u64()? + record["bounds"]["absolute_ms"].as_u64()?)
    })
}

fn recent(record: &Value, now_ms: u64) -> bool {
    record["updated_ms"]
        .as_u64()
        .is_none_or(|at| now_ms.saturating_sub(at) < LOOK_BACK_MS)
}

fn resources(record: &Value) -> BTreeSet<String> {
    ["creates", "boots"]
        .iter()
        .filter_map(|key| record[*key].as_array())
        .flatten()
        .filter_map(|entry| entry["resource"].as_str())
        .map(str::to_owned)
        .collect()
}

fn json_files(dir: &Path) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter_map(|path| std::fs::read(path).ok())
        .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_791_700_000_000;

    fn write(path: &Path, value: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }

    fn computer(id: &str, resource: &str, deadline: u64, phase: &str) -> Value {
        json!({
            "id": id,
            "created_ms": deadline - 7_200_000,
            "updated_ms": deadline - 7_000_000,
            "bounds": {"absolute_ms": 7_200_000},
            "phase": {"phase": phase},
            "creates": [{"resource": resource}],
            "boots": [{"number": 1, "resource": resource, "absolute_deadline_ms": deadline}],
        })
    }

    #[test]
    fn past_their_deadline_records_name_leftovers_and_live_ones_do_not() {
        let dir = tempfile::tempdir().unwrap();
        let computers = dir.path().join("environment-computers");
        let hour = 3_600_000;
        // A setup whose stop failed, hours past its deadline.
        write(
            &computers.join("setup-env-a-s1.json"),
            &computer("setup-env-a-s1", "bx_setup", NOW - 3 * hour, "stopping"),
        );
        // A build machine its record calls deleted, which wasn't.
        write(
            &computers.join("builder-bj-1.json"),
            &computer("builder-bj-1", "bx_build", NOW - 2 * hour, "deleted"),
        );
        // A check still within its deadline.
        write(
            &computers.join("verifier-vj-2.json"),
            &computer("verifier-vj-2", "bx_live", NOW + hour, "turn"),
        );
        // Just past its deadline: within the grace.
        write(
            &computers.join("verifier-vj-3.json"),
            &computer("verifier-vj-3", "bx_grace", NOW - hour / 2, "turn"),
        );
        // Long forgotten.
        let mut old = computer("setup-env-b-s1", "bx_old", NOW - 40 * 24 * hour, "deleted");
        old["updated_ms"] = json!(NOW - 40 * 24 * hour);
        write(&computers.join("setup-env-b-s1.json"), &old);
        let runs = dir.path().join("environment-claude").join("env-a");
        let run = |state: &str, resource: &str, updated: u64| json!({"id": "claude-env-a-1", "state": state, "resource": resource, "updated_ms": updated});
        write(
            &runs.join("claude-env-a-1.json"),
            &run("completed", "bx_ran", NOW - hour),
        );
        write(
            &runs.join("claude-env-a-2.json"),
            &run("running", "bx_running", NOW - hour),
        );
        write(
            &runs.join("claude-env-a-3.json"),
            &run("paused", "bx_paused", NOW - hour),
        );
        write(
            &runs.join("claude-env-a-4.json"),
            &run("completed", "bx_just", NOW - 60_000),
        );
        // A run on the person's signed-in computer.
        write(
            &runs.join("claude-env-a-5.json"),
            &run("completed", "bx_home", NOW - hour),
        );
        let keep: BTreeSet<String> = ["bx_home".to_owned()].into();
        let found = finished(dir.path(), NOW, &keep);
        let want: BTreeSet<String> = ["bx_setup", "bx_build", "bx_ran"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(found, want);
    }
}
