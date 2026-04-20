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
    sudo systemctl stop nexus-relay; \
    trap 'sudo systemctl start nexus-relay >/dev/null 2>&1 || true' EXIT; \
    ACCESS_TOKEN=\$(curl -fsS -H 'Metadata-Flavor: Google' 'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token'); \
    AR_HOST=\$(echo '$DEPLOY_IMAGE' | cut -d'/' -f1); \
    echo \"\$ACCESS_TOKEN\" | sudo docker login -u oauth2accesstoken --password-stdin \"https://\${AR_HOST}\" >/dev/null; \
    sudo docker pull '$DEPLOY_IMAGE' >/dev/null; \
    run_nexus_control() { \
      sudo docker run --rm \
        --entrypoint /usr/local/bin/nexus-control \
        --network host \
        --env-file /etc/nexus-relay/nexus-relay.env \
        -v '${NEXUS_DATA_DIR}:${NEXUS_DATA_DIR}' \
        '$DEPLOY_IMAGE' \
        \"\$@\"; \
    }; \
    if [[ '${NEXUS_TREASURY_RECOVERY_ACTION}' == 'report' || '${NEXUS_TREASURY_RECOVERY_ACTION}' == 'report-and-cutover' ]]; then \
      REPORT_JSON=\$(run_nexus_control treasury recovery-report --work-dir '${NEXUS_TREASURY_RECOVERY_WORK_DIR:-}' --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json); \
      printf '%s\n' \"\$REPORT_JSON\"; \
      if [[ '${NEXUS_TREASURY_RECOVERY_ACTION}' == 'report-and-cutover' ]]; then \
        jq -e '.comparison.validation_passed == true and .comparison.recommended_action == \"cutover_rebuilt_storage_after_service_stop\"' <<<\"\$REPORT_JSON\" >/dev/null; \
        run_nexus_control treasury recovery-cutover --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json; \
      fi; \
    else \
      run_nexus_control treasury recovery-cutover --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json; \
    fi; \
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
