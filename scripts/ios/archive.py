#!/usr/bin/env python3
"""Prepare a Coder iOS archive locally. This command never exports or uploads."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
MIN_FREE = 9 * 1024**3


def run(command, env):
    process = subprocess.Popen(command, cwd=ROOT, env=env, start_new_session=True)
    try:
        while process.poll() is None:
            if shutil.disk_usage(ROOT).free < MIN_FREE:
                raise RuntimeError("Archive stopped below 9 GiB free; clear space before retrying.")
            time.sleep(1)
        if process.returncode:
            raise RuntimeError("Archive command failed; inspect the preceding build output.")
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


def verify(app, version):
    info = plistlib.loads((app / "Info.plist").read_bytes())
    assert info["CFBundleIdentifier"] == "com.openagents.coder"
    assert info["CFBundleShortVersionString"] == version["version"]
    assert info["CFBundleVersion"] == version["build"]
    assert info.get("ITSAppUsesNonExemptEncryption") is False, "Export exemption must be explicit."
    privacy = plistlib.loads((app / "PrivacyInfo.xcprivacy").read_bytes())
    assert privacy["NSPrivacyTracking"] is False
    assert not privacy["NSPrivacyTrackingDomains"]
    types = {row["NSPrivacyCollectedDataType"] for row in privacy["NSPrivacyCollectedDataTypes"]}
    assert {"NSPrivacyCollectedDataTypeUserID", "NSPrivacyCollectedDataTypeDeviceID",
            "NSPrivacyCollectedDataTypeEmailsOrTextMessages", "NSPrivacyCollectedDataTypePhotosorVideos"} <= types
    for extension in (app / "PlugIns").glob("*.appex"):
        other = plistlib.loads((extension / "Info.plist").read_bytes())
        assert other["CFBundleShortVersionString"] == version["version"]
        assert other["CFBundleVersion"] == version["build"]
        assert other["CFBundleIdentifier"].startswith("com.openagents.coder.")
    binary = app / info["CFBundleExecutable"]
    symbols = subprocess.check_output(["/usr/bin/strings", "-a", str(binary)])
    assert b"sign_in_from_fixture" not in symbols
    assert b"requestAuthorizationWithOptions" not in symbols


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--team", help="Owner only: use an existing local signing setup for this team")
    parser.add_argument("--distribution-certificate", help="Installed distribution certificate SHA-1; uses local App Store profiles named for each bundle ID")
    parser.add_argument("--check-only", action="store_true", help="Check source metadata without building")
    args = parser.parse_args()
    if args.distribution_certificate and (not args.team or len(args.distribution_certificate) != 40
            or any(c not in "0123456789abcdefABCDEF" for c in args.distribution_certificate)):
        parser.error("Distribution signing requires a team and a certificate SHA-1.")
    target, output = args.target_dir.expanduser().resolve(), args.output.expanduser().resolve()
    if target.is_relative_to(ROOT) or target.name in ("coder-desktop-target-agent0", "coder-desktop-target-agent3"):
        parser.error("Choose your own external Cargo target directory.")
    if output.exists() or output.with_suffix(".json").exists():
        parser.error("Choose a new archive path; existing files are never replaced.")
    if output.suffix != ".xcarchive":
        parser.error("The output path must end with .xcarchive.")
    version = json.loads((ROOT / "apps/ios/release/version.json").read_text())
    project = plistlib.loads(subprocess.check_output(["plutil", "-convert", "xml1", "-o", "-",
        str(ROOT / "apps/ios/Zeron.xcodeproj/project.pbxproj")]))
    for entry in project["objects"].values():
        settings = entry.get("buildSettings", {})
        if "PRODUCT_BUNDLE_IDENTIFIER" in settings:
            assert settings["CURRENT_PROJECT_VERSION"] == version["build"]
            assert settings["MARKETING_VERSION"] == version["version"]
            assert not settings.get("DEVELOPMENT_TEAM"), "Do not commit a signing team."
    source_info = plistlib.loads((ROOT / "apps/ios/Zeron/Info.plist").read_bytes())
    assert source_info.get("ITSAppUsesNonExemptEncryption") is False
    entitlements = plistlib.loads((ROOT / "apps/ios/Zeron.entitlements").read_bytes())
    assert "aps-environment" not in entitlements
    assert not entitlements.get("get-task-allow", False)
    if args.check_only:
        print("PASS Coder release metadata, matching versions, and no push entitlement")
        return
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip():
        parser.error("Commit release changes before archiving.")
    subprocess.run(["git", "merge-base", "--is-ancestor", "origin/main", "HEAD"], cwd=ROOT, check=True)
    if shutil.disk_usage(ROOT).free < MIN_FREE:
        parser.error("At least 9 GiB free is required before archiving.")
    env = dict(os.environ, RUSTUP_TOOLCHAIN="1.98.1", CARGO_TARGET_DIR=str(target),
               CONFIGURATION="Release", ZERON_ACCOUNT_FIXTURE="0", ZERON_SKIP_CORE="0",
               BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1")
    run(["scripts/ios/build-core.sh", "iphoneos"], env)
    env["ZERON_SKIP_CORE"] = "1"
    command = ["xcodebuild", "-project", "apps/ios/Zeron.xcodeproj", "-scheme", "Zeron",
        "-configuration", "Release", "-destination", "generic/platform=iOS",
        "-derivedDataPath", str(target / "ios-derived"), "-archivePath", str(output),
        "SWIFT_ACTIVE_COMPILATION_CONDITIONS=", "archive"]
    if args.team:
        command += ["CODE_SIGNING_ALLOWED=YES", "DEVELOPMENT_TEAM=" + args.team]
        if args.distribution_certificate:
            command += ["CODE_SIGN_STYLE=Manual", "CODE_SIGN_IDENTITY=" + args.distribution_certificate,
                        "PROVISIONING_PROFILE_SPECIFIER=$(PRODUCT_BUNDLE_IDENTIFIER)"]
    else:
        command += ["CODE_SIGNING_ALLOWED=NO"]
    run(command, env)
    app = output / "Products/Applications/Zeron.app"
    verify(app, version)
    run(["python3", "scripts/ios/verify-identity.py", str(app)], env)
    if args.team:
        run(["codesign", "--verify", "--deep", "--strict", str(app)], env)
        signed = plistlib.loads(subprocess.check_output(["codesign", "-d", "--entitlements", ":-", str(app)], stderr=subprocess.DEVNULL))
        assert "aps-environment" not in signed
        if args.distribution_certificate:
            assert not signed.get("get-task-allow", False)
    proof = {**version, "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "signed": bool(args.team), "uploaded": False, "identity_privacy_and_fixture_checks": True}
    output.with_suffix(".json").write_text(json.dumps(proof, indent=2) + "\n")
    print("PASS local Coder archive prepared; no export or upload performed")


if __name__ == "__main__":
    main()
