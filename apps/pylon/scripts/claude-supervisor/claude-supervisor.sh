#!/usr/bin/env bash
#
# claude-supervisor.sh
#
# Durable, around-the-clock supervisor for the Khala -> Pylon -> Claude
# own-capacity coding delegation lane (Claude Agent SDK; the Claude-side
# analogue of the "Khala -> Pylon -> Codex Coding Delegation Runbook" in
# CLAUDE.md / AGENTS.md). It maintains a saturated pool of no-spend Claude
# coding sessions and refills each slot as it finishes, so the public Khala
# token counter keeps climbing from the owner's own local Claude capacity.
# NO paid API, NO spend, own-capacity only.
#
# ARCHITECTURE (account-aware N-worker pool):
#   * Enumerates READY Claude accounts (`pylon accounts list --json`, filtered
#     to provider=claude + readiness=ready).
#   * Target concurrency = min(SUP_MAX_SLOTS, ready_accounts * SUP_PER_ACCOUNT).
#     With 4 ready Claude logins and SUP_PER_ACCOUNT=2 this is >=8 concurrent.
#   * Runs SUP_MAX_SLOTS worker loops. Each active worker continuously fires a
#     `khala request --workflow claude_agent_task` (which auto-runs the matching
#     local `assignment run-no-spend` to closeout), pinned to a real backlog
#     issue + the current origin/main commit, round-robined across ready
#     accounts via `--account-ref`. When a session closes, the worker IMMEDIATELY
#     fires the next one -> continuous refill.
#   * A background heartbeater republishes presence + advertised Claude capacity
#     on a timer so presence never goes stale (#6354) and the dispatch gate sees
#     current availability.
#
# WHY THE PLATFORM TOOLS ALONE ARE NOT ENOUGH:
#   `khala burndown` / `khala spawn` cap lanes at the number of ready accounts
#   (one lane per account), so they cannot do >1 same-account parallel session.
#   This supervisor drives same-account parallelism (SUP_PER_ACCOUNT>1) via
#   independent worker loops, which is the proven manual-parallel path.
#
# SELF-THROTTLING (critical):
#   A single Claude login has a real concurrency ceiling. When the
#   dispatch gate refuses (HTTP 409 / target_pylon_unavailable / rate limit), the
#   worker backs off exponentially. The pool therefore self-settles at the
#   login's true headroom instead of rate-limit-fighting itself or other drivers.
#   => Do NOT run this as a second driver against a login that another runner
#      is already saturating; it will only contend.
#      The throughput win comes from MORE DISTINCT logins, not more runners.
#
# CONSERVATIVE DEFAULT FLEET SIZE:
#   The default SUP_MAX_SLOTS is intentionally a conservative 6 so this lane is
#   proven on 2-3 Claude accounts before fanning out a giant fleet. Once the
#   Claude own-capacity lane is proven healthy, the owner can raise SUP_MAX_SLOTS
#   (e.g. `SUP_MAX_SLOTS=12 ./launch.sh`).
#
# HARD SAFETY:
#   * NEVER runs `codex login` / `pylon auth` (codex OR claude). Only READS
#     existing logins. If it detects the owner's live session was broken
#     ("access token could not be refreshed" / "sign in again"), it PAUSES and
#     writes a NEEDS_OWNER note.
#   * Never touches the owner's live ~/.codex home.
#   * Uses PYLON_DISABLE_DAEMON_ROUTING=1 so a stale `pylon node` cannot answer
#     with old source.
#   * No spend / payout / destructive git on the live repo: each assignment runs
#     in its own bounded throwaway workspace materialized by Pylon.
#
# Stop with:  kill "$(cat "$SUP_STATE_DIR/supervisor.pid")"
#
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

# shellcheck source=../supervisor-task-pool.sh
source "$SCRIPT_DIR/../supervisor-task-pool.sh"
# Fresh-base resolver (#6719 deletion-poison fix): every dispatch must base on
# CURRENT origin/main, never the supervisor's stale local HEAD.
# shellcheck source=../supervisor-fresh-base.sh
source "$SCRIPT_DIR/../supervisor-fresh-base.sh"

