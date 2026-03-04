#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TRACKER="$ROOT_DIR/docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md"
IMPLEMENTATION_LOG="$ROOT_DIR/docs/AUTOPILOT_EARN_MVP_IMPLEMENTATION_LOG.md"

require_contains() {
    local file="$1"
    local needle="$2"
    local description="$3"
    if ! rg --fixed-strings --quiet "$needle" "$file"; then
        printf 'ERROR: %s (missing "%s" in %s)\n' "$description" "$needle" "$file" >&2
        exit 1
    fi
}

require_contains "$TRACKER" "## Reconciliation Follow-On Issues" "tracker must include reconciliation follow-on section"
require_contains "$TRACKER" "#2886" "tracker must include reconciliation issue #2886"
require_contains "$TRACKER" "#2890" "tracker must include reconciliation issue #2890"
require_contains "$TRACKER" "| [#2886](https://github.com/OpenAgentsInc/openagents/issues/2886) | Earn P1: Reconcile epic tracker closure with code evidence | CLOSED |" "tracker must show #2886 closed state"
require_contains "$TRACKER" "| [#2887](https://github.com/OpenAgentsInc/openagents/issues/2887) | Earn P2: Failure taxonomy + user-facing diagnostics | CLOSED |" "tracker must show #2887 closed state"
require_contains "$TRACKER" "| [#2888](https://github.com/OpenAgentsInc/openagents/issues/2888) | Earn P2: Loop integrity SLO metrics and alerts | OPEN |" "tracker must show remaining open reconciliation issues"
require_contains "$TRACKER" '## Historical Issue List (`#2814 - #2876`)' "tracker must clearly separate historical stream"

require_contains "$IMPLEMENTATION_LOG" 'historical stream `#2815 - #2876`, reconciliation stream `#2877 - #2890`' "implementation log must declare both streams"
require_contains "$IMPLEMENTATION_LOG" "Current-status note: this file is an evidence ledger, not a completion assertion." "implementation log must avoid stale completion claims"
require_contains "$IMPLEMENTATION_LOG" "| [#2886](https://github.com/OpenAgentsInc/openagents/issues/2886) | CLOSED | Tracker/log claim reconciliation |" "implementation log must include reconciliation issue #2886 closed state"
require_contains "$IMPLEMENTATION_LOG" "| [#2887](https://github.com/OpenAgentsInc/openagents/issues/2887) | CLOSED | Failure taxonomy + diagnostics |" "implementation log must include reconciliation issue #2887 closed state"

printf 'Autopilot Earn docs reconciliation check passed.\n'
