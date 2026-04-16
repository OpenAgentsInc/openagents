#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd git
require_cmd gcloud
require_cmd jq
require_cmd python3
require_cmd tar

ensure_gcloud_context

NEXUS_RELEASE_GIT_SHA="${NEXUS_RELEASE_GIT_SHA:-$(git -C "$ROOT_DIR" rev-parse HEAD)}"
NEXUS_RELEASE_SHORT_SHA="${NEXUS_RELEASE_SHORT_SHA:-${NEXUS_RELEASE_GIT_SHA:0:12}}"
BUILDER_ARTIFACT_ROOT="${NEXUS_BUILDER_CACHE_MOUNT_POINT}/artifacts/${NEXUS_RELEASE_GIT_SHA}"
ARCHIVE_NAME="nexus-release-${NEXUS_RELEASE_GIT_SHA}.tar.gz"
BUILDER_ARCHIVE_PATH="/tmp/${ARCHIVE_NAME}"
REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
LOCAL_RECEIPT_PATH="${REPORT_DIR}/${STAMP}-binary-release-upload-${NEXUS_RELEASE_SHORT_SHA}.json"
OVERALL_STARTED_MS="$(timestamp_unix_ms)"

if ! instance_exists "$NEXUS_BUILDER_VM"; then
  die "Warm builder VM does not exist: ${NEXUS_BUILDER_VM}. Run 11-provision-warm-builder.sh first."
fi
if ! instance_exists "$NEXUS_VM"; then
  die "Nexus VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-release-upload.XXXXXX")"
TMP_REMOTE_SCRIPT="$(mktemp "${TMPDIR:-/tmp}/openagents-nexus-upload.XXXXXX")"
trap 'rm -rf "$TMP_DIR" "$TMP_REMOTE_SCRIPT"' EXIT

log "Archiving builder artifact ${NEXUS_RELEASE_GIT_SHA} on ${NEXUS_BUILDER_VM}"
ARCHIVE_STARTED_MS="$(timestamp_unix_ms)"
BUILDER_ARCHIVE_SIZE_BYTES="$(
  gcloud compute ssh "$NEXUS_BUILDER_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "set -euo pipefail; rm -f '${BUILDER_ARCHIVE_PATH}'; tar -C '${BUILDER_ARTIFACT_ROOT}' -czf '${BUILDER_ARCHIVE_PATH}' .; stat -c '%s' '${BUILDER_ARCHIVE_PATH}'"
)"
ARCHIVE_FINISHED_MS="$(timestamp_unix_ms)"

LOCAL_ARCHIVE_PATH="${TMP_DIR}/${ARCHIVE_NAME}"
log "Downloading builder archive ${ARCHIVE_NAME} from ${NEXUS_BUILDER_VM}"
DOWNLOAD_STARTED_MS="$(timestamp_unix_ms)"
gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "${NEXUS_BUILDER_VM}:${BUILDER_ARCHIVE_PATH}" \
  "$LOCAL_ARCHIVE_PATH" >/dev/null
DOWNLOAD_FINISHED_MS="$(timestamp_unix_ms)"
[[ -f "$LOCAL_ARCHIVE_PATH" ]] || die "Builder archive was not downloaded: ${LOCAL_ARCHIVE_PATH}"

gcloud compute ssh "$NEXUS_BUILDER_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "rm -f '${BUILDER_ARCHIVE_PATH}'" >/dev/null