# --- Config (all overridable via env) ---
export PYLON_OPENAGENTS_BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"
# Default pylon home is the registered owner home (~/.pylon). A fresh/unknown
# home 404s on heartbeat (no registration); do not point this at an unregistered
# home, and do not run `pylon auth openagents` here to create one.
export PYLON_HOME="${PYLON_HOME:-$HOME/.pylon}"
export PYLON_DISABLE_DAEMON_ROUTING="${PYLON_DISABLE_DAEMON_ROUTING:-1}"

# The token-linked owner Pylon to target. The local home's resolved target ref
# can drift from the server registration, so we pin it explicitly.
SUP_PYLON_REF="${SUP_PYLON_REF:-pylon.33afd48282a649047e3a}"

SUP_STATE_DIR="${SUP_STATE_DIR:-$HOME/.claude-supervisor}"
SUP_LOG="${SUP_LOG:-$SUP_STATE_DIR/supervisor.log}"

# Dispatch-lockout helpers (shared with codex-supervisor): skip issues that
# already have an open PR so the Claude lane does not duplicate work that the
# high-throughput Codex lane already solved.
# shellcheck source=../codex-supervisor/lockout.sh
source "$SCRIPT_DIR/../codex-supervisor/lockout.sh"

# Same-account parallel sessions per READY Claude login (owner intent: >=2/acct).
SUP_PER_ACCOUNT="${SUP_PER_ACCOUNT:-2}"
# Hard ceiling on total concurrent sessions across all accounts. Conservative
# default (6) until the Claude own-capacity lane is proven on 2-3 accounts; the
# owner can raise it once proven (e.g. SUP_MAX_SLOTS=12).
SUP_MAX_SLOTS="${SUP_MAX_SLOTS:-6}"
SUP_REPO="${SUP_REPO:-OpenAgentsInc/openagents}"
# Lightweight, sanctioned-shape verification run inside each throwaway workspace.
SUP_VERIFY="${SUP_VERIFY:-bun run --cwd apps/openagents.com/workers/api test -- src/labor-earnings-routes.test.ts}"
# Presence heartbeat cadence (s) — keeps presence fresh + capacity advertised.
SUP_HEARTBEAT_SECS="${SUP_HEARTBEAT_SECS:-45}"
# Backoff bounds for refused/rate-limited dispatch.
SUP_BACKOFF_MIN="${SUP_BACKOFF_MIN:-15}"
SUP_BACKOFF_MAX="${SUP_BACKOFF_MAX:-300}"

# --- Self-heal watchdog (#6408): "fleet never silently stalls". ---
# If the pool sees this many consecutive NO-DISPATCH lines with ZERO OK in
# between, the loop has stalled (e.g. the dispatch gate is poisoned into
# duplicate_active_assignment by our own abandoned leases). Instead of backing
# off into silence, the watchdog logs a LOUD FLEET-STALL line and self-recovers:
# (a) re-asserts advertisement via `provider go-online`, (b) sweeps our own
# interrupted/stale local leases via `assignment run-no-spend` so they stop
# poisoning the gate. Then it cools down before checking again.
SUP_STALL_REFUSALS="${SUP_STALL_REFUSALS:-20}"
SUP_SELFHEAL_COOLDOWN_SECS="${SUP_SELFHEAL_COOLDOWN_SECS:-300}"
SUP_SELFHEAL_CHECK_SECS="${SUP_SELFHEAL_CHECK_SECS:-30}"

# Fallback only. The active task pool is resolved dynamically from the
# unsupported-request ledger and linked open GitHub issues.
SUP_TASK_POOL_FALLBACK_ISSUES="${SUP_TASK_POOL_FALLBACK_ISSUES:-${SUP_ISSUES:-6310 6311 6320 6354 6355 6358}}"

