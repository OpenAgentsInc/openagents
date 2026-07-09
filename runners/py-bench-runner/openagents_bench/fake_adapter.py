from typing import Tuple

from .artifacts import ArtifactRecorder, redact_text
from .schemas import BenchmarkTask, VerifierResult


class BenchmarkTimeout(Exception):
    pass


def fake_behavior(task: BenchmarkTask) -> str:
    value = task.metadata.get("fakeBehavior") or task.environment.get("fakeBehavior")
    if isinstance(value, str):
        return value
    if task.id.endswith("timeout"):
        return "timeout"
    if task.id.endswith("exception"):
        return "exception"
    if task.id.endswith("fail"):
        return "fail"
    return "pass"


def run_fake_task(
    task: BenchmarkTask,
    recorder: ArtifactRecorder,
    agent_slug: str,
    model: str,
) -> Tuple[str, VerifierResult, int, int]:
    behavior = fake_behavior(task)
    recorder.event("environment_ready", {"adapter": "fake", "behavior": behavior})

    transcript = [
        "# Benchmark Transcript",
        "",
        "Task: %s" % task.id,
        "Agent: %s" % agent_slug,
        "Model: %s" % model,
        "",
        "Instruction:",
        redact_text(task.instruction),
        "",
    ]
    recorder.jsonl(
        "commands.jsonl",
        [
            {
                "index": 0,
                "command": "fake_prepare_environment",
                "exitCode": 0,
            }
        ],
    )
    recorder.text("agent_stdout.log", "fake agent completed setup\n")
    recorder.text("agent_stderr.log", "")

    if behavior == "timeout":
        recorder.event("task_timeout", {"timeoutSeconds": task.limits.get("timeoutSeconds")})
        transcript.append("Result: simulated timeout")
        recorder.text("transcript.md", "\n".join(transcript) + "\n")
        raise BenchmarkTimeout("simulated benchmark timeout")

    if behavior == "exception":
        recorder.event("task_exception", {"message": "simulated runner exception"})
        transcript.append("Result: simulated exception")
        recorder.text("transcript.md", "\n".join(transcript) + "\n")
        raise RuntimeError("simulated runner exception")

    if behavior == "fail":
        recorder.text("verifier_stdout.log", "fake verifier failed\n")
        recorder.text("verifier_stderr.log", "")
        transcript.append("Result: fake verifier failed")
        recorder.text("transcript.md", "\n".join(transcript) + "\n")
        return (
            "failed",
            VerifierResult(exit_code=1, passed=False, score=0.0, summary="fake verifier failed"),
            1,
            1,
        )

    recorder.text("verifier_stdout.log", "fake verifier passed\n")
    recorder.text("verifier_stderr.log", "")
    transcript.append("Result: fake verifier passed")
    recorder.text("transcript.md", "\n".join(transcript) + "\n")
    return (
        "passed",
        VerifierResult(exit_code=0, passed=True, score=1.0, summary="fake verifier passed"),
        1,
        1,
    )
