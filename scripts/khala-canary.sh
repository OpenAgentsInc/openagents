#!/usr/bin/env bash
# Khala 500 RED-ALERT synthetic canary (AAR 2026-06-25).
#
# WHY THIS EXISTS. The 2026-06-25 gateway-wide 500 outage (every
# `POST /api/v1/chat/completions` returned 500 for ~10+ minutes) was NOT
# auto-detected — the owner noticed manually. The 15-minute liveness heartbeat
# (scripts/khala-heartbeat.sh) is too coarse and too heavy to be a fast outage
# detector. This canary is the TIGHT loop: ONE real `openagents/khala`
# completion every ~90s, and on a 500 / non-200 / empty usage it fires a RED
# ALERT. Public counter movement is required by default, including for internal
# dogfood, because the public headline counter is "all Khala tokens served."
#
# WHAT IT DOES each tick:
#   1. Reads the public tokens-served counter.
#   2. Fires ONE small real completion (model: openagents/khala).
#   3. Re-reads the counter.
#   4. Verdict: UP if http 200 + non-empty usage; DOWN if http 500 / any
#      non-200 / empty usage / public counter unreadable or backward. If
#      KHALA_CANARY_EXPECT_PUBLIC_COUNTER is not explicitly set to 0, DOWN also
#      includes 200 with served tokens but no public counter movement.
#   5. RED ALERT only on a healthy->down TRANSITION (edge-triggered, not every
#      tick), so a sustained outage does not spam. Recovery (down->up) is logged.
#
# RED ALERT on healthy->down:
#   - writes a prominent ~/work/.khala-heartbeat/RED-ALERT.log entry,
#   - appends ONE dated line to ~/work/NEEDS_OWNER.md,
#   - exits non-zero (1) so a scheduler/agent watcher can react.
# A 402/429 (quota/rate-limit) is DEGRADED, not an outage (exit 2) — the endpoint
# is alive, the canary key is just tapped out (rotate/add keys); no RED ALERT.
#
# SECRET-SAFE: reads bearer key(s) from a gitignored secrets file; NEVER prints a
# key. Logs carry only counts / durations / statuses — no prompts, completions,
# keys, or raw IPs.
#
# Exit: 0 = up, 2 = degraded (quota/rate-limit), 1 = DOWN (RED ALERT on transition).
set -uo pipefail

BASE="${KHALA_BASE_URL:-https://openagents.com/api/v1}"
PUBLIC_BASE="${KHALA_PUBLIC_BASE:-https://openagents.com}"
MODEL="${KHALA_MODEL:-openagents/khala}"
DEMAND_KIND="${KHALA_CANARY_DEMAND_KIND:-internal}"
DEMAND_SOURCE="${KHALA_CANARY_DEMAND_SOURCE:-canary}"
EXPECT_PUBLIC_COUNTER="${KHALA_CANARY_EXPECT_PUBLIC_COUNTER:-}"
# Reuse the heartbeat secrets file by default; an ops-only canary key pool can be
# pointed at via KHALA_CANARY_ENV (e.g. ~/work/.secrets/khala-ops-keys.env).
SECRETS="${KHALA_CANARY_ENV:-${KHALA_HEARTBEAT_ENV:-$HOME/work/.secrets/khala-heartbeat.env}}"
HOME_DIR="${KHALA_HEARTBEAT_HOME:-$HOME/work/.khala-heartbeat}"
LOG="$HOME_DIR/canary.jsonl"
STATUS="$HOME_DIR/canary-status.json"
REDALERT="$HOME_DIR/RED-ALERT.log"
STATE="$HOME_DIR/.canary-health"   # last verdict: up | down | degraded | unknown
NEEDS_OWNER="${NEEDS_OWNER_FILE:-$HOME/work/NEEDS_OWNER.md}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$HOME_DIR"

ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
prev_health="unknown"; [ -f "$STATE" ] && prev_health="$(cat "$STATE" 2>/dev/null || echo unknown)"

emit() { # state http detail delta counter_check
  local state="$1" http="$2" detail="$3"
  local line="{\"ts\":\"$(ts)\",\"kind\":\"canary\",\"state\":\"$state\",\"http\":$http,\"counterDelta\":${4:-0},\"publicCounterCheck\":\"${5:-unknown}\",\"demandKind\":\"$DEMAND_KIND\",\"detail\":\"$detail\"}"
  echo "$line" >> "$LOG"
  echo "$line" > "$STATUS"
  echo "$line"
}

red_alert() { # http detail
  local http="$1" detail="$2"
  {
    echo "================ KHALA RED ALERT ================"
    echo "time:   $(ts)"
    echo "state:  DOWN"
    echo "http:   $http"
    echo "detail: $detail"
    echo "action: investigate NOW. Likely classes (AAR 2026-06-25):"
    echo "  - code shipped ahead of schema -> run: cd apps/openagents.com && bun run check:pending-migrations"
    echo "    (if pending: cd workers/api && wrangler d1 migrations apply openagents-autopilot --remote)"
    echo "  - a backing inference lane is 5xx -> check GET $BASE/models and lane health"
    echo "  - check recent deploys; the ONLY sanctioned deploy is deploy:safe"
    echo "runbook: docs/inference/2026-06-25-khala-heartbeat-runbook.md (Canary section)"
    echo "AAR:     docs/incidents/2026-06-25-khala-500-completions-outage-aar.md"
    echo "================================================="
  } >> "$REDALERT"
}

