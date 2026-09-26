#!/usr/bin/env python3
"""Run an opt-in synthetic launchd acceptance with a supplied public Coder build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid

SCRIPT = Path(__file__).with_name("coder-host.py").resolve()
ENV = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "TMPDIR": "/tmp"}


def run(argv, allowed=(0,), cwd=None):
    result = subprocess.run(list(map(str, argv)), env=ENV, cwd=cwd, capture_output=True, timeout=30)
    if result.returncode not in allowed:
        raise RuntimeError(f"{argv[0]} returned {result.returncode}: {result.stderr.decode()[:2000]}")
    return result.stdout.decode()


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    if os.uname().sysname != "Darwin":
        raise RuntimeError("this runtime acceptance uses macOS launchd")
    root = Path(args.output).absolute()
    root.mkdir(mode=0o700)
    tasks = root / "tasks"
    home = root / "bundle"
    base = ["/usr/bin/python3", SCRIPT, "--root", home, "--tasks", tasks]
    binary = Path(args.binary).resolve()
    install = ["install", "--binary", binary, "--sha256", sha(binary), "--source-revision", args.source_revision, "--uncommitted-source"]
    first = json.loads(run([*base, *install]))
    doctor = json.loads(run([*base, "doctor"]))
    # A stripped, freshly ad-hoc-signed copy is a second real binary bundle,
    # not a claim of a different source revision or production release.
    second = root / "coder-stripped"
    run(["/usr/bin/strip", "-x", "-o", second, binary])
    run(["/usr/bin/codesign", "--force", "--sign", "-", second])
    upgrade = json.loads(run([*base, "install", "--binary", second, "--sha256", sha(second), "--source-revision", args.source_revision, "--uncommitted-source"]))
    repo = root / "repo"
    repo.mkdir()
    run(["/usr/bin/git", "init", "-q"], cwd=repo)
    (repo / "README.md").write_text("Public synthetic service fixture.\n")
    run(["/usr/bin/git", "add", "README.md"], cwd=repo)
    run(["/usr/bin/git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "Synthetic source"], cwd=repo)
    source = run(["/usr/bin/git", "rev-parse", "HEAD"], cwd=repo).strip()
    workspace = root / "workspace"
    run(["/usr/bin/git", "worktree", "add", "--detach", "-q", workspace], cwd=repo)
    repo = workspace.resolve()
    observations = []
    def one_task(name):
        current = json.loads(run([*base, "doctor"]))["binary"]
        command = {"schema": "openagents.coder.task-command.v1", "command_id": "submit-" + name, "task_id": name,
                   "expected_revision": None, "action": {"type": "submit", "intent": {
                       "title": "Synthetic one-shot service", "prompt": "Append one visible fixture line.",
                       "workspace": {"path": str(repo), "source_revision": source},
                       "configuration": {"adapter": "bounded-command", "model": None}}}}
        command_path = root / (name + "-command.json")
        command_path.write_text(json.dumps(command))
        run([current, "task", "submit", "--file", command_path, "--store", tasks])
        submitted = json.loads(run([current, "task", "show", name, "--store", tasks]))
        # CLI replies retain the task directly; no authority comes from stdout.
        grant = {"schema": "openagents.coder.task-execution-grant.v1", "task_id": name,
                 "intent_digest": submitted["intent_digest"], "expected_revision": submitted["revision"],
                 "expected_source_snapshot": None, "program": str(Path("/bin/sh").resolve()),
                 "arguments": ["-c", f"printf 'one\\n' >> {name}.txt"], "write_workspace": True,
                 "wall_seconds": 10, "stream_bytes": 4096, "memory_bytes": 268435456,
                 "requirements": None}
        grant_path = root / (name + "-grant.json")
        grant_path.write_text(json.dumps(grant)); grant_path.chmod(0o600)
        label = "org.openagents.host-fixture." + uuid.uuid4().hex
        service = root / (label + ".plist")
        service.write_text(run([*base, "service", "--platform", "macos", "--grant", grant_path, "--label", label]))
        run(["/usr/bin/plutil", "-lint", service])
        target = f"gui/{os.getuid()}/{label}"
        loaded = False
        try:
            run(["/bin/launchctl", "bootstrap", f"gui/{os.getuid()}", service])
            loaded = True
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                task = json.loads(run([current, "task", "show", name, "--store", tasks]))
                if task["execution"] in ["finished", "unknown"]:
                    break
                time.sleep(0.1)
            assert task["execution"] == "finished", task["execution"]
            assert task["run"]["result"]["exit_code"] == 0
            assert (repo / (name + ".txt")).read_text() == "one\n"
            before = task["run"]
            run(["/bin/launchctl", "kickstart", "-k", target])
            time.sleep(0.5)
            after = json.loads(run([current, "task", "show", name, "--store", tasks]))
            assert after["run"] == before
            assert (repo / (name + ".txt")).read_text() == "one\n"
            observations.append({"task": name, "binary_sha256": sha(current), "execution": "finished",
                                 "process_elapsed_ms": task["run"]["result"]["elapsed_ms"],
                                 "service_restart": "same grant refused; no duplicate effect",
                                 "launchd_template": service.name})
        finally:
            if loaded:
                run(["/bin/launchctl", "bootout", target], allowed=(0, 3, 5, 113))
    one_task("service-upgrade")
    rollback = json.loads(run([*base, "rollback"]))
    assert rollback["binary_sha256"] == first["binary_sha256"]
    one_task("service-rollback")
    before = sha(tasks / "tasks.json")
    uninstall = json.loads(run([*base, "uninstall"]))
    assert sha(tasks / "tasks.json") == before
    receipt = {"schema": "openagents.coder.host-runtime-fixture.v1", "platform": "macos-launchd",
               "synthetic": True, "production_service": False, "source_revision": args.source_revision,
               "install": first, "upgrade": upgrade, "rollback": rollback, "doctor": doctor,
               "services": observations, "uninstall": uninstall, "task_data_unchanged": True,
               "cleanup": "both uniquely named launchd services booted out",
               "model_calls": 0, "all_in_cost_usd": None}
    (root / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"result": "passed", "receipt": str(root / "receipt.json")}))


if __name__ == "__main__":
    main()
