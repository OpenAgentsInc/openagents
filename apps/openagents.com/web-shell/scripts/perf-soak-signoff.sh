#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_SHELL_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_DIR="$(cd "${WEB_SHELL_DIR}/.." && pwd)"
REPO_ROOT="$(git -C "${APP_DIR}" rev-parse --show-toplevel)"
SERVICE_MANIFEST="${APP_DIR}/service/Cargo.toml"
DIST_DIR="${WEB_SHELL_DIR}/dist"
MANIFEST_PATH="${DIST_DIR}/manifest.json"
OUTPUT_DIR="${WEB_SHELL_DIR}/perf"

BIND_ADDR="${BIND_ADDR:-127.0.0.1:8787}"
BASE_URL="${BASE_URL:-http://${BIND_ADDR}}"
LATENCY_SAMPLES="${LATENCY_SAMPLES:-60}"
AUTH_CHURN_SAMPLES="${AUTH_CHURN_SAMPLES:-40}"
SOAK_SECONDS="${SOAK_SECONDS:-180}"
FAIL_ON_BUDGET="${FAIL_ON_BUDGET:-1}"

ROOT_P95_BUDGET_MS="${ROOT_P95_BUDGET_MS:-120}"
MANIFEST_P95_BUDGET_MS="${MANIFEST_P95_BUDGET_MS:-100}"
WASM_P95_BUDGET_MS="${WASM_P95_BUDGET_MS:-250}"
VERIFY_P95_BUDGET_MS="${VERIFY_P95_BUDGET_MS:-250}"
SYNC_TOKEN_P95_BUDGET_MS="${SYNC_TOKEN_P95_BUDGET_MS:-250}"
SOAK_ERRORS_BUDGET="${SOAK_ERRORS_BUDGET:-0}"
RSS_GROWTH_BUDGET_KB="${RSS_GROWTH_BUDGET_KB:-51200}"

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required" >&2
  exit 1
fi

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  "${WEB_SHELL_DIR}/build-dist.sh"
fi

if [[ ! -f "${MANIFEST_PATH}" ]]; then
  echo "error: manifest not found at ${MANIFEST_PATH}" >&2
  exit 1
fi

JS_PATH="$(jq -r '.entry.js' "${MANIFEST_PATH}")"
WASM_PATH="$(jq -r '.entry.wasm' "${MANIFEST_PATH}")"
BUILD_ID="$(jq -r '.buildId' "${MANIFEST_PATH}")"
if [[ -z "${JS_PATH}" || "${JS_PATH}" == "null" || -z "${WASM_PATH}" || "${WASM_PATH}" == "null" ]]; then
  echo "error: manifest missing entry.js/entry.wasm" >&2
  exit 1
fi

