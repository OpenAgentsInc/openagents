"""Run the frozen 2026-09-23 matched-executor development comparison.

The plain arm shares the Coder adapter's installation and doctor, then
invokes Claude directly. It never starts a Coder episode. Native sessions,
the stream, the exact prompt, and the invocation record are retained.
"""

from __future__ import annotations

import argparse
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import time

from harbor.agents.installed.claude_code import ClaudeCode

from . import paths, runner, suite
from .cli import _load
from .coder_one import CoderOneDelegate
from .jobconfig import write_job_config

EXPERIMENT = paths.PACKAGE_DIR / "experiments/2026-09-23-matched-opus"


def protocol() -> dict:
    return json.loads((EXPERIMENT / "protocol.json").read_text())


def plain_argv(binary: str, system: str) -> list[str]:
    p = protocol()
    return [binary, "-p", "--output-format", "stream-json", "--verbose",
            "--model", p["model"], "--permission-mode", "bypassPermissions",
            "--tools", p["tools"], "--effort", p["effort"],
            "--system-prompt-file", system]


class MatchedPlain(CoderOneDelegate):
    """Install the same executables, then run Claude without the controller."""

    @staticmethod
    def name() -> str:
        return "matched-plain-opus"

    async def run(self, instruction, environment, context) -> None:
        p = protocol()
        rendered = self.render_instruction(instruction)
        local = self.logs_dir / "instruction.txt"
        local.write_text(rendered)
        system = self.logs_dir / "system-prompt.txt"
        system.write_bytes((EXPERIMENT / "system-prompt.md").read_bytes())
        remote = "/opt/openagents/matched"
        await environment.exec(command=f"mkdir -p {remote}")
        await environment.upload_file(local, remote + "/instruction.txt")
        await environment.upload_file(system, remote + "/system.txt")
        argv = plain_argv(self._claude_bin, remote + "/system.txt")
        env = {
            "CLAUDE_CODE_OAUTH_TOKEN": self._get_env("CLAUDE_CODE_OAUTH_TOKEN"),
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
            "PYTHONDONTWRITEBYTECODE": "1", "IS_SANDBOX": "1",
            "CLAUDE_CODE_PROMPT_CACHE_TTL": p["cache_ttl"],
            "BASH_MAX_TIMEOUT_MS": "3600000",
        }
        record = {"argv": argv, "environment": {k: v for k, v in env.items()
                  if k != "CLAUDE_CODE_OAUTH_TOKEN"},
                  "credential_source": "CLAUDE_CODE_OAUTH_TOKEN",
                  "timeout_sec": p["episode_allowance_sec"],
                  "system_sha256": hashlib.sha256(system.read_bytes()).hexdigest(),
                  "instruction_sha256": hashlib.sha256(local.read_bytes()).hexdigest()}
        (self.logs_dir / "invocation.txt").write_text(json.dumps(record, indent=2) + "\n")
        command = ("unset CLAUDECODE ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN; exec "
                   + shlex.join(argv) + f" < {remote}/instruction.txt"
                   + f" > {remote}/stream.jsonl 2> {remote}/stderr.txt")
        try:
            result = await environment.exec(command=command, env=env,
                                           timeout_sec=p["episode_allowance_sec"])
            (self.logs_dir / "exit.txt").write_text(str(result.return_code) + "\n")
        finally:
            await environment.download_file(remote + "/stream.jsonl", self.logs_dir / "claude-code.txt")
            await environment.download_file(remote + "/stderr.txt", self.logs_dir / "stderr.txt")
            home = await environment.exec(command='printf "%s" "$HOME"')
            (self.logs_dir / "sessions").mkdir(parents=True, exist_ok=True)
            await environment.download_dir(home.stdout.strip() + "/.claude/projects",
                                           self.logs_dir / "sessions/projects")

    def populate_context_post_run(self, context) -> None:
        # Use Harbor's pinned native-session converter. The CLI result is
        # authoritative for cost; don't replace it with a pricing estimate.
        converter = ClaudeCode(logs_dir=self.logs_dir, model_name=protocol()["model"],
                               version=protocol()["claude_version"])
        converter.populate_context_post_run(context)
        stream = self.logs_dir / "claude-code.txt"
        if not stream.exists():
            return
        for line in stream.read_text().splitlines():
            try:
                item = json.loads(line)
            except ValueError:
                continue
            if item.get("type") == "result":
                usage = item.get("usage") or {}
                context.cost_usd = item.get("total_cost_usd")
                # Harbor counts all prompt tokens here, including cache
                # reads and writes; the native stream keeps each category.
                uncached = usage.get("input_tokens")
                context.n_input_tokens = None if uncached is None else (
                    uncached + (usage.get("cache_read_input_tokens") or 0)
                    + (usage.get("cache_creation_input_tokens") or 0))
                context.n_cache_tokens = usage.get("cache_read_input_tokens")
                context.n_output_tokens = usage.get("output_tokens")


