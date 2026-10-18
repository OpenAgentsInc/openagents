#!/usr/bin/env bash
# Captures every screen of the current desktop app for Design Lab.
#
# Runs the design-lab-tour example (crates/ui/examples/design-lab-tour.rs),
# which draws each screen offscreen with fixture data in light and dark mode;
# no window opens and no account or engine is used. The shots go in
# $DESIGN_LAB_CAPTURES/desktop/<time>-<commit>/ (default
# ~/work/design-lab/captures/desktop). Each run is one version of every
# desktop screen; `cargo run -p design-lab -- serve` shows them under Desktop.
#
#   apps/openagents/scripts/design-lab-capture-desktop.sh [--only NAME[,NAME...]]
# Extra arguments go to the tour: `--only chat--runs-on` or `--only settings`
# takes some screens, `--light` or `--dark` one mode.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CAPTURES="${DESIGN_LAB_CAPTURES:-$HOME/work/design-lab/captures}/desktop"
COMMIT="$(git -C "$ROOT" rev-parse --short=10 HEAD)"
DIRTY=""
git -C "$ROOT" diff --quiet HEAD -- . || DIRTY="-dirty"
RUN="$CAPTURES/$(date +%Y%m%d-%H%M%S)-$COMMIT$DIRTY"
mkdir -p "$RUN"
# Design Lab ignores this run for "current" until the marker is gone.
touch "$RUN/.capturing"
trap 'rm -f "$RUN/.capturing"' EXIT

cd "$ROOT"
cargo run -p openagents-ui --example design-lab-tour --features account-fixture -- "$RUN" "$@" \
  || echo "design-lab-capture-desktop: some screens failed; keeping the ones taken" >&2

COUNT="$(find "$RUN" -name '*.png' | wc -l | tr -d ' ')"
if [ "$COUNT" = 0 ]; then
  rm -f "$RUN/.capturing"
  rmdir "$RUN"
  echo "design-lab-capture-desktop: no screens were captured" >&2
  exit 1
fi
echo "design-lab-capture-desktop: $COUNT pictures in $RUN"
