#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  maintenance-mode.sh status
  maintenance-mode.sh enable
  maintenance-mode.sh disable

Environment:
  PROJECT                              GCP project id (defaults to active gcloud project)
  REGION                               Cloud Run region (default: us-central1)
  SERVICE                              Cloud Run service (default: openagents-control-service)
  DRY_RUN                              1 to print commands without executing
  MAINTENANCE_ALLOWED_PATHS            CSV path/prefix allowlist (default: /healthz,/readyz)
  MAINTENANCE_BYPASS_COOKIE_NAME       Bypass cookie name (default: oa_maintenance_bypass)
  MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS Bypass cookie max-age (default: 900)
  MAINTENANCE_BYPASS_SECRET_NAME       Secret Manager secret for bypass token (default: openagents-control-maintenance-bypass-token)
  MAINTENANCE_BYPASS_TOKEN             Optional token value to rotate into Secret Manager during enable
EOF
}

run_cmd() {
  echo "+ $*"
  if [[ "${DRY_RUN:-0}" == "1" ]]; then
    return 0
  fi
  "$@"
}

ensure_secret_exists() {
  local project="$1"
  local secret_name="$2"
  if gcloud secrets describe "${secret_name}" --project "${project}" >/dev/null 2>&1; then
    return 0
  fi

  run_cmd gcloud secrets create "${secret_name}" \
    --project "${project}" \
    --replication-policy="automatic"
}

add_secret_version() {
  local project="$1"
  local secret_name="$2"
  local token="$3"
  if [[ -z "${token}" ]]; then
    return 0
  fi
  if [[ "${DRY_RUN:-0}" == "1" ]]; then
    echo "+ gcloud secrets versions add ${secret_name} --project ${project} --data-file=-"
    return 0
  fi
  printf '%s' "${token}" | gcloud secrets versions add "${secret_name}" \
    --project "${project}" \
    --data-file=- >/dev/null
}

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-control-service}"
MAINTENANCE_ALLOWED_PATHS="${MAINTENANCE_ALLOWED_PATHS:-/healthz,/readyz}"
MAINTENANCE_BYPASS_COOKIE_NAME="${MAINTENANCE_BYPASS_COOKIE_NAME:-oa_maintenance_bypass}"
MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS="${MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS:-900}"
MAINTENANCE_BYPASS_SECRET_NAME="${MAINTENANCE_BYPASS_SECRET_NAME:-openagents-control-maintenance-bypass-token}"
MAINTENANCE_BYPASS_TOKEN="${MAINTENANCE_BYPASS_TOKEN:-}"

if [[ $# -lt 1 ]]; then
  usage
  exit 2
fi

if [[ -z "${PROJECT}" || "${PROJECT}" == "(unset)" ]]; then
  echo "error: PROJECT is required and no active gcloud project is configured" >&2
  exit 1
fi

ACTION="$1"
shift

case "${ACTION}" in
  status)
    run_cmd gcloud run services describe "${SERVICE}" \
      --project "${PROJECT}" \
      --region "${REGION}" \
      --format "yaml(status.latestReadyRevisionName,status.traffic,spec.template.spec.containers[0].env)"
    ;;
  enable)
    env_updates="^:^OA_MAINTENANCE_MODE_ENABLED=true:OA_MAINTENANCE_BYPASS_COOKIE_NAME=${MAINTENANCE_BYPASS_COOKIE_NAME}:OA_MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS=${MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS}:OA_MAINTENANCE_ALLOWED_PATHS=${MAINTENANCE_ALLOWED_PATHS}"

    ensure_secret_exists "${PROJECT}" "${MAINTENANCE_BYPASS_SECRET_NAME}"
    add_secret_version "${PROJECT}" "${MAINTENANCE_BYPASS_SECRET_NAME}" "${MAINTENANCE_BYPASS_TOKEN}"

    run_cmd gcloud run services update "${SERVICE}" \
      --project "${PROJECT}" \
      --region "${REGION}" \
      --update-env-vars "${env_updates}" \
      --update-secrets "OA_MAINTENANCE_BYPASS_TOKEN=${MAINTENANCE_BYPASS_SECRET_NAME}:latest"
    ;;
  disable)
    run_cmd gcloud run services update "${SERVICE}" \
      --project "${PROJECT}" \
      --region "${REGION}" \
      --update-env-vars "OA_MAINTENANCE_MODE_ENABLED=false"
    ;;
  *)
    usage
    exit 2
    ;;
esac
