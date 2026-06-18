#!/usr/bin/env bash
# Build signed, standalone Pylon binaries (bun --compile) for all platforms, sign
# each with the OpenAgents ed25519 release key, and verify. RC artifacts only —
# published to our GCP feed (updates.openagents.com) by oa-updates (#5043).
#
# Usage: bash scripts/build-rc-binaries.sh [version]   (default 1.0.0-rc.1)
# Output: dist/rc/<version>/pylon-<platform> + .sig.json + manifest.json (gitignored)
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="${1:-1.0.0-rc.2}"
CHANNEL="${OA_PYLON_CHANNEL:-rc}"
OUT="dist/rc/$VERSION"
SIGNER="../oa-updates/scripts/sign-release.ts"
VERIFIER="../oa-updates/scripts/verify-release.ts"
mkdir -p "$OUT"

# Version-sync guard (rc.13 incident): the compiled binary reports
# src/version.ts PYLON_VERSION and the npm package ships package.json "version".
# If either drifts from the version being cut, binaries self-report the wrong
# version and auto-update-loop. Fail closed before building.
SRC_VERSION="$(bun -e 'import { PYLON_VERSION } from "./src/version.ts"; console.log(PYLON_VERSION)')"
PKG_VERSION="$(bun -e 'console.log(require("./package.json").version)')"
if [ "$SRC_VERSION" != "$VERSION" ] || [ "$PKG_VERSION" != "$VERSION" ]; then
  echo "FAIL: cutting $VERSION but src/version.ts=$SRC_VERSION, package.json=$PKG_VERSION — bump BOTH to $VERSION." >&2
  exit 1
fi
echo "== version sync OK: $VERSION (src/version.ts + package.json) =="

# bun --compile cross-targets. darwin-arm64 is native here; the rest cross-compile.
# #5404: bun-windows-x64 makes Pylon a full earning client on Windows (its
# embedded-FS compiled-binary detection was fixed so the embedded Spark WASM is
# extracted on Windows too, not just macOS/Linux).
TARGETS=(bun-darwin-arm64 bun-darwin-x64 bun-linux-x64 bun-linux-arm64 bun-windows-x64)
built=()

# #5404 detection guard: the Spark-SDK load guard (#5166) below can only EXECUTE
# the native target on this build host, so cross-compiled Windows can't be
# spark-selftested here. Instead, prove the cross-platform compiled-binary
# DETECTOR still matches the Windows embedded-FS URL shape (`B:\~BUN\root\…`) —
# the exact bug that left Windows receive-only/non-earning. A regression here
# would silently disable embedded-WASM extraction on Windows again, so fail
# closed before building any binary.
echo "== detection guard (#5404): Windows embedded-FS URL shape =="
bun test src/spark-wasm-runtime.test.ts

# Host platform — only the native target can be executed on this build host, so
# the Spark-SDK load guard (#5166) runs against it. Cross-compiled targets share
# the same bundle graph, so a passing native guard covers them.
case "$(uname -sm)" in
  "Darwin arm64") HOST_PLAT=darwin-arm64;;
  "Darwin x86_64") HOST_PLAT=darwin-x64;;
  "Linux x86_64") HOST_PLAT=linux-x64;;
  "Linux aarch64"|"Linux arm64") HOST_PLAT=linux-arm64;;
  *) HOST_PLAT="";;
esac

# #5166: base64-embed the Spark WASM into the bundle + patch the SDK loader to
# honor PYLON_SPARK_WASM_PATH, so the compiled binary carries its own WASM and
# loads it on ANY machine (the SDK otherwise reads it from a build-machine path).
echo "== embed Spark WASM (#5166) =="
bun scripts/embed-spark-wasm.ts

# Resolve the build host's SDK WASM so the guard can hide it (below).
SDK_WASM="$(dirname "$(bun -e 'console.log(require.resolve("@breeztech/breez-sdk-spark"))' 2>/dev/null)")/breez_sdk_spark_wasm_bg.wasm"

for t in "${TARGETS[@]}"; do
  plat="${t#bun-}"
  bin="$OUT/pylon-$plat"
  echo "== build $plat =="
  if bun build src/index.ts --compile --target="$t" --outfile "$bin" 2>/tmp/pylon-build-$plat.err; then
    # #5166 guard: prove the binary loads the Spark WASM from its OWN embedded
    # copy, NOT the build machine's node_modules — so a build-machine path can't
    # make this false-pass (which is exactly how the rc.8/rc.9 binaries shipped
    # broken). Hide the node_modules WASM for the check, then restore it.
    if [ "$plat" = "$HOST_PLAT" ]; then
      moved=0
      if [ -f "$SDK_WASM" ]; then mv "$SDK_WASM" "$SDK_WASM.guard-hidden"; moved=1; fi
      st="$(PYLON_SPARK_BACKUP_ENABLED=1 OPENAGENTS_PYLON_HOME="$(mktemp -d)/pylon" "$bin" wallet spark-selftest --json 2>/dev/null || true)"
      if [ "$moved" = 1 ]; then mv "$SDK_WASM.guard-hidden" "$SDK_WASM"; fi
      if printf '%s' "$st" | grep -q '"moduleLoaded": true'; then
        echo "  spark-selftest OK ($plat): binary loads its EMBEDDED Spark WASM (node_modules copy hidden)"
      else
        echo "  FAIL $plat: binary cannot load the Spark WASM without node_modules (#5166 guard)" >&2
        printf '%s\n' "$st" >&2
        exit 1
      fi
    fi
    OPENAGENTS_RELEASE_SIGNED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)" bun "$SIGNER" "$bin" > "$bin.sig.json"
    bun "$VERIFIER" "$bin" "$bin.sig.json"
    built+=("$plat")
  else
    echo "  SKIP $plat (cross-compile failed — see /tmp/pylon-build-$plat.err)" >&2
  fi
done

# Per-platform manifest (the seed oa-updates publishes to the rc feed; #5043).
{
  echo "{"
  echo "  \"schema\": \"openagents.pylon.release_manifest.v1\","
  echo "  \"product\": \"pylon\","
  echo "  \"version\": \"$VERSION\","
  echo "  \"channel\": \"$CHANNEL\","
  echo "  \"platforms\": {"
  first=1
  for plat in "${built[@]}"; do
    sig=$(python3 -c "import json;print(json.load(open('$OUT/pylon-$plat.sig.json'))['signature'])")
    sha=$(python3 -c "import json;print(json.load(open('$OUT/pylon-$plat.sig.json'))['sha256'])")
    [ $first -eq 0 ] && echo ","
    first=0
    printf '    "%s": { "file": "pylon-%s", "sha256": "%s", "signature": "%s", "kid": "%s" }' \
      "$plat" "$plat" "$sha" "$sig" "$(python3 -c "import json;print(json.load(open('$OUT/pylon-$plat.sig.json'))['kid'])")"
  done
  echo ""
  echo "  }"
  echo "}"
} > "$OUT/manifest.json"

echo "== built: ${built[*]:-none} =="
echo "RC binaries + signatures + manifest in $OUT (channel=$CHANNEL, version=$VERSION)"
echo "NOTE: rc channel only — do not publish to stable until owner GA."
