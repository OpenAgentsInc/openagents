#!/usr/bin/env bash
# Captures every screen of the current phone app for Design Lab.
#
# Runs DesignLabTourTests on the offline demo in the simulator, once in light
# and once in dark mode, and saves the shots in
# $DESIGN_LAB_CAPTURES/<time>-<commit>/ (default
# ~/work/design-lab/captures). Each run is one version of every screen;
# `cargo run -p design-lab -- serve` shows them all.
#
#   apps/openagents/scripts/design-lab-capture.sh [simulator name or id]
# The default simulator is made once with:
#   xcrun simctl create "Design Lab iPhone" "iPhone 17 Pro Max"
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SIM="${1:-Design Lab iPhone}"
CAPTURES="${DESIGN_LAB_CAPTURES:-$HOME/work/design-lab/captures}"
COMMIT="$(git -C "$ROOT" rev-parse --short=10 HEAD)"
DIRTY=""
git -C "$ROOT" diff --quiet HEAD -- . || DIRTY="-dirty"
RUN="$CAPTURES/$(date +%Y%m%d-%H%M%S)-$COMMIT$DIRTY"
mkdir -p "$RUN"
# Design Lab ignores this run for "current" until the marker is gone.
touch "$RUN/.capturing"
trap 'rm -f "$RUN/.capturing"' EXIT

# The simulator's id, so its appearance can be set.
UDID="$SIM"
if ! [[ "$SIM" =~ ^[0-9A-F-]{36}$ ]]; then
  UDID="$(xcrun simctl list devices available -j | python3 -c '
import json, sys
name = sys.argv[1]
for devices in json.load(sys.stdin)["devices"].values():
    for d in devices:
        if d["name"] == name:
            print(d["udid"]); sys.exit()
' "$SIM")"
fi
[ -n "$UDID" ] || { echo "design-lab-capture: no simulator named $SIM" >&2; exit 1; }
xcrun simctl boot "$UDID" 2>/dev/null || true

cd "$ROOT"
for APPEARANCE in light dark; do
  xcrun simctl ui "$UDID" appearance "$APPEARANCE"
  RUSTUP_TOOLCHAIN=1.98.1 TEST_RUNNER_DESIGN_LAB_DIR="$RUN" \
    TEST_RUNNER_DESIGN_LAB_APPEARANCE="$APPEARANCE" \
    xcodebuild -project apps/ios/OpenAgents.xcodeproj -scheme OpenAgents \
    -destination "platform=iOS Simulator,id=$UDID" ARCHS=arm64 \
    CODE_SIGNING_ALLOWED=NO -only-testing:OpenAgentsUITests/DesignLabTourTests test \
    || echo "design-lab-capture: some $APPEARANCE screens failed; keeping the ones taken" >&2
done
xcrun simctl ui "$UDID" appearance light

COUNT="$(find "$RUN" -name '*.png' | wc -l | tr -d ' ')"
if [ "$COUNT" = 0 ]; then
  rm -f "$RUN/.capturing"
  rmdir "$RUN"
  echo "design-lab-capture: no screens were captured" >&2
  exit 1
fi
echo "design-lab-capture: $COUNT screens in $RUN"
