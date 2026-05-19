#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd curl
require_cmd gcloud
require_cmd jq
require_cmd python3

ensure_gcloud_context

NEXUS_PYLON_HOSTS="${NEXUS_PYLON_HOSTS:-pylon-gcp-1 pylon-gcp-2 pylon-gcp-3 pylon-gcp-4 pylon-gcp-5 pylon-gcp-6 pylon-gcp-7}"
NEXUS_PYLON_SERVICE="${NEXUS_PYLON_SERVICE:-pylon.service}"
NEXUS_PYLON_REGISTER_DRY_RUN="${NEXUS_PYLON_REGISTER_DRY_RUN:-false}"
NEXUS_PYLON_REPLACE_PAYOUT_TARGETS="${NEXUS_PYLON_REPLACE_PAYOUT_TARGETS:-false}"
NEXUS_PYLON_WAIT_FOR_REGISTRATION="${NEXUS_PYLON_WAIT_FOR_REGISTRATION:-true}"
NEXUS_PYLON_REGISTRATION_TIMEOUT_SECONDS="${NEXUS_PYLON_REGISTRATION_TIMEOUT_SECONDS:-300}"
NEXUS_PYLON_REGISTRATION_POLL_SECONDS="${NEXUS_PYLON_REGISTRATION_POLL_SECONDS:-5}"
NEXUS_LDK_CLI_BASE_URL="${NEXUS_LDK_CLI_BASE_URL:-${NEXUS_LDK_VM}:3536}"

hosts=()
read -r -a hosts <<<"$NEXUS_PYLON_HOSTS"
[[ "${#hosts[@]}" -gt 0 ]] || die "NEXUS_PYLON_HOSTS resolved to an empty host list"

shell_quote() {
  printf "%q" "$1"
}

target_summary_json() {
  local target="$1"
  TARGET="$target" python3 - <<'PY'
import hashlib
import json
import os

target = os.environ["TARGET"].strip()
lower = target.lower()
if lower.startswith("lno1"):
    kind = "bolt12_offer"
elif lower.startswith(("lnbc", "lntb", "lnbcrt", "lntbs")):
    kind = "bolt11_invoice"
elif lower.startswith("lnurl"):
    kind = "lnurl_pay"
elif "@" in target:
    kind = "bip353_name"
elif target:
    kind = "unsupported"
else:
    kind = "missing"

print(json.dumps({
    "kind": kind,
    "sha256_16": hashlib.sha256(target.encode()).hexdigest()[:16] if target else None,
    "length": len(target),
}, sort_keys=True))
PY
}

target_kind() {
  target_summary_json "$1" | jq -r '.kind'
}

get_pylon_target() {
  local host="$1"
  gcloud compute ssh "$host" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo -u pylon /usr/local/bin/pylon config show" \
    2>/dev/null \
    | python3 -c 'import json,sys; print((json.load(sys.stdin).get("payout_destination") or "").strip())'
}

generate_ldk_bolt12_offer() {
  local host="$1"
  local description="OpenAgents-hosted-Pylon-${host}-payout-target"
  local quoted_description quoted_base_url
  quoted_description="$(shell_quote "$description")"
  quoted_base_url="$(shell_quote "$NEXUS_LDK_CLI_BASE_URL")"

  gcloud compute ssh "$NEXUS_LDK_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo -u ldk-server env LDK_OFFER_DESCRIPTION=${quoted_description} LDK_CLI_BASE_URL=${quoted_base_url} bash -s" <<'REMOTE'
set -euo pipefail

err="$(mktemp)"
api_key="$(xxd -p -c 64 /var/lib/ldk-server/bitcoin/api_key)"
if ! output="$(
  ldk-server-cli \
    --base-url "$LDK_CLI_BASE_URL" \
    --config /etc/ldk-server/ldk-server.toml \
    --tls-cert /var/lib/ldk-server/tls.crt \
    --api-key "$api_key" \
    bolt12-receive "$LDK_OFFER_DESCRIPTION" \
    2>"$err"
)"; then
  cat "$err" >&2
  rm -f "$err"
  exit 1
fi
rm -f "$err"

LDK_CLI_OUTPUT="$output" python3 - <<'PY'
import json
import os
import sys

raw = os.environ["LDK_CLI_OUTPUT"].strip()
try:
    decoded = json.loads(raw)
except json.JSONDecodeError:
    decoded = raw

if isinstance(decoded, dict):
    offer = str(decoded.get("offer") or "").strip()
else:
    offer = str(decoded).strip()

