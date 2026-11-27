#!/usr/bin/env bash
# Firm reason: 5 (thin Simulator test harness orchestrating Coax Cloud checks, Simulator build, binary cleanliness check, and XCTest unit + UI suites).
# Usage: bash apps/openagents-ios/e2e-sim-test.sh [SIMULATOR_UDID]
set -euo pipefail

cd "$(dirname "$0")"
ios_dir="$(pwd -P)"
repo="$(cd ../.. && pwd -P)"
coax="${COAX:-$repo/coax/bin/coax}"

echo "==> [1/4] Running Coax iOS Cloud root & GPU surface tests..."
if [[ -x "$coax" ]]; then
  "$coax" test "$ios_dir/ios-app.coax"
  "$coax" test "$ios_dir/gpu-page.coax"
else
  "$repo/coax/remote/remote.sh" run -- coax/bin/coax test apps/openagents-ios/ios-app.coax
  "$repo/coax/remote/remote.sh" run -- coax/bin/coax test apps/openagents-ios/gpu-page.coax
fi

echo "==> [2/4] Building OpenAgents iOS for Simulator & running binary cleanliness check..."
./build.sh
./check.sh

sim_id="${1:-}"
if [[ -z "$sim_id" ]]; then
  sim_id="$(xcrun simctl list devices booted -j | /usr/bin/python3 -c '
import json, sys
d = json.load(sys.stdin).get("devices", {})
for rt, devs in d.items():
    if "iOS" not in rt:
        continue
    for dev in devs:
        if dev.get("state") == "Booted" and dev.get("isAvailable", True):
            print(dev["udid"]); sys.exit(0)
' 2>/dev/null || true)"
fi
if [[ -z "$sim_id" ]]; then
  sim_id="$(xcrun simctl list devices available -j | /usr/bin/python3 -c '
import json, sys
d = json.load(sys.stdin).get("devices", {})
for rt, devs in sorted(d.items(), reverse=True):
    if "iOS" not in rt:
        continue
    for dev in devs:
        if "iPhone" in dev.get("name", "") and dev.get("isAvailable", True):
            print(dev["udid"]); sys.exit(0)
' 2>/dev/null || true)"
  [[ -n "$sim_id" ]] || { echo "e2e-sim-test: no available iOS Simulator found" >&2; exit 2; }
  xcrun simctl boot "$sim_id" || true
fi
xcrun simctl bootstatus "$sim_id" -b >/dev/null 2>&1 || true
echo "    Using iOS Simulator: $sim_id"

echo "==> [3/4] Running OpenAgentsTests (unit, Cloud Chat v2, Tier 1 tools, placement, Full Auto, Voice Duplex)..."
"$repo/coax/remote/remote.sh" run --local -- xcodebuild \
  -project OpenAgents.xcodeproj \
  -scheme OpenAgentsTests \
  -configuration Debug \
  -destination "platform=iOS Simulator,id=$sim_id" \
  -derivedDataPath build \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- \
  OA_IOS_BUNDLE_ID=com.openagents.app.e2etest \
  test

echo "==> [4/4] Running OpenAgentsUITests on Simulator..."
"$repo/coax/remote/remote.sh" run --local -- xcodebuild \
  -project OpenAgents.xcodeproj \
  -scheme OpenAgentsUITests \
  -configuration Debug \
  -destination "platform=iOS Simulator,id=$sim_id" \
  -derivedDataPath build \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- \
  OA_IOS_BUNDLE_ID=com.openagents.app.e2etest \
  test

echo "e2e-sim-test: ALL 4 STAGES PASSED!"
