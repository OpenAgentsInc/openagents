#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
OA_ACCESS_TOKEN="${OA_ACCESS_TOKEN:-}"
OA_PERF_EMAIL="${OA_PERF_EMAIL:-perf-check@openagents.com}"
REQUIRE_AUTH_FLOWS="${REQUIRE_AUTH_FLOWS:-1}"
HTMX_PERF_BASELINE_FILE="${HTMX_PERF_BASELINE_FILE:-}"

if [[ -z "${HTMX_PERF_BASELINE_FILE}" ]]; then
  HTMX_PERF_BASELINE_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/docs/HTMX_PERF_BASELINE.csv"
fi

if [[ "${REQUIRE_AUTH_FLOWS}" == "1" && -z "${OA_ACCESS_TOKEN}" ]]; then
  echo "ERROR: OA_ACCESS_TOKEN is required when REQUIRE_AUTH_FLOWS=1" >&2
  exit 2
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required" >&2
  exit 2
fi

RESULTS=()
FAILURES=0

baseline_for() {
  local name="$1"
  if [[ ! -f "${HTMX_PERF_BASELINE_FILE}" ]]; then
    echo ","
    return 0
  fi
  awk -F, -v probe="$name" 'NR>1 && $1==probe {print $2","$3; found=1} END{if(!found) print ","}' "${HTMX_PERF_BASELINE_FILE}"
}

record_probe() {
  local name="$1"
  local method="$2"
  local url_path="$3"
  local body="$4"
  local max_size_bytes="$5"
  local max_ttfb_ms="$6"
  shift 6
  local headers=("$@")

  local tmp_body tmp_headers
  tmp_body="$(mktemp)"
  tmp_headers="$(mktemp)"

  local cmd=(curl -sS -o "${tmp_body}" -D "${tmp_headers}" -w '%{http_code} %{size_download} %{time_total} %{time_starttransfer}' -X "${method}" "${BASE_URL}${url_path}")
  local header
  for header in "${headers[@]}"; do
    if [[ -n "$header" ]]; then
      cmd+=( -H "$header" )
    fi
  done
  if [[ -n "${body}" ]]; then
    cmd+=( --data "${body}" )
  fi

  local metrics
  metrics="$("${cmd[@]}")"
  local http_code size_download time_total time_ttfb
  read -r http_code size_download time_total time_ttfb <<<"${metrics}"

  local ttfb_ms total_ms
  ttfb_ms="$(awk -v value="${time_ttfb}" 'BEGIN { printf "%.0f", value * 1000 }')"
  total_ms="$(awk -v value="${time_total}" 'BEGIN { printf "%.0f", value * 1000 }')"

  local baseline baseline_size baseline_ttfb
  baseline="$(baseline_for "${name}")"
  baseline_size="${baseline%,*}"
  baseline_ttfb="${baseline#*,}"

  local delta_size="-"
  local delta_ttfb="-"
  if [[ -n "${baseline_size}" ]]; then
    delta_size="$((size_download - baseline_size))"
  fi
  if [[ -n "${baseline_ttfb}" ]]; then
    delta_ttfb="$((ttfb_ms - baseline_ttfb))"
  fi

  local status="PASS"
  if (( http_code >= 400 )); then
    status="FAIL"
  fi
  if (( size_download > max_size_bytes )); then
    status="FAIL"
  fi
  if (( ttfb_ms > max_ttfb_ms )); then
    status="FAIL"
  fi

  if [[ "${status}" == "FAIL" ]]; then
    FAILURES=$((FAILURES + 1))
  fi

  RESULTS+=("${status}|${name}|${http_code}|${size_download}|${max_size_bytes}|${delta_size}|${ttfb_ms}|${max_ttfb_ms}|${delta_ttfb}|${total_ms}|${url_path}")

  rm -f "${tmp_body}" "${tmp_headers}"
}

