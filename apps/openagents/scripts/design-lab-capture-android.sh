#!/usr/bin/env bash
# Captures every screen of the current Android app for Design Lab.
#
# Drives the debug app's offline demo (no account, no network) on a running
# emulator or device through adb, once in light and once in dark mode, and
# saves the shots in $DESIGN_LAB_CAPTURES/android/<time>-<commit>/ (default
# ~/work/design-lab/captures/android) as `<area>--<screen>[@dark].png`, each
# with its element tree (`.json`, frames in dp) read from uiautomator. Each run
# is one version of every Android screen; `cargo run -p design-lab -- serve`
# shows them under Android.
#
#   apps/openagents/scripts/design-lab-capture-android.sh [--apk PATH] [--serial SERIAL]
# Install the debug APK first (`scripts/android/app.sh apk` builds it), or pass
# --apk to install one. ADB (default: adb on PATH, else the SDK's) picks the
# device; --serial chooses one when several are attached.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CAPTURES="${DESIGN_LAB_CAPTURES:-$HOME/work/design-lab/captures}/android"
COMMIT="$(git -C "$ROOT" rev-parse --short=10 HEAD)"
DIRTY=""
git -C "$ROOT" diff --quiet HEAD -- . || DIRTY="-dirty"
RUN="$CAPTURES/$(date +%Y%m%d-%H%M%S)-$COMMIT$DIRTY"
mkdir -p "$RUN"
# Design Lab ignores this run for "current" until the marker is gone.
touch "$RUN/.capturing"
trap 'rm -f "$RUN/.capturing"' EXIT

ADB="${ADB:-$(command -v adb || echo "${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb")}"
python3 -I "$ROOT/scripts/android/design_lab_tour.py" --adb "$ADB" --out "$RUN" "$@" \
  || echo "design-lab-capture-android: some screens failed; keeping the ones taken" >&2

COUNT="$(find "$RUN" -name '*.png' | wc -l | tr -d ' ')"
if [ "$COUNT" = 0 ]; then
  rm -f "$RUN/.capturing"
  rmdir "$RUN"
  echo "design-lab-capture-android: no screens were captured" >&2
  exit 1
fi
echo "design-lab-capture-android: $COUNT pictures in $RUN"
