"""The runner's failure handling: refusals, interrupts, resume, and holds.

Each test stands in for a case observed on a real Harbor trial and
recorded in docs/terminal-bench/resilience.md.
"""

import hashlib
import json
import os
import signal
import sys
import threading
import time

import pytest

from tbench.agents import load_agents
from tbench.jobconfig import load_job_profile
from tbench.panel import load_panel
from tbench.runner import (
    RunError,
    RunRefused,
    RunRequest,
    collect,
    held_path,
    materialize,
    preserve_interrupted,
    run,
    run_harbor,
    tbench_held_aside,
)


def _request(tmp_path, agent="coder-v05", kwargs=None, job="resilience--t"):
    panel = load_panel()
    return RunRequest(
        panel=panel,
        profile=load_job_profile("smoke"),
        agent=load_agents()[agent],
        tasks=panel.select(["fix-git"]),
        agent_kwargs=kwargs,
        jobs_dir=tmp_path,
        job_name=job,
    )


def _artifact(tmp_path, payload=b"#!/bin/sh\n"):
    path = tmp_path / "artifact"
    path.write_bytes(payload)
    return str(path), hashlib.sha256(payload).hexdigest()


@pytest.fixture
def door_env(monkeypatch):
    monkeypatch.setenv("OPENAGENTS_API_KEY", "test-placeholder")
    monkeypatch.setenv("OPENAGENTS_DOOR_URL", "http://127.0.0.1:9")


def _refusals(tmp_path, job="resilience--t"):
    return sorted((tmp_path / job / "tbench" / "refusals").glob("*.json"))


def test_missing_credentials_leave_a_refusal_record(tmp_path, monkeypatch):
    monkeypatch.delenv("OPENAGENTS_API_KEY", raising=False)
    monkeypatch.delenv("OPENAGENTS_DOOR_URL", raising=False)
    path, digest = _artifact(tmp_path)
    request = _request(
        tmp_path, kwargs={"artifact_path": path, "artifact_sha256": digest}
    )
    with pytest.raises(RunRefused) as refused:
        run(request, harbor_argv0="/nonexistent/harbor")
    assert refused.value.stage == "credentials"
    [record_path] = _refusals(tmp_path)
    record = json.loads(record_path.read_text())
    assert record["schema"] == "openagents.tbench.refusal.v1"
    assert record["terminal_status"] == "setup_failure"
    assert record["stage"] == "credentials"
    assert "door-key needs OPENAGENTS_API_KEY" in record["reason"]
    assert "test-placeholder" not in record_path.read_text()
    # Nothing but the refusal exists: no config, no trial.
    assert not (tmp_path / "resilience--t" / "tbench" / "job-config.json").exists()


def test_digest_mismatch_refused_before_harbor(tmp_path, door_env):
    path, _ = _artifact(tmp_path)
    request = _request(
        tmp_path, kwargs={"artifact_path": path, "artifact_sha256": "00" * 32}
    )
    with pytest.raises(RunRefused, match="doesn't match the pin"):
        run(request, harbor_argv0="/nonexistent/harbor")
    [record_path] = _refusals(tmp_path)
    assert json.loads(record_path.read_text())["stage"] == "artifact"


def test_missing_artifact_refused_before_harbor(tmp_path, door_env):
    request = _request(
        tmp_path,
        kwargs={
            "artifact_path": str(tmp_path / "nope"),
            "artifact_sha256": "ab" * 32,
        },
    )
    with pytest.raises(RunRefused, match="no fallback binary"):
        run(request, harbor_argv0="/nonexistent/harbor")


def test_materialize_alone_records_no_refusal(tmp_path, monkeypatch):
    monkeypatch.delenv("OPENAGENTS_API_KEY", raising=False)
    monkeypatch.delenv("OPENAGENTS_DOOR_URL", raising=False)
    path, digest = _artifact(tmp_path)
    request = _request(
        tmp_path, kwargs={"artifact_path": path, "artifact_sha256": digest}
    )
    with pytest.raises(RunRefused):
        materialize(request)
    assert _refusals(tmp_path) == []


