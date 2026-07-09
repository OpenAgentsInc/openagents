import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Tuple

from .artifacts import ArtifactRecorder, redact_text
from .fake_adapter import BenchmarkTimeout
from .schemas import BenchmarkTask, VerifierResult


REPO_DATASETS = {"custom-repo", "swe-bench", "swt-bench"}


def is_repo_dataset(dataset: str) -> bool:
    return dataset in REPO_DATASETS


def verifier_commands(task: BenchmarkTask) -> list:
    commands = task.verifier.get("commands")
    if isinstance(commands, list) and all(isinstance(command, str) for command in commands):
        return commands
    return ["true"]


def _run_command(command: str, cwd: Path, timeout_seconds: int) -> subprocess.CompletedProcess:
    return subprocess.run(
        command,
        cwd=str(cwd),
        shell=True,
        check=False,
        text=True,
        capture_output=True,
        timeout=timeout_seconds,
    )


def _prepare_workspace(task: BenchmarkTask, workspace: Path) -> Path:
    if os.environ.get("OPENAGENTS_BENCH_REPO_DRY_RUN") == "1":
        repo = workspace / "repo"
        repo.mkdir(parents=True, exist_ok=True)
        (repo / "README.md").write_text("# dry-run repo\n", encoding="utf-8")
        return repo

    if not task.repo_url:
        raise ValueError("repo_url is required for non-dry-run repo benchmarks")
    if shutil.which("git") is None:
        raise RuntimeError("git is required for repo benchmark tasks")
    subprocess.run(["git", "clone", task.repo_url, str(workspace / "repo")], check=True)
    repo = workspace / "repo"
    if task.base_commit:
        subprocess.run(["git", "checkout", task.base_commit], cwd=str(repo), check=True)
    return repo


def run_repo_task(
    task: BenchmarkTask,
    recorder: ArtifactRecorder,
    agent_slug: str,
    model: str,
) -> Tuple[str, VerifierResult, int, int]:
    timeout_seconds = int(task.limits.get("timeoutSeconds") or 1800)
    dry_run = os.environ.get("OPENAGENTS_BENCH_REPO_DRY_RUN") == "1"
    recorder.event(
        "repo_task_queued",
        {
            "dataset": task.dataset,
            "taskId": task.id,
            "agent": agent_slug,
            "dryRun": dry_run,
            "repoUrl": task.repo_url or "dry-run",
            "baseCommit": task.base_commit or "none",
        },
    )

    with tempfile.TemporaryDirectory(prefix="oa-bench-repo-") as tmp:
        workspace = Path(tmp)
        repo = _prepare_workspace(task, workspace)
        commands = verifier_commands(task)

        recorder.text(
            "transcript.md",
            "\n".join(
                [
                    "# Repository Benchmark Run",
                    "",
                    "Dataset: %s@%s" % (task.dataset, task.version),
                    "Task: %s" % task.id,
                    "Agent: %s" % agent_slug,
                    "Model: %s" % model,
                    "Repo: %s" % (task.repo_url or "dry-run"),
                    "Base commit: %s" % (task.base_commit or "none"),
                    "",
                    "Instruction:",
                    redact_text(task.instruction),
                    "",
                ]
            ),
        )

        if dry_run:
            (repo / "solution.txt").write_text("dry-run solution\n", encoding="utf-8")
            recorder.text(
                "workspace.diff",
                "diff --git a/solution.txt b/solution.txt\nnew file mode 100644\n--- /dev/null\n+++ b/solution.txt\n@@ -0,0 +1 @@\n+dry-run solution\n",
            )
            recorder.text("patch.diff", (recorder.artifact_dir / "workspace.diff").read_text(encoding="utf-8"))
            recorder.text("verifier_stdout.log", "dry-run verifier passed\n")
            recorder.text("verifier_stderr.log", "")
            recorder.jsonl(
                "commands.jsonl",
                [{"index": index, "command": command, "exitCode": 0} for index, command in enumerate(commands)],
            )
            recorder.json("repo_result.json", {"dryRun": True, "passed": True, "commands": commands})
            recorder.event("repo_task_completed", {"taskId": task.id, "exitCode": 0})
            return "passed", VerifierResult(exit_code=0, passed=True, score=1.0, summary="dry-run repo verifier passed"), 1, len(commands)

        rows = []
        stdout_parts = []
        stderr_parts = []
        for index, command in enumerate(commands):
            try:
                completed = _run_command(command, repo, timeout_seconds)
            except subprocess.TimeoutExpired as exc:
                recorder.text("verifier_stdout.log", exc.stdout or "")
                recorder.text("verifier_stderr.log", exc.stderr or "")
                recorder.event("repo_task_timeout", {"timeoutSeconds": timeout_seconds})
                raise BenchmarkTimeout("repo verifier timed out")
            rows.append({"index": index, "command": command, "exitCode": completed.returncode})
            stdout_parts.append(completed.stdout)
            stderr_parts.append(completed.stderr)
            if completed.returncode != 0:
                break

        status_exit = rows[-1]["exitCode"] if rows else 0
        passed = status_exit == 0
        if shutil.which("git"):
            diff = subprocess.run(["git", "diff", "--binary"], cwd=str(repo), check=False, text=True, capture_output=True).stdout
        else:
            diff = ""
        recorder.jsonl("commands.jsonl", rows)
        recorder.text("workspace.diff", diff)
        recorder.text("patch.diff", diff)
        recorder.text("verifier_stdout.log", "\n".join(stdout_parts))
        recorder.text("verifier_stderr.log", "\n".join(stderr_parts))
        recorder.json("repo_result.json", {"dryRun": False, "passed": passed, "commands": rows})
        recorder.event("repo_task_completed", {"taskId": task.id, "exitCode": status_exit})
        return (
            "passed" if passed else "failed",
            VerifierResult(exit_code=status_exit, passed=passed, score=1.0 if passed else 0.0, summary="repo verifier completed"),
            1,
            len(rows),
        )
