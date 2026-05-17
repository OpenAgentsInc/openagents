#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

NEXUS_PYLON_HOSTS="${NEXUS_PYLON_HOSTS:-pylon-gcp-1 pylon-gcp-2 pylon-gcp-3 pylon-gcp-4 pylon-gcp-5 pylon-gcp-6 pylon-gcp-7}"
NEXUS_PYLON_RUNTIME_ARCHIVE="${NEXUS_PYLON_RUNTIME_ARCHIVE:-}"
NEXUS_PYLON_RUNTIME_DIR="${NEXUS_PYLON_RUNTIME_DIR:-/var/lib/pylon/psionic}"
NEXUS_PYLON_SERVICE="${NEXUS_PYLON_SERVICE:-pylon.service}"
NEXUS_PYLON_RUNTIME_INSTALL_DRY_RUN="${NEXUS_PYLON_RUNTIME_INSTALL_DRY_RUN:-false}"

[[ -n "$NEXUS_PYLON_RUNTIME_ARCHIVE" ]] || die "Set NEXUS_PYLON_RUNTIME_ARCHIVE to a local psionic-runtime tar.gz"
[[ -f "$NEXUS_PYLON_RUNTIME_ARCHIVE" ]] || die "Runtime archive does not exist: ${NEXUS_PYLON_RUNTIME_ARCHIVE}"

remote_archive="/tmp/openagents-psionic-runtime.tar.gz"
hosts=()
read -r -a hosts <<<"$NEXUS_PYLON_HOSTS"
[[ "${#hosts[@]}" -gt 0 ]] || die "NEXUS_PYLON_HOSTS resolved to an empty host list"

log "Installing Psionic runtime archive on Pylon hosts: ${NEXUS_PYLON_HOSTS}"
log "Runtime target: ${NEXUS_PYLON_RUNTIME_DIR}"

for host in "${hosts[@]}"; do
  instance_exists "$host" || die "Pylon VM does not exist: ${host}"

  if [[ "$NEXUS_PYLON_RUNTIME_INSTALL_DRY_RUN" == "true" ]]; then
    log "Dry run: would copy ${NEXUS_PYLON_RUNTIME_ARCHIVE} to ${host}:${remote_archive}"
    log "Dry run: would install runtime into ${NEXUS_PYLON_RUNTIME_DIR} and restart ${NEXUS_PYLON_SERVICE}"
    continue
  fi

  log "Copying runtime archive to ${host}"
  gcloud compute scp "$NEXUS_PYLON_RUNTIME_ARCHIVE" "${host}:${remote_archive}" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" >/dev/null

  log "Installing runtime on ${host}"
  gcloud compute ssh "$host" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo env \
      NEXUS_PYLON_RUNTIME_DIR='${NEXUS_PYLON_RUNTIME_DIR}' \
      NEXUS_PYLON_SERVICE='${NEXUS_PYLON_SERVICE}' \
      REMOTE_ARCHIVE='${remote_archive}' \
      bash -s" <<'REMOTE'
set -euo pipefail

case "$NEXUS_PYLON_RUNTIME_DIR" in
  /var/lib/pylon/psionic|/opt/openagents/psionic)
    ;;
  *)
    echo "Refusing unexpected runtime directory: ${NEXUS_PYLON_RUNTIME_DIR}" >&2
    exit 1
    ;;
esac

test -s "$REMOTE_ARCHIVE"
rm -rf "$NEXUS_PYLON_RUNTIME_DIR"
mkdir -p "$(dirname "$NEXUS_PYLON_RUNTIME_DIR")"
tar -C "$(dirname "$NEXUS_PYLON_RUNTIME_DIR")" -xzf "$REMOTE_ARCHIVE"
test -x "$NEXUS_PYLON_RUNTIME_DIR/TRAIN"
test -x "$NEXUS_PYLON_RUNTIME_DIR/target/release/psionic-train"
chown -R pylon:pylon "$NEXUS_PYLON_RUNTIME_DIR"

mkdir -p "/etc/systemd/system/${NEXUS_PYLON_SERVICE}.d"
cat >"/etc/systemd/system/${NEXUS_PYLON_SERVICE}.d/10-psionic-runtime.conf" <<ENV
[Service]
Environment=OPENAGENTS_PSIONIC_REPO=${NEXUS_PYLON_RUNTIME_DIR}
ENV

systemctl daemon-reload
systemctl restart "$NEXUS_PYLON_SERVICE"
systemctl is-active "$NEXUS_PYLON_SERVICE" >/dev/null
REMOTE
done

log "Pylon Psionic runtime install complete."
log "Verify with: sudo -u pylon /usr/local/bin/pylon training status --json"