def _trial(job_dir, name, exception_type=None, result=True):
    trial = job_dir / name
    (trial / "agent" / "episode").mkdir(parents=True)
    (trial / "agent" / "episode" / "manifest.json").write_text("{}")
    (trial / "config.json").write_text(
        json.dumps({"task": {"path": "/x/archive/fix-git"}, "trial_name": name})
    )
    (trial / "trial.log").write_text("")
    if result:
        (trial / "result.json").write_text(
            json.dumps(
                {
                    "id": f"id-{name}",
                    "trial_name": name,
                    "task_name": "terminal-bench/fix-git",
                    "config": {"agent": {"kwargs": {}}},
                    "exception_info": (
                        {"exception_type": exception_type}
                        if exception_type
                        else None
                    ),
                    "verifier_result": (
                        None if exception_type else {"rewards": {"reward": 1.0}}
                    ),
                }
            )
        )
    return trial


def test_resume_preserves_what_harbor_would_delete(tmp_path):
    job_dir = tmp_path / "job"
    _trial(job_dir, "fix-git__done")
    _trial(job_dir, "fix-git__cancel", exception_type="CancelledError")
    _trial(job_dir, "fix-git__timeout", exception_type="EpisodeTimeoutError")
    _trial(job_dir, "fix-git__killed", result=False)
    kept = preserve_interrupted(job_dir)
    assert sorted(p.name for p in kept) == ["fix-git__cancel", "fix-git__killed"]
    interrupted = job_dir / "tbench" / "interrupted"
    assert (interrupted / "fix-git__cancel" / "agent" / "episode" / "manifest.json").exists()
    # Idempotent: a second pass copies nothing new and loses nothing.
    assert len(preserve_interrupted(job_dir)) == 2


def test_collect_keeps_interrupted_and_unfinished_attempts(tmp_path):
    job_dir = tmp_path / "job"
    _trial(job_dir, "fix-git__done")
    _trial(job_dir, "fix-git__cancel", exception_type="CancelledError")
    _trial(job_dir, "fix-git__killed", result=False)
    preserve_interrupted(job_dir)
    # What `harbor job resume` does next: remove both, rerun one.
    import shutil

    shutil.rmtree(job_dir / "fix-git__cancel")
    shutil.rmtree(job_dir / "fix-git__killed")
    _trial(job_dir, "fix-git__rerun")
    collect(job_dir, _request(tmp_path))
    records = {
        p.stem: json.loads(p.read_text())
        for p in (job_dir / "tbench" / "attempts").glob("*.json")
    }
    assert set(records) == {
        "fix-git__done",
        "fix-git__rerun",
        "fix-git__cancel",
        "fix-git__killed",
    }
    assert records["fix-git__done"]["attempt"]["kind"] == "fresh"
    assert records["fix-git__cancel"]["attempt"]["kind"] == "interrupted"
    assert records["fix-git__cancel"]["outcome"]["terminal_status"] == "cancelled"
    assert records["fix-git__killed"]["attempt"]["kind"] == "interrupted"
    assert records["fix-git__killed"]["outcome"]["terminal_status"] == "unknown"
    assert records["fix-git__killed"]["evidence"]["trial_result"] is None


def test_collect_records_a_failed_bundle_collection(tmp_path):
    job_dir = tmp_path / "job"
    trial = _trial(job_dir, "fix-git__nocollect")
    (trial / "agent" / "episode" / "manifest.json").unlink()
    (trial / "agent" / "episode").rmdir()
    (trial / "agent" / "episode-collection-failed.txt").write_text("cp failed\n")
    collect(job_dir, _request(tmp_path))
    record = json.loads(
        (job_dir / "tbench" / "attempts" / "fix-git__nocollect.json").read_text()
    )
    assert record["completeness"]["bundle"] == "collection_failed"
    assert record["completeness"]["trace"] == "absent"
    manifest = json.loads(
        (job_dir / "tbench" / "manifests" / "fix-git__nocollect.json").read_text()
    )
    assert manifest["evidence"]["collection_failure"]["resolved"] is True


def test_held_aside_survives_harbor_deleting_unfinished_dirs(tmp_path):
    job_dir = tmp_path / "job"
    (job_dir / "tbench" / "attempts").mkdir(parents=True)
    (job_dir / "tbench" / "attempts" / "a.json").write_text("{}")
    with tbench_held_aside(job_dir) as held:
        assert not (job_dir / "tbench").exists()
        assert (held / "attempts" / "a.json").exists()
        assert held.parent.parent == tmp_path
        assert held.name != "tbench"
    assert (job_dir / "tbench" / "attempts" / "a.json").exists()
    assert not held_path(job_dir).parent.exists()


