#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd git
require_cmd gcloud
require_cmd jq
require_cmd python3

ensure_gcloud_context

NEXUS_RELEASE_GIT_SHA="${NEXUS_RELEASE_GIT_SHA:-$(git -C "$ROOT_DIR" rev-parse HEAD)}"
NEXUS_RELEASE_SHORT_SHA="${NEXUS_RELEASE_SHORT_SHA:-${NEXUS_RELEASE_GIT_SHA:0:12}}"
REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
LOCAL_RECEIPT_PATH="${REPORT_DIR}/${STAMP}-binary-release-activate-${NEXUS_RELEASE_SHORT_SHA}.json"
OVERALL_STARTED_MS="$(timestamp_unix_ms)"

if ! instance_exists "$NEXUS_VM"; then
  die "Nexus VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

TMP_REMOTE_SCRIPT="$(mktemp "${TMPDIR:-/tmp}/openagents-nexus-activate.XXXXXX")"
trap 'rm -f "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

release_sha="$1"
release_root="$2"
current_link="$3"
previous_link="$4"
service_user="$5"
service_group="$6"
service_uid="$7"
runtime_env_path="$8"
upstream_config_path="$9"
systemd_unit_path="${10}"
image_unit_backup_path="${11}"

release_dir="${release_root}/releases/${release_sha}"
old_target=""
old_mode="none"

sudo test -d "$release_dir"
sudo test -f "${release_dir}/nexus-relay"
sudo test -f "$runtime_env_path"
sudo test -f "$upstream_config_path"

if ! id -u "$service_user" >/dev/null 2>&1; then
  sudo useradd --system --uid "$service_uid" --user-group --no-create-home --shell /usr/sbin/nologin "$service_user"
fi

sudo install -d -m 0755 "$release_root" "${release_root}/releases" "${release_root}/shared"

if sudo test -L "$current_link"; then
  old_target="$(sudo readlink -f "$current_link")"
  old_mode="binary"
elif sudo test -f "$systemd_unit_path" && sudo grep -q '^ExecStart=/usr/bin/docker run ' "$systemd_unit_path"; then
  old_mode="image"
  sudo cp "$systemd_unit_path" "$image_unit_backup_path"
fi

if [[ -n "$old_target" && "$old_target" != "$release_dir" ]]; then
  sudo ln -sfn "$old_target" "$previous_link"
fi

sudo ln -sfn "$release_dir" "$current_link"

sudo tee "$systemd_unit_path" >/dev/null <<UNIT
[Unit]
Description=OpenAgents Nexus durable relay + authority host
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${service_user}
Group=${service_group}
WorkingDirectory=${current_link}
EnvironmentFile=${runtime_env_path}
ExecStart=${current_link}/nexus-relay
Restart=always
RestartSec=10
TimeoutStopSec=45
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable nexus-relay >/dev/null
sudo systemctl restart nexus-relay
sudo systemctl --no-pager --full status nexus-relay | sed -n '1,40p'

python3 - <<PY
import json
print(json.dumps({
    "activated_release_path": "${release_dir}",
    "current_link": "${current_link}",
    "previous_link": "${previous_link}",
    "previous_target": "${old_target}",
    "previous_mode": "${old_mode}",
    "image_unit_backup_path": "${image_unit_backup_path}",
}))
PY
REMOTE

chmod 755 "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-activate-binary-release.sh" >/dev/null

ACTIVATION_STARTED_MS="$(timestamp_unix_ms)"
ACTIVATION_RESULT="$(
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "chmod 755 /tmp/nexus-activate-binary-release.sh && /tmp/nexus-activate-binary-release.sh '${NEXUS_RELEASE_GIT_SHA}' '${NEXUS_RELEASE_ROOT}' '${NEXUS_CURRENT_LINK}' '${NEXUS_PREVIOUS_LINK}' '${NEXUS_SERVICE_USER}' '${NEXUS_SERVICE_GROUP}' '${NEXUS_SERVICE_UID}' '${NEXUS_RUNTIME_ENV_PATH}' '${NEXUS_UPSTREAM_CONFIG_PATH}' '${NEXUS_SYSTEMD_UNIT_PATH}' '${NEXUS_IMAGE_UNIT_BACKUP_PATH}'" \
    | tail -n 1
)"
ACTIVATION_FINISHED_MS="$(timestamp_unix_ms)"

mkdir -p "$REPORT_DIR"
OVERALL_FINISHED_MS="$(timestamp_unix_ms)"
python3 - "$LOCAL_RECEIPT_PATH" <<PY
import json
from pathlib import Path

receipt_path = Path("${LOCAL_RECEIPT_PATH}")
receipt = {
    "generated_at": "${STAMP}",
    "kind": "nexus_binary_release_activate",
    "vm": "${NEXUS_VM}",
    "git_sha": "${NEXUS_RELEASE_GIT_SHA}",
    "git_short_sha": "${NEXUS_RELEASE_SHORT_SHA}",
    "activation_result": json.loads("""${ACTIVATION_RESULT}"""),
    "timing": {
        "started_unix_ms": int("${OVERALL_STARTED_MS}"),
        "finished_unix_ms": int("${OVERALL_FINISHED_MS}"),
        "total_duration_ms": int("${OVERALL_FINISHED_MS}") - int("${OVERALL_STARTED_MS}"),
        "activation_started_unix_ms": int("${ACTIVATION_STARTED_MS}"),
        "activation_finished_unix_ms": int("${ACTIVATION_FINISHED_MS}"),
        "activation_duration_ms": int("${ACTIVATION_FINISHED_MS}") - int("${ACTIVATION_STARTED_MS}"),
    },
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
PY
jq empty "$LOCAL_RECEIPT_PATH" >/dev/null

log "Binary release activated"
log "release=${NEXUS_RELEASE_GIT_SHA}"
log "receipt=${LOCAL_RECEIPT_PATH}"
log "result=${ACTIVATION_RESULT}"