if not offer.startswith("lno1"):
    print("ldk-server-cli did not return a BOLT12 offer", file=sys.stderr)
    sys.exit(1)

print(offer)
PY
REMOTE
}

configure_pylon_target() {
  local host="$1"
  local target="$2"
  local quoted_target quoted_service
  quoted_target="$(shell_quote "$target")"
  quoted_service="$(shell_quote "$NEXUS_PYLON_SERVICE")"

  gcloud compute ssh "$host" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo env PYLON_PAYOUT_TARGET=${quoted_target} PYLON_SERVICE=${quoted_service} bash -s" <<'REMOTE'
set -euo pipefail

sudo -u pylon /usr/local/bin/pylon config set payout_destination "$PYLON_PAYOUT_TARGET" >/dev/null
systemctl restart "$PYLON_SERVICE"
systemctl is-active "$PYLON_SERVICE" >/dev/null
sleep 2
REMOTE
}

stats_value() {
  local key="$1"
  curl -fsS "${NEXUS_PUBLIC_URL%/}/api/stats" | jq -r --arg key "$key" '.[$key] // 0'
}

wait_for_registration_count() {
  local expected_min="$1"
  local start now current eligible missing
  start="$(date +%s)"

  while true; do
    current="$(stats_value nexus_ldk_payout_target_identities)"
    eligible="$(stats_value homework_worker_eligible_pylons_online_now)"
    missing="$(stats_value nexus_missing_payout_target_blocked_beneficiaries_now)"
    log "Pylon LDK registration stats: ldk_targets=${current} eligible_workers=${eligible} missing_targets=${missing} expected_min=${expected_min}"

    if [[ "$current" =~ ^[0-9]+$ ]] && (( current >= expected_min )); then
      return 0
    fi

    now="$(date +%s)"
    if (( now - start >= NEXUS_PYLON_REGISTRATION_TIMEOUT_SECONDS )); then
      die "Timed out waiting for Nexus to observe at least ${expected_min} LDK payout target identities"
    fi
    sleep "$NEXUS_PYLON_REGISTRATION_POLL_SECONDS"
  done
}

baseline_ldk_targets="$(stats_value nexus_ldk_payout_target_identities)"
[[ "$baseline_ldk_targets" =~ ^[0-9]+$ ]] || baseline_ldk_targets=0

log "Registering hosted Pylon LDK targets on hosts: ${NEXUS_PYLON_HOSTS}"
log "Baseline Nexus LDK payout target identities: ${baseline_ldk_targets}"

changed=0
eligible_target_count=0

for host in "${hosts[@]}"; do
  instance_exists "$host" || die "Pylon VM does not exist: ${host}"

  current_target="$(get_pylon_target "$host")"
  current_kind="$(target_kind "$current_target")"

  if [[ "$current_kind" != "missing" && "$current_kind" != "unsupported" && "$NEXUS_PYLON_REPLACE_PAYOUT_TARGETS" != "true" ]]; then
    log "Skipping ${host}; existing target $(target_summary_json "$current_target")"
    eligible_target_count=$((eligible_target_count + 1))
    continue
  fi

  if [[ "$current_kind" == "unsupported" && "$NEXUS_PYLON_REPLACE_PAYOUT_TARGETS" != "true" ]]; then
    die "${host} has unsupported existing payout target; set NEXUS_PYLON_REPLACE_PAYOUT_TARGETS=true to replace it"
  fi

  log "Generating LDK BOLT12 target for ${host}"
  next_target="$(generate_ldk_bolt12_offer "$host")"
  next_summary="$(target_summary_json "$next_target")"
  [[ "$(jq -r '.kind' <<<"$next_summary")" == "bolt12_offer" ]] || die "Generated target for ${host} was not BOLT12"

  if [[ "$NEXUS_PYLON_REGISTER_DRY_RUN" == "true" ]]; then
    log "Dry run: would configure ${host} target ${next_summary}"
  else
    log "Configuring ${host} target ${next_summary}"
    configure_pylon_target "$host" "$next_target"
    changed=$((changed + 1))
  fi
  eligible_target_count=$((eligible_target_count + 1))
done

log "Hosted Pylon LDK target rollout complete changed=${changed} configured_or_existing=${eligible_target_count}/${#hosts[@]}"

if [[ "$NEXUS_PYLON_REGISTER_DRY_RUN" != "true" && "$NEXUS_PYLON_WAIT_FOR_REGISTRATION" == "true" ]]; then
  expected_min="$baseline_ldk_targets"
  if (( eligible_target_count > expected_min )); then
    expected_min="$eligible_target_count"
  fi
  wait_for_registration_count "$expected_min"
fi
