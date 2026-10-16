#!/usr/bin/env python3
"""Check a dedicated account's desktop/phone flow on staging, then production."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
SIMULATOR = "8010197D-517D-43DA-9559-46742EFABC65"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--credential-file", type=Path, required=True)
    parser.add_argument("--account", default="acct_83d16a35955ef908")
    parser.add_argument("--deployment", choices=["both", "staging", "production"], default="both")
    parser.add_argument("--skip-build", action="store_true", help="Reuse the fixture binaries just built from this checkout")
    parser.add_argument("--custom-agents", type=Path, help="Run the custom-agent UI check with this installed fixture runner")
    args = parser.parse_args()
    target = args.target_dir.expanduser().resolve()
    if target.is_relative_to(ROOT) or target.name in ("coder-desktop-target-agent0", "coder-desktop-target-agent3"):
        parser.error("Choose your own external Cargo target directory.")
    key_file = args.credential_file.expanduser().resolve()
    if key_file.name == "openagents-mdk-treasury.env":
        parser.error("Choose a dedicated test-account credential file.")
    scratch = args.scratch.expanduser().resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, RUSTUP_TOOLCHAIN="1.98.1", CARGO_TARGET_DIR=str(target),
               BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1", ZERON_ACCOUNT_FIXTURE="1")
    if args.custom_agents:
        env["OA_CUSTOM_AGENT_FIXTURE_RUNNER"] = str(args.custom_agents.expanduser().resolve())
        env["OA_CUSTOM_AGENT_UI"] = "1"
    if not args.skip_build:
        for command in (["cargo", "build", "--locked", "-p", "zeron", "--profile", "mobile"],
                        ["cargo", "build", "--locked", "-p", "zeron-engine", "--example", "mobile_sync_fixture"],
                        ["scripts/ios/build-core.sh", "iphonesimulator"]):
            subprocess.run(command, cwd=ROOT, env=env, check=True)
    deployments = ("staging", "production") if args.deployment == "both" else (args.deployment,)
    for name in deployments:
        run = scratch / (name + "-" + str(time.time_ns()))
        origin = "https://coder-sync" + ("-staging" if name == "staging" else "") + "-157437760789.us-central1.run.app"
        with scratch.joinpath(run.name + "-coordinator.log").open("w") as log:
            process = subprocess.Popen([str(target / "debug/examples/mobile_sync_fixture"),
                str(target / "mobile/zeron"), origin, str(run), str(key_file), args.account],
                cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        config = None
        try:
            deadline = time.monotonic() + 120
            while not run.joinpath("config.json").exists():
                if process.poll() is not None:
                    raise RuntimeError("Coordinator stopped; inspect its private log.")
                if time.monotonic() >= deadline:
                    raise RuntimeError("Coordinator readiness timed out.")
                time.sleep(0.25)
            config = json.loads(run.joinpath("config.json").read_text())
            test_env = dict(env, TEST_RUNNER_CODER_MOBILE_SYNC_CONFIG=json.dumps(config), ZERON_SKIP_CORE="1")
            command = ["xcodebuild", "-project", "apps/ios/Zeron.xcodeproj", "-scheme", "Zeron",
                "-configuration", "Debug", "-destination", "platform=iOS Simulator,id=" + SIMULATOR,
                "-derivedDataPath", str(target / "ios-derived"), "CODE_SIGNING_ALLOWED=YES", "CODE_SIGN_IDENTITY=-",
                "SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE",
                "-resultBundlePath", str(scratch / (run.name + ".xcresult")),
                "-only-testing:ZeronUITests/" + ("OpenAgentsCustomAgentTests" if args.custom_agents else "OpenAgentsLiveSyncTests"), "test"]
            with scratch.joinpath(run.name + "-uitest.log").open("w") as log:
                result = subprocess.run(command, cwd=ROOT, env=test_env, stdout=log, stderr=subprocess.STDOUT)
            if result.returncode:
                raise RuntimeError("Native " + name + " check failed; inspect its result bundle.")
            container = subprocess.check_output(["xcrun", "simctl", "get_app_container", SIMULATOR,
                "com.openagents.app", "data"], text=True).strip()
            proof = Path(container) / "Documents/account-fixture-result.json"
            report = json.loads(proof.read_text())
            if not all(report.get(key) is True for key in ("signed_in", "saw_session", "keychain_empty")) or proof.stat().st_mtime <= run.joinpath("config.json").stat().st_mtime:
                raise RuntimeError("Missing fresh sign-in/session/Keychain-deletion proof.")
            run.joinpath("keychain-result.json").write_text(json.dumps(report))
            print("PASS " + name + ": native scenario and fresh Keychain-deletion proof", flush=True)
        finally:
            if config:
                try:
                    urllib.request.urlopen(urllib.request.Request(config["control"] + "/stop", method="POST"), timeout=15).read()
                except OSError:
                    process.send_signal(signal.SIGINT)
            elif process.poll() is None:
                process.send_signal(signal.SIGINT)
            process.wait(timeout=120)
        if process.returncode:
            raise RuntimeError("Coordinator cleanup failed.")
        report = json.loads(run.joinpath("result.json").read_text())
        expected_checks = 3 if args.custom_agents else 6
        expected_restarts = 0 if args.custom_agents else 1
        if len(report["checks"]) != expected_checks or not all(report["checks"].values()) or not report["child_stopped"] or not report["registry_fixtures_removed"] or report["host_restarts"] != expected_restarts:
            raise RuntimeError("Incomplete host-side assertions or cleanup.")
        print("PASS " + name + " cleanup", flush=True)
    print("Rebuild the normal core and bindings without ZERON_ACCOUNT_FIXTURE before committing or archiving.")


if __name__ == "__main__":
    main()
