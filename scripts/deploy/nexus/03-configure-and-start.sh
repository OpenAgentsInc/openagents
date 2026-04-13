#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WATCHDOG_INSTALL_SCRIPT="${SCRIPT_DIR}/10-install-treasury-watchdog.sh"
TREASURY_ENV_VARS=(
  NEXUS_CONTROL_TREASURY_ENABLED
  NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW
  NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS
  NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE
  NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS
  NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS
  NEXUS_CONTROL_TREASURY_STATE_PATH
  NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH
  NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR
  NEXUS_CONTROL_TREASURY_WALLET_NETWORK
  NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV
  NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS
  NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS
  NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS
  NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV
  NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE
  NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON
)
EXPLICIT_TREASURY_ENV_VARS=""
for var in "${TREASURY_ENV_VARS[@]}"; do
  if [[ ${!var+x} == x ]]; then
    EXPLICIT_TREASURY_ENV_VARS+=" ${var}"
  fi
done
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context

treasury_env_is_explicit() {
  case " ${EXPLICIT_TREASURY_ENV_VARS} " in
    *" $1 "*) return 0 ;;
  esac
  return 1
}

preserve_remote_treasury_env() {
  local remote_env_path="/etc/nexus-relay/nexus-relay.env"
  local remote_env
  remote_env="$(
    gcloud compute ssh "$NEXUS_VM" \
      --tunnel-through-iap \
      --project "$GCP_PROJECT" \
      --zone "$GCP_ZONE" \
      --command "sudo test -f '${remote_env_path}' && sudo cat '${remote_env_path}' || true"
  )"

  [[ -n "$remote_env" ]] || return 0

  while IFS='=' read -r key value; do
    case "$key" in
      NEXUS_CONTROL_TREASURY_*)
        case "$key" in
          NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV|\
          NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE|\
          NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON)
            continue
            ;;
        esac
        value="${value%$'\r'}"
        [[ -n "$value" ]] || continue
        if treasury_env_is_explicit "$key"; then
          continue
        fi
        export "${key}=${value}"
        log "Preserving live treasury env ${key}=${value}"
        ;;
    esac
  done <<< "$remote_env"
}

load_remote_treasury_policy() {
  local remote_state_path="${NEXUS_CONTROL_TREASURY_STATE_PATH:-}"
  [[ -n "$remote_state_path" ]] || return 0
  local remote_state_path_quoted
  printf -v remote_state_path_quoted '%q' "$remote_state_path"

  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo bash -lc 'state_path=${remote_state_path_quoted}; if [[ -f \"\$state_path\" ]]; then tmp=\$(mktemp); cp \"\$state_path\" \"\$tmp\"; jq -c \".active_policy // empty\" \"\$tmp\" 2>/dev/null || true; rm -f \"\$tmp\"; fi'"
}

treasury_policy_change_requested() {
  local persisted_policy_json="$1"
  [[ "$(jq -r '.treasury_enabled' <<<"$persisted_policy_json")" != "${NEXUS_CONTROL_TREASURY_ENABLED}" ]] && return 0
  [[ "$(jq -r '.payout_sats_per_window' <<<"$persisted_policy_json")" != "${NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW}" ]] && return 0
  [[ "$(jq -r '.payout_interval_seconds' <<<"$persisted_policy_json")" != "${NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS}" ]] && return 0
  [[ "$(jq -r '.require_sellable' <<<"$persisted_policy_json")" != "${NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE}" ]] && return 0
  [[ "$(jq -r '.daily_budget_cap_sats' <<<"$persisted_policy_json")" != "${NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS}" ]] && return 0
  return 1
}

treasury_policy_change_is_destructive() {
  local persisted_policy_json="$1"
  local persisted_enabled persisted_payout persisted_interval persisted_require_sellable persisted_budget
  persisted_enabled="$(jq -r '.treasury_enabled' <<<"$persisted_policy_json")"
  persisted_payout="$(jq -r '.payout_sats_per_window' <<<"$persisted_policy_json")"
  persisted_interval="$(jq -r '.payout_interval_seconds' <<<"$persisted_policy_json")"
  persisted_require_sellable="$(jq -r '.require_sellable' <<<"$persisted_policy_json")"
  persisted_budget="$(jq -r '.daily_budget_cap_sats' <<<"$persisted_policy_json")"

  [[ "$persisted_enabled" == "true" && "${NEXUS_CONTROL_TREASURY_ENABLED}" != "true" ]] && return 0
  (( NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW < persisted_payout )) && return 0
  (( NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS > persisted_interval )) && return 0
  (( NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS < persisted_budget )) && return 0
  [[ "$persisted_require_sellable" != "true" && "${NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE}" == "true" ]] && return 0
  return 1
}

