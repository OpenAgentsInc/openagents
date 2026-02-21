#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:?set OPENAGENTS_BASE_URL (for example https://openagents.com)}"

curl -fsS "${BASE_URL}/healthz" >/dev/null
curl -fsS "${BASE_URL}/readyz" >/dev/null

echo "ok: ${BASE_URL}/healthz and ${BASE_URL}/readyz"
