#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context
ensure_services

if ! instance_exists "$SYMPHONY_VM"; then
  die "VM does not exist: ${SYMPHONY_VM}"
fi

INSTANCE_ID="$(gcloud compute instances describe "$SYMPHONY_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(id)')"
[[ -n "$INSTANCE_ID" ]] || die "Could not resolve instance id for ${SYMPHONY_VM}"

TMP_REMOTE_SCRIPT="$(mktemp)"
TMP_POLICY_DIR="$(mktemp -d)"
trap 'rm -f "$TMP_REMOTE_SCRIPT"; rm -rf "$TMP_POLICY_DIR"' EXIT

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

if ! dpkg -s google-cloud-ops-agent >/dev/null 2>&1; then
  curl -fsS https://dl.google.com/cloudagents/add-google-cloud-ops-agent-repo.sh -o /tmp/add-google-cloud-ops-agent-repo.sh
  sudo bash /tmp/add-google-cloud-ops-agent-repo.sh --also-install
fi

sudo mkdir -p /usr/local/bin
sudo tee /usr/local/bin/symphony-healthcheck.sh >/dev/null <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail

CFG="/etc/symphony/mainnet.toml"

rpc_user="$(grep '^rpc_user' "$CFG" | cut -d'"' -f2 || true)"
rpc_pass="$(grep '^rpc_pass' "$CFG" | cut -d'"' -f2 || true)"
rpc_addr="$(grep '^rpc_address' "$CFG" | cut -d'"' -f2 || true)"

if [[ -z "$rpc_user" || -z "$rpc_pass" || -z "$rpc_addr" ]]; then
  logger -t symphony-healthcheck "SYMPHONY_HEALTHCHECK status=fail reason=config_parse"
  exit 0
fi

tip_json="$(curl -fsS http://127.0.0.1:8080/tip 2>/dev/null || true)"
if [[ -z "$tip_json" ]]; then
  logger -t symphony-healthcheck "SYMPHONY_HEALTHCHECK status=fail reason=api_unreachable"
  exit 0
fi

symphony_height="$(printf '%s' "$tip_json" | jq -r '.height // .block_height // .data.block_height // empty')"
if [[ -z "$symphony_height" ]]; then
  logger -t symphony-healthcheck "SYMPHONY_HEALTHCHECK status=fail reason=api_malformed"
  exit 0
fi

rpc_payload='{"jsonrpc":"1.0","id":"symphony","method":"getblockchaininfo","params":[]}'
bitcoind_json="$(curl -fsS --user "${rpc_user}:${rpc_pass}" \
  --data-binary "$rpc_payload" \
  -H 'content-type: text/plain;' \
  "$rpc_addr" 2>/dev/null || true)"

bitcoind_height="$(printf '%s' "$bitcoind_json" | jq -r '.result.blocks // empty' 2>/dev/null || true)"
if [[ -z "$bitcoind_height" ]]; then
  logger -t symphony-healthcheck "SYMPHONY_HEALTHCHECK status=fail reason=bitcoind_rpc"
  exit 0
fi

lag=$((bitcoind_height - symphony_height))
if (( lag < 0 )); then
  lag=0
fi

if (( lag > 6 )); then
  logger -t symphony-healthcheck "SYMPHONY_HEALTHCHECK status=fail reason=tip_lag lag_blocks=${lag} bitcoind_height=${bitcoind_height} symphony_height=${symphony_height}"
else
  logger -t symphony-healthcheck "SYMPHONY_HEALTHCHECK status=ok lag_blocks=${lag} bitcoind_height=${bitcoind_height} symphony_height=${symphony_height}"
fi
SCRIPT

sudo chmod +x /usr/local/bin/symphony-healthcheck.sh

sudo tee /etc/systemd/system/symphony-healthcheck.service >/dev/null <<'UNIT'
[Unit]
Description=Symphony healthcheck probe
After=network-online.target symphony.service
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/symphony-healthcheck.sh
UNIT

sudo tee /etc/systemd/system/symphony-healthcheck.timer >/dev/null <<'TIMER'
[Unit]
Description=Run Symphony healthcheck every 2 minutes

[Timer]
OnBootSec=90s
OnUnitActiveSec=120s
AccuracySec=30s
Unit=symphony-healthcheck.service

[Install]
WantedBy=timers.target
TIMER

sudo systemctl daemon-reload
sudo systemctl enable symphony-healthcheck.timer
sudo systemctl restart symphony-healthcheck.timer
sudo systemctl start symphony-healthcheck.service
sudo systemctl --no-pager --full status symphony-healthcheck.timer | sed -n '1,30p'
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${SYMPHONY_VM}:/tmp/symphony-ops-bootstrap.sh"

gcloud compute ssh "$SYMPHONY_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/symphony-ops-bootstrap.sh && /tmp/symphony-ops-bootstrap.sh"