# --- key pool (rotate round-robin) -------------------------------------------
[ -f "$SECRETS" ] && . "$SECRETS"
# Prefer a dedicated canary pool if present, else fall back to the heartbeat pool.
IFS=',' read -r -a KEYS <<< "${KHALA_CANARY_KEYS:-${KHALA_HEARTBEAT_KEYS:-}}"
if [ "${#KEYS[@]}" -eq 0 ] || [ -z "${KEYS[0]:-}" ]; then
  emit "down" 0 "no_keys: populate $SECRETS (KHALA_CANARY_KEYS or KHALA_HEARTBEAT_KEYS)"
  # Missing keys is a config problem, not a gateway outage: surface once, no RED ALERT spam.
  if [ "$prev_health" != "down" ]; then
    echo "$(ts) NEEDS-OWNER: Khala canary has no keys ($SECRETS). Provision an ops free-key pool." >> "$NEEDS_OWNER"
  fi
  echo "down" > "$STATE"
  exit 1
fi
IDX_STATE="$HOME_DIR/.canary-keystate"
idx=0; [ -f "$IDX_STATE" ] && idx=$(cat "$IDX_STATE" 2>/dev/null || echo 0)
KEY="${KEYS[$(( idx % ${#KEYS[@]} ))]}"
echo $(( (idx + 1) % 1000000 )) > "$IDX_STATE"
# DEMAND-ORIGIN SELF-TAG (#6298). The canary defaults to INTERNAL dogfood, not
# external demand. Tag every completion so the token ledger AND the captured
# trace corpus classify it as internal (source `canary`) and segment it out of
# the genuine external real-user corpus. See
# docs/inference/2026-06-25-khala-heartbeat-runbook.md ("Demand-origin self-tag").
AUTH=(-H "Authorization: Bearer $KEY" -H "content-type: application/json" \
  -H "x-openagents-demand-kind: $DEMAND_KIND" -H "x-openagents-demand-source: $DEMAND_SOURCE")
if [ -z "$EXPECT_PUBLIC_COUNTER" ]; then EXPECT_PUBLIC_COUNTER=1; fi
if [ "$EXPECT_PUBLIC_COUNTER" = "1" ]; then counter_check="required"; else counter_check="not_required"; fi

# --- one tick ----------------------------------------------------------------
counter_readable=1
before=$(curl -s --max-time 10 "$PUBLIC_BASE/api/public/khala-tokens-served" | jq -r '.tokensServed // empty')
if ! [[ "$before" =~ ^[0-9]+$ ]]; then counter_readable=0; before=0; fi

body=$(jq -nc --arg m "$MODEL" \
  '{model:$m,max_tokens:48,temperature:0.2,messages:[{role:"user",content:"Canary: reply with one short sentence."}]}')
resp=$(curl -s --max-time 60 -w '\n%{http_code}' "$BASE/chat/completions" "${AUTH[@]}" -d "$body")
http=$(printf '%s' "$resp" | tail -1)
[ -z "$http" ] && http=0
tok=$(printf '%s' "$resp" | sed '$d' | jq -r '((.usage.prompt_tokens//0)+(.usage.completion_tokens//0)) // 0' 2>/dev/null || echo 0)
[[ "$tok" =~ ^[0-9]+$ ]] || tok=0

sleep 2
after=$(curl -s --max-time 10 "$PUBLIC_BASE/api/public/khala-tokens-served" | jq -r '.tokensServed // empty')
if ! [[ "$after" =~ ^[0-9]+$ ]]; then counter_readable=0; after=0; fi
delta=$(( after - before ))

# --- verdict -----------------------------------------------------------------
state="up"; detail="ok"
if [ "$http" = "402" ] || [ "$http" = "429" ]; then
  state="degraded"; detail="quota_or_rate_limited http=$http (rotate/add canary keys)"
elif [ "$http" != "200" ]; then
  state="down"; detail="non_200 http=$http"
elif [ "$tok" -le 0 ]; then
  state="down"; detail="empty_usage http=200"
elif [ "$counter_readable" -eq 0 ]; then
  state="down"; detail="counter_unreadable"
elif [ "$delta" -lt 0 ]; then
  state="down"; detail="counter_regressed before=$before after=$after"
elif [ "$EXPECT_PUBLIC_COUNTER" = "1" ] && [ "$delta" -le 0 ]; then
  state="down"; detail="counter_did_not_move http=200 served=$tok delta=$delta"
elif [ "$EXPECT_PUBLIC_COUNTER" != "1" ]; then
  detail="ok_counter_delta_not_required"
fi

emit "$state" "$http" "$detail" "$delta" "$counter_check"

# --- edge-triggered alerting -------------------------------------------------
case "$state" in
  down)
    if [ "$prev_health" != "down" ]; then
      red_alert "$http" "$detail"
      echo "$(ts) RED-ALERT: Khala completions DOWN (http=$http, $detail). See ~/work/.khala-heartbeat/RED-ALERT.log + AAR docs/incidents/2026-06-25-khala-500-completions-outage-aar.md. Run \`cd apps/openagents.com && bun run check:pending-migrations\`." >> "$NEEDS_OWNER"
    fi
    echo "down" > "$STATE"; exit 1 ;;
  degraded)
    echo "degraded" > "$STATE"; exit 2 ;;
  up)
    if [ "$prev_health" = "down" ]; then
      echo "$(ts) RECOVERED: Khala completions back UP (http=200, $detail)." >> "$REDALERT"
    fi
    echo "up" > "$STATE"; exit 0 ;;
esac
