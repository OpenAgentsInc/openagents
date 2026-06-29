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
printf '%s' 'HTTP 429 too many requests' > "$WORK/rate.txt"
printf '%s' 'plain failure' > "$WORK/other.txt"

if [ "$(sup_dispatch_failure_signature "$WORK/executor-refused.json")" = "codex_agent_execution_refused" ]; then
  ok "executor refusal is classified distinctly"
else
  bad "executor refusal classification failed"
fi

if [ "$(sup_dispatch_failure_signature "$WORK/gate-refused.json")" = "refused" ]; then
  ok "gate refusal remains generic refused"
else
  bad "gate refusal classification failed"
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

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
