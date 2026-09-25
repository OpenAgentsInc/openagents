"""Retention copies a trial's full closure and reports what it can't."""

import hashlib
import json

import pytest

from tbench.retain import (
    RETENTION_SCHEMA,
    RetentionError,
    known_credentials,
    retain_jobs,
    retain_trial,
)

SECRET = "sk-fixture-secret-value-0123456789"


def _write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return hashlib.sha256(text.encode()).hexdigest()


def _job(tmp_path, *, stream_text='{"type":"turn"}\n', drop_stream=False):
    job = tmp_path / "jobs" / "extended--coder-one-x--task"
    trial = job / "task__abc"
    episode = trial / "agent" / "episode"
    atif = json.dumps({"schema_version": "ATIF-v1.7", "steps": [{}, {}]})
    atif_sha = _write(episode / "trajectory.atif.json", atif)
    _write(trial / "agent" / "trajectory.json", atif)
    stream_sha = _write(episode / "artifacts" / "delegate-1.stream.jsonl", stream_text)
    if drop_stream:
        (episode / "artifacts" / "delegate-1.stream.jsonl").unlink()
    usage_sha = _write(episode / "evaluation" / "usage.json", "{}")
    manifest = {
        "contract": "openagents.coder.episode.v1",
        "delegate": {
            "delegation": {
                "stream": {
                    "path": "artifacts/delegate-1.stream.jsonl",
                    "sha256": stream_sha,
                    "truncated": False,
                }
            }
        },
        "files": {
            "trajectory": {"path": "trajectory.atif.json", "sha256": atif_sha},
            "usage": {"path": "evaluation/usage.json", "sha256": usage_sha},
            "delegate-1_stream_jsonl": {
                "path": "artifacts/delegate-1.stream.jsonl",
                "sha256": stream_sha,
            },
            "state": {"path": "artifacts/state.json", "sha256": "0" * 64},
        },
    }
    _write(episode / "manifest.json", json.dumps(manifest))
    _write(trial / "verifier" / "reward.txt", "0")
    _write(trial / "verifier" / "test-stdout.txt", "FAILED test_summary\n")
    _write(trial / "artifacts" / "manifest.json", json.dumps(
        [{"source": "/logs/artifacts", "status": "empty"}]
    ))
    _write(trial / "result.json", json.dumps({
        "trial_name": "task__abc",
        "task_name": "terminal-bench/task",
        "config": {"agent": {"env": {"KEY": "not retained"}}},
        "verifier_result": {"rewards": {"reward": 0.0}},
        "agent_result": {"n_input_tokens": 10, "cost_usd": 0.1},
    }))
    return job, trial


def test_every_reference_is_copied_or_reported(tmp_path):
    _, trial = _job(tmp_path)
    traces = tmp_path / "traces"
    out = retain_trial(trial, traces, {})
    record = out.record
    assert record["schema"] == RETENTION_SCHEMA
    episode = traces / "extended--coder-one-x--task" / "task__abc.episode"
    for relative in (
        "manifest.json",
        "trajectory.atif.json",
        "evaluation/usage.json",
        "artifacts/delegate-1.stream.jsonl",
        "verifier/test-stdout.txt",
        "verifier/reward.txt",
        "produced/manifest.json",
        "harbor-result.json",
        "retention.json",
    ):
        assert (episode / relative).is_file(), relative
    assert (episode.parent / "task__abc.json").is_file()
    # The manifest names state.json, which the episode never wrote.
    assert [m["reference"] for m in record["missing"]] == [
        "artifacts/state.json",
        "verifier/ctrf.json",
    ]
    stream = next(f for f in record["files"] if f["kind"] == "native delegate stream")
    assert stream["digest"] == "match"
    assert "byte-identical" in record["atif"]["conversion"]
    assert any("collected nothing" in note for note in record["notes"])
    harbor = json.loads((episode / "harbor-result.json").read_text())
    assert "config" not in harbor
    assert harbor["agent_result"]["cost_usd"] == 0.1


def test_missing_stream_is_named_not_dropped(tmp_path):
    _, trial = _job(tmp_path, drop_stream=True)
    out = retain_trial(trial, tmp_path / "traces", {})
    assert any(
        m["reference"] == "artifacts/delegate-1.stream.jsonl"
        and m["kind"] == "native delegate stream"
        for m in out.missing
    )


def test_credential_scan_refuses_to_write(tmp_path):
    _, trial = _job(tmp_path, stream_text=f'{{"token":"{SECRET}"}}\n')
    traces = tmp_path / "traces"
    with pytest.raises(RetentionError) as caught:
        retain_trial(trial, traces, {"env TEST_KEY": SECRET})
    assert SECRET not in str(caught.value)
    assert "env TEST_KEY" in str(caught.value)
    assert not traces.exists()


def test_clean_scan_is_recorded(tmp_path):
    _, trial = _job(tmp_path)
    out = retain_trial(trial, tmp_path / "traces", {"env TEST_KEY": SECRET})
    scan = out.record["credential_scan"]
    assert scan["matches"] == 0
    assert scan["credentials_checked"] == ["env TEST_KEY"]
    assert scan["files_scanned"] > 5


def test_size_bound_reports_instead_of_copying(tmp_path):
    _, trial = _job(tmp_path, stream_text="x" * 5000)
    out = retain_trial(trial, tmp_path / "traces", {}, max_file_bytes=4000)
    assert any(
        m["reference"] == "artifacts/delegate-1.stream.jsonl"
        and "file bound" in m["reason"]
        for m in out.missing
    )


def test_retain_jobs_resolves_names_and_reports_unknown(tmp_path):
    _job(tmp_path)
    retained, errors = retain_jobs(
        ["extended--coder-one-x--task", "no-such-job"],
        jobs_dir=tmp_path / "jobs",
        traces_dir=tmp_path / "traces",
        credentials={},
    )
    assert [r.trial for r in retained] == ["task__abc"]
    assert errors == ["no-such-job: no job directory"]


def test_known_credentials_reads_files_without_short_values(tmp_path, monkeypatch):
    for name in (
        "OPENAGENTS_API_KEY",
        "TYPESAFE_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
    ):
        monkeypatch.delenv(name, raising=False)
    _write(tmp_path / ".openagents" / "bearer", SECRET + "\n")
    _write(tmp_path / ".openagents" / "jev.json", json.dumps({"api_key": "short"}))
    _write(tmp_path / ".codex" / "auth.json", json.dumps(
        {"tokens": {"access_token": SECRET + "a", "account_id": "abc"}}
    ))
    found = known_credentials(tmp_path)
    assert found == {
        "~/.openagents/bearer": SECRET,
        "~/.codex/auth.json tokens.access_token": SECRET + "a",
    }


def test_candidate_checkpoint_and_coverage_are_in_the_retained_closure(tmp_path):
    _, trial = _job(tmp_path)
    relative = 'candidate-checkpoints/lean-1-session-1'
    _write(trial / 'agent' / relative / 'receipt.json', '{"complete":true}')
    _write(trial / 'agent' / relative / 'artifacts/db/dump.sql', 'synthetic checkpoint')
    _write(trial / 'agent/candidate-preflight.json', '{"supported":true}')
    out = retain_trial(trial, tmp_path / 'traces', {})
    assert (out.destination / (trial.name + '.episode') / relative / 'receipt.json').read_text() == '{"complete":true}'
    assert (out.destination / (trial.name + '.episode') / relative / 'artifacts/db/dump.sql').read_text() == 'synthetic checkpoint'
    assert (out.destination / (trial.name + '.episode') / 'candidate-preflight.json').exists()
