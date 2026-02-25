#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

if ! command -v rg >/dev/null 2>&1; then
  echo "missing required command: rg" >&2
  exit 2
fi

scan_targets=(
  "apps/autopilot-desktop/src/main.rs"
  "apps/runtime/src/lib.rs"
  "apps/runtime/src/spacetime_publisher.rs"
  "apps/openagents.com/service/src/lib.rs"
  "apps/openagents.com/service/src/route_domains.rs"
)

blocked_patterns=(
  "/sync/socket/websocket"
  "/api/spacetime/token"
  "/api/v1/sync/token"
  "/api/v1/spacetime/token"
  "phx_join"
  "phx_reply"
  "sync:update_batch"
)

failures=0

for pattern in "${blocked_patterns[@]}"; do
  if matches="$(rg -n --color never "$pattern" "${scan_targets[@]}" || true)"; then
    if [[ -n "$matches" ]]; then
      echo "blocked legacy sync symbol detected: $pattern" >&2
      echo "$matches" >&2
      failures=$((failures + 1))
    fi
  fi
done

if (( failures > 0 )); then
  echo "spacetime-only symbol verification failed (${failures} pattern hit(s))." >&2
  exit 1
fi

echo "spacetime-only symbol verification passed."
