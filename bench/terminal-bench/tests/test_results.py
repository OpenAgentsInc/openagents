"""Attempt records: provenance, unknown semantics, terminal statuses."""

import json

from tbench.counts import count_trajectory, unknown_counts
from tbench.results import attempt_record


def _trial_result(**overrides):
    base = {
        "id": "trial-1",
        "trial_name": "fix-git__abc__attempt-1",
        "task_name": "fix-git",
        "started_at": "2026-09-22T10:00:00Z",
        "finished_at": "2026-09-22T10:10:00Z",
        "task_checksum": "deadbeef" * 8,
        "task_id": {
            "path": "archive/fix-git",
            "git_url": "https://example/t.git",
            "git_commit_id": "c0ffee" * 6,
        },
        "config": {
            "agent": {"name": "claude-code", "model_name": "claude-sonnet-4-5"},
        },
        "agent_info": {
            "name": "claude-code",
            "version": "2.1.278",
            "model_info": {"name": "claude-sonnet-4-5", "provider": "anthropic"},
        },
        "agent_result": {
            "n_input_tokens": 1200,
            "n_cache_tokens": 40,
            "n_output_tokens": 300,
            "cost_usd": 0.0123,
        },
        "verifier_result": {"rewards": {"reward": 1.0}},
        "environment_setup": {
            "started_at": "2026-09-22T10:00:00Z",
            "finished_at": "2026-09-22T10:01:00Z",
        },
        "agent_execution": {
            "started_at": "2026-09-22T10:02:00Z",
            "finished_at": "2026-09-22T10:09:00Z",
        },
    }
    base.update(overrides)
    return base


def _record(result, **kwargs):
    return attempt_record(
        result,
        job_name="smoke--claude-code",
        trial_dir=kwargs.pop("trial_dir", __import__("pathlib").Path("/x")),
        arm="claude-code",
        profile_id="smoke",
        auth_mode="api-key",
        declared_cost_provenance="provider_reported",
        **kwargs,
    )


def test_record_schema_and_identity(tmp_path):
    record = _record(_trial_result(), trial_dir=tmp_path)
    assert record["schema"] == "openagents.tbench.attempt.v1"
    assert record["task"]["git_commit_id"] == "c0ffee" * 6
    assert record["task"]["checksum"] == "deadbeef" * 8
    assert record["outcome"]["reward"] == 1.0
    assert record["outcome"]["terminal_status"] == "completed"


def test_timing_phases_are_separate(tmp_path):
    record = _record(_trial_result(), trial_dir=tmp_path)
    timing = record["timing"]
    assert timing["environment_setup_ms"] == 60_000
    assert timing["agent_execution_ms"] == 420_000
    assert timing["total_ms"] == 600_000


def test_missing_usage_is_unknown_not_zero(tmp_path):
    result = _trial_result(agent_result={})
    record = _record(result, trial_dir=tmp_path)
    assert record["usage"]["input_tokens"] is None
    assert record["usage"]["coverage"] == "unknown"


def test_partial_usage_marked(tmp_path):
    result = _trial_result(agent_result={"n_input_tokens": 5})
    record = _record(result, trial_dir=tmp_path)
    assert record["usage"]["coverage"] == "partial"


def test_missing_cost_overrides_provenance(tmp_path):
    result = _trial_result(agent_result={"cost_usd": None})
    record = _record(result, trial_dir=tmp_path)
    assert record["cost"]["amount_usd"] is None
    assert record["cost"]["provenance"] == "unknown"


def test_timeout_and_refusal_statuses(tmp_path):
    timed_out = _trial_result(
        exception_info={"exception_type": "AgentTimeoutError", "message": "x"}
    )
    assert _record(timed_out, trial_dir=tmp_path)["outcome"]["terminal_status"] == "timeout"

    refused = _trial_result(
        exception_info={"exception_type": "AgentSafetyRefusalError"}
    )
    assert _record(refused, trial_dir=tmp_path)["outcome"]["terminal_status"] == "provider_refusal"

    no_verifier = _trial_result(verifier_result=None)
    assert _record(no_verifier, trial_dir=tmp_path)["outcome"]["terminal_status"] == "unverifiable"