create_or_update_counter_metric() {
  local name="$1"
  local description="$2"
  local filter="$3"

  if gcloud logging metrics describe "$name" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "Updating log metric: ${name}"
    gcloud logging metrics update "$name" \
      --project "$GCP_PROJECT" \
      --description "$description" \
      --log-filter "$filter" >/dev/null
  else
    log "Creating log metric: ${name}"
    gcloud logging metrics create "$name" \
      --project "$GCP_PROJECT" \
      --description "$description" \
      --log-filter "$filter" >/dev/null
  fi
}

create_or_update_counter_metric \
  "symphony_healthcheck_fail_count" \
  "Count of Symphony healthcheck failures" \
  "resource.type=\"gce_instance\" AND resource.labels.instance_id=\"${INSTANCE_ID}\" AND textPayload:\"SYMPHONY_HEALTHCHECK status=fail\""

create_or_update_counter_metric \
  "symphony_tip_lag_fail_count" \
  "Count of Symphony healthcheck failures due to tip lag" \
  "resource.type=\"gce_instance\" AND resource.labels.instance_id=\"${INSTANCE_ID}\" AND textPayload:\"SYMPHONY_HEALTHCHECK status=fail reason=tip_lag\""

CHANNELS_JSON='[]'
if [[ -n "${MONITORING_NOTIFICATION_CHANNELS:-}" ]]; then
  CHANNELS_JSON="$(printf '%s' "$MONITORING_NOTIFICATION_CHANNELS" | tr ',' '\n' | jq -R . | jq -s .)"
fi

write_policy_file() {
  local file_path="$1"
  local payload="$2"
  printf '%s\n' "$payload" >"$file_path"
}

write_policy_file "${TMP_POLICY_DIR}/process-down.json" "{
  \"displayName\": \"Symphony Process Down (${SYMPHONY_VM})\",
  \"combiner\": \"OR\",
  \"enabled\": true,
  \"notificationChannels\": ${CHANNELS_JSON},
  \"conditions\": [
    {
      \"displayName\": \"instance uptime absent 5m\",
      \"conditionAbsent\": {
        \"filter\": \"resource.type=\\\"gce_instance\\\" AND resource.label.instance_id=\\\"${INSTANCE_ID}\\\" AND metric.type=\\\"compute.googleapis.com/instance/uptime\\\"\",
        \"duration\": \"300s\",
        \"aggregations\": [
          {
            \"alignmentPeriod\": \"60s\",
            \"perSeriesAligner\": \"ALIGN_RATE\"
          }
        ],
        \"trigger\": { \"count\": 1 }
      }
    }
  ],
  \"documentation\": {
    \"content\": \"Symphony VM is not reporting uptime metrics for 5 minutes.\",
    \"mimeType\": \"text/markdown\"
  }
}"

write_policy_file "${TMP_POLICY_DIR}/api-health.json" "{
  \"displayName\": \"Symphony API Healthcheck Failures (${SYMPHONY_VM})\",
  \"combiner\": \"OR\",
  \"enabled\": true,
  \"notificationChannels\": ${CHANNELS_JSON},
  \"conditions\": [
    {
      \"displayName\": \"healthcheck fail count > 0\",
      \"conditionThreshold\": {
        \"filter\": \"resource.type=\\\"gce_instance\\\" AND resource.label.instance_id=\\\"${INSTANCE_ID}\\\" AND metric.type=\\\"logging.googleapis.com/user/symphony_healthcheck_fail_count\\\"\",
        \"comparison\": \"COMPARISON_GT\",
        \"thresholdValue\": 0,
        \"duration\": \"0s\",
        \"aggregations\": [
          {
            \"alignmentPeriod\": \"120s\",
            \"perSeriesAligner\": \"ALIGN_DELTA\"
          }
        ],
        \"trigger\": { \"count\": 1 }
      }
    }
  ],
  \"documentation\": {
    \"content\": \"Symphony healthcheck emitted one or more failures in the last 2 minutes.\",
    \"mimeType\": \"text/markdown\"
  }
}"

write_policy_file "${TMP_POLICY_DIR}/tip-lag.json" "{
  \"displayName\": \"Symphony Tip Lag Failures (${SYMPHONY_VM})\",
  \"combiner\": \"OR\",
  \"enabled\": true,
  \"notificationChannels\": ${CHANNELS_JSON},
  \"conditions\": [
    {
      \"displayName\": \"tip lag fail count > 0\",
      \"conditionThreshold\": {
        \"filter\": \"resource.type=\\\"gce_instance\\\" AND resource.label.instance_id=\\\"${INSTANCE_ID}\\\" AND metric.type=\\\"logging.googleapis.com/user/symphony_tip_lag_fail_count\\\"\",
        \"comparison\": \"COMPARISON_GT\",
        \"thresholdValue\": 0,
        \"duration\": \"0s\",
        \"aggregations\": [
          {
            \"alignmentPeriod\": \"120s\",
            \"perSeriesAligner\": \"ALIGN_DELTA\"
          }
        ],
        \"trigger\": { \"count\": 1 }
      }
    }
  ],
  \"documentation\": {
    \"content\": \"Symphony healthcheck detected tip lag above threshold.\",
    \"mimeType\": \"text/markdown\"
  }
}"

