#!/usr/bin/env python3
"""Check the apps' sync clients (apps/openagents) against an isolated Rust server and headless host."""
import argparse
import fcntl
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "deploy/coder-sync/Cargo.toml"
APPS = ROOT / "apps/openagents"
CLIENTS = {"zeron-sync", "zeron-client", "zeron-proto", "zeron-doc", "zeron-rpc"}
MIN_FREE = 8 * 1024**3


def disk_check():
    if shutil.disk_usage(ROOT).free < MIN_FREE:
        raise RuntimeError("Less than 8 GiB of disk space remains. Stop and free space before continuing.")


def check_clients(text):
    """The server must use the sync clients from this checkout's apps, never a Git pin."""
    declarations = dict(re.findall(r'^(zeron-[a-z-]+) = \{ path = "([^"]+)" \}$', text, re.M))
    if set(declarations) != CLIENTS:
        raise RuntimeError("Every sync client must be a path dependency on apps/openagents/crates.")
    for name, path in declarations.items():
        if not (MANIFEST.parent / path / "Cargo.toml").resolve().is_relative_to(APPS.resolve()):
            raise RuntimeError(name + " must come from apps/openagents.")


def run(args, env, scratch, label):
    disk_check()
    print(label, flush=True)
    log = scratch / "conformance-last-command.log"
    with log.open("w") as output:
        process = subprocess.Popen(args, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        last_update = time.monotonic()
        try:
            while process.poll() is None:
                disk_check()
                if time.monotonic() - last_update >= 30:
                    print(label + ": still running", flush=True)
                    last_update = time.monotonic()
                time.sleep(1)
        except BaseException:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait()
            raise
    if process.returncode:
        print("\n".join(log.read_text(errors="replace").splitlines()[-60:]), file=sys.stderr)
        raise RuntimeError(label + " failed. See " + str(log))
    # Keep a separate complete log for each phase.
    shutil.copyfile(log, scratch / (re.sub(r"[^a-z0-9]+", "-", label.lower()).strip("-") + ".log"))


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    check_clients(MANIFEST.read_text())
    env = os.environ.copy()
    env.update(RUSTUP_TOOLCHAIN="1.98.1", BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1")
    scratch_root = Path(env.get("OPENAGENTS_SCRATCH") or subprocess.check_output(["openagents", "scratch"], text=True).strip())
    scratch = scratch_root / "coder-sync-conformance"
    scratch.mkdir(parents=True, exist_ok=True)
    env["OPENAGENTS_SCRATCH"] = str(scratch_root)
    if not env.get("CODER_SYNC_POSTGRES_BIN") and Path("/opt/homebrew/opt/postgresql@16/bin/initdb").is_file():
        env["CODER_SYNC_POSTGRES_BIN"] = "/opt/homebrew/opt/postgresql@16/bin"
    env.setdefault("CARGO_TARGET_DIR", str(Path.home() / "work/openagents-target-agent1"))
    apps_target = Path(env.get("CODER_SYNC_APPS_TARGET_DIR", str(Path.home() / "work/coder-desktop-target-agent1")))
    apps_target.mkdir(parents=True, exist_ok=True)
    with (apps_target / ".coder-sync-conformance.lock").open("a") as guard:
        try:
            fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("Another conformance run is using this apps target directory.")
        run(["cargo", "metadata", "--manifest-path", str(MANIFEST), "--format-version", "1", "--locked"], env, scratch, "Resolve the sync clients")
        # Only the server's packages: --all would also check its path dependencies,
        # including upstream Zeron code that is deliberately not reformatted.
        run(["cargo", "fmt", "--manifest-path", str(MANIFEST), "-p", "coder-sync-service", "-p", "coder-sync-conformance", "--", "--check"], env, scratch, "Check server formatting")
        # The headless engine is the desktop app's own binary, from this checkout.
        build_env = dict(env, CARGO_TARGET_DIR=str(apps_target))
        run(["cargo", "build", "--locked", "--manifest-path", str(APPS / "Cargo.toml"), "-p", "zeron", "--profile", "mobile"], build_env, scratch, "Build the headless engine")
        executable = apps_target / "mobile/zeron"
        if not executable.is_file():
            raise RuntimeError("The headless engine was not built.")
        test_env = dict(env, ZERON_TEST_HEADLESS_BIN=str(executable))
        run([str(ROOT / "scripts/coder-sync/test.sh"), "--", "--include-ignored"], test_env, scratch, "Run protocol conformance")
        run(["cargo", "build", "--locked", "--manifest-path", str(MANIFEST), "--features", "server"], env, scratch, "Build server executable")
        commit = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, text=True).strip()
        print("PASS: chat, registry, device, blob, image, storage, and notification rules; apps/openagents at " + commit, flush=True)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print("Conformance stopped: " + str(error), file=sys.stderr)
        sys.exit(1)
