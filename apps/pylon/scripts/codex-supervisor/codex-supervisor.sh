#!/usr/bin/env bash
#
# codex-supervisor.sh
#
# Durable, around-the-clock supervisor for the Khala -> Pylon -> Codex
# own-capacity coding delegation lane (see the "Khala -> Pylon -> Codex Coding
# Delegation Runbook" in CLAUDE.md / AGENTS.md). It maintains a saturated pool
# of no-spend Codex coding sessions and refills each slot as it finishes, so the
# public Khala token counter keeps climbing from the owner's own local Codex
# capacity. NO paid API, NO spend, own-capacity only.
#
# ARCHITECTURE (account-aware N-worker pool):
#   * Enumerates READY Codex accounts (`codex accounts list --json`).
#   * Target concurrency = min(SUP_MAX_SLOTS, ready_accounts * SUP_PER_ACCOUNT).
#     With 4 ready Codex logins and SUP_PER_ACCOUNT=2 this is >=8 concurrent.
#   * Runs SUP_MAX_SLOTS worker loops. Each active worker continuously fires a
#     `khala request --workflow codex_agent_task` (which auto-runs the matching
#     local `assignment run-no-spend` to closeout), pinned to a real backlog
#     issue + the current origin/main commit, round-robined across ready
#     accounts via `--account-ref`. When a session closes, the worker IMMEDIATELY
#     fires the next one -> continuous refill.
#   * A background heartbeater republishes presence + advertised Codex capacity
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
#   A single Codex/ChatGPT login has a real concurrency ceiling. When the
#   dispatch gate refuses (HTTP 409 / target_pylon_unavailable / rate limit), the
#   worker backs off exponentially. The pool therefore self-settles at the
#   login's true headroom instead of rate-limit-fighting itself or other drivers.
#   => Do NOT run this as a second driver against a login that another runner
#      (e.g. standing-pylon.sh) is already saturating; it will only contend.
#      The throughput win comes from MORE DISTINCT logins, not more runners.
#
# HARD SAFETY:
#   * NEVER runs `codex login` / `pylon auth codex`. Only READS existing logins.
#     If it detects the owner's live ~/.codex was broken ("access token could not
#     be refreshed" / "sign in again"), it PAUSES and writes a NEEDS_OWNER note.
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

# Dispatch-lockout helpers (issue #6439 reopen): skip issues that are CLOSED or
# already have an open PR so the fleet never re-solves resolved/already-PR'd work
# (e.g. a PR against an already-closed issue) and never spawns dupes.
# shellcheck source=lockout.sh
source "$SCRIPT_DIR/lockout.sh"
# shellcheck source=virtual-merge-queue.sh
source "$SCRIPT_DIR/virtual-merge-queue.sh"
# shellcheck source=../supervisor-task-pool.sh
source "$SCRIPT_DIR/../supervisor-task-pool.sh"
# Fresh-base resolver (#6719 deletion-poison fix): every dispatch must base on
# CURRENT origin/main, never the supervisor's stale local HEAD.
# shellcheck source=../supervisor-fresh-base.sh
source "$SCRIPT_DIR/../supervisor-fresh-base.sh"
# Fleet-saturation engine (#6711): label-priority dispatch ordering so slots
# always burn the highest tier first and FALL THROUGH locked/empty tiers, plus
# standing-task auto-recreate so the priority queue is self-sustaining.
# shellcheck source=priority-dispatch.sh
source "$SCRIPT_DIR/priority-dispatch.sh"
# shellcheck source=standing-tasks.sh
source "$SCRIPT_DIR/standing-tasks.sh"
# LOCKOUT auto-replenishment (#6822): on sustained full lockout, create/reuse
# bounded real-work issues (continual learning, audits, test sweeps) instead of
# sleeping at the max backoff with all slots idle.
# shellcheck source=replenishment.sh
source "$SCRIPT_DIR/replenishment.sh"
# Refusal-aware backoff and per-account concurrency tuning (#6900).
# shellcheck source=backoff-policy.sh
source "$SCRIPT_DIR/backoff-policy.sh"

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

SUP_STATE_DIR="${SUP_STATE_DIR:-$HOME/.codex-supervisor}"
SUP_LOG="${SUP_LOG:-$SUP_STATE_DIR/supervisor.log}"