PYLON=(bun "$REPO_ROOT/apps/pylon/src/index.ts")
mkdir -p "$SUP_STATE_DIR"
DESIRED_FILE="$SUP_STATE_DIR/desired-slots"
PAUSE_FILE="$SUP_STATE_DIR/paused"
echo 0 > "$DESIRED_FILE"
rm -f "$PAUSE_FILE"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >> "$SUP_LOG"; }

bound_log() {
  if [ -f "$SUP_LOG" ] && [ "$(wc -c < "$SUP_LOG" 2>/dev/null || echo 0)" -gt 4194304 ]; then
    tail -c 2097152 "$SUP_LOG" > "$SUP_LOG.tmp" 2>/dev/null && mv "$SUP_LOG.tmp" "$SUP_LOG"
  fi
}

counter_now() {
  curl -fsS "$PYLON_OPENAGENTS_BASE_URL/api/public/khala-tokens-served" 2>/dev/null \
    | sed -n 's/.*"tokensServed":\([0-9]*\).*/\1/p' | head -1
}

# Print ready Claude account refs, one per line. Default-home account (accountRef
# null) is printed as the literal token "default".
ready_claude_account_refs() {
  "${PYLON[@]}" accounts list --json 2>/dev/null | python3 -c "import sys,json
try: d=json.load(sys.stdin)
except Exception: sys.exit(0)
for a in d.get('accounts',[]):
    if a.get('provider') in ('claude_agent','claude') and a.get('readiness',{}).get('state')=='ready':
        print(a.get('accountRef') or 'default')" 2>/dev/null
}

owner_session_broken() {
  grep -qiE "access token could not be refreshed|please sign in again|reauthenticate" "$@" 2>/dev/null
}

global_pause() {
  touch "$PAUSE_FILE"
  log "!!! GLOBAL PAUSE: owner Claude session appears broken; not hammering."
  {
    echo ""
    echo "## NEEDS-OWNER ($(date -u +%Y-%m-%dT%H:%M:%SZ)): Claude login broken"
    echo "claude-supervisor saw 'access token could not be refreshed / sign in again'."
    echo "Re-authenticate the local Claude account yourself (\`pylon auth claude\`);"
    echo "the supervisor will NEVER do this. It is paused until you clear"
    echo "$PAUSE_FILE."
  } >> "$REPO_ROOT/NEEDS_OWNER.md" 2>/dev/null || true
}

