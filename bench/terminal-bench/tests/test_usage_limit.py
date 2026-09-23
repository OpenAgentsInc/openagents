"""Usage-limited trials: detection from retained streams, and what follows.

The fixtures under ``fixtures/usage-limit`` are retained Claude Code
streams from the Terminal-Bench 4.0 trials the Claude subscription limit
throttled on 2026-09-23: Harbor's ``claude-code.txt`` from a
claude-code-opus trial, and a Coder One delegate stream.
"""

import asyncio
import json
from pathlib import Path

import pytest

from tbench import agents, usage_limit
from tbench.results import attempt_record
from tbench.usage_limit import (
    UsageLimitError,
    arm_providers,
    reset_from_message,
    says_limited,
    scan_stream,
    trial_usage_limit,
)

FIXTURES = Path(__file__).parent / "fixtures" / "usage-limit"
# 2026-09-23T11:30:58Z, when the retained sessions were throttled.
NOW = 1_790_163_058
# 2026-09-23T11:50:00Z, the reset their streams stated.
RESET = 1_790_164_200
CODEX_LIMIT = (
    "You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), "
    "visit https://chatgpt.com/codex/settings/usage to purchase more credits or "
    "try again at 3:04 PM."
)


def _codex_stream(message: str) -> str:
    return "\n".join(
        [
            json.dumps({"type": "thread.started", "thread_id": "t-1"}),
            json.dumps({"type": "turn.started"}),
            json.dumps({"type": "error", "message": message}),
            json.dumps({"type": "turn.failed", "error": {"message": message}}),
        ]
    )


def test_reset_times_read_like_the_rust_episode_reads_them():
    said = "You've hit your session limit · resets 11:50am (UTC)"
    assert says_limited(said)
    assert reset_from_message(said, NOW) == RESET
    assert reset_from_message(said, RESET + 60) == RESET + 86400
    assert reset_from_message(
        "You've hit your weekly limit · resets Sep 24, 5pm (UTC)", NOW
    ) == 1_790_269_200
    assert reset_from_message("resets 5pm (America/New_York)", NOW) is None
    assert reset_from_message(CODEX_LIMIT, NOW) == 1_790_175_840
    assert reset_from_message(
        "You've hit your usage limit. Try again at Sep 25th, 2026 9:15 AM.", NOW
    ) == 1_790_327_700
    assert reset_from_message("You've hit your usage limit.", NOW) is None
    assert not says_limited("API Error: 500 internal_server_error")


def test_harbors_claude_code_log_shows_the_limit_and_its_reset():
    found = scan_stream((FIXTURES / "claude-code.txt").read_text())
    assert found["provider"] == "anthropic"
    assert found["status"] == 429
    assert found["message"] == "You've hit your session limit · resets 11:50am (UTC)"
    assert found["resets_at"] == RESET
    assert found["reset_source"] == "rate_limit_event"
    assert found["window"] == "five_hour"


def test_a_coder_one_delegate_stream_shows_the_same_limit():
    found = scan_stream((FIXTURES / "delegate-3.stream.jsonl").read_text())
    assert found["provider"] == "anthropic"
    assert found["resets_at"] == RESET


def test_a_session_that_recovers_is_not_limited():
    rejected = next(
        line
        for line in (FIXTURES / "claude-code.txt").read_text().splitlines()
        if '"rejected"' in line
    )
    clean = json.dumps(
        {"type": "result", "subtype": "success", "is_error": False, "result": "Done."}
    )
    assert scan_stream(f"{rejected}\n{clean}\n") is None
    failed = json.dumps(
        {"type": "result", "is_error": True, "result": "API Error: 500"}
    )
    assert scan_stream(failed) is None


def test_a_codex_log_shows_a_chatgpt_plan_limit():
    found = scan_stream(_codex_stream(CODEX_LIMIT), now=NOW)
    assert found["provider"] == "openai"
    assert found["resets_at"] == 1_790_175_840
    assert found["reset_source"] == "message"
    assert scan_stream(_codex_stream("stream disconnected")) is None
    # A tail that lost `thread.started` is still a Codex stream.
    tail = json.dumps({"type": "turn.failed", "error": {"message": CODEX_LIMIT}})
    assert scan_stream(tail, now=NOW)["provider"] == "openai"


def _trial(tmp_path: Path, name: str = "cad-model__abc") -> Path:
    trial = tmp_path / name
    (trial / "agent").mkdir(parents=True)
    return trial


def test_a_trial_is_limited_by_any_of_its_evidence(tmp_path):
    # Harbor's claude-code log.
    harbor = _trial(tmp_path / "harbor")
    (harbor / "agent" / "claude-code.txt").write_text(
        (FIXTURES / "claude-code.txt").read_text()
    )
    assert trial_usage_limit(harbor)["source"] == "agent/claude-code.txt"
    # Coder One's manifest, which the episode writes on exit 6.
    episode = _trial(tmp_path / "episode")
    (episode / "agent" / "episode").mkdir()
    (episode / "agent" / "episode" / "manifest.json").write_text(
        json.dumps(
            {"outcome": "usage_limited", "usage_limit": {"provider": "anthropic", "resets_at": RESET}}
        )
    )
    found = trial_usage_limit(episode)
    assert found["source"] == "episode manifest"
    assert found["resets_at"] == RESET
    # A delegate stream of an episode from before the exit code existed.
    older = _trial(tmp_path / "older")
    artifacts = older / "agent" / "episode" / "artifacts"
    artifacts.mkdir(parents=True)
    (artifacts / "delegate-1.stream.jsonl").write_text('{"type":"result","is_error":false,"result":"ok"}\n')
    (artifacts / "delegate-3.stream.jsonl").write_text(
        (FIXTURES / "delegate-3.stream.jsonl").read_text()
    )
    assert trial_usage_limit(older)["source"].endswith("delegate-3.stream.jsonl")
    # The adapter's exception alone.
    raised = _trial(tmp_path / "raised")
    result = {
        "exception_info": {
            "exception_type": "UsageLimitError",
            "exception_message": "episode exited 6: a delegate session hit a usage limit",
        }
    }
    assert trial_usage_limit(raised, result)["source"] == "exception UsageLimitError"
    # A clean trial is not limited.
    assert trial_usage_limit(_trial(tmp_path / "clean"), {"verifier_result": {}}) is None


