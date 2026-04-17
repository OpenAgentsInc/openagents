#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq
require_cmd python3

ensure_gcloud_context

REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
REQUESTED_RELEASE_SHORT_SHA="${NEXUS_RELEASE_GIT_SHA:-auto}"
REQUESTED_RELEASE_SHORT_SHA="${REQUESTED_RELEASE_SHORT_SHA:0:12}"
LOCAL_RECEIPT_PATH="${REPORT_DIR}/${STAMP}-binary-release-rollback-${REQUESTED_RELEASE_SHORT_SHA}.json"
OVERALL_STARTED_MS="$(timestamp_unix_ms)"

if ! instance_exists "$NEXUS_VM"; then
  die "Nexus VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

TMP_REMOTE_SCRIPT="$(mktemp "${TMPDIR:-/tmp}/openagents-nexus-rollback.XXXXXX")"
trap 'rm -f "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

requested_release_sha="${1:-}"
release_root="$2"
current_link="$3"
previous_link="$4"
systemd_unit_path="$5"
image_unit_backup_path="$6"

target_release_dir=""
current_target=""
restored_mode="binary"

if sudo test -L "$current_link"; then
  current_target="$(sudo readlink -f "$current_link")"
fi

if [[ -n "$requested_release_sha" ]]; then
  target_release_dir="${release_root}/releases/${requested_release_sha}"
  sudo test -d "$target_release_dir"
elif sudo test -L "$previous_link"; then
  target_release_dir="$(sudo readlink -f "$previous_link")"
else
  target_release_dir=""
fi

if [[ -n "$target_release_dir" ]]; then
  if [[ -n "$current_target" && "$current_target" != "$target_release_dir" ]]; then
    sudo ln -sfn "$current_target" "$previous_link"
  fi
  sudo ln -sfn "$target_release_dir" "$current_link"
else
  sudo test -f "$image_unit_backup_path"
  sudo cp "$image_unit_backup_path" "$systemd_unit_path"
  restored_mode="image"
fi

sudo systemctl daemon-reload
sudo systemctl restart nexus-relay
sudo systemctl --no-pager --full status nexus-relay | sed -n '1,40p'

python3 - <<PY
import json
print(json.dumps({
    "restored_mode": "${restored_mode}",
    "restored_release_path": "${target_release_dir}",
    "current_target_before_rollback": "${current_target}",
}))
PY
REMOTE

chmod 755 "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-rollback-binary-release.sh" >/dev/null

REQUESTED_RELEASE_ARG=""
if [[ -n "${NEXUS_RELEASE_GIT_SHA:-}" ]]; then
  REQUESTED_RELEASE_ARG="${NEXUS_RELEASE_GIT_SHA}"
fi
REQUESTED_RELEASE_JSON="null"
if [[ -n "$REQUESTED_RELEASE_ARG" ]]; then
  REQUESTED_RELEASE_JSON="$(jq -Rn --arg value "$REQUESTED_RELEASE_ARG" '$value')"
fi

ROLLBACK_STARTED_MS="$(timestamp_unix_ms)"
ROLLBACK_RESULT="$(
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "chmod 755 /tmp/nexus-rollback-binary-release.sh && /tmp/nexus-rollback-binary-release.sh '${REQUESTED_RELEASE_ARG}' '${NEXUS_RELEASE_ROOT}' '${NEXUS_CURRENT_LINK}' '${NEXUS_PREVIOUS_LINK}' '${NEXUS_SYSTEMD_UNIT_PATH}' '${NEXUS_IMAGE_UNIT_BACKUP_PATH}'" \
    | tail -n 1
)"
ROLLBACK_FINISHED_MS="$(timestamp_unix_ms)"

mkdir -p "$REPORT_DIR"
OVERALL_FINISHED_MS="$(timestamp_unix_ms)"
python3 - "$LOCAL_RECEIPT_PATH" <<PY
import json
from pathlib import Path

receipt_path = Path("${LOCAL_RECEIPT_PATH}")
receipt = {
    "generated_at": "${STAMP}",
    "kind": "nexus_binary_release_rollback",
    "vm": "${NEXUS_VM}",
    "requested_release_git_sha": json.loads("""${REQUESTED_RELEASE_JSON}"""),
    "rollback_result": json.loads("""${ROLLBACK_RESULT}"""),
    "timing": {
        "started_unix_ms": int("${OVERALL_STARTED_MS}"),
        "finished_unix_ms": int("${OVERALL_FINISHED_MS}"),
        "total_duration_ms": int("${OVERALL_FINISHED_MS}") - int("${OVERALL_STARTED_MS}"),
        "rollback_started_unix_ms": int("${ROLLBACK_STARTED_MS}"),
        "rollback_finished_unix_ms": int("${ROLLBACK_FINISHED_MS}"),
        "rollback_duration_ms": int("${ROLLBACK_FINISHED_MS}") - int("${ROLLBACK_STARTED_MS}"),
    },
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
PY
jq empty "$LOCAL_RECEIPT_PATH" >/dev/null

log "Binary rollback finished"
log "receipt=${LOCAL_RECEIPT_PATH}"
log "result=${ROLLBACK_RESULT}"
