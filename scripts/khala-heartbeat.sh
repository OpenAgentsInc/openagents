#!/usr/bin/env bash
# Khala liveness heartbeat — fires ~50k tokens across diverse configs against the
# LIVE production endpoint every run, verifies the public counter records them, and
# writes a public-safe status + JSONL log. Designed to run on a 15-minute schedule
# (launchd / cron). See docs/inference/2026-06-25-khala-heartbeat-runbook.md.
#
# SECRET-SAFE: reads bearer key(s) from a gitignored secrets file; NEVER prints a
# key. Logs carry only counts / durations / statuses — no prompts, completions, or
# keys. These are INTERNAL dogfood tokens (heartbeat), not external demand.
#
# Exit: 0 = healthy, 2 = degraded (e.g. quota-exhausted but endpoint alive), 1 = DOWN.
set -uo pipefail

BASE="${KHALA_BASE_URL:-https://openagents.com/api/v1}"
PUBLIC_BASE="${KHALA_PUBLIC_BASE:-https://openagents.com}"
MODEL="${KHALA_MODEL:-openagents/khala}"
SECRETS="${KHALA_HEARTBEAT_ENV:-$HOME/work/.secrets/khala-heartbeat.env}"
HOME_DIR="${KHALA_HEARTBEAT_HOME:-$HOME/work/.khala-heartbeat}"
LOG="$HOME_DIR/heartbeat.jsonl"
STATUS="$HOME_DIR/status.json"
FAILURES="$HOME_DIR/FAILURES.log"
STATE="$HOME_DIR/.keystate"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$HOME_DIR"

ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
fail_note() { echo "$(ts) $1" >> "$FAILURES"; }

# --- key pool (rotate round-robin so each key stays under its UTC-day quota) ---
[ -f "$SECRETS" ] && . "$SECRETS"
IFS=',' read -r -a KEYS <<< "${KHALA_HEARTBEAT_KEYS:-}"
if [ "${#KEYS[@]}" -eq 0 ] || [ -z "${KEYS[0]:-}" ]; then
  echo "{\"ts\":\"$(ts)\",\"state\":\"down\",\"error\":\"no_keys\",\"detail\":\"$SECRETS missing KHALA_HEARTBEAT_KEYS\"}" | tee -a "$LOG" > "$STATUS"
  fail_note "no_keys: populate $SECRETS"
  exit 1
fi
idx=0; [ -f "$STATE" ] && idx=$(cat "$STATE" 2>/dev/null || echo 0)
KEY="${KEYS[$(( idx % ${#KEYS[@]} ))]}"
echo $(( (idx + 1) % 1000000 )) > "$STATE"

AUTH=(-H "Authorization: Bearer $KEY" -H "content-type: application/json")
ok=0; fail=0; q402=0; summed=0; details=""

before=$(curl -s --max-time 15 "$PUBLIC_BASE/api/public/khala-tokens-served" | jq -r '.tokensServed // empty')
[ -z "$before" ] && before=0

# record one config outcome into the running tallies
record() { # name http_code tokens
  local name="$1" code="$2" tok="$3"
  if [ "$code" = "200" ]; then ok=$((ok+1)); summed=$((summed + tok));
  elif [ "$code" = "402" ] || [ "$code" = "429" ]; then q402=$((q402+1));
  else fail=$((fail+1)); fail_note "config=$name http=$code"; fi
  details="$details{\"c\":\"$name\",\"http\":$code,\"tok\":$tok},"
}

# non-streaming request -> echoes "<http_code> <total_tokens>"
fire() { # name max_tokens temperature prompt
  local name="$1" mt="$2" temp="$3" prompt="$4"
  local body resp code tok
  body=$(jq -nc --arg m "$MODEL" --argjson mt "$mt" --argjson t "$temp" --arg p "$prompt" \
    '{model:$m,max_tokens:$mt,temperature:$t,messages:[{role:"user",content:$p}]}')
  resp=$(curl -s --max-time 120 -w '\n%{http_code}' "$BASE/chat/completions" "${AUTH[@]}" -d "$body")
  code=$(printf '%s' "$resp" | tail -1)
  tok=$(printf '%s' "$resp" | sed '$d' | jq -r '((.usage.prompt_tokens//0)+(.usage.completion_tokens//0)) // 0' 2>/dev/null || echo 0)
  record "$name" "$code" "${tok:-0}"
}

# --- Config A: short, low-temp, non-streaming ---
fire "short" 64 0.2 "Heartbeat: reply with one short sentence about open inference."
# --- Config B: medium, higher-temp, non-streaming ---
fire "medium" 512 0.7 "Heartbeat: write a short paragraph about why verifiable AI work matters."
# --- Config C: content-array (OpenCode-style 'parts') shape — exercises the tool-compat path ---
ca_body=$(jq -nc --arg m "$MODEL" '{model:$m,max_tokens:300,messages:[{role:"user",content:[{type:"text",text:"Heartbeat content-array check: name three properties of a good inference API."}]}]}')
ca=$(curl -s --max-time 60 -w '\n%{http_code}' "$BASE/chat/completions" "${AUTH[@]}" -d "$ca_body")
ca_code=$(printf '%s' "$ca" | tail -1)
ca_tok=$(printf '%s' "$ca" | sed '$d' | jq -r '((.usage.prompt_tokens//0)+(.usage.completion_tokens//0)) // 0' 2>/dev/null || echo 0)
record "content-array" "$ca_code" "${ca_tok:-0}"
# --- Config D: streaming (SSE) — liveness of the stream path (tokens counted via ledger delta) ---
st_code=$(curl -s --max-time 60 -o "$WORK/stream.txt" -w '%{http_code}' "$BASE/chat/completions" "${AUTH[@]}" \
  -d "$(jq -nc --arg m "$MODEL" '{model:$m,max_tokens:400,stream:true,messages:[{role:"user",content:"Heartbeat streaming check: count from one to ten in words."}]}')")
