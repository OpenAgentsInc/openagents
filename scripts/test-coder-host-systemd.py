#!/usr/bin/env python3
"""Exercise rendered systemd service lifecycle with an explicit shell fixture.

This proves service-manager plumbing only. It does not run the Rust task owner.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--installed-coder", required=True)
    args = parser.parse_args()
    if os.uname().sysname != "Linux":
        raise RuntimeError("Linux systemd acceptance only")
    root = Path(args.output).absolute()
    root.mkdir(mode=0o700)
    helper = Path(__file__).with_name("coder-host.py").resolve()
    environment = {name: os.environ[name] for name in ["PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"] if name in os.environ}
    def run(argv, allowed=(0,)):
        result = subprocess.run(list(map(str, argv)), capture_output=True, env=environment, timeout=30)
        if result.returncode not in allowed:
            raise RuntimeError(f"command failed: {argv[0]}: {result.stderr.decode()[:2000]}")
        return result.stdout.decode()
    python = shutil.which("python3")
    old = Path(args.installed_coder).resolve()
    old_args = [python, helper, "--root", root / "old-bundle", "--tasks", root / "old-tasks", "install", "--binary", old,
                "--sha256", hashlib.sha256(old.read_bytes()).hexdigest(), "--source-revision", "0" * 40]
    refused = subprocess.run(list(map(str, old_args)), capture_output=True, env=environment, timeout=30)
    assert refused.returncode == 1
    assert "does not expose" in refused.stderr.decode()
    # The following shell program is test infrastructure, deliberately labeled
    # a fixture. It is not a replacement implementation of the task owner.
    fixture = root / "synthetic-service-fixture"
    shell = Path(shutil.which("sh")).resolve()
    marker = root / "invocations.txt"
    fixture.write_text(f"#!{shell}\ncase \"$*\" in\n *--version*|*--help*) printf 'synthetic fixture; task execute\\n' ;;\n *) printf 'one\\n' >> '{marker}' ;;\nesac\n")
    fixture.chmod(0o700)
    base = [python, helper, "--root", root / "fixture-bundle", "--tasks", root / "fixture-tasks"]
    identity = hashlib.sha256(fixture.read_bytes()).hexdigest()
    installed = json.loads(run([*base, "install", "--binary", fixture, "--sha256", identity,
                                "--source-revision", "0" * 40, "--uncommitted-source"]))
    grant = root / "fixture-grant.json"
    grant.write_text(json.dumps({"schema": "openagents.coder.task-execution-grant.v1", "wall_seconds": 10}))
    grant.chmod(0o600)
    label = "openagents-host-fixture-" + uuid.uuid4().hex + ".service"
    unit = root / label
    unit.write_text(run([*base, "service", "--platform", "linux", "--grant", grant, "--label", label]))
    verify = subprocess.run([shutil.which("systemd-analyze"), "--user", "verify", str(unit)], capture_output=True, env=environment, timeout=30)
    assert verify.returncode == 0, verify.stderr.decode()[:2000]
    linked = False
    try:
        run(["systemctl", "--user", "link", unit])
        linked = True
        run(["systemctl", "--user", "daemon-reload"])
        run(["systemctl", "--user", "start", label])
        assert run(["systemctl", "--user", "is-active", label]).strip() == "active"
        assert marker.read_text() == "one\n"
        run(["systemctl", "--user", "start", label])
        assert marker.read_text() == "one\n"
        state = run(["systemctl", "--user", "show", label, "--property=Result,ExecMainStatus,Restart,Type"]).strip()
    finally:
        if linked:
            run(["systemctl", "--user", "stop", label])
            link = Path(environment["HOME"]) / ".config/systemd/user" / label
            if link.is_symlink() and link.resolve() == unit.resolve():
                link.unlink()
            run(["systemctl", "--user", "daemon-reload"])
    receipt = {"schema": "openagents.coder.systemd-fixture.v1", "platform": "linux-systemd",
               "fixture_only": True, "rust_task_owner_executed": False, "systemd_verify": "passed",
               "service_start": "passed", "duplicate_start": "one process invocation while active",
               "service_state": state.splitlines(), "install": installed,
               "existing_coder": "refused: durable task execute is unavailable",
               "usr_bin_git": Path("/usr/bin/git").is_file(), "cleanup": "unique service stopped and unlinked; manager reloaded",
               "model_calls": 0, "production_services_changed": False, "all_in_cost_usd": None,
               "unit": unit.name}
    (root / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"result": "passed", "receipt": str(root / "receipt.json")}))


if __name__ == "__main__":
    main()
