import json
from pathlib import Path

from tbench import looptime


def invocation(at, event, inv_id, component, parent="inv-1"):
    return {
        "record": "step",
        "step": {
            "at": at,
            "source": "System",
            "message": "",
            "extensions": {
                "invocation": {
                    "event": event,
                    "id": inv_id,
                    "parent": parent,
                    "component": component,
                    "name": component,
                }
            },
        },
    }


def write_trial(trial: Path, *, finished: bool = True, episode_ends: bool = True) -> None:
    trial.mkdir(parents=True)
    (trial / "trial.log").write_text("")
    result = {
        "task_name": "terminal-bench/cargo-flight-dispatch",
        "trial_name": trial.name,
        "environment_setup": {
            "started_at": "2026-09-23T14:10:16Z",
            "finished_at": "2026-09-23T14:10:18Z",
        },
        "agent_setup": {
            "started_at": "2026-09-23T14:10:18Z",
            "finished_at": "2026-09-23T14:10:23Z",
        },
        "agent_result": {"cost_usd": 1.25},
    }
    if finished:
        result.update(
            {
                "started_at": "2026-09-23T14:10:16Z",
                "finished_at": "2026-09-23T14:30:16Z",
                "agent_execution": {
                    "started_at": "2026-09-23T14:10:23Z",
                    "finished_at": "2026-09-23T14:29:23Z",
                },
                "verifier": {
                    "started_at": "2026-09-23T14:29:30Z",
                    "finished_at": "2026-09-23T14:30:10Z",
                },
                "verifier_result": {"rewards": {"reward": 1.0}},
            }
        )
    (trial / "result.json").write_text(json.dumps(result))
    steps = [
        invocation(0, "start", "inv-1", "episode", parent=None),
        invocation(0, "start", "inv-2", "task.requirements"),
        invocation(4000, "end", "inv-2", "task.requirements"),
        invocation(4000, "start", "inv-3", "exec.session"),
        invocation(604000, "end", "inv-3", "exec.session"),
        invocation(604000, "start", "inv-4", "verify.checks"),
        invocation(606000, "end", "inv-4", "verify.checks"),
        invocation(606000, "start", "inv-5", "verify.repair"),
    ]
    if episode_ends:
        steps += [
            invocation(906000, "end", "inv-5", "verify.repair"),
            invocation(906000, "start", "inv-6", "exec.session"),
            invocation(966000, "end", "inv-6", "exec.session"),
            invocation(966000, "end", "inv-1", "episode", parent=None),
        ]
    where = "episode" if episode_ends else "live"
    log = trial / "agent" / where / "episode.atif.jsonl"
    log.parent.mkdir(parents=True)
    log.write_text("".join(json.dumps(s) + "\n" for s in steps))
    (trial / "tbench-environment.jsonl").write_text(
        json.dumps({"role": "environment", "cache": "warm"}) + "\n"
    )


def test_a_finished_trial_splits_its_time_by_phase(tmp_path: Path):
    trial = tmp_path / "job" / "cargo-flight-dispatch__abc"
    write_trial(trial)
    row = looptime.trial_looptime(trial)
    assert row["stage"] == "done"
    assert row["reward"] == 1.0
    assert row["image"] == "warm"
    seconds = row["seconds"]
    assert seconds["env"] == 2.0
    assert seconds["agent_setup"] == 5.0
    assert seconds["prep"] == 4.0
    assert seconds["executor"] == 600.0
    assert seconds["checks"] == 2.0
    assert seconds["repair"] == 300.0
    # The second executor session counts as later work.
    assert seconds["later"] == 60.0
    assert seconds["verifier"] == 40.0
    assert seconds["total"] == 1200.0


def test_a_running_trial_names_the_component_it_is_in(tmp_path: Path):
    trial = tmp_path / "job" / "cargo-flight-dispatch__abc"
    write_trial(trial, finished=False, episode_ends=False)
    row = looptime.trial_looptime(trial)
    assert row["stage"] == "verify.repair"
    assert row["seconds"]["total"] is None


def test_the_table_sums_every_trial(tmp_path: Path):
    job = tmp_path / "job"
    write_trial(job / "a__1")
    write_trial(job / "b__2")
    (job / "tbench").mkdir()
    rows = looptime.job_looptimes(job)
    assert [row["trial"] for row in rows] == ["a__1", "b__2"]
    summary = looptime.totals(rows)
    assert summary["passed"] == 2
    assert summary["cost_usd"] == 2.5
    assert summary["seconds"]["total"] == 2400.0
    table = looptime.render(rows)
    assert "2 trials" in table and "2/2 passed" in table and "$2.50" in table


def test_stop_records_are_not_starts(tmp_path):
    (tmp_path / "tbench-environment.jsonl").write_text(
        "\n".join(
            json.dumps(record)
            for record in (
                {"role": "environment", "cache": "warm"},
                {"event": "stop", "role": "environment", "stop_ms": 2000},
                {"event": "start", "role": "tests", "cache": "cold"},
            )
        )
        + "\n"
    )
    starts = looptime.environment_starts(tmp_path)
    assert [start["role"] for start in starts] == ["environment", "tests"]
    assert [stop["stop_ms"] for stop in looptime.environment_stops(tmp_path)] == [2000]