create_thread_id() {
  local tmp_headers
  tmp_headers="$(mktemp)"
  curl -sS -o /dev/null -D "${tmp_headers}" -X POST \
    -H "authorization: Bearer ${OA_ACCESS_TOKEN}" \
    "${BASE_URL}/chat/new"

  local location
  location="$(awk 'BEGIN{IGNORECASE=1} /^location:/ {print $2}' "${tmp_headers}" | tr -d '\r' | head -n1)"
  rm -f "${tmp_headers}"

  if [[ -z "${location}" ]]; then
    return 1
  fi

  echo "${location}" | sed -E 's#^/chat/([^?]+).*$#\1#'
}

record_probe \
  "login_email_hx" \
  "POST" \
  "/login/email" \
  "email=$(printf '%s' "${OA_PERF_EMAIL}" | sed 's/@/%40/g')" \
  "12000" \
  "500" \
  "content-type: application/x-www-form-urlencoded" \
  "hx-request: true" \
  "hx-target: login-status"

record_probe \
  "feed_main_fragment_hx" \
  "GET" \
  "/feed/fragments/main?zone=all" \
  "" \
  "180000" \
  "700" \
  "hx-request: true" \
  "hx-target: feed-main-panel"

if [[ -n "${OA_ACCESS_TOKEN}" ]]; then
  record_probe \
    "settings_profile_update_hx" \
    "POST" \
    "/settings/profile/update" \
    "name=HTMX+Perf+Check" \
    "12000" \
    "600" \
    "authorization: Bearer ${OA_ACCESS_TOKEN}" \
    "content-type: application/x-www-form-urlencoded" \
    "hx-request: true" \
    "hx-target: settings-status"

  thread_id="$(create_thread_id || true)"
  if [[ -z "${thread_id}" ]]; then
    echo "WARN: unable to create chat thread for chat_send_hx probe" >&2
    FAILURES=$((FAILURES + 1))
    RESULTS+=("FAIL|chat_send_hx|000|0|12000|-|0|600|-|0|/chat/<thread>/send")
  else
    record_probe \
      "chat_send_hx" \
      "POST" \
      "/chat/${thread_id}/send" \
      "text=perf+ping" \
      "12000" \
      "600" \
      "authorization: Bearer ${OA_ACCESS_TOKEN}" \
      "content-type: application/x-www-form-urlencoded" \
      "hx-request: true" \
      "hx-target: chat-status"
  fi
elif [[ "${REQUIRE_AUTH_FLOWS}" == "1" ]]; then
  echo "ERROR: auth flows required but OA_ACCESS_TOKEN missing" >&2
  exit 2
else
  echo "INFO: OA_ACCESS_TOKEN not set; skipping auth-required probes (settings/chat)." >&2
fi

echo "HTMX Perf Check"
printf 'Base URL: %s\n' "${BASE_URL}"
printf 'Baseline: %s\n\n' "${HTMX_PERF_BASELINE_FILE}"
printf '%-6s %-30s %-6s %-10s %-11s %-10s %-9s %-10s %-10s %-9s %s\n' \
  "status" "probe" "http" "size" "size_budget" "size_delta" "ttfb_ms" "ttfb_budget" "ttfb_delta" "total_ms" "path"

for row in "${RESULTS[@]}"; do
  IFS='|' read -r status name http_code size_download max_size delta_size ttfb_ms max_ttfb delta_ttfb total_ms path <<<"${row}"
  printf '%-6s %-30s %-6s %-10s %-11s %-10s %-9s %-10s %-10s %-9s %s\n' \
    "${status}" "${name}" "${http_code}" "${size_download}" "${max_size}" "${delta_size}" "${ttfb_ms}" "${max_ttfb}" "${delta_ttfb}" "${total_ms}" "${path}"
done

if (( FAILURES > 0 )); then
  echo "\nHTMX perf check failed: ${FAILURES} probe(s) exceeded threshold or returned errors." >&2
  exit 1
fi

echo "\nHTMX perf check passed."
