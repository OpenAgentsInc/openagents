import json
import os
import shutil
import subprocess
from typing import Optional, Tuple

from .artifacts import ArtifactRecorder, redact_text
from .fake_adapter import BenchmarkTimeout
from .schemas import BenchmarkTask, VerifierResult
from .signature_routing import PROBE_CODEX_AGENT_SLUGS, signature_prompt_addendum


CODEX_AGENT_SLUGS = {"codex", "openagents-codex", "openagents-coder", *PROBE_CODEX_AGENT_SLUGS}


def is_codex_agent(agent_slug: str) -> bool:
    return agent_slug in CODEX_AGENT_SLUGS


def codex_command(task: BenchmarkTask, agent_slug: str = "codex") -> list:
    sandbox = str(task.metadata.get("sandbox") or "danger-full-access")
    signature_addendum = signature_prompt_addendum(task, agent_slug)
    prompt_lines = [
        "You are running a bounded OpenAgents Benchmark Cloud task.",
        "Dataset: %s@%s" % (task.dataset, task.version),
        "Task id: %s" % task.id,
        "",
        redact_text(task.instruction),
    ]
    if signature_addendum:
        prompt_lines.extend(["", signature_addendum])
    prompt_lines.extend(["", "Do not print credentials, tokens, or auth files. Record useful steps and final answer only."])
    prompt = "\n".join(prompt_lines)
    return [
        "codex",
        "exec",
        "--skip-git-repo-check",
        "--json",
        "--sandbox",
        sandbox,
        prompt,
    ]


def _score(exit_code: int, dry_run: bool) -> Tuple[str, VerifierResult]:
    passed = exit_code == 0
    status = "passed" if passed else "failed"
    summary = "dry-run Codex adapter passed" if dry_run else "Codex adapter exited with code %s" % exit_code
    return status, VerifierResult(exit_code=exit_code, passed=passed, score=1.0 if passed else 0.0, summary=summary)


def run_codex_task(
    task: BenchmarkTask,
    recorder: ArtifactRecorder,
    agent_slug: str,
    model: str,
) -> Tuple[str, VerifierResult, int, int]:
    command = codex_command(task, agent_slug)
    timeout_seconds: Optional[int] = None
    if isinstance(task.limits.get("timeoutSeconds"), int):
        timeout_seconds = int(task.limits["timeoutSeconds"])
    dry_run = os.environ.get("OPENAGENTS_BENCH_CODEX_DRY_RUN") == "1"

    recorder.event(
        "openagents_codex_queued",
        {
            "taskId": task.id,
            "agent": agent_slug,
            "model": model,
            "dryRun": dry_run,
            "sandbox": task.metadata.get("sandbox") or "danger-full-access",
            "signatureContextEnabled": bool(signature_prompt_addendum(task, agent_slug)),
        },
    )
    recorder.jsonl(
        "commands.jsonl",
        [
            {
                "index": 0,
                "command": "codex exec --skip-git-repo-check --json --sandbox <profile> <prompt>",
                "exitCode": None,
            }
        ],
    )

    if dry_run:
        stdout = json.dumps({"type": "codex_dry_run", "status": "completed", "taskId": task.id}) + "\n"
        stderr = ""
        exit_code = 0
    else:
        if shutil.which(command[0]) is None:
            raise RuntimeError("Codex executable not found: %s" % command[0])
        try:
            completed = subprocess.run(command, check=False, text=True, capture_output=True, timeout=timeout_seconds)
        except subprocess.TimeoutExpired as exc:
            recorder.text("codex_stdout.jsonl", exc.stdout or "")
            recorder.text("codex_stderr.log", exc.stderr or "")
            recorder.event("openagents_codex_timeout", {"timeoutSeconds": timeout_seconds})
            raise BenchmarkTimeout("OpenAgents/Codex benchmark task timed out")
        stdout = completed.stdout
        stderr = completed.stderr
        exit_code = completed.returncode

    recorder.text("codex_stdout.jsonl", stdout)
    recorder.text("codex_stderr.log", stderr)
    recorder.text("agent_stdout.log", stdout)
    recorder.text("agent_stderr.log", stderr)
    recorder.text("verifier_stdout.log", "adapter smoke verifier exitCode=%s\n" % exit_code)
    recorder.text("verifier_stderr.log", "")
    recorder.text("workspace.diff", "")
    recorder.text(
        "transcript.md",
        "\n".join(
            [
                "# OpenAgents/Codex Benchmark Run",
                "",
                "Dataset: %s@%s" % (task.dataset, task.version),
                "Task: %s" % task.id,
                "Agent: %s" % agent_slug,
                "Model: %s" % model,
                "",
                "Instruction:",
                redact_text(task.instruction),
                "",
                "Codex mode: %s" % ("dry-run" if dry_run else "codex exec"),
                "",
                signature_prompt_addendum(task, agent_slug),
                "",
            ]
        ),
    )
    recorder.json(
        "codex_result.json",
        {
            "taskId": task.id,
            "agent": agent_slug,
            "model": model,
            "exitCode": exit_code,
            "dryRun": dry_run,
            "signatureContextEnabled": bool(signature_prompt_addendum(task, agent_slug)),
        },
    )
    recorder.event("openagents_codex_completed", {"taskId": task.id, "exitCode": exit_code})
    status, verifier = _score(exit_code, dry_run)
    return status, verifier, 1, 1
