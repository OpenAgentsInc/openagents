#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around coax, the Apple SDKs' clang and the Android NDK's clang; holds no logic)
# The host core's conformance driver built for every native platform this Mac has an SDK for
# (GitHub #13304): the one file metaverse-engine/host/core/conformance.c over host-core.h, against
# the fixtures and expected.txt that conformance.coax writes. A platform whose program can run here
# runs and is compared; the others are built only, and the line says so.
#   metaverse-engine/host/core/platforms.sh SCRATCH
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
out="${1:?usage: platforms.sh SCRATCH}"; mkdir -p "$out"; out="$(cd "$out" && pwd)"
"$repo/coax/web/tool.sh" "$here/conformance.coax" write "$out" > /dev/null
"${COAX:-$repo/coax/bin/coax}" c "$here/core-face.coax" --library --header "$out/aethyr-core-face.h" > "$out/aethyr-core-face.c"
flags=(-std=gnu11 -O1 -w -DCOAXIAL_NATIVE_ADDRESS -I"$out" -I"$repo" "$here/conformance.c" "$out/aethyr-core-face.c" -lm)
/usr/bin/clang "${flags[@]}" -o "$out/conformance-mac"
"$out/conformance-mac" "$out" | diff - "$out/expected.txt" > "$out/mac.diff"
echo "ok mac: built, run, and what it printed is what the engine says"
xcrun -sdk iphonesimulator clang -target arm64-apple-ios17.0-simulator "${flags[@]}" -o "$out/conformance-iphone-simulator"
booted="$(xcrun simctl list devices booted | grep -c Booted || true)"
if [[ "$booted" != 0 ]]; then
  xcrun simctl spawn booted "$out/conformance-iphone-simulator" "$out" | diff - "$out/expected.txt" > "$out/iphone-simulator.diff"
  echo "ok iphone-simulator: built, run in the booted simulator, and what it printed is what the engine says"
else
  echo "ok iphone-simulator: built; not run (no simulator is booted)"
fi
xcrun -sdk iphoneos clang -target arm64-apple-ios17.0 "${flags[@]}" -o "$out/conformance-iphone"
echo "ok iphone: built; not run (it needs a device)"
ndk="${ANDROID_NDK_HOME:-$(ls -d "${ANDROID_HOME:-$HOME/Library/Android/sdk}"/ndk/* 2>/dev/null | sort -V | tail -1)}"
if [[ -n "$ndk" && -d "$ndk" ]]; then
  "$(echo "$ndk"/toolchains/llvm/prebuilt/*)/bin/aarch64-linux-android30-clang" "${flags[@]}" -o "$out/conformance-android"
  # A device or an emulator that adb sees runs it: the program and the three fixtures go to the
  # device's scratch folder, and what it prints there is compared here. To start an emulator with
  # no window: "$ANDROID_HOME/emulator/emulator" -avd NAME -no-window -no-audio -no-boot-anim -read-only
  adb="${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb"
  serial="$([[ -x "$adb" ]] && "$adb" devices 2>/dev/null | awk 'NR > 1 && $2 == "device" { print $1; exit }' || true)"
  if [[ -n "$serial" ]]; then
    there="/data/local/tmp/oa-host-core-$$"
    "$adb" -s "$serial" shell mkdir -p "$there"
    "$adb" -s "$serial" push "$out/conformance-android" "$out/conformance.pack" "$out/conformance.vpk3" "$out/conformance.plan" "$there/" > /dev/null 2>&1
    "$adb" -s "$serial" shell "chmod 755 $there/conformance-android && $there/conformance-android $there" | tr -d '\r' > "$out/android.txt" || true
    "$adb" -s "$serial" shell rm -r "$there"
    diff "$out/android.txt" "$out/expected.txt" > "$out/android.diff"
    echo "ok android: built, run on $("$adb" -s "$serial" shell getprop ro.product.model | tr -d '\r') ($("$adb" -s "$serial" shell getprop ro.product.cpu.abi | tr -d '\r'), Android $("$adb" -s "$serial" shell getprop ro.build.version.release | tr -d '\r')), and what it printed is what the engine says"
  else
    echo "ok android: built; not run (adb sees no device and no emulator)"
  fi
else
  echo "skipped android: no NDK on this Mac"
fi
