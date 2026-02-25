#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  canary-rollout.sh status
  canary-rollout.sh deploy-no-traffic <image>
  canary-rollout.sh set-traffic <stable-revision> <canary-revision> <canary-percent>
  canary-rollout.sh rollback <stable-revision>

Environment:
  PROJECT     GCP project id (defaults to active gcloud project)
  REGION      Cloud Run region (default: us-central1)
  SERVICE     Cloud Run service (default: openagents-control-service)
  DRY_RUN     1 to print commands without executing
EOF
}

run_cmd() {
  echo "+ $*"
  if [[ "${DRY_RUN:-0}" == "1" ]]; then
    return 0
  fi
  "$@"
}

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-control-service}"

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
      --format "yaml(status.traffic,status.latestCreatedRevisionName,status.latestReadyRevisionName)"
    ;;
  deploy-no-traffic)
    if [[ $# -ne 1 ]]; then
      echo "error: deploy-no-traffic requires <image>" >&2
      exit 2
    fi
    IMAGE="$1"
    run_cmd gcloud run deploy "${SERVICE}" \
      --project "${PROJECT}" \
      --region "${REGION}" \
      --image "${IMAGE}" \
      --no-traffic
    ;;
  set-traffic)
    if [[ $# -ne 3 ]]; then
      echo "error: set-traffic requires <stable-revision> <canary-revision> <canary-percent>" >&2
      exit 2
    fi
    STABLE_REV="$1"
    CANARY_REV="$2"
    CANARY_PERCENT="$3"
    if ! [[ "${CANARY_PERCENT}" =~ ^[0-9]+$ ]] || (( CANARY_PERCENT < 0 || CANARY_PERCENT > 100 )); then
      echo "error: canary percent must be 0..100" >&2
      exit 2
    fi
    STABLE_PERCENT=$((100 - CANARY_PERCENT))
    run_cmd gcloud run services update-traffic "${SERVICE}" \
      --project "${PROJECT}" \
      --region "${REGION}" \
      --to-revisions "${STABLE_REV}=${STABLE_PERCENT},${CANARY_REV}=${CANARY_PERCENT}"
    ;;
  rollback)
    if [[ $# -ne 1 ]]; then
      echo "error: rollback requires <stable-revision>" >&2
      exit 2
    fi
    STABLE_REV="$1"
    run_cmd gcloud run services update-traffic "${SERVICE}" \
      --project "${PROJECT}" \
      --region "${REGION}" \
      --to-revisions "${STABLE_REV}=100"
    ;;
  *)
    usage
    exit 2
    ;;
esac
