#!/usr/bin/env bash
# Runs host_grant_native.coax's cases as emitted C on native, wasm32 (WASI,
# under bun) and, when a simulator is booted, the iOS simulator; every target
# must print the line the evaluator's checks expect.
# Firm reason: 5 (a thin wrapper around coax, clang and simctl; no logic).
#   coax/tests/nostr/host_grant_targets.sh
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)"; cd "$repo"
want="0 4 2 12 12 5 0 7 11 11 9 8 8"
d="$(mktemp -d)"; trap 'rm -rf "$d"' EXIT
coax/bin/coax c coax/tests/nostr/host_grant_native.coax --run-steps 250000000 > "$d/hg.c"
check() { [[ "$2" == "$want" ]] || { echo "$1: got '$2', want '$want'" >&2; exit 1; }; echo "$1: $2"; }
cc -std=c11 -O2 -ffp-contract=off -fno-fast-math -w "$d/hg.c" -o "$d/hg" && check native "$("$d/hg")"
source coax/web/toolchain.sh
"$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" -std=c11 -O2 \
  -ffp-contract=off -fno-fast-math -w "$d/hg.c" -o "$d/hg.wasm" -Wl,-z,stack-size=16777216
cat > "$d/run.mjs" <<'JS'
import { WASI } from "node:wasi"; import { readFileSync } from "node:fs"
const wasi = new WASI({ version: "preview1", args: [], env: {} })
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), { wasi_snapshot_preview1: wasi.wasiImport })
wasi.start(instance)
JS
check wasm32 "$(bun "$d/run.mjs" "$d/hg.wasm")"
sim="$(xcrun simctl list devices booted 2>/dev/null | grep -o '([0-9A-F-]\{36\})' | head -1 | tr -d '()' || true)"
if [[ -n "$sim" ]]; then
  xcrun --sdk iphonesimulator clang -arch arm64 -mios-simulator-version-min=17.0 -std=c11 -O2 -ffp-contract=off -fno-fast-math -w "$d/hg.c" -o "$d/hg-ios"
  check ios-simulator "$(xcrun simctl spawn "$sim" "$d/hg-ios")"
else
  echo "ios-simulator: skipped (none booted)"
fi
