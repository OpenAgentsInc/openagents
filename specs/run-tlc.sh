#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

passing_configs=(
  "specs/khala-fleet-delegate/FleetDelegateSupervisor.cfg"
  "specs/khala-fleet-delegate/FleetDelegateSupervisorLiveness.cfg"
  "specs/approval-protocol/ApprovalProtocol.cfg"
  "specs/session-thread-mapping/SessionThreadMapping.cfg"
)

failing_configs=(
  "specs/mutations/fleet-paused-claim/FleetPausedClaim.cfg"
  "specs/mutations/approval-stale-forgery/ApprovalStaleForgery.cfg"
  "specs/mutations/session-crash-divergence/SessionCrashDivergence.cfg"
)

if ! command -v tlc >/dev/null 2>&1; then
  echo "tlc is not installed. Install the TLA+ tools and rerun specs/run-tlc.sh." >&2
  exit 127
fi

for cfg in "${passing_configs[@]}"; do
  echo "==> TLC PASS ${cfg}"
  tla="${cfg%.cfg}.tla"
  if [ "$cfg" = "specs/khala-fleet-delegate/FleetDelegateSupervisorLiveness.cfg" ]; then
    tla="specs/khala-fleet-delegate/FleetDelegateSupervisor.tla"
  fi
  (cd "$ROOT" && tlc -config "$cfg" "$tla")
done

for cfg in "${failing_configs[@]}"; do
  echo "==> TLC EXPECT-VIOLATION ${cfg}"
  tmp="$(mktemp)"
  set +e
  (cd "$ROOT" && tlc -config "$cfg" "${cfg%.cfg}.tla") >"$tmp" 2>&1
  status=$?
  set -e
  if [ "$status" -eq 0 ]; then
    cat "$tmp"
    rm -f "$tmp"
    echo "expected TLC violation for mutation ${cfg}, but it passed" >&2
    exit 1
  fi
  if ! grep -Eq "Invariant .* is violated|Temporal properties were violated" "$tmp"; then
    cat "$tmp"
    rm -f "$tmp"
    echo "mutation ${cfg} failed without a TLC counterexample violation" >&2
    exit 1
  fi
  grep -E "Invariant .* is violated|Temporal properties were violated" "$tmp" | head -n 1
  rm -f "$tmp"
done
