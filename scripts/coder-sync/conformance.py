#!/usr/bin/env python3
"""Check the pinned clients against an isolated Rust server and headless host."""
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
LOCK = MANIFEST.with_name("Cargo.lock")
FORK_URL = "https://github.com/OpenAgentsInc/coder-desktop"
CLIENTS = {"zeron-sync", "zeron-client", "zeron-proto", "zeron-doc", "zeron-rpc"}
MIN_FREE = 8 * 1024**3


def disk_check():
    if shutil.disk_usage(ROOT).free < MIN_FREE:
        raise RuntimeError("Less than 8 GiB of disk space remains. Stop and free space before continuing.")


def pin_from(text):
    match = re.search(r'^fork-revision = "([0-9a-f]{40})"$', text, re.M)
    if not match:
        raise RuntimeError("The manifest has no full fork revision.")
    pin = match.group(1)
    declarations = re.findall(r'^(zeron-[a-z-]+) = \{ git = "' + re.escape(FORK_URL) + r'", rev = "([0-9a-f]{40})" \}$', text, re.M)
    if {name for name, _ in declarations} != CLIENTS or len(declarations) != len(CLIENTS) or any(value != pin for _, value in declarations):
        raise RuntimeError("Every fork client dependency must use the same revision.")
    return pin


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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pin", help="Full candidate fork commit; update the manifest and lockfile on success.")
    parser.add_argument("--fork-repo", type=Path, default=Path.home() / "work/coder-desktop")
    args = parser.parse_args()
    original_manifest = MANIFEST.read_bytes()
    original_lock = LOCK.read_bytes()
    original_pin = pin_from(original_manifest.decode())
    pin = args.pin or original_pin
    if not re.fullmatch(r"[0-9a-f]{40}", pin):
        raise RuntimeError("Use a full 40-character fork commit.")
    if args.pin and subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip():
        raise RuntimeError("Use a clean OpenAgents worktree before changing the pin.")
    env = os.environ.copy()
    env.update(RUSTUP_TOOLCHAIN="1.98.1", BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1", CARGO_NET_GIT_FETCH_WITH_CLI="true")
    scratch_root = Path(env.get("OPENAGENTS_SCRATCH") or subprocess.check_output(["openagents", "scratch"], text=True).strip())
    scratch = scratch_root / "coder-sync-conformance"
    scratch.mkdir(parents=True, exist_ok=True)
    env["OPENAGENTS_SCRATCH"] = str(scratch_root)
    if not env.get("CODER_SYNC_POSTGRES_BIN") and Path("/opt/homebrew/opt/postgresql@16/bin/initdb").is_file():
        env["CODER_SYNC_POSTGRES_BIN"] = "/opt/homebrew/opt/postgresql@16/bin"
    env.setdefault("CARGO_TARGET_DIR", str(Path.home() / "work/openagents-target-agent1"))
    fork_target = Path(env.get("CODER_SYNC_FORK_TARGET_DIR", str(Path.home() / "work/coder-desktop-target-agent1")))
    fork_target.mkdir(parents=True, exist_ok=True)
    with (fork_target / ".coder-sync-conformance.lock").open("a") as guard:
        try:
            fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("Another conformance run is using this fork target directory.")
        fork = args.fork_repo.resolve()
        if subprocess.run(["git", "-C", str(fork), "cat-file", "-e", pin + "^{commit}"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            run(["git", "-C", str(fork), "fetch", "origin", pin], env, scratch, "Fetch fork commit")
        resolved = subprocess.check_output(["git", "-C", str(fork), "rev-parse", pin + "^{commit}"], text=True).strip()
        if resolved != pin:
            raise RuntimeError("The fork revision does not name the requested commit.")
        # Cargo can test an unpublished candidate without changing global Git config.
        count = int(env.get("GIT_CONFIG_COUNT", "0"))
        env["GIT_CONFIG_COUNT"] = str(count + 1)
        env["GIT_CONFIG_KEY_" + str(count)] = "url." + fork.as_uri() + ".insteadOf"
        env["GIT_CONFIG_VALUE_" + str(count)] = FORK_URL
        expected_manifest = original_manifest
        expected_lock = original_lock
        changed = bool(args.pin and pin != original_pin)
        try:
            if changed:
                text = original_manifest.decode().replace(original_pin, pin)
                pin_from(text)
                MANIFEST.write_text(text)
                expected_manifest = MANIFEST.read_bytes()
            metadata = ["cargo", "metadata", "--manifest-path", str(MANIFEST), "--format-version", "1"]
            if not changed:
                metadata.append("--locked")
            run(metadata, env, scratch, "Resolve pinned clients")
            expected_lock = LOCK.read_bytes()
            run(["cargo", "fmt", "--manifest-path", str(MANIFEST), "--all", "--", "--check"], env, scratch, "Check server formatting")
            source = scratch / ("fork-" + pin)
            # This directory contains only this script's exported source, never a checkout.
            if source.exists():
                shutil.rmtree(source)
            source.mkdir()
            archive = subprocess.Popen(["git", "-C", str(fork), "archive", pin], stdout=subprocess.PIPE, env=env)
            try:
                subprocess.run(["tar", "-x", "-C", str(source)], stdin=archive.stdout, check=True)
            finally:
                archive.stdout.close()
            if archive.wait():
                raise RuntimeError("Could not export the pinned fork source.")
            build_env = dict(env, CARGO_TARGET_DIR=str(fork_target))
            run(["cargo", "build", "--locked", "--manifest-path", str(source / "Cargo.toml"), "-p", "zeron", "--profile", "mobile"], build_env, scratch, "Build pinned headless engine")
            executable = fork_target / "mobile/zeron"
            if not executable.is_file():
                raise RuntimeError("The pinned headless engine was not built.")
            test_env = dict(env, ZERON_TEST_HEADLESS_BIN=str(executable))
            run([str(ROOT / "scripts/coder-sync/test.sh"), "--", "--include-ignored"], test_env, scratch, "Run protocol conformance")
            run(["cargo", "build", "--locked", "--manifest-path", str(MANIFEST), "--features", "server"], env, scratch, "Build server executable")
            print("PASS: chat, registry, device, blob, image, storage, and notification rules; fork=" + pin, flush=True)
            if changed:
                print("Commit the updated Cargo.toml and Cargo.lock after publishing this fork commit.", flush=True)
        except BaseException:
            if changed:
                for path, before, expected in [(MANIFEST, original_manifest, expected_manifest), (LOCK, original_lock, expected_lock)]:
                    if path.read_bytes() == expected:
                        path.write_bytes(before)
                    else:
                        print("Left an unexpectedly changed file for review: " + str(path), file=sys.stderr)
            raise


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print("Conformance stopped: " + str(error), file=sys.stderr)
        sys.exit(1)