def test_atif_counts_semantics():
    trajectory = {
        "steps": [
            {
                "source": "agent",
                "tool_calls": [
                    {"tool_name": "bash"},
                    {"tool_name": "read_file"},
                ],
            },
            {"source": "system"},
            {
                "source": "agent",
                "tool_calls": [{"tool_name": "bash", "is_retry": True}],
                "extra": {"decision_call": True},
            },
        ]
    }
    counts = count_trajectory(trajectory)
    assert counts["atif_steps"] == 3
    assert counts["tool_calls"] == 3
    assert counts["shell_commands"] == 2
    assert counts["retries"] == 1
    assert counts["typed_decisions"] == 1


def test_unknown_counts_never_zero():
    counts = unknown_counts("no trajectory.json retained")
    assert counts["atif_steps"] == "unknown"
    assert counts["tool_calls"] == "unknown"


def test_contract_refusals_are_install_failures(tmp_path):
    for etype in ("EpisodeContractError", "ArtifactIdentityError"):
        result = _trial_result(
            exception_info={"exception_type": etype}, verifier_result=None
        )
        record = _record(result, trial_dir=tmp_path)
        assert record["outcome"]["terminal_status"] == "install_failure"
    timed_out = _trial_result(
        exception_info={"exception_type": "EpisodeTimeoutError"}
    )
    assert _record(timed_out, trial_dir=tmp_path)["outcome"]["terminal_status"] == "timeout"
    cancelled = _trial_result(exception_info={"exception_type": "CancelledError"})
    assert _record(cancelled, trial_dir=tmp_path)["outcome"]["terminal_status"] == "cancelled"


def test_artifact_digest_recorded(tmp_path):
    result = _trial_result(
        config={"agent": {"import_path": "x:Y", "kwargs": {"artifact_sha256": "ab" * 32}}}
    )
    assert _record(result, trial_dir=tmp_path)["agent"]["artifact_sha256"] == "ab" * 32


def _image_trial(tmp_path, *, docker_image="alexgshaw/fix-git:20260403",
                 force_build=False, log="", setup_finished=True):
    task = tmp_path / "task"
    task.mkdir(exist_ok=True)
    env = f'docker_image = "{docker_image}"\n' if docker_image else ""
    (task / "task.toml").write_text(f"[environment]\n{env}")
    trial = tmp_path / "trial"
    trial.mkdir(exist_ok=True)
    log_path = trial / "trial.log"
    if log is None:
        log_path.unlink(missing_ok=True)
    else:
        log_path.write_text(log)
    result = _trial_result(
        config={
            "task": {"path": str(task)},
            "environment": {"type": "docker", "force_build": force_build},
            "agent": {},
        },
        environment_setup=(
            {"started_at": "2026-09-22T10:00:00Z", "finished_at": "2026-09-22T10:00:01Z"}
            if setup_finished
            else {"started_at": "2026-09-22T10:00:00Z"}
        ),
    )
    return _record(result, trial_dir=trial)["environment"]


def test_image_cold_when_harbor_found_no_local_image(tmp_path):
    state = _image_trial(
        tmp_path,
        log=(
            "Skipping image OS validation for alexgshaw/fix-git:20260403: "
            "docker inspect returned 1\nRunning command: true\n"
        ),
    )
    assert state["image_state"] == "cold"
    assert state["image_action"] == "pulled"
    assert state["image_source"] == "prebuilt"
    assert "trial.log" in state["image_state_method"]


def test_image_warm_when_the_check_passed_silently(tmp_path):
    state = _image_trial(tmp_path, log="Running command: true\n")
    assert state["image_state"] == "warm"
    assert state["image_action"] == "reused"


def test_image_unknown_without_evidence(tmp_path):
    assert _image_trial(tmp_path, log=None)["image_state"] == "unknown"
    assert (
        _image_trial(tmp_path, log="", setup_finished=False)["image_state"]
        == "unknown"
    )
    other_error = _image_trial(
        tmp_path,
        log="Skipping image OS validation for alexgshaw/fix-git:20260403: boom\n",
    )
    assert other_error["image_state"] == "unknown"


def test_dockerfile_build_is_built_with_unknown_cache_state(tmp_path):
    state = _image_trial(tmp_path, docker_image=None)
    assert state["image_source"] == "dockerfile"
    assert state["image_action"] == "built"
    assert state["image_state"] == "unknown"
    forced = _image_trial(tmp_path, force_build=True)
    assert forced["image_action"] == "built"


def test_unreadable_task_leaves_image_unknown(tmp_path):
    result = _trial_result(
        config={"task": {"path": str(tmp_path / "gone")}, "agent": {}}
    )
    state = _record(result, trial_dir=tmp_path)["environment"]
    assert state["image_state"] == "unknown"
    assert state["image_source"] == "unknown"