def request_for(arm: str, task: str, repetition: int, artifact: Path):
    request = _load("tb4", "coder-one-tunable-v2")
    request.tasks = request.panel.select([task])
    request.profile = replace(request.profile, n_attempts=1, n_concurrent_trials=1,
                              retry={"max_retries": 0})
    request.agent = replace(request.agent, id=f"matched-{arm}-opus",
        role="candidate" if arm == "coder" else "external-baseline",
        harbor_import_path=("tbench.coder_one:CoderOneDelegate" if arm == "coder"
                            else "tbench.matched:MatchedPlain"),
        kwargs={"policy": str(EXPERIMENT / "coder-policy.json"),
                "artifact_path": str(artifact),
                "artifact_sha256": protocol()["artifact_sha256"]})
    request.auth_mode = "subscription-oauth"
    request.job_name = f"{protocol()['id']}--{task}--r{repetition}--{arm}"
    return request


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--run", action="store_true", help="execute the frozen schedule")
    args = parser.parse_args()
    p = protocol()
    # Host credentials stay in memory and are passed through Harbor's
    # existing redaction path. A setup token avoids refreshing active jobs.
    home = Path.home()
    os.environ["OPENAGENTS_API_KEY"] = (home / ".openagents/bearer").read_text().strip()
    os.environ["TYPESAFE_API_KEY"] = json.loads((home / ".openagents/jev.json").read_text())["api_key"]
    setup_token = home / ".openagents/claude-setup-token"
    if setup_token.exists():
        token = setup_token.read_text().strip()
    else:
        # Reuse the active login; refreshing it would revoke other jobs.
        oauth = json.loads((home / ".claude/.credentials.json").read_text())["claudeAiOauth"]
        if oauth["expiresAt"] / 1000 <= time.time():
            raise RuntimeError("Claude login expired; do not refresh while other jobs are active.")
        token = oauth["accessToken"]
    os.environ["CLAUDE_CODE_OAUTH_TOKEN"] = token
    os.environ["CODEX_AUTH_JSON_PATH"] = str(home / ".codex/auth.json")
    for key in ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"):
        os.environ.pop(key, None)
    frozen = paths.state_dir() / "experiments" / p["id"]
    frozen.mkdir(parents=True, exist_ok=True)
    pins = {name: hashlib.sha256((EXPERIMENT / name).read_bytes()).hexdigest()
            for name in ("protocol.json", "coder-policy.json", "system-prompt.md")}
    pins["matched.py"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    pin_path = frozen / "pins.json"
    if pin_path.exists() and json.loads(pin_path.read_text()) != pins:
        raise RuntimeError("The frozen protocol or runner changed; use a new experiment ID.")
    write_job_config(pins, pin_path)
    for repetition, order in enumerate(p["orders"], 1):
        for task in p["tasks"]:
            for arm in order:
                request = request_for(arm, task, repetition, args.artifact)
                job_dir, config = runner.materialize(request, record_refusal=True)
                config["agents"][0]["override_timeout_sec"] = p["harbor_agent_timeout_sec"]
                write_job_config(config, runner.TrialPaths(job_dir).config_path)
                write_job_config(config, frozen / (request.job_name + ".json"))
                print(json.dumps({"job": request.job_name, "state": "staged"}), flush=True)
                if not args.run or (job_dir / "result.json").exists():
                    continue
                slot = None
                while slot is None:
                    if shutil.disk_usage(home).free / 2**30 >= p["min_free_disk_gib"]:
                        slot = suite.acquire_claude_slot(suite.claude_slot_dir(), request.job_name,
                                                         p["max_claude_concurrent"])
                    if slot is None:
                        time.sleep(10)
                try:
                    print(json.dumps({"job": request.job_name, "state": "running"}), flush=True)
                    with runner.tbench_held_aside(job_dir) as held:
                        command = ["harbor", "run", "--config",
                                   str(held / runner.TrialPaths(job_dir).config_path.name),
                                   "--jobs-dir", str(config["jobs_dir"]),
                                   "--job-name", request.job_name, "--yes"]
                        code, signal = runner.run_harbor(command)
                    runner._finish(job_dir, request, "matched run", code, signal)
                finally:
                    os.close(slot)
                # Provider limits are operational outcomes, never task failures.
                for trial in runner.trial_dirs(job_dir):
                    from .usage_limit import trial_usage_limit
                    if trial_usage_limit(trial):
                        raise RuntimeError("Provider usage limit; remaining schedule stopped.")


if __name__ == "__main__":
    main()
