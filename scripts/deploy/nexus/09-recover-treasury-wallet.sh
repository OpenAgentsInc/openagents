#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:latest}"
NEXUS_TREASURY_RECOVERY_ACTION="${NEXUS_TREASURY_RECOVERY_ACTION:-}"
NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS="${NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS:-120000}"
NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS="${NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS:-false}"
NEXUS_TREASURY_RECOVERY_RUST_LOG="${NEXUS_TREASURY_RECOVERY_RUST_LOG:-warn}"
NEXUS_TREASURY_RECOVERY_REPORT_ATTEMPTS="${NEXUS_TREASURY_RECOVERY_REPORT_ATTEMPTS:-3}"

if [[ -z "${NEXUS_TREASURY_RECOVERY_ACTION}" ]]; then
  if [[ -n "${NEXUS_TREASURY_RECOVERY_REPORT_PATH:-}" ]]; then
    NEXUS_TREASURY_RECOVERY_ACTION="cutover"
  else
    NEXUS_TREASURY_RECOVERY_ACTION="report"
  fi
fi

case "${NEXUS_TREASURY_RECOVERY_ACTION}" in
  report|cutover|report-and-cutover) ;;
  *)
    die "Unsupported NEXUS_TREASURY_RECOVERY_ACTION=${NEXUS_TREASURY_RECOVERY_ACTION}; expected report, cutover, or report-and-cutover"
    ;;
esac

if [[ "${NEXUS_TREASURY_RECOVERY_ACTION}" == "cutover" && -z "${NEXUS_TREASURY_RECOVERY_REPORT_PATH:-}" ]]; then
  die "Set NEXUS_TREASURY_RECOVERY_REPORT_PATH to the validated recovery report path on the VM data disk"
fi

if [[ "${NEXUS_TREASURY_RECOVERY_ACTION}" != "cutover" ]]; then
  STAMP="$(date -u +%Y%m%d-%H%M%S)"
  NEXUS_TREASURY_RECOVERY_WORK_DIR="${NEXUS_TREASURY_RECOVERY_WORK_DIR:-${NEXUS_DATA_DIR}/treasury/treasury-wallet-recovery-${STAMP}}"
  NEXUS_TREASURY_RECOVERY_REPORT_PATH="${NEXUS_TREASURY_RECOVERY_REPORT_PATH:-${NEXUS_TREASURY_RECOVERY_WORK_DIR}/recovery-report.json}"
fi

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "set -euo pipefail; \
    exec 9>/tmp/openagents-nexus-treasury-wallet-recovery.lock; \
    flock -n 9 || { echo 'another Nexus treasury wallet recovery action is already running on this VM' >&2; exit 75; }; \
    cleanup_relay_service() { \
      sudo systemctl unmask nexus-relay >/dev/null 2>&1 || true; \
      sudo systemctl start nexus-relay >/dev/null 2>&1 || true; \
    }; \
    trap 'cleanup_relay_service' EXIT; \
    sudo systemctl mask --runtime nexus-relay >/dev/null; \
    sudo systemctl stop nexus-relay; \
    sudo docker rm -f nexus-relay >/dev/null 2>&1 || true; \
    RELAY_STOPPED=0; \
    for attempt in 1 2 3 4 5 6 7 8 9 10; do \
      if ! systemctl is-active nexus-relay >/dev/null; then \
        RELAY_STOPPED=1; \
        break; \
      fi; \
      sleep 1; \
    done; \
    [[ \"\$RELAY_STOPPED\" == \"1\" ]] || { echo 'nexus-relay did not stop before treasury recovery inspection' >&2; exit 1; }; \
    ACCESS_TOKEN=\$(curl -fsS -H 'Metadata-Flavor: Google' 'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token'); \
    AR_HOST=\$(echo '$DEPLOY_IMAGE' | cut -d'/' -f1); \
    echo \"\$ACCESS_TOKEN\" | sudo docker login -u oauth2accesstoken --password-stdin \"https://\${AR_HOST}\" >/dev/null; \
    sudo docker pull '$DEPLOY_IMAGE' >/dev/null; \
    run_nexus_control() { \
      sudo docker run --rm \
        --entrypoint /usr/local/bin/nexus-control \
        --network host \
        --env-file /etc/nexus-relay/nexus-relay.env \
        --env NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS='${NEXUS_TREASURY_RECOVERY_INSPECTION_TIMEOUT_MS}' \
        --env NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS='${NEXUS_TREASURY_RECOVERY_PARALLEL_INSPECTIONS}' \
        --env RUST_LOG='${NEXUS_TREASURY_RECOVERY_RUST_LOG}' \
        -v '${NEXUS_DATA_DIR}:${NEXUS_DATA_DIR}' \
        '$DEPLOY_IMAGE' \
        \"\$@\"; \
    }; \
    run_recovery_report() { \
      local attempt=1; \
      local max_attempts='${NEXUS_TREASURY_RECOVERY_REPORT_ATTEMPTS}'; \
      local status=0; \
      while true; do \
        if REPORT_JSON=\$(run_nexus_control treasury recovery-report --work-dir '${NEXUS_TREASURY_RECOVERY_WORK_DIR:-}' --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json); then \
          printf '%s\n' \"\$REPORT_JSON\"; \
          return 0; \
        fi; \
        status=\$?; \
        if (( attempt >= max_attempts )); then \
          return \"\$status\"; \
        fi; \
        sudo rm -rf '${NEXUS_TREASURY_RECOVERY_WORK_DIR:-}'; \
        sleep \$((attempt * 15)); \
        attempt=\$((attempt + 1)); \
      done; \
    }; \
    if [[ '${NEXUS_TREASURY_RECOVERY_ACTION}' == 'report' || '${NEXUS_TREASURY_RECOVERY_ACTION}' == 'report-and-cutover' ]]; then \
      REPORT_JSON=\$(run_recovery_report); \
      printf '%s\n' \"\$REPORT_JSON\"; \
      if [[ '${NEXUS_TREASURY_RECOVERY_ACTION}' == 'report-and-cutover' ]]; then \
        jq -e '.comparison.validation_passed == true and .comparison.recommended_action == \"cutover_rebuilt_storage_after_service_stop\"' <<<\"\$REPORT_JSON\" >/dev/null; \
        run_nexus_control treasury recovery-cutover --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json; \
      fi; \
    else \
      run_nexus_control treasury recovery-cutover --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json; \
    fi; \
    sudo systemctl unmask nexus-relay >/dev/null; \
    sudo systemctl start nexus-relay; \
    trap - EXIT; \
    systemctl is-active nexus-relay >/dev/null; \
    READY=0; \
    for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do \
      if curl -fsS --max-time 5 http://127.0.0.1:8080/v1/treasury/status >/dev/null; then \
        READY=1; \
        break; \
      fi; \
      sleep 5; \
    done; \
    [[ \"\$READY\" == \"1\" ]]"

log "Treasury wallet recovery action ${NEXUS_TREASURY_RECOVERY_ACTION} completed on ${NEXUS_VM}; report=${NEXUS_TREASURY_RECOVERY_REPORT_PATH}"
