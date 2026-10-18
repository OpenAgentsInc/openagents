#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || ! -f "$1" ]]; then
  echo "usage: $0 RELEASE_APK" >&2
  exit 2
fi
apk="$1"

badging="$(aapt dump badging "$apk")"
grep -F "package: name='com.openagents.app'" <<<"$badging" >/dev/null
grep -F "sdkVersion:'26'" <<<"$badging" >/dev/null
for abi in arm64-v8a x86_64; do
  unzip -Z1 "$apk" | grep -Fx "lib/$abi/libopenagents_mobile.so" >/dev/null
done

if unzip -Z1 "$apk" | grep -E '^classes[0-9]*\.dex$' | while read -r dex; do
  unzip -p "$apk" "$dex" | strings
done | grep -E 'openagents\.(demo|route|lab)'; then
  echo "release APK contains debug-only launch extras" >&2
  exit 1
fi

echo "release APK uses the mobile bundle ID, API 26, both ABIs, and no debug launch extras"
