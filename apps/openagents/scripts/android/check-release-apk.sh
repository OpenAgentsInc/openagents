#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || ! -f "$1" ]]; then
  echo "usage: $0 RELEASE_APK" >&2
  exit 2
fi
apk="$1"

aapt="${ANDROID_HOME:-}/build-tools/35.0.0/aapt"
if [[ ! -x "$aapt" ]]; then
  aapt="$(command -v aapt || true)"
fi
if [[ -z "$aapt" || ! -x "$aapt" ]]; then
  echo "aapt was not found in the Android SDK or PATH" >&2
  exit 1
fi

python3 - "$apk" "$aapt" <<'PY'
import re
import subprocess
import sys
import zipfile

apk, aapt = sys.argv[1:]
badging = subprocess.run(
    [aapt, "dump", "badging", apk], check=True, capture_output=True, text=True
).stdout
if "package: name='com.openagents.app'" not in badging:
    raise SystemExit("release APK has the wrong application ID")
if "sdkVersion:'26'" not in badging:
    raise SystemExit("release APK does not declare API 26 as its minimum SDK")

debug_extra = re.compile(rb"openagents\.(demo|route|lab)")
with zipfile.ZipFile(apk) as archive:
    names = set(archive.namelist())
    for abi in ("arm64-v8a", "x86_64"):
        for library in ("libopenagents_mobile.so", "libc++_shared.so"):
            if f"lib/{abi}/{library}" not in names:
                raise SystemExit(f"release APK is missing {library} for {abi}")
    stale = [
        name
        for name in names
        if re.fullmatch(
            r"lib/(arm64-v8a|x86_64)/lib(breez_sdk_(common|spark)|iroh(_relay)?)-[0-9a-f]+\.so",
            name,
        )
    ]
    if stale:
        raise SystemExit(f"release APK contains Cargo dependency libraries: {stale}")
    for name in names:
        if re.fullmatch(r"classes[0-9]*\.dex", name):
            if debug_extra.search(archive.read(name)):
                raise SystemExit("release APK contains debug-only launch extras")

print("release APK uses the mobile bundle ID, API 26, both ABIs, and no debug launch extras")
PY