# Same-account parallel sessions per READY Codex login (owner intent: >=2/acct).
SUP_PER_ACCOUNT="${SUP_PER_ACCOUNT:-2}"
# Hard ceiling on total concurrent sessions across all accounts.
SUP_MAX_SLOTS="${SUP_MAX_SLOTS:-8}"
SUP_REPO="${SUP_REPO:-OpenAgentsInc/openagents}"
# Lightweight, sanctioned-shape verification run inside each throwaway workspace.
SUP_VERIFY="${SUP_VERIFY:-bun run --cwd apps/openagents.com/workers/api test -- src/labor-earnings-routes.test.ts}"
# Optional comma/space separated allowlist for multi-host fleet offload. When
# set, a host only schedules the copied Codex profile refs assigned to it.
SUP_ACCOUNT_REFS="${SUP_ACCOUNT_REFS:-}"
# Presence heartbeat cadence (s) — keeps presence fresh + capacity advertised.
SUP_HEARTBEAT_SECS="${SUP_HEARTBEAT_SECS:-45}"
# Backoff bounds for refused/rate-limited dispatch.
SUP_BACKOFF_MIN="${SUP_BACKOFF_MIN:-15}"
SUP_BACKOFF_MAX="${SUP_BACKOFF_MAX:-300}"

# --- Timeout guards (#6646 wedge fix). ---
# The #1 token-burn failure mode: an external `gh`/network call in the dispatch
# loop hangs with NO timeout and silently stalls async dispatch (alive +
# heartbeating, but never dispatching) while the independent heartbeat keeps
# firing. EVERY external call is now bounded. `gh`/`curl` timeouts live in
# lockout.sh + supervisor-task-pool.sh (SUP_GH_TIMEOUT_SECS / SUP_CURL_TIMEOUT_SECS,
# via sup_run_timeout). Quick local Pylon control calls (heartbeat, go-online,
# accounts list) get a short bound; the actual labor dispatch (`khala request` /
# stale-closeout sweep) gets a generous-but-finite bound so it can never hang
# forever yet normal multi-minute Codex work is not interrupted.
SUP_PYLON_TIMEOUT_SECS="${SUP_PYLON_TIMEOUT_SECS:-60}"
SUP_DISPATCH_TIMEOUT_SECS="${SUP_DISPATCH_TIMEOUT_SECS:-1800}"

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

# --- Standing-task keeper (#6711): self-sustaining priority queue. ---
# Cadence (s) at which the supervisor re-grows any CLOSED `standing-task` issue
# (the four prio task(ops) issues) so the queue never runs dry. Recreation is
# idempotent by title, so a title that already has an open standing issue is
# never duplicated.
SUP_STANDING_TASK_CHECK_SECS="${SUP_STANDING_TASK_CHECK_SECS:-300}"

# Fallback only. The active task pool is resolved dynamically from the
# unsupported-request ledger and linked open GitHub issues.
SUP_TASK_POOL_FALLBACK_ISSUES="${SUP_TASK_POOL_FALLBACK_ISSUES:-${SUP_ISSUES:-6987 6958 6902 6637 6822 6824 6831 6656 6695 6963}}"

PYLON=(bun "$REPO_ROOT/apps/pylon/src/index.ts")
mkdir -p "$SUP_STATE_DIR"
DESIRED_FILE="$SUP_STATE_DIR/desired-slots"
PAUSE_FILE="$SUP_STATE_DIR/paused"
# Wedge telemetry (#6646): epoch-ms timestamp of the most recent dispatch
# ATTEMPT (pick+dispatch cycle). The liveness check + watcher read this to tell
# "alive but stalled" (wedged) from "alive and dispatching" (healthy).
LAST_DISPATCH_FILE="$SUP_STATE_DIR/last_dispatch_time"
HEARTBEAT_PAYLOAD_FILE="$SUP_STATE_DIR/heartbeat_payload.json"
echo 0 > "$DESIRED_FILE"
rm -f "$PAUSE_FILE"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >> "$SUP_LOG"; }

# Epoch milliseconds (portable: python3 is already a dependency here; `date`
# fallback for environments without it). BSD `date` has no %N, so do not use it.
now_epoch_ms() {
  python3 -c 'import time;print(int(time.time()*1000))' 2>/dev/null \
    || echo $(( $(date +%s) * 1000 ))
}