preserve_or_validate_persisted_treasury_policy() {
  local persisted_policy_json
  persisted_policy_json="$(load_remote_treasury_policy)"
  [[ -n "$persisted_policy_json" ]] || return 0

  if [[ "${NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV}" != "true" ]]; then
    export NEXUS_CONTROL_TREASURY_ENABLED="$(jq -r '.treasury_enabled' <<<"$persisted_policy_json")"
    export NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW="$(jq -r '.payout_sats_per_window' <<<"$persisted_policy_json")"
    export NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS="$(jq -r '.payout_interval_seconds' <<<"$persisted_policy_json")"
    export NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE="$(jq -r '.require_sellable' <<<"$persisted_policy_json")"
    export NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS="$(jq -r '.daily_budget_cap_sats' <<<"$persisted_policy_json")"
    log "Preserving persisted treasury policy checksum=$(jq -r '.checksum' <<<"$persisted_policy_json")"
    return 0
  fi

  if ! treasury_policy_change_requested "$persisted_policy_json"; then
    log "Treasury policy apply requested but the requested policy already matches persisted state"
    return 0
  fi

  [[ -n "${NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON}" ]] || die "Explicit treasury policy apply requires NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON"

  if treasury_policy_change_is_destructive "$persisted_policy_json" \
    && [[ "${NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE}" != "true" ]]; then
    die "Refusing destructive treasury policy change without NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE=true"
  fi

  log "Applying explicit treasury policy override against persisted checksum=$(jq -r '.checksum' <<<"$persisted_policy_json")"
}

current_remote_deploy_image() {
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo systemctl cat nexus-relay.service 2>/dev/null | awk '/^ExecStart=\\/usr\\/bin\\/docker run / { print \$NF }' | tail -n 1"
}

remote_nexus_service_start_unix_s() {
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command 'timestamp="$(systemctl show -p ActiveEnterTimestamp --value nexus-relay 2>/dev/null || true)"; if [[ -n "$timestamp" && "$timestamp" != "n/a" ]]; then date -d "$timestamp" +%s; else echo 0; fi'
}

remote_recent_completed_sends_since() {
  local since_unix_s="$1"
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo journalctl -u nexus-relay --since '@${since_unix_s}' --no-pager 2>/dev/null | grep -Ec 'Inserted payment: Payment \\{.*payment_type: Send, status: Completed, amount:' || true"
}

remote_treasury_status_json() {
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "curl -fsS --max-time 15 'http://127.0.0.1:8080/v1/treasury/status' 2>/dev/null || true"
}

post_restart_smoke_check_enabled() {
  [[ "${NEXUS_DEPLOY_POST_RESTART_SMOKE_ENABLED}" == "true" ]] \
    && [[ "${NEXUS_SKIP_POST_DEPLOY_SMOKE_CHECK}" != "true" ]]
}

