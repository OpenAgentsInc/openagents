#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:?set OPENAGENTS_BASE_URL (for example https://openagents.com)}"

curl -fsS "${BASE_URL}/readyz" >/dev/null

echo "[smoke] readiness: ok (${BASE_URL}/readyz)"

# Some environments may terminate /healthz at the edge (404) even when the service is healthy.
# Treat /readyz as canonical and keep /healthz as best-effort.
if ! curl -fsS "${BASE_URL}/healthz" >/dev/null 2>&1; then
  echo "[smoke] warn: ${BASE_URL}/healthz unavailable; continuing with /readyz as canonical probe" >&2
else
  echo "[smoke] health: ok (${BASE_URL}/healthz)"
fi
