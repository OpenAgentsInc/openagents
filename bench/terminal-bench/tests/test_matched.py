"""Protect the matched comparison from executor and accounting drift."""

import asyncio
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace

from tbench.coder_one import CoderOneDelegate
from tbench.matched import EXPERIMENT, MatchedPlain, plain_argv, protocol, request_for


def test_executor_and_harbor_controls_match():
    p = protocol()
    policy = json.loads((EXPERIMENT / "coder-policy.json").read_text())["policy"]
    executor = policy["executor"]
    args = plain_argv("claude", "system.txt")
    for flag, field in (("--model", "model"), ("--effort", "effort"), ("--tools", "tools")):
        assert args[args.index(flag) + 1] == executor[field] == p[field]
    assert executor["version"] == p["claude_version"]
    assert executor["prompt_cache_ttl"] == p["cache_ttl"]
    assert "route" not in policy["control"] and "handoff" not in policy["control"]
    a = request_for("plain", p["tasks"][0], 1, Path("/unused"))
    b = request_for("coder", p["tasks"][0], 1, Path("/unused"))
    assert a.tasks == b.tasks and a.profile == b.profile
    assert a.agent.kwargs == b.agent.kwargs
    assert a.agent.env_forward == b.agent.env_forward
    assert a.agent.extra_allowed_hosts == b.agent.extra_allowed_hosts


def test_plain_runs_claude_directly_and_retains_no_token(tmp_path, monkeypatch):
    binary = tmp_path / "binary"
    binary.write_bytes(b"fixture")
    agent = MatchedPlain(logs_dir=tmp_path, artifact_path=str(binary),
                         artifact_sha256=hashlib.sha256(b"fixture").hexdigest(),
                         policy=str(EXPERIMENT / "coder-policy.json"))
    agent._claude_bin = "/usr/local/bin/claude"
    monkeypatch.setenv("CLAUDE_CODE_OAUTH_TOKEN", "secret-must-stay-in-memory")
    calls = []

    class Environment:
        async def exec(self, **kwargs):
            calls.append(kwargs)
            return SimpleNamespace(return_code=0, stdout="/root")

        async def upload_file(self, *args):
            pass

        async def download_file(self, source, target):
            Path(target).write_text("")

        async def download_dir(self, source, target):
            # Docker compose cp requires the destination's parent.
            assert Path(target).parent.is_dir()

    asyncio.run(agent.run("Solve the task.", Environment(), None))
    invocation = next(c for c in calls if "stream.jsonl" in c["command"])
    assert "coder-one episode" not in invocation["command"]
    assert "--effort medium" in invocation["command"]
    assert invocation["timeout_sec"] == 1680
    assert invocation["env"]["CLAUDE_CODE_OAUTH_TOKEN"] == "secret-must-stay-in-memory"
    assert all("secret-must-stay-in-memory" not in f.read_text()
               for f in tmp_path.glob("*.txt"))
    assert MatchedPlain.install == CoderOneDelegate.install


def test_cost_comes_from_cli_result(tmp_path):
    binary = tmp_path / "binary"
    binary.write_bytes(b"fixture")
    agent = MatchedPlain(logs_dir=tmp_path, artifact_path=str(binary),
                         artifact_sha256=hashlib.sha256(b"fixture").hexdigest(),
                         policy=str(EXPERIMENT / "coder-policy.json"))
    (tmp_path / "claude-code.txt").write_text(json.dumps({"type": "result",
        "total_cost_usd": 1.23, "usage": {"input_tokens": 12,
        "cache_read_input_tokens": 34, "output_tokens": 56}}) + "\n")
    context = SimpleNamespace()
    agent.populate_context_post_run(context)
    assert context.cost_usd == 1.23
    assert context.n_input_tokens == 12 and context.n_cache_tokens == 34
    assert context.n_output_tokens == 56