LOCAL_RELEASE_DIR="${TMP_DIR}/${NEXUS_RELEASE_GIT_SHA}"
mkdir -p "$LOCAL_RELEASE_DIR"
tar -xzf "$LOCAL_ARCHIVE_PATH" -C "$LOCAL_RELEASE_DIR"
[[ -d "$LOCAL_RELEASE_DIR" ]] || die "Builder artifact was not downloaded: ${LOCAL_RELEASE_DIR}"
[[ -f "${LOCAL_RELEASE_DIR}/nexus-relay" ]] || die "Missing nexus-relay binary in ${LOCAL_RELEASE_DIR}"
[[ -f "${LOCAL_RELEASE_DIR}/build-metadata.json" ]] || die "Missing build metadata in ${LOCAL_RELEASE_DIR}"
jq empty "${LOCAL_RELEASE_DIR}/build-metadata.json" >/dev/null
BINARY_SHA256="$(if [[ -f "${LOCAL_RELEASE_DIR}/nexus-relay.sha256" ]]; then tr -d '[:space:]' <"${LOCAL_RELEASE_DIR}/nexus-relay.sha256"; else shasum -a 256 "${LOCAL_RELEASE_DIR}/nexus-relay" | awk '{print $1}'; fi)"
BUILD_METADATA_JSON="$(cat "${LOCAL_RELEASE_DIR}/build-metadata.json")"

log "Uploading release archive ${ARCHIVE_NAME} to ${NEXUS_VM}"
UPLOAD_STARTED_MS="$(timestamp_unix_ms)"
gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$LOCAL_ARCHIVE_PATH" \
  "${NEXUS_VM}:/tmp/${ARCHIVE_NAME}" >/dev/null
UPLOAD_FINISHED_MS="$(timestamp_unix_ms)"

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

release_sha="$1"
release_root="$2"
service_user="$3"
service_group="$4"
service_uid="$5"
runtime_env_path="$6"
upstream_config_path="$7"

archive_path="/tmp/nexus-release-${release_sha}.tar.gz"
tmp_release_dir="/tmp/${release_sha}"
release_dir="${release_root}/releases/${release_sha}"
release_tmp_dir="${release_dir}.tmp"
shared_dir="${release_root}/shared"
releases_dir="${release_root}/releases"

sudo install -d -m 0755 "$release_root" "$releases_dir" "$shared_dir"

if ! id -u "$service_user" >/dev/null 2>&1; then
  sudo useradd --system --uid "$service_uid" --user-group --no-create-home --shell /usr/sbin/nologin "$service_user"
fi

sudo test -f "$runtime_env_path"
sudo test -f "$upstream_config_path"
sudo test -f "$archive_path"

reused_existing_release="false"
if sudo test -d "$release_dir"; then
  reused_existing_release="true"
else
  sudo rm -rf "$tmp_release_dir"
  sudo install -d -m 0755 "$tmp_release_dir"
  sudo tar -xzf "$archive_path" -C "$tmp_release_dir"
  sudo test -f "${tmp_release_dir}/nexus-relay"
  sudo test -f "${tmp_release_dir}/build-metadata.json"
  sudo rm -rf "$release_tmp_dir"
  sudo mv "$tmp_release_dir" "$release_tmp_dir"
  sudo chmod 0755 "${release_tmp_dir}/nexus-relay"
  sudo find "$release_tmp_dir" -type f ! -name 'nexus-relay' -exec chmod 0644 {} +
  sudo chown -R root:root "$release_tmp_dir"
  sudo mv "$release_tmp_dir" "$release_dir"
fi
sudo rm -f "$archive_path"

export RELEASE_DIR="$release_dir"
export REUSED_EXISTING_RELEASE="$reused_existing_release"
export RUNTIME_ENV_PATH="$runtime_env_path"
export UPSTREAM_CONFIG_PATH="$upstream_config_path"
python3 - <<'PY'
import json
import os

print(json.dumps({
    "release_path": os.environ["RELEASE_DIR"],
    "reused_existing_release": os.environ["REUSED_EXISTING_RELEASE"] == "true",
    "runtime_env_path": os.environ["RUNTIME_ENV_PATH"],
    "upstream_config_path": os.environ["UPSTREAM_CONFIG_PATH"],
}))
PY
REMOTE

chmod 755 "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-upload-binary-release.sh" >/dev/null

