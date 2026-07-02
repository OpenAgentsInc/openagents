#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

configs=(
  "specs/khala-fleet-delegate/FleetDelegateSupervisor.cfg"
  "specs/approval-protocol/ApprovalProtocol.cfg"
  "specs/session-thread-mapping/SessionThreadMapping.cfg"
)

if ! command -v tlc >/dev/null 2>&1; then
  echo "tlc is not installed. Install the TLA+ tools and rerun specs/run-tlc.sh." >&2
  exit 127
fi

for cfg in "${configs[@]}"; do
  echo "==> TLC ${cfg}"
  (cd "$ROOT" && tlc -deadlock -config "$cfg" "${cfg%.cfg}.tla")
done
