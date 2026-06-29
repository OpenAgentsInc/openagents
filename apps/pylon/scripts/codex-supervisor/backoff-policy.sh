#!/usr/bin/env bash
#
# backoff-policy.sh — idle/backoff helpers for codex-supervisor (#6900).
#
# Source-only, test-friendly policy for two supervisor saturation problems:
#   * deep open backlog should not wait 15s+ just because every current top
#     candidate is claimed/settling;
#   * codex_agent_execution_refused is a per-account local executor ceiling, not
#     the same signal as a remote 429. Repeated executor refusals tune that
#     account's same-account concurrency down one slot.

: "${SUP_STATE_DIR:=$HOME/.codex-supervisor}"
: "${SUP_PER_ACCOUNT:=2}"
: "${SUP_MAX_SLOTS:=8}"
: "${SUP_BACKOFF_MIN:=15}"
: "${SUP_CODEX_REFUSAL_TUNE_THRESHOLD:=3}"
: "${SUP_CLAIMED_DEEP_BACKLOG_SLEEP_SECS:=1}"
: "${SUP_FAILURE_BACKOFF_ESCALATE_THRESHOLD:=2}"

sup_backoff_policy_dir() {
  local d="${SUP_BACKOFF_POLICY_DIR:-$SUP_STATE_DIR/backoff-policy}"
  mkdir -p "$d" 2>/dev/null || true
  printf '%s' "$d"
}

sup_account_policy_ref() {
  printf '%s' "${1:-default}" | tr -c 'A-Za-z0-9._-' '_'
}

sup_account_limit_file() {
  printf '%s/account-limit.%s' "$(sup_backoff_policy_dir)" "$(sup_account_policy_ref "$1")"
}

sup_account_refusal_file() {
  printf '%s/refusals.%s' "$(sup_backoff_policy_dir)" "$(sup_account_policy_ref "$1")"
}

sup_account_effective_limit() {
  local acc="$1"
  local limit="$SUP_PER_ACCOUNT"
  local file; file="$(sup_account_limit_file "$acc")"
  if [ -f "$file" ]; then
    local stored; stored="$(cat "$file" 2>/dev/null)"
    if [ "$stored" -ge 1 ] 2>/dev/null; then
      limit="$stored"
    fi
  fi
  [ "$limit" -lt 1 ] 2>/dev/null && limit=1
  [ "$limit" -gt "$SUP_PER_ACCOUNT" ] 2>/dev/null && limit="$SUP_PER_ACCOUNT"
  printf '%s' "$limit"
}

sup_set_account_effective_limit() {
  local acc="$1"
  local limit="$2"
  [ "$limit" -lt 1 ] 2>/dev/null && limit=1
  [ "$limit" -gt "$SUP_PER_ACCOUNT" ] 2>/dev/null && limit="$SUP_PER_ACCOUNT"
  printf '%s' "$limit" > "$(sup_account_limit_file "$acc")" 2>/dev/null || true
}

# sup_expand_account_slots <account-ref...>
#   Prints one account ref per currently admitted slot, respecting the tuned
#   per-account limit. The caller caps the final total at SUP_MAX_SLOTS.
sup_expand_account_slots() {
  local acc limit i emitted=0
  for acc in "$@"; do
    [ -n "$acc" ] || continue
    limit="$(sup_account_effective_limit "$acc")"
    for (( i=0; i<limit; i++ )); do
      [ "$emitted" -ge "$SUP_MAX_SLOTS" ] 2>/dev/null && return 0
      printf '%s\n' "$acc"
      emitted=$(( emitted + 1 ))
    done
  done
}

sup_reset_account_refusals() {
  rm -f "$(sup_account_refusal_file "$1")" 2>/dev/null || true
}

# sup_record_account_refusal <account> <signature>
#   Prints a public-safe action line when a repeated local Codex executor
#   refusal lowers that account's limit, else prints nothing.
sup_record_account_refusal() {
  local acc="$1"
  local sig="$2"
  if [ "$sig" != "codex_agent_execution_refused" ]; then
    return 0
  fi

  local file count limit next
  file="$(sup_account_refusal_file "$acc")"
  count="$(cat "$file" 2>/dev/null || echo 0)"
  [ "$count" -ge 0 ] 2>/dev/null || count=0
  count=$(( count + 1 ))
  printf '%s' "$count" > "$file" 2>/dev/null || true

  if [ "$count" -lt "$SUP_CODEX_REFUSAL_TUNE_THRESHOLD" ]; then
    return 0
  fi

  limit="$(sup_account_effective_limit "$acc")"
  if [ "$limit" -le 1 ] 2>/dev/null; then
    printf 'account=%s refusal_cause=%s repeated=%s limit_held=%s' "$acc" "$sig" "$count" "$limit"
    return 0
  fi

  next=$(( limit - 1 ))
  sup_set_account_effective_limit "$acc" "$next"
  printf 'account=%s refusal_cause=%s repeated=%s limit_tuned=%s->%s' "$acc" "$sig" "$count" "$limit" "$next"
  printf '0' > "$file" 2>/dev/null || true
}

sup_dispatch_failure_signature() {
  local out="$1"
  if grep -qiE 'codex_agent_execution_refused|execution[_ -]?refused' "$out" 2>/dev/null; then
    printf 'codex_agent_execution_refused'
    return 0
  fi
  if grep -qiE '429|rate.?limit|too many requests|quota|lockout' "$out" 2>/dev/null; then
    printf 'rate_limited'
    return 0
  fi
  if grep -qiE '409|dispatch gate refused|target_pylon_unavailable|duplicate_active_assignment' "$out" 2>/dev/null; then
    printf 'dispatch_gate_conflict'
    return 0
  fi
  printf 'other'
}

sup_claimed_pick_backoff_secs() {
  local open_count="$1"
  local desired_slots="$2"
  local current_backoff="$3"
  if [ "$open_count" -gt "$desired_slots" ] 2>/dev/null && [ "$desired_slots" -gt 0 ] 2>/dev/null; then
    printf '%s' "$SUP_CLAIMED_DEEP_BACKLOG_SLEEP_SECS"
    return 0
  fi
  printf '%s' "$current_backoff"
}

sup_should_escalate_failure_backoff() {
  local sig="$1"
  local repeated="$2"
  if [ "$sig" = "codex_agent_execution_refused" ]; then
    return 1
  fi
  if [ "$sig" = "dispatch_gate_conflict" ]; then
    return 1
  fi
  [ "$repeated" -ge "$SUP_FAILURE_BACKOFF_ESCALATE_THRESHOLD" ] 2>/dev/null
}
