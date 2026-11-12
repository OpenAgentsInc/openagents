#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper with no logic: copies the recorded responses to a fresh folder, because a replay keeps its place in its folder, then runs the rlm tool on them with GW_REPLAY set)
# The RLM over the gateway's replay path (coax/lib/gateway/transport.coax, GW_REPLAY): a live
# run recorded with GW_RECORD (fixtures/replay/*.http: the root's turns and a batch of
# sub-calls on Vertex's native stream), replayed with nothing sent. The tool exits 3 unless
# the run's answer is the recorded run's count (--must-score), so this fails when the driver,
# the interpreter or the gateway parse a recorded answer differently.
#   coax/lib/rlm/replay.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
fx="$here/fixtures/replay"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cp "$fx"/*.http "$tmp/"
GW_REPLAY="$tmp" "$repo/coax/web/tool.sh" "$here/rlm.coax" ask --file "$fx/context.txt" \
  --question-file "$fx/question.txt" --expect "$(cat "$fx/expect.txt")" --must-score
