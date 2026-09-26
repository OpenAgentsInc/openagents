#!/usr/bin/env python3
"""Build the Rust-only synthetic mobile probe with a restricted environment.

This is acceptance infrastructure. It does not install, sign for distribution,
launch, or contact a service. Build products stay outside the repository.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import time
import zipfile


ROOT = Path(__file__).resolve().parent.parent
PACKAGE = "com.openagents.coder.platformprobe"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["host", "ios-simulator", "ios-device", "android"])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sdk", type=Path, help="Android SDK root")
    parser.add_argument("--ndk", type=Path, help="Android NDK root")
    parser.add_argument("--jdk", type=Path, help="Java home for Android packaging")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT.parent / "target")).resolve()
    # Build helpers can include their environment in failure reports. Forward
    # only toolchain paths, never inherited provider keys or service tokens.
    env = {key: os.environ[key] for key in ("PATH", "HOME", "TMPDIR") if key in os.environ}
    env.update({"CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0",
                "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
                "CARGO_BUILD_JOBS": "2"})
    commands = []

    def run(command, destination=None):
        start = time.monotonic()
        result = subprocess.run([str(part) for part in command], cwd=ROOT,
                                env=env, capture_output=True)
        name = f"command-{len(commands) + 1}.log"
        (output / name).write_bytes(result.stdout + result.stderr)
        commands.append({"command": [str(part) for part in command],
                         "exit_code": result.returncode,
                         "elapsed_seconds": round(time.monotonic() - start, 3), "log": name})
        if destination is not None and result.returncode == 0:
            destination.write_bytes(result.stdout)
        if result.returncode:
            raise RuntimeError(f"command failed; read {output / name}")

    error = None
    product = None
    try:
        if args.platform == "host":
            run(["cargo", "test", "--locked", "-p", "coder-mobile-probe"])
            run(["cargo", "build", "--locked", "-p", "coder-mobile-probe", "--bin", "coder-mobile-probe"])
            binary = target / "debug" / "coder-mobile-probe"
            run([binary, "--html"], output / "transcript.html")
            run([binary, "--atif"], output / "trajectory.json")
            run([binary, "--text"], output / "transcript.txt")
            product = output / "transcript.html"
        elif args.platform.startswith("ios"):
            triple = "aarch64-apple-ios-sim" if args.platform == "ios-simulator" else "aarch64-apple-ios"
            command = ["cargo", "rustc", "--locked", "-p", "coder-mobile-probe", "--bin", "coder-mobile-probe", "--target", triple]
            if args.platform == "ios-simulator":
                entitlements = output / "simulator-entitlements.plist"
                entitlements.write_bytes(plistlib.dumps({
                    "application-identifier": "SIMULATOR." + PACKAGE,
                    "keychain-access-groups": ["SIMULATOR." + PACKAGE],
                }))
                # The simulator reads iOS entitlements from the Mach-O section.
                # Do not give the host kernel synthetic device entitlements.
                command.extend(["--", "-C", f"link-arg=-Wl,-sectcreate,__TEXT,__entitlements,{entitlements}"])
            run(command)
            product = output / "CoderProbe.app"
            product.mkdir(exist_ok=True)
            shutil.copy2(target / triple / "debug" / "coder-mobile-probe", product / "CoderProbe")
            info = {
                "CFBundleIdentifier": PACKAGE, "CFBundleName": "Coder probe",
                "CFBundleDisplayName": "Coder probe", "CFBundleExecutable": "CoderProbe",
                "CFBundlePackageType": "APPL", "CFBundleVersion": "1",
                "CFBundleShortVersionString": "0.1", "MinimumOSVersion": "16.0",
                "LSRequiresIPhoneOS": True, "UIDeviceFamily": [1, 2],
                "UILaunchScreen": {},
                "UISupportedInterfaceOrientations": ["UIInterfaceOrientationPortrait", "UIInterfaceOrientationLandscapeLeft", "UIInterfaceOrientationLandscapeRight"],
            }
            (product / "Info.plist").write_bytes(plistlib.dumps(info))
            if args.platform == "ios-simulator":
                run(["codesign", "--force", "--sign", "-", product])
        else:
            if not all((args.sdk, args.ndk, args.jdk)):
                raise RuntimeError("Android packaging requires --sdk, --ndk, and --jdk")
            tools = args.sdk.resolve() / "build-tools" / "36.0.0"
            compiler = args.ndk.resolve() / "toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android26-clang"
            env["CC_aarch64_linux_android"] = str(compiler)
            env["CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"] = str(compiler)
            env["JAVA_HOME"] = str(args.jdk.resolve())
            run(["cargo", "build", "--locked", "-p", "coder-mobile-probe", "--lib", "--target", "aarch64-linux-android"])
            manifest = output / "AndroidManifest.xml"
            manifest.write_text(f'''<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{PACKAGE}" android:versionCode="1" android:versionName="0.1">
  <uses-sdk android:minSdkVersion="26" android:targetSdkVersion="35"/>
  <application android:hasCode="false" android:debuggable="true" android:label="Coder probe" android:theme="@android:style/Theme.Material.Light.NoActionBar" android:allowBackup="false">
    <activity android:name="android.app.NativeActivity" android:exported="true">
      <meta-data android:name="android.app.lib_name" android:value="coder_mobile_probe"/>
      <intent-filter><action android:name="android.intent.action.MAIN"/><category android:name="android.intent.category.LAUNCHER"/></intent-filter>
    </activity>
  </application>
</manifest>\n''')
            unsigned = output / "unsigned.apk"
            run([tools / "aapt2", "link", "--manifest", manifest, "-I", args.sdk / "platforms/android-35/android.jar", "-o", unsigned])
            with zipfile.ZipFile(unsigned, "a", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.write(target / "aarch64-linux-android/debug/libcoder_mobile_probe.so", "lib/arm64-v8a/libcoder_mobile_probe.so")
            aligned = output / "aligned.apk"
            run([tools / "zipalign", "-f", "4", unsigned, aligned])
            # This key signs only a synthetic debug app. Never use it for a
            # release or import an operator's production signing credentials.
            key = output / "synthetic-debug.keystore"
            if not key.exists():
                run([args.jdk / "bin/keytool", "-genkeypair", "-keystore", key, "-storepass", "android", "-keypass", "android", "-alias", "probe", "-dname", "CN=Synthetic Coder Probe", "-keyalg", "RSA", "-validity", "30"])
            product = output / "coder-probe.apk"
            run([tools / "apksigner", "sign", "--ks", key, "--ks-key-alias", "probe", "--ks-pass", "pass:android", "--out", product, aligned])
    except Exception as failure:
        error = str(failure)
    finally:
        source_paths = sorted((ROOT / "crates/coder-mobile-probe").rglob("*.rs"))
        source_paths.extend([ROOT / "crates/coder-mobile-probe/Cargo.toml", Path(__file__)])
        receipt = {"schema": "openagents.mobile-probe-build.v1", "platform": args.platform,
                   "result": "failed" if error else "built", "error": error,
                   "device_validation": "not_run", "synthetic": True,
                   "product": str(product) if product else None, "commands": commands,
                   "source_sha256": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths}}
        (output / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        print(json.dumps({"result": receipt["result"], "receipt": str(output / "build-receipt.json"), "product": receipt["product"], "error": error}))
    return 1 if error else 0


if __name__ == "__main__":
    raise SystemExit(main())