st_chunks=$(grep -c '^data:' "$WORK/stream.txt" 2>/dev/null || echo 0)
if [ "$st_code" = "200" ] && [ "$st_chunks" -gt 1 ]; then ok=$((ok+1)); else
  if [ "$st_code" = "402" ] || [ "$st_code" = "429" ]; then q402=$((q402+1)); else fail=$((fail+1)); fail_note "config=streaming http=$st_code chunks=$st_chunks"; fi
fi
details="$details{\"c\":\"streaming\",\"http\":$st_code,\"chunks\":$st_chunks},"

# --- Config E: long-gen burst, looped to a token TARGET (models stop well under
# max_tokens, so fire waves of `WAVE` concurrent long-gens until ~TARGET served). ---
TARGET="${KHALA_HEARTBEAT_TOKEN_TARGET:-50000}"
WAVE="${KHALA_HEARTBEAT_LONG_CONC:-6}"          # concurrency per wave (also the load probe)
MAX_WAVES="${KHALA_HEARTBEAT_MAX_WAVES:-15}"     # safety cap on total requests
PROMPTS=(
  "write a detailed multi-paragraph technical explanation of how the Lightning Network settles micropayments for AI inference, covering HTLCs, invoices, and routing."
  "explain in depth how an OpenAI-compatible inference gateway routes a request across multiple model providers, with fan-out, verification, and cost accounting."
  "describe thoroughly how speculative decoding and tensor parallelism affect throughput and latency on multi-GPU inference servers."
  "give a comprehensive overview of how verifiable agent work is benchmarked: task sets, executed verifiers, acceptance contracts, and cost-per-accepted-outcome."
)
gi=0; prev=0; wave=0
while [ "$summed" -lt "$TARGET" ] && [ "$wave" -lt "$MAX_WAVES" ]; do
  wave=$((wave+1))
  for j in $(seq 1 "$WAVE"); do
    gi=$((gi+1))
    p="${PROMPTS[$(( gi % ${#PROMPTS[@]} ))]}"
    ( curl -s --max-time 180 -w '\n%{http_code}' "$BASE/chat/completions" "${AUTH[@]}" \
        -d "$(jq -nc --arg m "$MODEL" --argjson n "$gi" --arg p "$p" '{model:$m,max_tokens:8000,messages:[{role:"user",content:("Heartbeat long-gen \($n): \($p) Be thorough.")}]}')" \
        > "$WORK/long_$gi.out" 2>&1 ) &
  done
  wait
  any_ok=0
  for k in $(seq $((prev+1)) "$gi"); do
    code=$(tail -1 "$WORK/long_$k.out" 2>/dev/null)
    tok=$(sed '$d' "$WORK/long_$k.out" 2>/dev/null | jq -r '((.usage.prompt_tokens//0)+(.usage.completion_tokens//0)) // 0' 2>/dev/null || echo 0)
    record "long-$k" "${code:-000}" "${tok:-0}"
    [ "${code:-000}" = "200" ] && any_ok=1
  done
  prev=$gi
  # if a whole wave came back quota-limited/failed, stop looping (don't hammer)
  [ "$any_ok" -eq 0 ] && break
done

sleep 3
after=$(curl -s --max-time 15 "$PUBLIC_BASE/api/public/khala-tokens-served" | jq -r '.tokensServed // empty')
[ -z "$after" ] && after=0
delta=$(( after - before ))

# --- verdict ---
# DOWN: a hard failure on a config, OR the counter did not move while we served tokens.
# DEGRADED: everything that ran was quota-limited (402/429) — endpoint alive, key tapped out.
state="ok"
if [ "$fail" -gt 0 ]; then state="down"; fi
if [ "$ok" -gt 0 ] && [ "$delta" -le 0 ]; then state="down"; fail_note "counter_did_not_move ok=$ok delta=$delta"; fi
if [ "$ok" -eq 0 ] && [ "$q402" -gt 0 ] && [ "$fail" -eq 0 ]; then state="degraded"; fail_note "quota_exhausted (rotate/add keys)"; fi

line="{\"ts\":\"$(ts)\",\"state\":\"$state\",\"ok\":$ok,\"fail\":$fail,\"quota\":$q402,\"summedTokens\":$summed,\"counterBefore\":$before,\"counterAfter\":$after,\"counterDelta\":$delta,\"keyIndex\":$(( idx % ${#KEYS[@]} )),\"configs\":[${details%,}]}"
echo "$line" >> "$LOG"
echo "$line" > "$STATUS"
echo "$line"

case "$state" in
  ok) exit 0 ;;
  degraded) exit 2 ;;
  *) exit 1 ;;
esac