perform_post_restart_smoke_check() {
  local deployed_image="$1"
  local previous_image="$2"

  post_restart_smoke_check_enabled || return 0

  local timeout_seconds="${NEXUS_DEPLOY_POST_RESTART_SMOKE_TIMEOUT_SECONDS}"
  local warmup_grace_seconds="${NEXUS_DEPLOY_POST_RESTART_WARMUP_GRACE_SECONDS:-${NEXUS_TREASURY_WATCHDOG_STARTUP_GRACE_SECONDS:-180}}"
  local poll_seconds="${NEXUS_DEPLOY_POST_RESTART_SMOKE_POLL_SECONDS}"
  local deadline_unix_s
  deadline_unix_s="$(( $(date +%s) + timeout_seconds ))"

  while (( $(date +%s) < deadline_unix_s )); do
    local service_state service_start_unix_s recent_completed status_json
    local sellable_targets wallet_runtime_status last_dispatch_at_unix_ms service_uptime_seconds

    service_state="$(
      gcloud compute ssh "$NEXUS_VM" \
        --tunnel-through-iap \
        --project "$GCP_PROJECT" \
        --zone "$GCP_ZONE" \
        --command 'systemctl is-active nexus-relay 2>/dev/null || true'
    )"

    if [[ "$service_state" != "active" ]]; then
      log "Waiting for post-deploy smoke: nexus-relay state=${service_state:-unknown}"
      sleep "$poll_seconds"
      continue
    fi

    service_start_unix_s="$(remote_nexus_service_start_unix_s)"
    service_uptime_seconds="unknown"
    if [[ "$service_start_unix_s" =~ ^[0-9]+$ ]] && (( service_start_unix_s > 0 )); then
      service_uptime_seconds="$(( $(date +%s) - service_start_unix_s ))"
    fi
    recent_completed="0"
    if [[ "$service_start_unix_s" =~ ^[0-9]+$ ]] && (( service_start_unix_s > 0 )); then
      recent_completed="$(remote_recent_completed_sends_since "$service_start_unix_s" | tr -d '[:space:]')"
    fi

    if [[ "$recent_completed" =~ ^[0-9]+$ ]] && (( recent_completed > 0 )); then
      log "Post-deploy smoke passed image=${deployed_image} recent_completed=${recent_completed}"
      return 0
    fi

    status_json="$(remote_treasury_status_json)"
    sellable_targets="unknown"
    wallet_runtime_status="unknown"
    last_dispatch_at_unix_ms="0"
    if [[ -n "$status_json" ]]; then
      sellable_targets="$(jq -r '.sellable_pylons_online_now // .eligible_online_payout_targets // 0' <<<"$status_json")"
      wallet_runtime_status="$(jq -r '.wallet_runtime_status // empty' <<<"$status_json")"
      last_dispatch_at_unix_ms="$(jq -r '.last_dispatch_at_unix_ms // 0' <<<"$status_json")"
      if [[ "$sellable_targets" =~ ^[0-9]+$ ]] && (( sellable_targets == 0 )) \
        && [[ "$wallet_runtime_status" == "connected" ]]; then
        log "Post-deploy smoke passed image=${deployed_image} with zero sellable payout targets"
        return 0
      fi
    fi

    if [[ "$service_uptime_seconds" =~ ^[0-9]+$ ]] && (( service_uptime_seconds < warmup_grace_seconds )); then
      log "Warming up post-deploy smoke image=${deployed_image} service_state=${service_state} service_uptime_seconds=${service_uptime_seconds} warmup_grace_seconds=${warmup_grace_seconds} recent_completed=${recent_completed} sellable=${sellable_targets} wallet_runtime_status=${wallet_runtime_status} last_dispatch_at_unix_ms=${last_dispatch_at_unix_ms}"
      sleep "$poll_seconds"
      continue
    fi

    log "Waiting for post-deploy payout smoke image=${deployed_image} phase=stalled_candidate service_state=${service_state} service_uptime_seconds=${service_uptime_seconds} warmup_grace_seconds=${warmup_grace_seconds} recent_completed=${recent_completed} sellable=${sellable_targets} wallet_runtime_status=${wallet_runtime_status} last_dispatch_at_unix_ms=${last_dispatch_at_unix_ms}"
    sleep "$poll_seconds"
  done

  if [[ -n "$previous_image" && "$previous_image" != "$deployed_image" ]]; then
    log "Post-deploy smoke failed for ${deployed_image}; rolling back to ${previous_image}"
    NEXUS_SKIP_POST_DEPLOY_SMOKE_CHECK=true DEPLOY_IMAGE="$previous_image" bash "$0"
    return 0
  fi

  die "Post-deploy smoke failed for ${deployed_image} and no rollback image was available"
}

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${NEXUS_IMAGE}}"
UPSTREAM_CONFIG_SOURCE="${ROOT_DIR}/apps/nexus-relay/deploy/upstream-config.toml"

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi
[[ -f "$UPSTREAM_CONFIG_SOURCE" ]] || die "Missing upstream config template: ${UPSTREAM_CONFIG_SOURCE}"

PREVIOUS_DEPLOY_IMAGE="$(current_remote_deploy_image || true)"

preserve_remote_treasury_env

: "${NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV:=false}"
: "${NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE:=false}"
: "${NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON:=}"
: "${NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV:=}"
: "${NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS:=3}"
: "${NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS:=16}"
: "${NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS:=300}"
: "${TOKIO_WORKER_THREADS:=16}"

preserve_or_validate_persisted_treasury_policy

