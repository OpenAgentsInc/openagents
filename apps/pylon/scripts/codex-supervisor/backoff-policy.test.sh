#!/usr/bin/env bash
#
# backoff-policy.test.sh — no-network tests for codex-supervisor idle/refusal policy.
#
# Run: bash apps/pylon/scripts/codex-supervisor/backoff-policy.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

export SUP_STATE_DIR="$WORK/state"
export SUP_BACKOFF_POLICY_DIR="$WORK/policy"
export SUP_PER_ACCOUNT=3
export SUP_MAX_SLOTS=8
export SUP_CODEX_REFUSAL_TUNE_THRESHOLD=3
export SUP_CLAIMED_DEEP_BACKLOG_SLEEP_SECS=1
export SUP_FAILURE_BACKOFF_ESCALATE_THRESHOLD=2
export SUP_LOCKOUT_BACKOFF_MAX=120
export SUP_LOCKOUT_IDLE_SECS=2
export SUP_REPLENISHMENT_LOCKOUTS=3
mkdir -p "$SUP_STATE_DIR" "$SUP_BACKOFF_POLICY_DIR"

# shellcheck source=backoff-policy.sh
source "$SCRIPT_DIR/backoff-policy.sh"

slots="$(sup_expand_account_slots codex codex-2 | tr '\n' ' ')"
if [ "$slots" = "codex codex codex codex-2 codex-2 codex-2 " ]; then
  ok "account slots expand to SUP_PER_ACCOUNT per ready account"
else
  bad "unexpected expanded slots: '$slots'"
fi

sup_set_account_effective_limit codex 2
slots="$(sup_expand_account_slots codex codex-2 | tr '\n' ' ')"
if [ "$slots" = "codex codex codex-2 codex-2 codex-2 " ]; then
  ok "tuned account limit shrinks only that account"
else
  bad "unexpected tuned slots: '$slots'"
fi

printf '%s' '{"error":"codex_agent_execution_refused"}' > "$WORK/executor-refused.json"
printf '%s' '{"error":"target_pylon_unavailable"}' > "$WORK/gate-refused.json"
printf '%s' '{"error":"pylon_api_conflict"} HTTP 409' > "$WORK/gate-conflict.json"
printf '%s' '{"blockerRefs":["blocker.public.pylon_dispatch.duplicate_active_assignment"]}' > "$WORK/duplicate-active.json"
printf '%s' '503 could not read linked owner registration' > "$WORK/d1-flake.txt"
printf '%s' 'HTTP 429 too many requests' > "$WORK/rate.txt"
printf '%s' 'plain failure' > "$WORK/other.txt"

if [ "$(sup_dispatch_failure_signature "$WORK/executor-refused.json")" = "codex_agent_execution_refused" ]; then
  ok "executor refusal is classified distinctly"
else
  bad "executor refusal classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/gate-refused.json")" = "dispatch_gate_conflict" ]; then
  ok "target_pylon_unavailable is classified as fast dispatch gate conflict"
else
  bad "gate refusal classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/gate-conflict.json")" = "dispatch_gate_conflict" ]; then
  ok "pylon_api_conflict/409 is classified as fast dispatch gate conflict"
else
  bad "pylon_api_conflict classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/duplicate-active.json")" = "refused" ]; then
  ok "duplicate_active_assignment stays parked as refused"
else
  bad "duplicate_active_assignment classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/d1-flake.txt")" = "dispatch_gate_transient" ]; then
  ok "500/503 D1 read flakes are classified as fast dispatch gate transient"
else
  bad "D1 flake classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/rate.txt")" = "rate_limited" ]; then
  ok "429/rate limit remains rate_limited"
else
  bad "rate limit classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/other.txt")" = "other" ]; then
  ok "unknown failure is classified as other"
else
  bad "other classification failed"
fi

sup_set_account_effective_limit codex 3
first="$(sup_record_account_refusal codex codex_agent_execution_refused)"
second="$(sup_record_account_refusal codex codex_agent_execution_refused)"
third="$(sup_record_account_refusal codex codex_agent_execution_refused)"
if [ -z "$first" ] && [ -z "$second" ] &&
   printf '%s' "$third" | grep -q 'limit_tuned=3->2' &&
   [ "$(sup_account_effective_limit codex)" = "2" ]; then
  ok "repeated executor refusals tune account limit down one slot"
else
  bad "executor refusal tuning failed: first='$first' second='$second' third='$third' limit=$(sup_account_effective_limit codex)"
fi

before="$(sup_account_effective_limit codex)"
sup_record_account_refusal codex rate_limited >/dev/null
after="$(sup_account_effective_limit codex)"
if [ "$before" = "$after" ]; then
  ok "rate limits do not tune local account concurrency"
else
  bad "rate limit changed account limit: $before -> $after"
fi

if [ "$(sup_claimed_pick_backoff_secs 12 4 15)" = "1" ]; then
  ok "deep open backlog uses near-immediate claimed-pick retry"
else
  bad "deep backlog retry did not shorten"
fi

if [ "$(sup_claimed_pick_backoff_secs 4 4 15)" = "15" ]; then
  ok "shallow/equal backlog keeps normal backoff"
else
  bad "shallow backlog should keep normal backoff"
fi

if [ "$(sup_lockout_pick_backoff_secs 4 4 300 2)" = "120" ]; then
  ok "lockout pick backoff caps minute-scale idle waits"
else
  bad "lockout pick backoff did not cap at SUP_LOCKOUT_BACKOFF_MAX"
fi

if [ "$(sup_lockout_pick_backoff_secs 12 4 300 2)" = "1" ]; then
  ok "deep backlog near-immediate retry still wins under lockout cap"
else
  bad "lockout cap should preserve deep backlog immediate retry"
fi

if [ "$(sup_lockout_pick_backoff_secs 4 4 120 3)" = "2" ]; then
  ok "sustained lockout stays on short replenishment cadence"
else
  bad "sustained lockout should not keep escalating backoff"
fi

if [ "$(sup_lockout_pick_backoff_secs 4 4 15 2)" = "15" ]; then
  ok "pre-replenishment lockout keeps normal backoff policy"
else
  bad "pre-replenishment lockout should keep normal backoff policy"
fi

if sup_should_escalate_failure_backoff refused 1; then
  bad "first genuine refusal should not escalate backoff"
else
  ok "first genuine refusal does not escalate backoff"
fi

if sup_should_escalate_failure_backoff refused 2; then
  ok "repeated genuine refusal escalates backoff"
else
  bad "repeated genuine refusal should escalate backoff"
fi

if sup_should_escalate_failure_backoff codex_agent_execution_refused 9; then
  bad "executor refusal should not escalate general backoff"
else
  ok "executor refusal never escalates general backoff"
fi

if sup_should_escalate_failure_backoff dispatch_gate_conflict 9; then
  bad "dispatch gate conflict should not escalate general backoff"
else
  ok "dispatch gate conflict never escalates general backoff"
fi

if sup_should_escalate_failure_backoff dispatch_gate_transient 9; then
  bad "dispatch gate transient should not escalate general backoff"
else
  ok "dispatch gate transient never escalates general backoff"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
