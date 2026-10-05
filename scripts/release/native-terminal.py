#!/usr/bin/env python3
"""Build the standalone macOS app and both helpers from one source commit."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import urllib.request
import re

REPO = Path(__file__).resolve().parents[2]
PREFIX = "openagents-terminal"
APP = "OpenAgents Terminal.app"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def output(*args):
    return subprocess.check_output(args, cwd=REPO, text=True).strip()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def build(args):
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise ValueError("The native release supports macOS arm64; build on that platform.")
    commit = output("git", "rev-parse", f"{args.commit}^{{commit}}")
    tree = output("git", "rev-parse", f"{commit}^{{tree}}")
    if commit != output("git", "rev-parse", "origin/main"):
        raise ValueError("Build the current origin/main commit; fetch before starting the release.")
    target = os.environ.get("CARGO_TARGET_DIR")
    if not target or not Path(target).is_absolute():
        raise ValueError("Set CARGO_TARGET_DIR to an absolute, reusable directory outside the checkout.")
    if Path(target).resolve().is_relative_to(REPO):
        raise ValueError("CARGO_TARGET_DIR must be outside the checkout.")
    destination = Path(args.out).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="native-terminal-source-") as directory:
        source = Path(directory)
        archive = source / "source.tar"
        with archive.open("wb") as file:
            run("git", "archive", commit, cwd=REPO, stdout=file)
        with tarfile.open(archive) as file:
            file.extractall(source, filter="data")
        archive.unlink()
        snapshot = {str(path.relative_to(source)): digest(path) for path in source.rglob("*") if path.is_file()}
        versions = [tomllib.loads((source / f"crates/{crate}/Cargo.toml").read_text())["package"]["version"]
                    for crate in ("terminal-app", "openagents-cli", "microcoder")]
        if len(set(versions)) != 1:
            raise ValueError("App and helper versions must match.")
        version = versions[0]
        stage = destination / version
        if stage.exists():
            raise ValueError(f"Refusing to replace {stage}.")
        stage.mkdir()
        bundle = stage / APP
        contents = bundle / "Contents"
        binaries = contents / "MacOS"
        binaries.mkdir(parents=True)
        shutil.copy2(source / "scripts/release/openagents-terminal.entitlements", stage / "release-entitlements.plist")
        shutil.copy2(source / "scripts/release/install-native-terminal.py", stage / "install-native-terminal.py")
        env = dict(os.environ, OPENAGENTS_BUILD_COMMIT=commit)
        # Separate graphs keep the CLI's Verse features out of the GUI executable.
        run("cargo", "build", "--locked", "--release", "-p", "terminal-app", cwd=source, env=env)
        shutil.copy2(Path(target) / "release/openagents-terminal", binaries)
        run("cargo", "build", "--locked", "--release", "-p", "openagents-cli", "-p", "microcoder", cwd=source, env=env)
        for name in ("openagents", "microcoder"):
            shutil.copy2(Path(target) / f"release/{name}", binaries)
        for relative, expected in snapshot.items():
            if digest(source / relative) != expected:
                raise ValueError(f"A build changed archived source: {relative}")
        for binary in binaries.iterdir():
            identity = subprocess.check_output(["file", str(binary)], text=True)
            if "Mach-O" not in identity or "arm64" not in identity:
                raise ValueError(f"Wrong executable architecture: {binary.name}")
        with (contents / "Info.plist").open("wb") as file:
            plistlib.dump({"CFBundleIdentifier": "com.openagents.terminal", "CFBundleName": "OpenAgents Terminal",
                          "CFBundleExecutable": "openagents-terminal", "CFBundlePackageType": "APPL",
                          "CFBundleShortVersionString": version.split("-")[0], "CFBundleVersion": version.replace("-rc.", "."),
                          "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "13.0"}, file)
        manifest = {"schema": "openagents.native-terminal.release.v1", "version": version,
                    "commit": commit, "tree": tree, "prefix": PREFIX, "platform": "darwin-arm64",
                    "toolchain": output("rustc", "--version"), "bundle": APP,
                    "executables": {name: digest(binaries / name) for name in ("openagents-terminal", "openagents", "microcoder")},
                    "installer_sha256": digest(stage / "install-native-terminal.py"), "signing": "not-run", "notarization": "not-run", "gatekeeper": "not-run", "public_readback": "not-run"}
        (stage / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(stage)


def read_manifest(stage):
    manifest = json.loads((stage / "release-manifest.json").read_text())
    if (manifest.get("schema") != "openagents.native-terminal.release.v1" or
        manifest.get("prefix") != PREFIX or manifest.get("platform") != "darwin-arm64" or
        not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[1-9][0-9]*)?", manifest.get("version", "")) or
        set(manifest.get("executables", {})) != {"openagents-terminal", "openagents", "microcoder"}):
        raise ValueError("Invalid native release manifest.")
    for name, expected in manifest["executables"].items():
        if digest(stage / APP / "Contents/MacOS" / name) != expected:
            raise ValueError(f"Executable changed since the manifest was written: {name}")
    return manifest


def save_manifest(stage, manifest):
    temporary = stage / "release-manifest.json.pending"
    temporary.write_text(json.dumps(manifest, indent=2) + "\n")
    temporary.replace(stage / "release-manifest.json")


def sign(args):
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise ValueError("Sign and notarize on macOS arm64.")
    stage = Path(args.stage).resolve()
    manifest = read_manifest(stage)
    identity = args.identity or os.environ.get("OA_DEVELOPER_ID_APPLICATION")
    profile = args.notary_profile or os.environ.get("NOTARY_KEYCHAIN_PROFILE")
    if not identity:
        raise ValueError("Set OA_DEVELOPER_ID_APPLICATION or pass --identity.")
    if profile:
        auth = ["--keychain-profile", profile]
    elif all(os.environ.get(name) for name in ("ASC_API_KEY_ID", "ASC_API_ISSUER_ID", "ASC_API_PRIVATE_KEY_PATH")):
        auth = ["--key", os.environ["ASC_API_PRIVATE_KEY_PATH"], "--key-id", os.environ["ASC_API_KEY_ID"], "--issuer", os.environ["ASC_API_ISSUER_ID"]]
    else:
        raise ValueError("Set NOTARY_KEYCHAIN_PROFILE or the existing ASC_API_KEY_ID, ASC_API_ISSUER_ID, and ASC_API_PRIVATE_KEY_PATH variables.")
    bundle = stage / APP
    entitlements = stage / "release-entitlements.plist"
    for name in manifest["executables"]:
        run("codesign", "--force", "--options", "runtime", "--timestamp", "--entitlements", str(entitlements),
            "--sign", identity, str(bundle / "Contents/MacOS" / name))
    run("codesign", "--force", "--options", "runtime", "--timestamp", "--sign", identity, str(bundle))
    run("codesign", "--verify", "--deep", "--strict", str(bundle))
    manifest["executables"] = {name: digest(bundle / "Contents/MacOS" / name) for name in manifest["executables"]}
    manifest.update(signing="passed", notarization="not-run", gatekeeper="not-run", public_readback="not-run")
    save_manifest(stage, manifest)
    archive = stage / "OpenAgents-Terminal.zip"
    run("ditto", "-c", "-k", "--keepParent", str(bundle), str(archive))
    response = subprocess.check_output(["xcrun", "notarytool", "submit", str(archive), *auth,
                                        "--wait", "--timeout", "30m", "--output-format", "json"], text=True)
    verdict = json.loads(response)
    if verdict.get("status") != "Accepted":
        raise ValueError("Notarization was not accepted; nothing will be published.")
    run("xcrun", "stapler", "staple", str(bundle))
    run("xcrun", "stapler", "validate", str(bundle))
    run("codesign", "--verify", "--deep", "--strict", str(bundle))
    run("spctl", "--assess", "--type", "execute", "--verbose=2", str(bundle))
    # The public archive contains the stapled app, not the pre-submission archive.
    archive.unlink()
    run("ditto", "-c", "-k", "--keepParent", str(bundle), str(archive))
    manifest.update(signing="passed", notarization="passed", gatekeeper="passed", notarization_id=verdict.get("id"),
                    archive_sha256=digest(archive), public_readback="not-run")
    manifest["executables"] = {name: digest(bundle / "Contents/MacOS" / name) for name in manifest["executables"]}
    save_manifest(stage, manifest)
    (stage / "SHA256SUMS").write_text(f"{manifest['archive_sha256']}  {archive.name}\n{digest(stage / 'release-manifest.json')}  release-manifest.json\n{manifest['installer_sha256']}  install-native-terminal.py\n")
    print("Signed, notarized, stapled, and Gatekeeper accepted; publication has not run.")


def verify_release(stage):
    manifest = read_manifest(stage)
    if any(manifest.get(key) != "passed" for key in ("signing", "notarization", "gatekeeper")):
        raise ValueError("Publication requires passed signing, notarization, and Gatekeeper checks.")
    if digest(stage / "OpenAgents-Terminal.zip") != manifest.get("archive_sha256"):
        raise ValueError("Archive changed after admission.")
    if digest(stage / "install-native-terminal.py") != manifest.get("installer_sha256"):
        raise ValueError("Installer changed after the archived build.")
    sums = (stage / "SHA256SUMS").read_text()
    expected = f"{manifest['archive_sha256']}  OpenAgents-Terminal.zip\n{digest(stage / 'release-manifest.json')}  release-manifest.json\n{manifest['installer_sha256']}  install-native-terminal.py\n"
    if sums != expected:
        raise ValueError("Checksum file changed after admission.")
    return manifest


def public_digest(url):
    h = hashlib.sha256()
    with urllib.request.urlopen(url, timeout=120) as response:
        for block in iter(lambda: response.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def publish(args):
    stage = Path(args.stage).resolve()
    manifest = verify_release(stage)
    bucket = args.bucket
    if not re.fullmatch(r"[a-z0-9][a-z0-9._-]+", bucket):
        raise ValueError("Invalid bucket name.")
    prefix = f"gs://{bucket}/{PREFIX}/{manifest['version']}"
    # Refuse any pre-existing version, including an interrupted publication.
    root = f"gs://{bucket}/{PREFIX}/"
    def listing(path):
        return subprocess.check_output(["gcloud", "storage", "ls", path], text=True).splitlines()
    # Listing the existing bucket first distinguishes a new prefix from an auth failure.
    roots = [line.strip() for line in listing(f"gs://{bucket}/")]
    if root in roots and any(line.strip().startswith(prefix + "/") for line in listing(root)):
        raise ValueError("The version is already present; use a new committed version.")
    files = ("OpenAgents-Terminal.zip", "release-manifest.json", "SHA256SUMS")
    for name in files:
        run("gcloud", "storage", "cp", "--if-generation-match=0", str(stage / name), f"{prefix}/{name}")
    # Versioned installer does not overwrite either native or TUI channel pointers.
    installer = stage / "install-native-terminal.py"
    run("gcloud", "storage", "cp", "--if-generation-match=0", str(installer), f"{prefix}/install-native-terminal.py")
    base = f"https://storage.googleapis.com/{bucket}/{PREFIX}/{manifest['version']}"
    expected = {name: digest(stage / name) for name in files}
    expected["install-native-terminal.py"] = digest(installer)
    for name, checksum in expected.items():
        if public_digest(f"{base}/{name}") != checksum:
            raise ValueError(f"Public readback failed: {name}; no installation receipt is claimed.")
    receipt = {"schema": "openagents.native-terminal.publication.v1", "commit": manifest["commit"],
               "version": manifest["version"], "base": base, "public_readback": "passed", "objects": expected,
               "installed_flow": "not-run", "both_surface_demo": "not-run"}
    (stage / "publication-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print("Public archive, manifest, checksums, and installer read back correctly; installed-flow verification remains.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("build")
    command.add_argument("--commit", default="origin/main")
    command.add_argument("--out", default=str(REPO / "dist/releases/openagents-terminal"))
    command = commands.add_parser("sign")
    command.add_argument("--stage", required=True)
    command.add_argument("--identity")
    command.add_argument("--notary-profile")
    command = commands.add_parser("publish")
    command.add_argument("--stage", required=True)
    command.add_argument("--bucket", default=os.environ.get("OPENAGENTS_RELEASES_BUCKET", "openagentsgemini-cli-releases"))
    args = parser.parse_args()
    {"build": build, "sign": sign, "publish": publish}[args.command](args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