if [[ "$NEXUS_VM" == "nexus-mainnet-1" ]] \
  && [[ "${NEXUS_ALLOW_ZERO_TREASURY_IN_PRODUCTION}" != "true" ]] \
  && { [[ "${NEXUS_CONTROL_TREASURY_ENABLED}" != "true" ]] \
    || [[ "${NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW}" == "0" ]]; }; then
  die "Refusing to deploy ${NEXUS_VM} with treasury disabled or zero payout. Export the production treasury envs first, or set NEXUS_ALLOW_ZERO_TREASURY_IN_PRODUCTION=true to override."
fi

TMP_ENV="$(mktemp)"
TMP_UPSTREAM_CONFIG="$(mktemp)"
TMP_REMOTE_SCRIPT="$(mktemp)"
trap 'rm -f "$TMP_ENV" "$TMP_UPSTREAM_CONFIG" "$TMP_REMOTE_SCRIPT"' EXIT

cp "$UPSTREAM_CONFIG_SOURCE" "$TMP_UPSTREAM_CONFIG"

cat >"$TMP_ENV" <<ENV
# Managed by scripts/deploy/nexus/03-configure-and-start.sh
RUST_LOG=info
TOKIO_WORKER_THREADS=${TOKIO_WORKER_THREADS}
NEXUS_RELAY_LISTEN_ADDR=${NEXUS_LISTEN_ADDR}
NEXUS_RELAY_UPSTREAM_LISTEN_ADDR=${NEXUS_UPSTREAM_LISTEN_ADDR}
NEXUS_RELAY_DATA_DIR=${NEXUS_DATA_DIR}
NEXUS_RELAY_PUBLIC_WS_URL=${NEXUS_PUBLIC_WS_URL}
NEXUS_RELAY_UPSTREAM_CONFIG_FILE=/etc/nexus-relay/upstream-config.toml
NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL=${NEXUS_PUBLIC_WS_URL}
NEXUS_CONTROL_RECEIPT_LOG_PATH=${NEXUS_RECEIPT_LOG_PATH}
NEXUS_CONTROL_TREASURY_ENABLED=${NEXUS_CONTROL_TREASURY_ENABLED}
NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW=${NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW}
NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS=${NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS}
NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE=${NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE}
NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS=${NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS}
NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS=${NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS}
NEXUS_CONTROL_TREASURY_STATE_PATH=${NEXUS_CONTROL_TREASURY_STATE_PATH}
NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH=${NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH}
NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR=${NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR}
NEXUS_CONTROL_TREASURY_WALLET_NETWORK=${NEXUS_CONTROL_TREASURY_WALLET_NETWORK}
NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV=${NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV}
NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS=${NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS}
NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS=${NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS}
NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS=${NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS}
NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV=${NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV}
NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE=${NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE}
NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON=${NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON}
ENV

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

DEPLOY_IMAGE="$1"
ENV_SOURCE_PATH="$2"
UPSTREAM_CONFIG_SOURCE_PATH="$3"
NEXUS_DATA_DIR="$4"
NEXUS_DATA_DISK_DEVICE_NAME="$5"

