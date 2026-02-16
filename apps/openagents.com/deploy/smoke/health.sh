#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents-web-ezxz4mgdsq-uc.a.run.app}"

curl -fsS "$BASE_URL/up" >/dev/null

echo "ok: $BASE_URL/up"
