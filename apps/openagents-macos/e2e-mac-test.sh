#!/usr/bin/env bash
# Firm reason: 5 (thin macOS E2E harness wrapping coax build, xcodebuild, check.sh, live dual-host NIP-44 tests, and package-dmg.sh verification; holds no app logic).
# Comprehensive OpenAgents macOS End-to-End Test Harness (GitHub #13993, 4.3 & 4.5):
#   1. Generates Coax C bridge + XcodeGen project, builds OpenAgents.app, and runs check.sh
#   2. Spins up live dual Coax NIP-44 TCP hosts (--legacy-v1 strict 16-key + v2 push/approval/workspace)
#   3. Runs OpenAgentsTests (unit + live dual-host NIP-44 E2E including multimodal image + Plan downgrade + approvals + subagent + branch switch)
#   4. Packages OpenAgents.dmg via package-dmg.sh, mounts it read-only, and verifies bundled coder-host, agyworker, and AppIcon.icns
set -euo pipefail

cd "$(dirname "$0")/../.."
repo="$(pwd -P)"
coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then
  echo "e2e-mac-test.sh: coax binary not found at $coax" >&2
  exit 1
fi

tmp="$(mktemp -d /tmp/oa-mac-e2e-XXXXXX)"
legacy_pid=""
v2_pid=""
mounted_vol=""
cleanup() {
  if [[ -n "$mounted_vol" && -d "$mounted_vol" ]]; then
    hdiutil detach "$mounted_vol" -quiet 2>/dev/null || true
  fi
  for p in "$legacy_pid" "$v2_pid"; do
    if [[ -n "$p" ]]; then
      kill "$p" 2>/dev/null || true
      wait "$p" 2>/dev/null || true
    fi
  done
  rm -rf "$tmp"
}
trap cleanup EXIT

echo "==> [1/4] Generating Coax C bridge, building OpenAgents.app, and checking invariants"
cd "$repo/apps/openagents-macos"
./generate.sh
xcodegen generate
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Debug -sdk macosx \
  -derivedDataPath "$tmp/derived" CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- build
./check.sh "$tmp/derived/Build/Products/Debug/OpenAgents.app"

echo "==> [2/4] Starting live dual Coax NIP-44 TCP hosts (legacy v1 + modern v2)"
cd "$repo"
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/apps/openagents-macos/Tests/e2e-host.coax" --idle-ms 1 </dev/null >/dev/null 2>&1 || true

COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/apps/openagents-macos/Tests/e2e-host.coax" --legacy-v1 --port-file "$tmp/legacy.port" --idle-ms 120000 &
legacy_pid=$!
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/apps/openagents-macos/Tests/e2e-host.coax" --port-file "$tmp/v2.port" --idle-ms 120000 &
v2_pid=$!

for _ in $(seq 1 80); do
  if [[ -s "$tmp/legacy.port" && -s "$tmp/v2.port" ]]; then break; fi
  sleep 0.1
done
legacy_port="$(awk '{print $1}' "$tmp/legacy.port")"
v2_port="$(awk '{print $1}' "$tmp/v2.port")"
echo "Live E2E hosts ready: legacy-v1=127.0.0.1:$legacy_port, v2=127.0.0.1:$v2_port"

echo "==> [3/4] Running OpenAgentsTests (unit + live dual-host NIP-44 E2E)"
cd "$repo/apps/openagents-macos"
OA_MAC_E2E_LEGACY_PORT="$legacy_port" OA_MAC_E2E_V2_PORT="$v2_port" \
  xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgentsTests -configuration Debug -sdk macosx \
  -derivedDataPath "$tmp/derived" CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- test

echo "==> [4/4] Packaging & verifying OpenAgents.dmg (bundled coder-host + agyworker + AppIcon)"
./package-dmg.sh "$tmp/dist"
dmg="$tmp/dist/OpenAgents.dmg"
test -s "$dmg"

mount_pt="$tmp/mnt"
mkdir -p "$mount_pt"
hdiutil attach "$dmg" -mountpoint "$mount_pt" -nobrowse -readonly -quiet
mounted_vol="$mount_pt"

test -d "$mount_pt/OpenAgents.app"
test -L "$mount_pt/Applications"
test -x "$mount_pt/OpenAgents.app/Contents/Resources/coder-host"
test -x "$mount_pt/OpenAgents.app/Contents/Resources/oa-worker"
test -x "$mount_pt/OpenAgents.app/Contents/Resources/agyworker"
test -s "$mount_pt/OpenAgents.app/Contents/Resources/AppIcon.icns"
./check.sh "$mount_pt/OpenAgents.app"

hdiutil detach "$mount_pt" -quiet
mounted_vol=""

echo ""
echo "ALL 4 OPENAGENTS MACOS E2E & RELEASE DMG STAGES PASSED!"
