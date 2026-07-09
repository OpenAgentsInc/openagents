import json
import os
import shutil
import subprocess
from typing import Optional, Tuple

from .artifacts import ArtifactRecorder, redact_text
from .codex_adapter import is_codex_agent, run_codex_task
from .fake_adapter import BenchmarkTimeout
from .schemas import BenchmarkTask, VerifierResult


def terminal_bench_task_id(task: BenchmarkTask) -> str:
    value = task.metadata.get("terminalBenchTaskId") or task.id
    if not isinstance(value, str) or not value:
        raise ValueError("Terminal-Bench task id is required")
    return value


def harbor_dataset(task: BenchmarkTask) -> str:
    return "terminal-bench@%s" % task.version


def harbor_command(task: BenchmarkTask, agent_slug: str) -> list:
    return [
        "harbor",
        "run",
        "--dataset",
        harbor_dataset(task),
        "--agent",
        agent_slug,
        "--n-concurrent",
        "1",
        "--task-id",
        terminal_bench_task_id(task),
    ]


def _score_from_harbor_stdout(stdout: str, exit_code: int) -> Tuple[str, VerifierResult]:
    status = "passed" if exit_code == 0 else "failed"
    passed = exit_code == 0
    summary = "Harbor exited with code %s" % exit_code

    for line in reversed(stdout.splitlines()):
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            raw_passed = payload.get("passed")
            if isinstance(raw_passed, bool):
                passed = raw_passed
                status = "passed" if passed else "failed"
            raw_score = payload.get("score")
            score = float(raw_score) if isinstance(raw_score, (int, float)) else (1.0 if passed else 0.0)
            summary = str(payload.get("summary") or summary)
            return status, VerifierResult(exit_code=exit_code, passed=passed, score=score, summary=summary)

    return status, VerifierResult(exit_code=exit_code, passed=passed, score=1.0 if passed else 0.0, summary=summary)


def run_terminal_bench_task(
    task: BenchmarkTask,
    recorder: ArtifactRecorder,
    agent_slug: str,
    model: str,
) -> Tuple[str, VerifierResult, int, int]:
    if is_codex_agent(agent_slug):
        return run_codex_task(task, recorder, agent_slug, model)

    task_id = terminal_bench_task_id(task)
    command = harbor_command(task, agent_slug)
    timeout_seconds: Optional[int] = None
    if isinstance(task.limits.get("timeoutSeconds"), int):
        timeout_seconds = int(task.limits["timeoutSeconds"])

    recorder.event(
        "terminal_bench_harbor_queued",
        {
            "dataset": harbor_dataset(task),
            "taskId": task_id,
            "agent": agent_slug,
            "dryRun": os.environ.get("OPENAGENTS_BENCH_HARBOR_DRY_RUN") == "1",
        },
    )
    recorder.jsonl(
        "commands.jsonl",
        [
            {
                "index": 0,
                "command": " ".join(command),
                "exitCode": None,
            }
        ],
    )

    if os.environ.get("OPENAGENTS_BENCH_HARBOR_DRY_RUN") == "1":
        stdout = json.dumps({"passed": True, "score": 1.0, "summary": "dry-run Harbor oracle passed"}) + "\n"
        stderr = ""
        exit_code = 0
    else:
        if shutil.which(command[0]) is None:
            raise RuntimeError("Harbor executable not found: %s" % command[0])
        try:
            completed = subprocess.run(command, check=False, text=True, capture_output=True, timeout=timeout_seconds)
        except subprocess.TimeoutExpired as exc:
            recorder.text("harbor_stdout.log", exc.stdout or "")
            recorder.text("harbor_stderr.log", exc.stderr or "")
            recorder.event("terminal_bench_timeout", {"timeoutSeconds": timeout_seconds})
            raise BenchmarkTimeout("Terminal-Bench Harbor task timed out")
        stdout = completed.stdout
        stderr = completed.stderr
        exit_code = completed.returncode

    recorder.text("harbor_stdout.log", stdout)
    recorder.text("harbor_stderr.log", stderr)
    recorder.text("verifier_stdout.log", stdout)
    recorder.text("verifier_stderr.log", stderr)
    recorder.text(
        "transcript.md",
        "\n".join(
            [
                "# Terminal-Bench Harbor Run",
                "",
                "Dataset: %s" % harbor_dataset(task),
                "Task: %s" % task_id,
                "Agent: %s" % agent_slug,
                "Model: %s" % model,
                "",
                "Instruction:",
                redact_text(task.instruction),
                "",
                "Harbor command:",
                " ".join(command),
                "",
            ]
        ),
    )
    recorder.json(
        "raw_harbor_result.json",
        {
            "dataset": harbor_dataset(task),
            "taskId": task_id,
            "agent": agent_slug,
            "exitCode": exit_code,
        },
    )
    recorder.event("terminal_bench_harbor_completed", {"taskId": task_id, "exitCode": exit_code})

    status, verifier = _score_from_harbor_stdout(stdout, exit_code)
    return status, verifier, 1, 1