def test_held_aside_restores_a_crashed_hold(tmp_path):
    job_dir = tmp_path / "job"
    held = held_path(job_dir)
    (held / "attempts").mkdir(parents=True)
    (held / "attempts" / "a.json").write_text("{}")
    job_dir.mkdir()
    (job_dir / "tbench").mkdir()
    with pytest.raises(RunError, match="Merge them by hand"):
        with tbench_held_aside(job_dir):
            pass
    (job_dir / "tbench").rmdir()
    with tbench_held_aside(job_dir):
        pass
    assert (job_dir / "tbench" / "attempts" / "a.json").exists()


FAKE_HARBOR = """
import os, signal, sys, time
from pathlib import Path
job = Path(sys.argv[-1])
# What Harbor does to an existing job dir: delete every subdirectory
# without a result.json as an unfinished trial, and, on resume, every
# cancelled trial.
import json, shutil
for child in job.iterdir():
    if not child.is_dir():
        continue
    result = child / "result.json"
    if not result.exists():
        shutil.rmtree(child)
    elif "CancelledError" in result.read_text():
        shutil.rmtree(child)
marker = Path(os.environ["FAKE_HARBOR_MARKER"])
def on_int(signum, frame):
    with marker.open("a") as f:
        f.write("SIGINT\\n")
    time.sleep(0.5)  # Harbor stopping its environments.
    sys.exit(130)
signal.signal(signal.SIGINT, on_int)
marker.write_text("")
for _ in range(200):
    time.sleep(0.05)
    if os.environ.get("FAKE_HARBOR_QUICK"):
        break
"""


def _fake_harbor(tmp_path):
    script = tmp_path / "fake_harbor.py"
    script.write_text(FAKE_HARBOR)
    launcher = tmp_path / "harbor"
    launcher.write_text(f"#!/bin/sh\nexec {sys.executable} {script} \"$@\"\n")
    launcher.chmod(0o755)
    return str(launcher)


def test_resume_keeps_tbench_when_harbor_clears_the_job_dir(
    tmp_path, door_env, monkeypatch
):
    monkeypatch.setenv("FAKE_HARBOR_MARKER", str(tmp_path / "marker"))
    monkeypatch.setenv("FAKE_HARBOR_QUICK", "1")
    path, digest = _artifact(tmp_path)
    request = _request(
        tmp_path, kwargs={"artifact_path": path, "artifact_sha256": digest}
    )
    job_dir = tmp_path / "resilience--t"
    _trial(job_dir, "fix-git__cancel", exception_type="CancelledError")
    from tbench.runner import resume

    resume(request, harbor_argv0=_fake_harbor(tmp_path))
    assert (job_dir / "tbench" / "job-config.json").exists()
    assert (job_dir / "tbench" / "interrupted" / "fix-git__cancel").is_dir()
    record = json.loads(
        (job_dir / "tbench" / "attempts" / "fix-git__cancel.json").read_text()
    )
    assert record["attempt"]["kind"] == "interrupted"


def test_interrupt_is_forwarded_once_and_waited_for(tmp_path, monkeypatch):
    marker = tmp_path / "marker"
    monkeypatch.setenv("FAKE_HARBOR_MARKER", str(marker))
    monkeypatch.delenv("FAKE_HARBOR_QUICK", raising=False)
    job = tmp_path / "job"
    job.mkdir()
    command = [_fake_harbor(tmp_path), str(job)]

    def interrupt_twice():
        for _ in range(100):
            if marker.exists():
                break
            time.sleep(0.02)
        os.kill(os.getpid(), signal.SIGINT)
        time.sleep(0.1)
        os.kill(os.getpid(), signal.SIGTERM)

    thread = threading.Thread(target=interrupt_twice)
    thread.start()
    returncode, got = run_harbor(command)
    thread.join()
    assert got == "SIGINT"
    assert returncode == 130
    assert marker.read_text() == "SIGINT\n"
    # The handlers are back to what they were.
    assert signal.getsignal(signal.SIGINT) is signal.default_int_handler