image_uses_remote_registry() {
  local image="$1"
  if [[ "$image" != */* ]]; then
    return 1
  fi
  local first_component="${image%%/*}"
  [[ "$first_component" == "localhost" || "$first_component" == *.* || "$first_component" == *:* ]]
}

sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get update -y
sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a \
  apt-get install -y \
  -o Dpkg::Options::=--force-confdef \
  -o Dpkg::Options::=--force-confold \
  ca-certificates curl jq docker.io sqlite3

sudo systemctl enable docker
sudo systemctl start docker

DATA_DISK_PATH="/dev/disk/by-id/google-${NEXUS_DATA_DISK_DEVICE_NAME}"
if [[ ! -b "$DATA_DISK_PATH" ]]; then
  echo "Could not locate Nexus data disk by-id path: ${DATA_DISK_PATH}" >&2
  exit 1
fi

if ! sudo blkid "$DATA_DISK_PATH" >/dev/null 2>&1; then
  sudo mkfs.ext4 -F "$DATA_DISK_PATH"
fi

sudo mkdir -p "$NEXUS_DATA_DIR"
if ! grep -q "${DATA_DISK_PATH} ${NEXUS_DATA_DIR} ext4" /etc/fstab; then
  echo "${DATA_DISK_PATH} ${NEXUS_DATA_DIR} ext4 defaults,nofail 0 2" | sudo tee -a /etc/fstab >/dev/null
fi
sudo mount -a

sudo mkdir -p /etc/nexus-relay
sudo mv "$ENV_SOURCE_PATH" /etc/nexus-relay/nexus-relay.env
sudo mv "$UPSTREAM_CONFIG_SOURCE_PATH" /etc/nexus-relay/upstream-config.toml
sudo chmod 640 /etc/nexus-relay/nexus-relay.env
sudo chmod 644 /etc/nexus-relay/upstream-config.toml
sudo chown root:root /etc/nexus-relay/nexus-relay.env
sudo chown root:root /etc/nexus-relay/upstream-config.toml

sudo mkdir -p "$NEXUS_DATA_DIR"
sudo chown -R 60000:60000 "$NEXUS_DATA_DIR"

if image_uses_remote_registry "$DEPLOY_IMAGE"; then
  AR_HOST="$(echo "$DEPLOY_IMAGE" | cut -d'/' -f1)"
  ACCESS_TOKEN="$(curl -fsS -H 'Metadata-Flavor: Google' \
    'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token')"

  sudo tee /usr/local/bin/nexus-registry-login.sh >/dev/null <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
AR_HOST="${AR_HOST}"
ACCESS_TOKEN="\$(curl -fsS -H 'Metadata-Flavor: Google' \
  'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token')"
printf '%s' "\${ACCESS_TOKEN}" | /usr/bin/docker login -u oauth2accesstoken --password-stdin "https://\${AR_HOST}"
SCRIPT

  sudo tee /usr/local/bin/nexus-prepare-image.sh >/dev/null <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
/usr/bin/docker pull "${DEPLOY_IMAGE}"
SCRIPT

  sudo chmod 755 /usr/local/bin/nexus-registry-login.sh /usr/local/bin/nexus-prepare-image.sh
  echo "$ACCESS_TOKEN" | sudo docker login -u oauth2accesstoken --password-stdin "https://${AR_HOST}"
  sudo /usr/local/bin/nexus-prepare-image.sh
else
  sudo tee /usr/local/bin/nexus-registry-login.sh >/dev/null <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
exit 0
SCRIPT

  sudo tee /usr/local/bin/nexus-prepare-image.sh >/dev/null <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
/usr/bin/docker image inspect "${DEPLOY_IMAGE}" >/dev/null
SCRIPT

  sudo chmod 755 /usr/local/bin/nexus-registry-login.sh /usr/local/bin/nexus-prepare-image.sh
  sudo /usr/local/bin/nexus-prepare-image.sh
fi

sudo tee /etc/systemd/system/nexus-relay.service >/dev/null <<UNIT
[Unit]
Description=OpenAgents Nexus durable relay + authority host
After=network-online.target docker.service
Wants=network-online.target
Requires=docker.service

[Service]
Type=simple
Restart=always
RestartSec=10
ExecStartPre=-/usr/bin/docker rm -f nexus-relay
ExecStartPre=/usr/local/bin/nexus-registry-login.sh
ExecStartPre=/usr/local/bin/nexus-prepare-image.sh
ExecStart=/usr/bin/docker run --rm --name nexus-relay --network host \
  --env-file /etc/nexus-relay/nexus-relay.env \
  -v /etc/nexus-relay:/etc/nexus-relay:ro \
  -v ${NEXUS_DATA_DIR}:${NEXUS_DATA_DIR} \
  ${DEPLOY_IMAGE}
ExecStop=/usr/bin/docker stop nexus-relay
TimeoutStopSec=45

[Install]
WantedBy=multi-user.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable nexus-relay
sudo systemctl restart nexus-relay

sudo systemctl --no-pager --full status nexus-relay | sed -n '1,40p'
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_ENV" "${NEXUS_VM}:/tmp/nexus-relay.env"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_UPSTREAM_CONFIG" "${NEXUS_VM}:/tmp/upstream-config.toml"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-bootstrap.sh"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/nexus-bootstrap.sh && /tmp/nexus-bootstrap.sh '$DEPLOY_IMAGE' '/tmp/nexus-relay.env' '/tmp/upstream-config.toml' '$NEXUS_DATA_DIR' '$NEXUS_DATA_DISK_DEVICE_NAME'"

if [[ -x "$WATCHDOG_INSTALL_SCRIPT" ]]; then
  bash "$WATCHDOG_INSTALL_SCRIPT"
fi

perform_post_restart_smoke_check "$DEPLOY_IMAGE" "$PREVIOUS_DEPLOY_IMAGE"

log "Nexus deployment refreshed on ${NEXUS_VM} using image ${DEPLOY_IMAGE}"