def test_the_attempt_record_withholds_a_limited_trials_reward(tmp_path):
    trial = _trial(tmp_path)
    (trial / "agent" / "claude-code.txt").write_text(
        (FIXTURES / "claude-code.txt").read_text()
    )
    result = {
        "trial_name": trial.name,
        "task_name": "cad-model",
        "exception_info": {"exception_type": "ApiRateLimitError"},
        "verifier_result": {"rewards": {"reward": 0.0}},
    }
    record = attempt_record(
        result,
        job_name="tb4--claude-code-opus--cad-model",
        trial_dir=trial,
        arm="claude-code-opus",
        profile_id="tb4",
        auth_mode=None,
        declared_cost_provenance="cli_list_price",
    )
    outcome = record["outcome"]
    assert outcome["terminal_status"] == "usage_limited"
    assert outcome["reward"] is None
    assert outcome["verifier_rewards"] == {"reward": 0.0}
    assert outcome["usage_limit"]["resets_at"] == RESET


def test_each_arm_names_the_providers_it_draws_on():
    profiles = agents.load_agents()
    assert arm_providers(profiles["claude-code-opus"]) == {"anthropic"}
    assert arm_providers(profiles["codex-gpt-6-luna"]) == {"openai"}
    assert arm_providers(profiles["coder-one-delegate-opus"]) == {"anthropic"}
    assert arm_providers(profiles["coder-one-delegate-luna"]) == {"openai"}
    assert "anthropic" in arm_providers(profiles["coder-one-tunable-v5"])
    assert arm_providers(profiles["coder-one"]) == frozenset()


class _Result:
    def __init__(self, return_code: int):
        self.return_code = return_code
        self.stdout = ""
        self.stderr = ""


class _Environment:
    """Runs the episode as exit 6 and hands back a bundle whose manifest
    records the limit."""

    def __init__(self, manifest: dict):
        self.manifest = manifest

    async def upload_file(self, *_: object) -> None:
        pass

    async def exec(self, command: str, **_: object) -> _Result:
        return _Result(usage_limit.USAGE_LIMIT_EXIT_CODE)

    async def download_dir(self, _source: str, target: Path) -> None:
        target.mkdir(parents=True, exist_ok=True)
        (target / "manifest.json").write_text(json.dumps(self.manifest))


def test_the_adapter_raises_an_infrastructure_error_on_exit_6(tmp_path):
    import hashlib

    from tbench.coder_v05 import OUTCOME_EXIT_CODES, CoderV05

    assert usage_limit.USAGE_LIMIT_EXIT_CODE not in OUTCOME_EXIT_CODES
    binary = tmp_path / "coder-one"
    binary.write_bytes(b"#!/bin/sh\n")
    agent = CoderV05(
        logs_dir=tmp_path / "logs",
        artifact_path=str(binary),
        artifact_sha256=hashlib.sha256(b"#!/bin/sh\n").hexdigest(),
        live_interval_sec=0,
    )
    (tmp_path / "logs").mkdir()
    environment = _Environment(
        {
            "outcome": "usage_limited",
            "usage_limit": {
                "provider": "anthropic",
                "message": "You've hit your session limit · resets 11:50am (UTC)",
                "resets_at_iso": "2026-09-23T11:50:00.000Z",
            },
        }
    )
    with pytest.raises(UsageLimitError, match="resets 2026-09-23T11:50:00"):
        asyncio.run(agent.run("do the task", environment, None))
    assert (tmp_path / "logs" / "episode-exit.txt").read_text() == "exit 6: usage_limited\n"
    # Harbor catches only its own agent-exit errors before grading, so this
    # one ends the trial without a verifier run.
    from harbor.agents.installed.base import NonZeroAgentExitCodeError

    assert not issubclass(UsageLimitError, NonZeroAgentExitCodeError)


def test_the_scoreboard_leaves_limited_trials_out_and_counts_them(tmp_path):
    import importlib.util

    spec = importlib.util.spec_from_file_location(
        "tb4_scoreboard", Path(__file__).parent.parent / "tools" / "tb4_scoreboard.py"
    )
    scoreboard = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(scoreboard)
    jobs = tmp_path / "jobs"
    passed = jobs / "tb4--claude-code-opus--fix-git" / "fix-git__a"
    passed.mkdir(parents=True)
    (passed / "result.json").write_text(
        json.dumps({"verifier_result": {"rewards": {"reward": 1.0}}})
    )
    throttled = jobs / "tb4--claude-code-opus--cad-model" / "cad-model__b"
    (throttled / "agent").mkdir(parents=True)
    (throttled / "agent" / "claude-code.txt").write_text(
        (FIXTURES / "claude-code.txt").read_text()
    )
    (throttled / "result.json").write_text(
        json.dumps(
            {
                "exception_info": {"exception_type": "ApiRateLimitError"},
                "verifier_result": {"rewards": {"reward": 0.0}},
            }
        )
    )
    limited: list[str] = []
    graded = scoreboard.graded(jobs, "claude-code-opus", limited)
    assert list(graded) == ["fix-git"]
    assert limited == [str(throttled)]