# --- Self-heal watchdog: count consecutive NO-DISPATCH since the last OK. ---
# Reads the recent log tail; resets on an OK turn or a completed self-heal.
consecutive_refusals() {
  tail -n 800 "$SUP_LOG" 2>/dev/null | awk '
    /OK \(rc=/                          { c=0; next }
    /FLEET-STALL: self-heal complete/   { c=0; next }
    /NO-DISPATCH/                       { c++ }
    END                                 { print c+0 }'
}

selfheal_watchdog_loop() {
  while true; do
    sleep "$SUP_SELFHEAL_CHECK_SECS"
    [ -f "$PAUSE_FILE" ] && continue
    local n; n=$(consecutive_refusals)
    if [ "${n:-0}" -ge "$SUP_STALL_REFUSALS" ]; then
      log "!!!!!! FLEET-STALL: $n consecutive NO-DISPATCH with 0 OK -> self-healing (re-advertise + stale-closeout sweep)"
      # (a) re-assert advertisement so a dropped/ignored heartbeat is refreshed.
      "${PYLON[@]}" provider go-online >> "$SUP_LOG" 2>&1 || true
      # (b) clear our OWN abandoned/interrupted local leases so they stop
      #     poisoning the dispatch gate into duplicate_active_assignment. The
      #     no-spend runner submits public-safe stale closeouts at load.
      "${PYLON[@]}" assignment run-no-spend --json >> "$SUP_LOG" 2>&1 || true
      log "FLEET-STALL: self-heal complete (re-advertised + swept stale closeouts); cooldown ${SUP_SELFHEAL_COOLDOWN_SECS}s"
      sleep "$SUP_SELFHEAL_COOLDOWN_SECS"
    fi
  done
}

# --- Heartbeater: recompute desired slots + advertise capacity on a timer. ---
heartbeater_loop() {
  while true; do
    [ -f "$PAUSE_FILE" ] && { sleep "$SUP_HEARTBEAT_SECS"; continue; }
    local ready desired
    ready=$(ready_claude_account_refs | grep -c . || echo 0)
    desired=$(( ready * SUP_PER_ACCOUNT ))
    [ "$desired" -gt "$SUP_MAX_SLOTS" ] && desired="$SUP_MAX_SLOTS"
    echo "$desired" > "$DESIRED_FILE"
    OPENAGENTS_PYLON_CLAUDE_CONCURRENCY="$desired" \
    OPENAGENTS_PYLON_CLAUDE_BUSY=0 \
    OPENAGENTS_PYLON_CLAUDE_QUEUED=0 \
      "${PYLON[@]}" presence heartbeat --json >> "$SUP_LOG" 2>&1 || true
    log "heartbeat ready_claude=$ready desired_slots=$desired"
    bound_log
    sleep "$SUP_HEARTBEAT_SECS"
  done
}

# --- Worker: one continuously-refilling Claude session slot. ---
worker_loop() {
  local slot="$1"
  local backoff="$SUP_BACKOFF_MIN"
  local iter=0
  while true; do
    if [ -f "$PAUSE_FILE" ]; then sleep 30; continue; fi
    local desired; desired=$(cat "$DESIRED_FILE" 2>/dev/null || echo 0)
    if [ "$slot" -ge "$desired" ]; then sleep 10; continue; fi

    # Round-robin account across the live ready set; default omits --account-ref.
    local refs=(); while IFS= read -r r; do refs+=("$r"); done < <(ready_claude_account_refs)
    [ "${#refs[@]}" -eq 0 ] && { sleep 20; continue; }
    local acc="${refs[$(( slot % ${#refs[@]} ))]}"
    local acc_args=(); [ "$acc" != "default" ] && acc_args=(--account-ref "$acc")

    # Refresh the active pool from the unsupported-request ledger, with a short
    # cache and a bounded fallback so transient route/GitHub failures do not idle
    # the fleet.
    local issues=(); while IFS= read -r i; do issues+=("$i"); done < <(supervisor_task_pool_issues)
    if [ "${#issues[@]}" -eq 0 ]; then
      log "slot=$slot TASK-POOL empty; backing off ${backoff}s"
      sleep "$backoff"
      backoff=$(( backoff * 2 )); [ "$backoff" -gt "$SUP_BACKOFF_MAX" ] && backoff="$SUP_BACKOFF_MAX"
      continue
    fi

    # Dispatch lockout: skip any issue that already has an open PR; only pick an
    # untouched one. If every backlog issue already has a PR, back off instead of
    # re-solving solved work.
    local start_idx=$(( (slot + iter) % ${#issues[@]} ))
    iter=$(( iter + 1 ))
    local issue
    issue=$(pick_unlocked_issue "$start_idx" "${issues[@]}")
    if [ -z "$issue" ]; then
      log "slot=$slot LOCKOUT all backlog issues already have open PRs; backing off ${backoff}s"
      sleep "$backoff"
      backoff=$(( backoff * 2 )); [ "$backoff" -gt "$SUP_BACKOFF_MAX" ] && backoff="$SUP_BACKOFF_MAX"
      continue
    fi
    # Base every assignment on FRESH origin/main, never the supervisor's stale
    # local HEAD. $REPO_ROOT can lag origin/main badly (idle clone, or another
    # agent's dirty worktree). main moves fast, so a stale base makes the worker
    # branch from an old commit and the PR diff vs current main looks like a mass
    # deletion of everything added since (the #6719 deletion-poison pattern).
    # Resolve the live remote main SHA with a pure `git ls-remote` (no
    # working-tree / local-ref mutation; safe in a dirty or shared checkout). If
    # it can't be resolved, SKIP this dispatch rather than poison it.
    local commit local_head
    commit=$(supervisor_resolve_fresh_origin_main_sha "$SUP_REPO")
    if [ -z "$commit" ]; then
      log "slot=$slot SKIP could not resolve fresh origin/main sha for $SUP_REPO; not dispatching a stale base (anti-#6719)"
      sleep 15; continue
    fi
    local_head=$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo "")
    if [ -n "$local_head" ] && [ "$local_head" != "$commit" ]; then
      log "slot=$slot BASE-DRIFT local HEAD ${local_head:0:12} != origin/main ${commit:0:12}; dispatching FRESH origin/main (anti-#6719 deletion-poison)"
    fi

    local out="$SUP_STATE_DIR/slot.$slot.json"
    "${PYLON[@]}" khala request \
      --prompt "Implement public issue #$issue and run the named verification." \
      --workflow claude_agent_task \
      --pylon-ref "$SUP_PYLON_REF" \
      "${acc_args[@]}" \
      --repo "$SUP_REPO" --branch main --commit "$commit" \
      --verify "$SUP_VERIFY" \
      --json > "$out" 2>>"$SUP_LOG"
    local rc=$?

    if owner_session_broken "$out"; then global_pause; continue; fi

    if grep -qiE '"ok": ?true|"closeout"|accepted' "$out" 2>/dev/null && [ "$rc" -eq 0 ]; then
      backoff="$SUP_BACKOFF_MIN"
      log "slot=$slot acc=$acc issue=#$issue OK (rc=$rc)"
      continue
    fi

    # Refused / unavailable / rate-limited / error -> exponential backoff so the
    # pool settles at the login's real headroom.
    local sig="other"
    grep -qiE '409|dispatch gate refused|target_pylon_unavailable' "$out" 2>/dev/null && sig="refused"
    grep -qiE '429|rate.?limit|too many requests|quota' "$out" 2>/dev/null && sig="rate_limited"
    log "slot=$slot acc=$acc issue=#$issue NO-DISPATCH ($sig rc=$rc); backoff ${backoff}s"
    sleep "$backoff"
    backoff=$(( backoff * 2 )); [ "$backoff" -gt "$SUP_BACKOFF_MAX" ] && backoff="$SUP_BACKOFF_MAX"
  done
}

cleanup() {
  log "supervisor stopping (pid $$); terminating children"
  pkill -P $$ 2>/dev/null
  rm -f "$SUP_STATE_DIR/supervisor.pid"
  exit 0
}
trap cleanup INT TERM

# --- Preconditions ---
if [ -z "${OPENAGENTS_AGENT_TOKEN:-}" ]; then
  echo "FATAL: OPENAGENTS_AGENT_TOKEN is not set" >&2
  exit 1
fi

echo $$ > "$SUP_STATE_DIR/supervisor.pid"
log "=== claude-supervisor START pid=$$ repo=$REPO_ROOT pylon=$SUP_PYLON_REF per_account=$SUP_PER_ACCOUNT max_slots=$SUP_MAX_SLOTS ==="

"${PYLON[@]}" provider go-online >> "$SUP_LOG" 2>&1 || log "provider go-online nonzero (continuing)"

heartbeater_loop &
log "heartbeater pid=$! cadence=${SUP_HEARTBEAT_SECS}s"

selfheal_watchdog_loop &
log "selfheal-watchdog pid=$! stall_refusals=$SUP_STALL_REFUSALS cooldown=${SUP_SELFHEAL_COOLDOWN_SECS}s"

# Give the first heartbeat a moment to publish desired slots.
sleep 5

for slot in $(seq 0 $(( SUP_MAX_SLOTS - 1 ))); do
  worker_loop "$slot" &
  log "worker slot=$slot pid=$!"
done

# Periodic public-counter progress line for observability.
while true; do
  bound_log
  log "counter tokensServed=$(counter_now) desired_slots=$(cat "$DESIRED_FILE" 2>/dev/null)"
  sleep 120
done