SERVICE_LOG="$(mktemp -t openagents-web-shell-signoff.XXXX.log)"
cleanup() {
  if [[ -n "${SERVICE_PID:-}" ]] && kill -0 "${SERVICE_PID}" >/dev/null 2>&1; then
    kill "${SERVICE_PID}" >/dev/null 2>&1 || true
    wait "${SERVICE_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

OA_AUTH_PROVIDER_MODE=mock \
OA_CONTROL_STATIC_DIR="${DIST_DIR}" \
OA_CONTROL_BIND_ADDR="${BIND_ADDR}" \
cargo run --manifest-path "${SERVICE_MANIFEST}" >"${SERVICE_LOG}" 2>&1 &
SERVICE_PID=$!

for _ in $(seq 1 120); do
  if curl -fsS "${BASE_URL}/healthz" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done

if ! curl -fsS "${BASE_URL}/healthz" >/dev/null 2>&1; then
  echo "error: control service failed health check" >&2
  echo "service log: ${SERVICE_LOG}" >&2
  exit 1
fi

TMP_DIR="$(mktemp -d -t openagents-web-shell-perf.XXXX)"
trap 'rm -rf "${TMP_DIR}"; cleanup' EXIT

measure_get_latencies() {
  local url="$1"
  local samples="$2"
  local out_file="$3"
  : >"${out_file}"
  for _ in $(seq 1 "${samples}"); do
    curl -sS -o /dev/null -w '%{time_total}\n' "${url}" >>"${out_file}"
  done
}

metric_count() {
  local file="$1"
  wc -l <"${file}" | tr -d ' '
}

metric_avg_ms() {
  local file="$1"
  awk '{sum+=$1} END { if (NR==0) printf "0.00"; else printf "%.2f", (sum/NR)*1000 }' "${file}"
}

metric_percentile_ms() {
  local file="$1"
  local percentile="$2"
  local sorted="${TMP_DIR}/sorted.$$.txt"
  sort -n "${file}" >"${sorted}"
  local count
  count="$(metric_count "${file}")"
  if [[ "${count}" -eq 0 ]]; then
    echo "0.00"
    return
  fi
  local rank=$(( (count * percentile + 99) / 100 ))
  if [[ "${rank}" -lt 1 ]]; then
    rank=1
  fi
  local value
  value="$(sed -n "${rank}p" "${sorted}")"
  awk -v v="${value}" 'BEGIN { printf "%.2f", v*1000 }'
}

metric_max_ms() {
  local file="$1"
  awk 'BEGIN { max=0 } { if ($1 > max) max=$1 } END { printf "%.2f", max*1000 }' "${file}"
}

ROOT_TIMES="${TMP_DIR}/root.times"
MANIFEST_TIMES="${TMP_DIR}/manifest.times"
JS_TIMES="${TMP_DIR}/js.times"
WASM_TIMES="${TMP_DIR}/wasm.times"

measure_get_latencies "${BASE_URL}/" "${LATENCY_SAMPLES}" "${ROOT_TIMES}"
measure_get_latencies "${BASE_URL}/manifest.json" "${LATENCY_SAMPLES}" "${MANIFEST_TIMES}"
measure_get_latencies "${BASE_URL}/${JS_PATH}" "${LATENCY_SAMPLES}" "${JS_TIMES}"
measure_get_latencies "${BASE_URL}/${WASM_PATH}" "${LATENCY_SAMPLES}" "${WASM_TIMES}"

SEND_TIMES="${TMP_DIR}/auth_send.times"
VERIFY_TIMES="${TMP_DIR}/auth_verify.times"
SYNC_TIMES="${TMP_DIR}/sync_token.times"
AUTH_ERRORS=0
: >"${SEND_TIMES}"
: >"${VERIFY_TIMES}"
: >"${SYNC_TIMES}"

for i in $(seq 1 "${AUTH_CHURN_SAMPLES}"); do
  COOKIE_FILE="${TMP_DIR}/cookie-${i}.txt"
  EMAIL="perf-signoff-${i}-${RANDOM}@openagents.local"

  SEND_TIME="$(curl -sS -c "${COOKIE_FILE}" -o /dev/null -w '%{time_total}' \
    -H 'content-type: application/json' \
    -d "{\"email\":\"${EMAIL}\"}" \
    "${BASE_URL}/api/auth/email")"
  echo "${SEND_TIME}" >>"${SEND_TIMES}"

  VERIFY_RESPONSE="${TMP_DIR}/verify-${i}.json"
  VERIFY_TIME="$(curl -sS -b "${COOKIE_FILE}" -o "${VERIFY_RESPONSE}" -w '%{time_total}' \
    -H 'content-type: application/json' \
    -d '{"code":"123456"}' \
    "${BASE_URL}/api/auth/verify")"
  echo "${VERIFY_TIME}" >>"${VERIFY_TIMES}"

  ACCESS_TOKEN="$(jq -r '.token // empty' "${VERIFY_RESPONSE}")"
  if [[ -z "${ACCESS_TOKEN}" ]]; then
    AUTH_ERRORS=$((AUTH_ERRORS + 1))
    continue
  fi

  SYNC_TIME="$(curl -sS -o /dev/null -w '%{time_total}' \
    -H "authorization: Bearer ${ACCESS_TOKEN}" \
    -H 'content-type: application/json' \
    -d '{"scopes":["runtime.codex_worker_events"]}' \
    "${BASE_URL}/api/sync/token")"
  echo "${SYNC_TIME}" >>"${SYNC_TIMES}"
done

SOAK_TIMES="${TMP_DIR}/soak_manifest.times"
RSS_SAMPLES_FILE="${TMP_DIR}/rss.samples"
SOAK_ERRORS=0
: >"${SOAK_TIMES}"
: >"${RSS_SAMPLES_FILE}"

SOAK_START="$(date +%s)"
while true; do
  NOW="$(date +%s)"
  if [[ $((NOW - SOAK_START)) -ge "${SOAK_SECONDS}" ]]; then
    break
  fi

  MANIFEST_TIME="$(curl -sS -o /dev/null -w '%{time_total}' "${BASE_URL}/manifest.json" || true)"
  if [[ -z "${MANIFEST_TIME}" ]]; then
    SOAK_ERRORS=$((SOAK_ERRORS + 1))
  else
    echo "${MANIFEST_TIME}" >>"${SOAK_TIMES}"
  fi

  RSS_KB="$(ps -o rss= -p "${SERVICE_PID}" | tr -d ' ' || true)"
  if [[ -n "${RSS_KB}" ]]; then
    echo "${RSS_KB}" >>"${RSS_SAMPLES_FILE}"
  fi

  sleep 1
done

RSS_MIN_KB="$(awk 'NR==1 {min=$1} {if ($1<min) min=$1} END {if (NR==0) print 0; else print min}' "${RSS_SAMPLES_FILE}")"
RSS_MAX_KB="$(awk 'NR==1 {max=$1} {if ($1>max) max=$1} END {if (NR==0) print 0; else print max}' "${RSS_SAMPLES_FILE}")"
RSS_AVG_KB="$(awk '{sum+=$1} END {if (NR==0) printf "0"; else printf "%.0f", sum/NR }' "${RSS_SAMPLES_FILE}")"
RSS_GROWTH_KB=$((RSS_MAX_KB - RSS_MIN_KB))

ROOT_P95_MS="$(metric_percentile_ms "${ROOT_TIMES}" 95)"
MANIFEST_P95_MS="$(metric_percentile_ms "${MANIFEST_TIMES}" 95)"
WASM_P95_MS="$(metric_percentile_ms "${WASM_TIMES}" 95)"
VERIFY_P95_MS="$(metric_percentile_ms "${VERIFY_TIMES}" 95)"
SYNC_P95_MS="$(metric_percentile_ms "${SYNC_TIMES}" 95)"

BUDGET_FAILS=()
if awk -v a="${ROOT_P95_MS}" -v b="${ROOT_P95_BUDGET_MS}" 'BEGIN{exit !(a>b)}'; then BUDGET_FAILS+=("root.p95_ms>${ROOT_P95_BUDGET_MS}"); fi
if awk -v a="${MANIFEST_P95_MS}" -v b="${MANIFEST_P95_BUDGET_MS}" 'BEGIN{exit !(a>b)}'; then BUDGET_FAILS+=("manifest.p95_ms>${MANIFEST_P95_BUDGET_MS}"); fi
if awk -v a="${WASM_P95_MS}" -v b="${WASM_P95_BUDGET_MS}" 'BEGIN{exit !(a>b)}'; then BUDGET_FAILS+=("wasm.p95_ms>${WASM_P95_BUDGET_MS}"); fi
if awk -v a="${VERIFY_P95_MS}" -v b="${VERIFY_P95_BUDGET_MS}" 'BEGIN{exit !(a>b)}'; then BUDGET_FAILS+=("auth_verify.p95_ms>${VERIFY_P95_BUDGET_MS}"); fi
if awk -v a="${SYNC_P95_MS}" -v b="${SYNC_TOKEN_P95_BUDGET_MS}" 'BEGIN{exit !(a>b)}'; then BUDGET_FAILS+=("sync_token.p95_ms>${SYNC_TOKEN_P95_BUDGET_MS}"); fi
if [[ "${SOAK_ERRORS}" -gt "${SOAK_ERRORS_BUDGET}" ]]; then BUDGET_FAILS+=("soak.errors>${SOAK_ERRORS_BUDGET}"); fi
if [[ "${AUTH_ERRORS}" -gt 0 ]]; then BUDGET_FAILS+=("auth_churn.errors>0"); fi
if [[ "${RSS_GROWTH_KB}" -gt "${RSS_GROWTH_BUDGET_KB}" ]]; then BUDGET_FAILS+=("rss_growth_kb>${RSS_GROWTH_BUDGET_KB}"); fi

PASS=true
if [[ "${#BUDGET_FAILS[@]}" -gt 0 ]]; then
  PASS=false
fi

TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_JSON="${OUTPUT_DIR}/signoff-${TIMESTAMP}.json"

FAILS_JSON='[]'
if [[ "${#BUDGET_FAILS[@]}" -gt 0 ]]; then
  FAILS_JSON="$(printf '%s\n' "${BUDGET_FAILS[@]}" | jq -R . | jq -s .)"
fi

jq -n \
  --arg timestamp "${TIMESTAMP}" \
  --arg build_id "${BUILD_ID}" \
  --arg base_url "${BASE_URL}" \
  --argjson latency_samples "${LATENCY_SAMPLES}" \
  --argjson auth_churn_samples "${AUTH_CHURN_SAMPLES}" \
  --argjson soak_seconds "${SOAK_SECONDS}" \
  --argjson pass "${PASS}" \
  --argjson budget_failures "${FAILS_JSON}" \
  --arg root_p95_ms "${ROOT_P95_MS}" \
  --arg root_avg_ms "$(metric_avg_ms "${ROOT_TIMES}")" \
  --arg root_max_ms "$(metric_max_ms "${ROOT_TIMES}")" \
  --arg manifest_p95_ms "${MANIFEST_P95_MS}" \
  --arg manifest_avg_ms "$(metric_avg_ms "${MANIFEST_TIMES}")" \
  --arg wasm_p95_ms "${WASM_P95_MS}" \
  --arg wasm_avg_ms "$(metric_avg_ms "${WASM_TIMES}")" \
  --arg verify_p95_ms "${VERIFY_P95_MS}" \
  --arg verify_avg_ms "$(metric_avg_ms "${VERIFY_TIMES}")" \
  --arg sync_p95_ms "${SYNC_P95_MS}" \
  --arg sync_avg_ms "$(metric_avg_ms "${SYNC_TIMES}")" \
  --argjson auth_errors "${AUTH_ERRORS}" \
  --arg soak_p95_ms "$(metric_percentile_ms "${SOAK_TIMES}" 95)" \
  --arg soak_avg_ms "$(metric_avg_ms "${SOAK_TIMES}")" \
  --argjson soak_errors "${SOAK_ERRORS}" \
  --argjson rss_min_kb "${RSS_MIN_KB}" \
  --argjson rss_max_kb "${RSS_MAX_KB}" \
  --argjson rss_avg_kb "${RSS_AVG_KB}" \
  --argjson rss_growth_kb "${RSS_GROWTH_KB}" \
  --argjson budgets "$(jq -n \
    --argjson root_p95_ms "${ROOT_P95_BUDGET_MS}" \
    --argjson manifest_p95_ms "${MANIFEST_P95_BUDGET_MS}" \
    --argjson wasm_p95_ms "${WASM_P95_BUDGET_MS}" \
    --argjson verify_p95_ms "${VERIFY_P95_BUDGET_MS}" \
    --argjson sync_p95_ms "${SYNC_TOKEN_P95_BUDGET_MS}" \
    --argjson soak_errors "${SOAK_ERRORS_BUDGET}" \
    --argjson rss_growth_kb "${RSS_GROWTH_BUDGET_KB}" \
    '{root_p95_ms:$root_p95_ms, manifest_p95_ms:$manifest_p95_ms, wasm_p95_ms:$wasm_p95_ms, auth_verify_p95_ms:$verify_p95_ms, sync_token_p95_ms:$sync_p95_ms, soak_errors:$soak_errors, rss_growth_kb:$rss_growth_kb}')" \
  '{
    timestamp: $timestamp,
    build_id: $build_id,
    base_url: $base_url,
    samples: { latency: $latency_samples, auth_churn: $auth_churn_samples, soak_seconds: $soak_seconds },
    budgets: $budgets,
    metrics: {
      boot: {
        root: { p95_ms: ($root_p95_ms|tonumber), avg_ms: ($root_avg_ms|tonumber), max_ms: ($root_max_ms|tonumber) },
        manifest: { p95_ms: ($manifest_p95_ms|tonumber), avg_ms: ($manifest_avg_ms|tonumber) },
        wasm_asset: { p95_ms: ($wasm_p95_ms|tonumber), avg_ms: ($wasm_avg_ms|tonumber) }
      },
      interaction: {
        auth_verify: { p95_ms: ($verify_p95_ms|tonumber), avg_ms: ($verify_avg_ms|tonumber) },
        sync_token: { p95_ms: ($sync_p95_ms|tonumber), avg_ms: ($sync_avg_ms|tonumber) },
        auth_errors: $auth_errors
      },
      soak: {
        manifest_poll: { p95_ms: ($soak_p95_ms|tonumber), avg_ms: ($soak_avg_ms|tonumber) },
        errors: $soak_errors,
        rss_kb: { min: $rss_min_kb, max: $rss_max_kb, avg: $rss_avg_kb, growth: $rss_growth_kb }
      }
    },
    pass: $pass,
    budget_failures: $budget_failures
  }' >"${OUTPUT_JSON}"

cp "${OUTPUT_JSON}" "${OUTPUT_DIR}/latest.json"

jq . "${OUTPUT_JSON}"

echo "[signoff] wrote ${OUTPUT_JSON}"

echo "[signoff] service log: ${SERVICE_LOG}"

if [[ "${PASS}" != "true" && "${FAIL_ON_BUDGET}" == "1" ]]; then
  echo "[signoff] budget check failed" >&2
  exit 1
fi