write_policy_file "${TMP_POLICY_DIR}/memory-pressure.json" "{
  \"displayName\": \"Symphony Memory Pressure (${SYMPHONY_VM})\",
  \"combiner\": \"OR\",
  \"enabled\": true,
  \"notificationChannels\": ${CHANNELS_JSON},
  \"conditions\": [
    {
      \"displayName\": \"memory percent used > 90\",
      \"conditionThreshold\": {
        \"filter\": \"resource.type=\\\"gce_instance\\\" AND resource.label.instance_id=\\\"${INSTANCE_ID}\\\" AND metric.type=\\\"agent.googleapis.com/memory/percent_used\\\"\",
        \"comparison\": \"COMPARISON_GT\",
        \"thresholdValue\": 90,
        \"duration\": \"300s\",
        \"aggregations\": [
          {
            \"alignmentPeriod\": \"60s\",
            \"perSeriesAligner\": \"ALIGN_MEAN\"
          }
        ],
        \"trigger\": { \"count\": 1 }
      }
    }
  ],
  \"documentation\": {
    \"content\": \"Symphony VM memory usage has been above 90% for 5 minutes.\",
    \"mimeType\": \"text/markdown\"
  }
}"

write_policy_file "${TMP_POLICY_DIR}/disk-saturation.json" "{
  \"displayName\": \"Symphony Disk Saturation (${SYMPHONY_VM})\",
  \"combiner\": \"OR\",
  \"enabled\": true,
  \"notificationChannels\": ${CHANNELS_JSON},
  \"conditions\": [
    {
      \"displayName\": \"disk percent used > 85\",
      \"conditionThreshold\": {
        \"filter\": \"resource.type=\\\"gce_instance\\\" AND resource.label.instance_id=\\\"${INSTANCE_ID}\\\" AND metric.type=\\\"agent.googleapis.com/disk/percent_used\\\"\",
        \"comparison\": \"COMPARISON_GT\",
        \"thresholdValue\": 85,
        \"duration\": \"300s\",
        \"aggregations\": [
          {
            \"alignmentPeriod\": \"60s\",
            \"perSeriesAligner\": \"ALIGN_MAX\"
          }
        ],
        \"trigger\": { \"count\": 1 }
      }
    }
  ],
  \"documentation\": {
    \"content\": \"Symphony data disk usage has been above 85% for 5 minutes.\",
    \"mimeType\": \"text/markdown\"
  }
}"

upsert_policy() {
  local display_name="$1"
  local file_path="$2"
  local existing

  existing="$(gcloud monitoring policies list \
    --project "$GCP_PROJECT" \
    --format='json' | jq -r ".[] | select(.displayName == \"${display_name}\") | .name" | head -n1)"

  if [[ -n "$existing" ]]; then
    log "Updating policy: ${display_name}"
    gcloud monitoring policies update "$existing" \
      --project "$GCP_PROJECT" \
      --policy-from-file "$file_path" >/dev/null
  else
    log "Creating policy: ${display_name}"
    gcloud monitoring policies create \
      --project "$GCP_PROJECT" \
      --policy-from-file "$file_path" >/dev/null
  fi
}

upsert_policy "Symphony Process Down (${SYMPHONY_VM})" "${TMP_POLICY_DIR}/process-down.json"
upsert_policy "Symphony API Healthcheck Failures (${SYMPHONY_VM})" "${TMP_POLICY_DIR}/api-health.json"
upsert_policy "Symphony Tip Lag Failures (${SYMPHONY_VM})" "${TMP_POLICY_DIR}/tip-lag.json"
upsert_policy "Symphony Memory Pressure (${SYMPHONY_VM})" "${TMP_POLICY_DIR}/memory-pressure.json"
upsert_policy "Symphony Disk Saturation (${SYMPHONY_VM})" "${TMP_POLICY_DIR}/disk-saturation.json"

if ! gcloud compute resource-policies describe symphony-daily-7d \
  --project "$GCP_PROJECT" \
  --region "$GCP_REGION" >/dev/null 2>&1; then
  log "Creating snapshot schedule: symphony-daily-7d"
  gcloud compute resource-policies create snapshot-schedule symphony-daily-7d \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --max-retention-days 7 \
    --daily-schedule \
    --start-time 03:00 >/dev/null
fi

if ! gcloud compute disks describe "$SYMPHONY_DATA_DISK" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(resourcePolicies)' | grep -q 'symphony-daily-7d'; then
  log "Attaching snapshot policy to disk: ${SYMPHONY_DATA_DISK}"
  gcloud compute disks add-resource-policies "$SYMPHONY_DATA_DISK" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --resource-policies symphony-daily-7d >/dev/null
fi

log "Ops bootstrap complete"