# Record one dispatch ATTEMPT (called each pick+dispatch cycle, before spawning
# the slot's khala request). Powers last_dispatch_time telemetry + wedge detect.
record_dispatch_attempt() {
  now_epoch_ms > "$LAST_DISPATCH_FILE" 2>/dev/null || true
}

read_last_dispatch_time() {
  cat "$LAST_DISPATCH_FILE" 2>/dev/null || echo ""
}

bound_log() {
  if [ -f "$SUP_LOG" ] && [ "$(wc -c < "$SUP_LOG" 2>/dev/null || echo 0)" -gt 4194304 ]; then
    tail -c 2097152 "$SUP_LOG" > "$SUP_LOG.tmp" 2>/dev/null && mv "$SUP_LOG.tmp" "$SUP_LOG"
  fi
}

counter_now() {
  curl -fsS --max-time "${SUP_CURL_TIMEOUT_SECS:-15}" --connect-timeout 10 \
    "$PYLON_OPENAGENTS_BASE_URL/api/public/khala-tokens-served" 2>/dev/null \
    | sed -n 's/.*"tokensServed":\([0-9]*\).*/\1/p' | head -1
}

# Print ready Codex account refs, one per line. Default-home account (accountRef
# null) is printed as the literal token "default".
ready_codex_account_refs() {
  sup_run_timeout "$SUP_PYLON_TIMEOUT_SECS" "${PYLON[@]}" codex accounts list --json 2>/dev/null | python3 -c "import os,sys,json
allow_raw=os.environ.get('SUP_ACCOUNT_REFS','').replace(',', ' ').split()
allow=set(allow_raw)
try: d=json.load(sys.stdin)
except Exception: sys.exit(0)
for a in d.get('accounts',[]):
    if a.get('provider')=='codex' and a.get('readiness',{}).get('state')=='ready':
        ref=a.get('accountRef') or 'default'
        if not allow or ref in allow:
            print(ref)" 2>/dev/null
}

owner_session_broken() {
  grep -qiE "access token could not be refreshed|please sign in again|reauthenticate" "$@" 2>/dev/null
}