REMOTE_INSTALL_STARTED_MS="$(timestamp_unix_ms)"
UPLOAD_RESULT="$(
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "chmod 755 /tmp/nexus-upload-binary-release.sh && /tmp/nexus-upload-binary-release.sh '${NEXUS_RELEASE_GIT_SHA}' '${NEXUS_RELEASE_ROOT}' '${NEXUS_SERVICE_USER}' '${NEXUS_SERVICE_GROUP}' '${NEXUS_SERVICE_UID}' '${NEXUS_RUNTIME_ENV_PATH}' '${NEXUS_UPSTREAM_CONFIG_PATH}'"
)"
REMOTE_INSTALL_FINISHED_MS="$(timestamp_unix_ms)"

mkdir -p "$REPORT_DIR"
OVERALL_FINISHED_MS="$(timestamp_unix_ms)"
python3 - "$LOCAL_RECEIPT_PATH" <<PY
import json
from pathlib import Path

receipt_path = Path("${LOCAL_RECEIPT_PATH}")
receipt = {
    "generated_at": "${STAMP}",
    "kind": "nexus_binary_release_upload",
    "vm": "${NEXUS_VM}",
    "builder_vm": "${NEXUS_BUILDER_VM}",
    "git_sha": "${NEXUS_RELEASE_GIT_SHA}",
    "git_short_sha": "${NEXUS_RELEASE_SHORT_SHA}",
    "builder_artifact_root": "${BUILDER_ARTIFACT_ROOT}",
    "builder_archive_path": "${BUILDER_ARCHIVE_PATH}",
    "builder_archive_size_bytes": int("${BUILDER_ARCHIVE_SIZE_BYTES//$'\n'/}"),
    "binary_sha256": "${BINARY_SHA256}",
    "build_metadata": json.loads("""${BUILD_METADATA_JSON}"""),
    "upload_result": json.loads("""${UPLOAD_RESULT}"""),
    "timing": {
        "started_unix_ms": int("${OVERALL_STARTED_MS}"),
        "finished_unix_ms": int("${OVERALL_FINISHED_MS}"),
        "total_duration_ms": int("${OVERALL_FINISHED_MS}") - int("${OVERALL_STARTED_MS}"),
        "archive_started_unix_ms": int("${ARCHIVE_STARTED_MS}"),
        "archive_finished_unix_ms": int("${ARCHIVE_FINISHED_MS}"),
        "archive_duration_ms": int("${ARCHIVE_FINISHED_MS}") - int("${ARCHIVE_STARTED_MS}"),
        "download_started_unix_ms": int("${DOWNLOAD_STARTED_MS}"),
        "download_finished_unix_ms": int("${DOWNLOAD_FINISHED_MS}"),
        "download_duration_ms": int("${DOWNLOAD_FINISHED_MS}") - int("${DOWNLOAD_STARTED_MS}"),
        "upload_started_unix_ms": int("${UPLOAD_STARTED_MS}"),
        "upload_finished_unix_ms": int("${UPLOAD_FINISHED_MS}"),
        "upload_duration_ms": int("${UPLOAD_FINISHED_MS}") - int("${UPLOAD_STARTED_MS}"),
        "remote_install_started_unix_ms": int("${REMOTE_INSTALL_STARTED_MS}"),
        "remote_install_finished_unix_ms": int("${REMOTE_INSTALL_FINISHED_MS}"),
        "remote_install_duration_ms": int("${REMOTE_INSTALL_FINISHED_MS}") - int("${REMOTE_INSTALL_STARTED_MS}"),
    },
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
PY
jq empty "$LOCAL_RECEIPT_PATH" >/dev/null

log "Binary release uploaded"
log "release=${NEXUS_RELEASE_GIT_SHA}"
log "receipt=${LOCAL_RECEIPT_PATH}"
log "result=${UPLOAD_RESULT//$'\n'/}"
