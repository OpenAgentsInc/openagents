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

# bun --compile cross-targets. darwin-arm64 is native here; the rest cross-compile.
TARGETS=(bun-darwin-arm64 bun-darwin-x64 bun-linux-x64 bun-linux-arm64)
built=()

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

for t in "${TARGETS[@]}"; do
  plat="${t#bun-}"
  bin="$OUT/pylon-$plat"
  echo "== build $plat =="
  if bun build src/index.ts --compile --target="$t" --outfile "$bin" 2>/tmp/pylon-build-$plat.err; then
    # #5166 guard: a standalone binary that cannot load the Spark SDK must NOT
    # ship (the offline-receive rail would be silently dead). Run the native
    # target's own spark-selftest (no network, no seed) and require moduleLoaded.
    if [ "$plat" = "$HOST_PLAT" ]; then
      st="$(PYLON_SPARK_BACKUP_ENABLED=1 OPENAGENTS_PYLON_HOME="$(mktemp -d)/pylon" "$bin" wallet spark-selftest --json 2>/dev/null || true)"
      if printf '%s' "$st" | grep -q '"moduleLoaded": true'; then
        echo "  spark-selftest OK ($plat): Spark SDK loads in the compiled binary"
      else
        echo "  FAIL $plat: Spark SDK did not load in the compiled binary (#5166 guard)" >&2
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