global_pause() {
  touch "$PAUSE_FILE"
  log "!!! GLOBAL PAUSE: owner Codex session appears broken; not hammering."
  {
    echo ""
    echo "## NEEDS-OWNER ($(date -u +%Y-%m-%dT%H:%M:%SZ)): Codex login broken"
    echo "codex-supervisor saw 'access token could not be refreshed / sign in again'."
    echo "Re-authenticate the local Codex (~/.codex) yourself (\`codex login\`);"
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
      sup_run_timeout "$SUP_PYLON_TIMEOUT_SECS" "${PYLON[@]}" provider go-online >> "$SUP_LOG" 2>&1 || true
      # (b) clear our OWN abandoned/interrupted local leases so they stop
      #     poisoning the dispatch gate into duplicate_active_assignment. The
      #     no-spend runner submits public-safe stale closeouts at load.
      sup_run_timeout "$SUP_DISPATCH_TIMEOUT_SECS" "${PYLON[@]}" assignment run-no-spend --json >> "$SUP_LOG" 2>&1 || true
      log "FLEET-STALL: self-heal complete (re-advertised + swept stale closeouts); cooldown ${SUP_SELFHEAL_COOLDOWN_SECS}s"
      sleep "$SUP_SELFHEAL_COOLDOWN_SECS"
    fi
  done
}

# --- Standing-task keeper (#6711): keep the priority queue self-sustaining. ---
# On a timer, re-grow any CLOSED `standing-task` issue (the four prio task(ops)
# issues) that has no open sibling, so a finished standing cycle immediately
# reopens fresh work and no slot ever idles for lack of a queue. Idempotent by
# title: a title that already has an open standing issue is never duplicated.
standing_task_keeper_loop() {
  while true; do
    if [ -f "$PAUSE_FILE" ]; then sleep "$SUP_STANDING_TASK_CHECK_SECS"; continue; fi
    local made
    made=$(sup_recreate_closed_standing_tasks 2>>"$SUP_LOG")
    if [ -n "$made" ] && [ "$made" -gt 0 ] 2>/dev/null; then
      log "standing-task keeper recreated $made closed standing task(s)"
    fi
    sleep "$SUP_STANDING_TASK_CHECK_SECS"
  done
}

# --- Heartbeater: recompute desired slots + advertise capacity on a timer. ---
heartbeater_loop() {
  while true; do
    [ -f "$PAUSE_FILE" ] && { sleep "$SUP_HEARTBEAT_SECS"; continue; }
    local ready desired account_slots=()
    ready=$(ready_codex_account_refs | grep -c . || echo 0)
    while IFS= read -r slot_acc; do account_slots+=("$slot_acc"); done < <(sup_expand_account_slots $(ready_codex_account_refs))
    desired="${#account_slots[@]}"
    echo "$desired" > "$DESIRED_FILE"
    heartbeat_tmp="$SUP_STATE_DIR/heartbeat_payload.tmp"
    if OPENAGENTS_PYLON_CODEX_CONCURRENCY="$desired" \
      OPENAGENTS_PYLON_CODEX_BUSY=0 \
      OPENAGENTS_PYLON_CODEX_QUEUED=0 \
      sup_run_timeout "$SUP_PYLON_TIMEOUT_SECS" "${PYLON[@]}" presence heartbeat --json > "$heartbeat_tmp" 2>> "$SUP_LOG"; then
      python3 - "$heartbeat_tmp" "$HEARTBEAT_PAYLOAD_FILE" "$(read_last_dispatch_time)" <<'PY' 2>> "$SUP_LOG" || mv "$heartbeat_tmp" "$HEARTBEAT_PAYLOAD_FILE" 2>/dev/null || true
import json
import sys

src, dst, last_dispatch_time = sys.argv[1], sys.argv[2], sys.argv[3]
with open(src, "r", encoding="utf-8") as handle:
    payload = json.load(handle)
if isinstance(payload, dict):
    payload["last_dispatch_time"] = last_dispatch_time
with open(dst, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, sort_keys=True)
    handle.write("\n")
PY
      rm -f "$heartbeat_tmp" 2>/dev/null || true
      cat "$HEARTBEAT_PAYLOAD_FILE" >> "$SUP_LOG" 2>/dev/null || true
      printf '\n' >> "$SUP_LOG" 2>/dev/null || true
    else
      cat "$heartbeat_tmp" >> "$SUP_LOG" 2>/dev/null || true
      rm -f "$heartbeat_tmp" 2>/dev/null || true
    fi
    # last_dispatch_time telemetry (#6646): emitted on the heartbeat line so a
    # wedged loop (heartbeat firing, dispatch stalled) is visible in the log and
    # the watcher's liveness check has a fresh value to read.
    log "heartbeat ready_codex=$ready desired_slots=$desired tuned_account_slots=${account_slots[*]:-none} last_dispatch_time=$(read_last_dispatch_time)"
    bound_log
    sleep "$SUP_HEARTBEAT_SECS"
  done
}

# --- Worker: one continuously-refilling Codex session slot. ---
worker_loop() {
  local slot="$1"
  local backoff="$SUP_BACKOFF_MIN"
  local iter=0
  local last_failure_sig=""
  local failure_repeat=0
  local lockout_repeat=0
  while true; do
    if [ -f "$PAUSE_FILE" ]; then sleep 30; continue; fi
    local desired; desired=$(cat "$DESIRED_FILE" 2>/dev/null || echo 0)
    if [ "$slot" -ge "$desired" ]; then sleep 10; continue; fi

    # Round-robin account across the live tuned slot set; default omits
    # --account-ref. Repeated local Codex executor refusals shrink a specific
    # account's expansion here, so the fleet finds the clean per-account ceiling
    # without forcing every account down.
    local refs=(); while IFS= read -r r; do refs+=("$r"); done < <(ready_codex_account_refs)
    [ "${#refs[@]}" -eq 0 ] && { sleep 20; continue; }
    local account_slots=(); while IFS= read -r slot_acc; do account_slots+=("$slot_acc"); done < <(sup_expand_account_slots "${refs[@]}")
    [ "${#account_slots[@]}" -eq 0 ] && { sleep 20; continue; }
    if [ "$slot" -ge "${#account_slots[@]}" ]; then sleep 10; continue; fi
    local acc="${account_slots[$slot]}"
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

    # DYNAMIC open-set refetch (#6602-class fix): the task pool can carry a stale
    # snapshot (SUP_ISSUES fallback / the 120s task-pool cache), so an issue that
    # was CLOSED mid-run would otherwise be re-dispatched. Refetch the live OPEN
    # issue set (short TTL cache inside sup_open_issue_numbers) and intersect it
    # with the pool. If gh can't produce the open set, leave the pool as-is;
    # pick_unlocked_issue's per-issue issue_is_open backstop still skips any
    # issue that is no longer open.
    local open_nums=(); while IFS= read -r onum; do open_nums+=("$onum"); done < <(sup_open_issue_numbers)
    if [ "${#open_nums[@]}" -gt 0 ]; then
      local filtered=() cand onum2
      for cand in "${issues[@]}"; do
        for onum2 in "${open_nums[@]}"; do
          if [ "$cand" = "$onum2" ]; then filtered+=("$cand"); break; fi
        done
      done
      if [ "${#filtered[@]}" -gt 0 ]; then
        issues=("${filtered[@]}")
      else
        issues=()
      fi
    fi
    if [ "${#issues[@]}" -eq 0 ]; then
      log "slot=$slot OPEN-SET no task-pool issues are still open; backing off ${backoff}s"
      sleep "$backoff"
      backoff=$(( backoff * 2 )); [ "$backoff" -gt "$SUP_BACKOFF_MAX" ] && backoff="$SUP_BACKOFF_MAX"
      continue
    fi

    # Dispatch lockout: skip any issue that is CLOSED or already has an open PR;
    # only pick a still-open, untouched one. If every backlog issue is locked,
    # back off instead of re-solving solved work (the redo / duplicate-PR root
    # cause).
    local start_idx=$(( (slot + iter) % ${#issues[@]} ))
    iter=$(( iter + 1 ))

    # Label-priority dispatch (#6711): refresh the open-issue label map (cached,
    # TTL) and reorder the pool by priority tier (prio:0-pr-burndown first ...
    # prio:4-backstop-burn last, then unlabelled), keeping intra-tier round-robin
    # spread via start_idx. pick_unlocked_issue then scans from the TOP so slots
    # prefer the highest tier and FALL THROUGH locked/lower tiers — never idling.
    sup_fetch_issue_label_map >/dev/null 2>&1 || true
    local ordered=()
    while IFS= read -r oissue; do
      [ -n "$oissue" ] && ordered+=("$oissue")
    done < <(sup_order_pool_by_priority "$start_idx" "${issues[@]}")
    [ "${#ordered[@]}" -gt 0 ] || ordered=("${issues[@]}")

    # Concurrent-slot distinct-issue selection: ATOMICALLY CLAIM a distinct
    # unlocked issue so two slots in the same cycle never pick the same one (the
    # duplicate-dispatch under-fill bug). In addition to the existing lockout
    # (skip CLOSED + open-PR issues), pick_and_claim_unlocked_issue skips any
    # issue already claimed by another slot's in-flight assignment and reserves
    # the chosen issue under SUP_CLAIM_TTL_SECS. N slots -> up to N distinct
    # issues, restoring same-account parallelism across different issues.
    local issue
    issue=$(pick_and_claim_unlocked_issue 0 "${ordered[@]}")
    if [ -z "$issue" ]; then
      lockout_repeat=$(( lockout_repeat + 1 ))
      if [ "$lockout_repeat" -ge "$SUP_REPLENISHMENT_LOCKOUTS" ] 2>/dev/null; then
        local replenishment=()
        while IFS= read -r rissue; do
          [ -n "$rissue" ] && replenishment+=("$rissue")
        done < <(sup_ensure_replenishment_issues 2>>"$SUP_LOG")
        if [ "${#replenishment[@]}" -gt 0 ]; then
          local replenished_ordered=()
          sup_fetch_issue_label_map >/dev/null 2>&1 || true
          while IFS= read -r oissue; do
            [ -n "$oissue" ] && replenished_ordered+=("$oissue")
          done < <(sup_order_pool_by_priority 0 "${replenishment[@]}")
          [ "${#replenished_ordered[@]}" -gt 0 ] || replenished_ordered=("${replenishment[@]}")
          issue=$(pick_and_claim_unlocked_issue 0 "${replenished_ordered[@]}")
          if [ -n "$issue" ]; then
            lockout_repeat=0
            log "slot=$slot REPLENISHMENT picked issue=#$issue after sustained LOCKOUT (${#replenishment[@]} replenishment candidate(s))"
          else
            log "slot=$slot REPLENISHMENT candidates locked too after sustained LOCKOUT (${#replenishment[@]} candidate(s))"
          fi
        else
          log "slot=$slot REPLENISHMENT unavailable after sustained LOCKOUT (gh/auth missing or create failed)"
        fi
      fi
    fi
    if [ -z "$issue" ]; then
      local pick_sleep
      pick_sleep="$(sup_lockout_pick_backoff_secs "${#issues[@]}" "$desired" "$backoff" "$lockout_repeat")"
      log "slot=$slot LOCKOUT all backlog issues are closed, already have open PRs, or are claimed by other slots; open_backlog=${#issues[@]} desired_slots=$desired repeated=$lockout_repeat backing off ${pick_sleep}s"
      sleep "$pick_sleep"
      if [ "$pick_sleep" = "$backoff" ]; then
        backoff=$(( backoff * 2 )); [ "$backoff" -gt "$SUP_BACKOFF_MAX" ] && backoff="$SUP_BACKOFF_MAX"
      fi
      continue
    fi
    # Base every assignment on FRESH origin/main, never the supervisor's stale
    # local HEAD. $REPO_ROOT can lag origin/main badly (idle clone, or another
    # agent's dirty worktree). main moves fast, so a stale base makes the worker
    # branch from an old commit and the PR diff vs current main looks like a mass
    # deletion of everything added since (the #6719 deletion-poison pattern,
    # recurred on #6734/#6736, nearly #6761). Resolve the live remote main SHA
    # with a pure `git ls-remote` (no working-tree / local-ref mutation; safe in
    # a dirty or shared checkout). If it can't be resolved, SKIP this dispatch
    # rather than poison it with a stale base.
    local commit fresh_main local_head
    fresh_main=$(supervisor_resolve_fresh_origin_main_sha "$SUP_REPO")
    if [ -z "$fresh_main" ]; then
      # Pre-dispatch bail: release the claim so this issue is not parked while we
      # never actually dispatched it.
      sup_release_claim "$issue"
      log "slot=$slot SKIP could not resolve fresh origin/main sha for $SUP_REPO; not dispatching a stale base (anti-#6719)"
      sleep 15; continue
    fi
    local_head=$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo "")
    if [ -n "$local_head" ] && [ "$local_head" != "$fresh_main" ]; then
      log "slot=$slot BASE-DRIFT local HEAD ${local_head:0:12} != origin/main ${fresh_main:0:12}; dispatching FRESH origin/main (anti-#6719 deletion-poison)"
    fi
    # Optional virtual-merge-queue projection still anchors on FRESH main, so the
    # projected base is fresh main plus in-flight assignment branches.
    commit=$(sup_vmq_project_head "$REPO_ROOT" "$fresh_main" 2>/dev/null)
    [ -z "$commit" ] && commit="$fresh_main"
    [ -z "$commit" ] && { sup_release_claim "$issue"; sleep 15; continue; }

    # Record the dispatch ATTEMPT timestamp (#6646) right before firing the
    # request, so last_dispatch_time advances every pick+dispatch cycle and a
    # wedged loop is detectable as "alive but last_dispatch_time stale".
    record_dispatch_attempt

    local out="$SUP_STATE_DIR/slot.$slot.json"
    # Bound the labor dispatch so a hung request can never stall this slot
    # forever (#6646). The bound is generous (SUP_DISPATCH_TIMEOUT_SECS) so
    # normal multi-minute Codex work completes; on timeout (rc 124) the slot
    # treats it as a failed dispatch and continues with backoff below.
    sup_run_timeout "$SUP_DISPATCH_TIMEOUT_SECS" "${PYLON[@]}" khala request \
      --prompt "Implement public issue #$issue and run the named verification." \
      --workflow codex_agent_task \
      --pylon-ref "$SUP_PYLON_REF" \
      "${acc_args[@]}" \
      --repo "$SUP_REPO" --branch main --commit "$commit" \
      --verify "$SUP_VERIFY" \
      --json > "$out" 2>>"$SUP_LOG"
    local rc=$?

    if owner_session_broken "$out"; then global_pause; continue; fi

    if grep -qiE '"ok": ?true|"closeout"|accepted' "$out" 2>/dev/null && [ "$rc" -eq 0 ]; then
      backoff="$SUP_BACKOFF_MIN"
      last_failure_sig=""
      failure_repeat=0
      lockout_repeat=0
      sup_reset_account_refusals "$acc"
      # Keep the issue claimed (refresh TTL) so no other slot re-picks it while
      # its PR/lockout state settles; the open-PR lockout then takes over and the
      # claim eventually GCs.
      sup_refresh_claim "$issue"
      log "slot=$slot acc=$acc issue=#$issue OK (rc=$rc)"
      continue
    fi

    # Refused / unavailable / rate-limited / error -> exponential backoff so the
    # pool settles at the login's real headroom.
    local sig="other"
    sig="$(sup_dispatch_failure_signature "$out")"
    if [ "$sig" = "$last_failure_sig" ]; then
      failure_repeat=$(( failure_repeat + 1 ))
    else
      last_failure_sig="$sig"
      failure_repeat=1
    fi
    if [ "$sig" = "refused" ]; then
      # The gate already has an active assignment for this issue (it is busy
      # elsewhere). PARK it for the full claim TTL so the fleet stops hammering
      # the same stuck issue (#6661/#6662) and other slots spread to distinct
      # work; the claim GCs once the TTL elapses for a later retry.
      sup_refresh_claim "$issue"
    else
      # Transient rate-limit/error on our side, not the issue's fault: release
      # the claim so another (less throttled) slot/account can retry this issue.
      sup_release_claim "$issue"
    fi
    local tune_action
    tune_action="$(sup_record_account_refusal "$acc" "$sig")"
    [ -n "$tune_action" ] && log "slot=$slot acc=$acc REFUSAL-TUNE $tune_action"
    local failure_sleep="$backoff"
    local escalate_backoff=1
    if [ "$sig" = "codex_agent_execution_refused" ]; then
      failure_sleep="$SUP_CLAIMED_DEEP_BACKLOG_SLEEP_SECS"
      escalate_backoff=0
    elif [ "$sig" = "dispatch_gate_conflict" ]; then
      failure_sleep="$SUP_CLAIMED_DEEP_BACKLOG_SLEEP_SECS"
      escalate_backoff=0
    elif [ "$sig" = "dispatch_gate_transient" ]; then
      failure_sleep="$SUP_CLAIMED_DEEP_BACKLOG_SLEEP_SECS"
      escalate_backoff=0
    elif ! sup_should_escalate_failure_backoff "$sig" "$failure_repeat"; then
      escalate_backoff=0
    fi
    log "slot=$slot acc=$acc issue=#$issue NO-DISPATCH ($sig rc=$rc repeated=$failure_repeat); backoff ${failure_sleep}s"
    sleep "$failure_sleep"
    if [ "$escalate_backoff" -eq 1 ]; then
      backoff=$(( backoff * 2 )); [ "$backoff" -gt "$SUP_BACKOFF_MAX" ] && backoff="$SUP_BACKOFF_MAX"
    fi
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
# Seed last_dispatch_time at startup so the liveness check has a baseline and the
# fresh process is not flagged wedged before its first dispatch (#6646).
record_dispatch_attempt
log "=== codex-supervisor START pid=$$ repo=$REPO_ROOT pylon=$SUP_PYLON_REF per_account=$SUP_PER_ACCOUNT max_slots=$SUP_MAX_SLOTS account_refs=${SUP_ACCOUNT_REFS:-all} ==="
sup_gc_orphaned_claims
log "startup claim GC swept orphaned in-flight claims"

sup_run_timeout "$SUP_PYLON_TIMEOUT_SECS" "${PYLON[@]}" provider go-online >> "$SUP_LOG" 2>&1 || log "provider go-online nonzero (continuing)"

heartbeater_loop &
log "heartbeater pid=$! cadence=${SUP_HEARTBEAT_SECS}s"

selfheal_watchdog_loop &
log "selfheal-watchdog pid=$! stall_refusals=$SUP_STALL_REFUSALS cooldown=${SUP_SELFHEAL_COOLDOWN_SECS}s"

standing_task_keeper_loop &
log "standing-task-keeper pid=$! cadence=${SUP_STANDING_TASK_CHECK_SECS}s label=${SUP_STANDING_TASK_LABEL:-standing-task}"

# Give the first heartbeat a moment to publish desired slots.
sleep 5

for slot in $(seq 0 $(( SUP_MAX_SLOTS - 1 ))); do
  worker_loop "$slot" &
  log "worker slot=$slot pid=$!"
done

# Periodic public-counter progress line for observability.
while true; do
  bound_log
  log "counter tokensServed=$(counter_now) desired_slots=$(cat "$DESIRED_FILE" 2>/dev/null) last_dispatch_time=$(read_last_dispatch_time)"
  sleep 120
done
