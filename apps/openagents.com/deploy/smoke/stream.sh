#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents-web-ezxz4mgdsq-uc.a.run.app}"
SECRET="${OA_SMOKE_SECRET:?set OA_SMOKE_SECRET}"

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# -N: disable buffering on the client side.
# Note: Cloud Run/Nginx buffering issues will show up as missing incremental frames.
curl -N -fsS \
  -H "x-oa-smoke-secret: $SECRET" \
  "$BASE_URL/api/smoke/stream" \
  | tee "$TMP" >/dev/null

rg -q 'data: \[DONE\]' "$TMP"

# Expect multiple deltas (ensures this is not a single buffered chunk).
COUNT="$(rg -c 'data: \{"type":"text-delta"' "$TMP" || true)"
if [[ "$COUNT" -lt 3 ]]; then
  echo "expected >=3 text-delta frames, got $COUNT" >&2
  exit 1
fi

echo "ok: stream done ($COUNT deltas)"
