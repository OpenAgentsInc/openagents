#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/cloud/deploy-managed-sandbox-bridge.sh \
    --project PROJECT --region REGION --control-ip INTERNAL_IP \
    --control-token-secret SECRET --image-tag TAG [--apply]

Deploys the dedicated bearer-gated Cloud Run edge for only
/v1/managed-sandbox/runtime/* and a matching Direct-VPC-to-control firewall.
Default mode is dry-run.
USAGE
}

project=""
region="us-central1"
control_ip=""
control_token_secret=""
image_tag=""
service="oa-managed-sandbox-bridge"
network="default"
subnet="default"
bridge_tag="oa-managed-sandbox-bridge"
control_tag="oa-managed-sandbox-control"
firewall="oa-managed-sandbox-bridge-to-control"
service_account=""
apply="false"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project="${2:-}"; shift 2 ;;
    --region) region="${2:-}"; shift 2 ;;
    --control-ip) control_ip="${2:-}"; shift 2 ;;
    --control-token-secret) control_token_secret="${2:-}"; shift 2 ;;
    --image-tag) image_tag="${2:-}"; shift 2 ;;
    --service) service="${2:-}"; shift 2 ;;
    --firewall) firewall="${2:-}"; shift 2 ;;
    --network) network="${2:-}"; shift 2 ;;
    --subnet) subnet="${2:-}"; shift 2 ;;
    --bridge-tag) bridge_tag="${2:-}"; shift 2 ;;
    --control-tag) control_tag="${2:-}"; shift 2 ;;
    --service-account) service_account="${2:-}"; shift 2 ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done
if [[ -z "$project" || -z "$region" || -z "$control_ip" || \
      -z "$control_token_secret" || -z "$image_tag" ]]; then
  usage >&2
  exit 2
fi
if [[ ! "$control_ip" =~ ^10\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
  echo "control-ip must be an internal RFC1918 address" >&2
  exit 2
fi
if [[ -z "$service_account" ]]; then
  project_number="$(gcloud projects describe "$project" --format='value(projectNumber)')"
  service_account="${project_number}-compute@developer.gserviceaccount.com"
fi
image="${region}-docker.pkg.dev/${project}/oa-cloud/oa-cloud-run-bridge:${image_tag}"

run() {
  if [[ "$apply" == "true" ]]; then
    "$@"
  else
    printf '+'; printf ' %q' "$@"; printf '\n'
  fi
}

revision="$(git rev-parse HEAD)"
if [[ "$apply" == "true" ]]; then
  build_id="$({
    gcloud builds submit . \
      --project "$project" \
      --region "$region" \
      --async \
      --format='value(id)' \
      --config docker/cloud/cloudbuild-oa-cloud-run-bridge.yaml \
      --substitutions "_IMAGE=${image},_REVISION=${revision}"
  })"
  for _ in $(seq 1 240); do
    build_status="$(gcloud builds describe "$build_id" \
      --project "$project" --region "$region" --format='value(status)')"
    case "$build_status" in
      SUCCESS) break ;;
      FAILURE|INTERNAL_ERROR|TIMEOUT|CANCELLED|EXPIRED)
        echo "managed-sandbox bridge build failed: $build_id ($build_status)" >&2
        exit 1
        ;;
    esac
    sleep 5
  done
  if [[ "${build_status:-}" != "SUCCESS" ]]; then
    echo "managed-sandbox bridge build did not reach SUCCESS: $build_id" >&2
    exit 1
  fi
else
  run gcloud builds submit . \
    --project "$project" --region "$region" --async --format='value(id)' \
    --config docker/cloud/cloudbuild-oa-cloud-run-bridge.yaml \
    --substitutions "_IMAGE=${image},_REVISION=${revision}"
fi

if [[ "$apply" == "true" ]] && gcloud compute firewall-rules describe "$firewall" \
  --project "$project" >/dev/null 2>&1; then
  run gcloud compute firewall-rules update "$firewall" \
    --project "$project" \
    --rules tcp:8787 \
    --priority 900 \
    --source-ranges="" \
    --source-tags "$bridge_tag" \
    --target-tags "$control_tag"
else
  run gcloud compute firewall-rules create "$firewall" \
    --project "$project" \
    --direction INGRESS \
    --action ALLOW \
    --rules tcp:8787 \
    --priority 900 \
    --source-tags "$bridge_tag" \
    --target-tags "$control_tag"
fi

run gcloud run deploy "$service" \
  --project "$project" \
  --region "$region" \
  --platform managed \
  --image "$image" \
  --service-account "$service_account" \
  --allow-unauthenticated \
  --clear-vpc-connector \
  --network "$network" \
  --subnet "$subnet" \
  --network-tags "$bridge_tag" \
  --vpc-egress private-ranges-only \
  --set-env-vars "OA_BRIDGE_CONTROL_URL=http://${control_ip}:8787,OA_BRIDGE_ALLOWED_PATH_PREFIXES=/v1/managed-sandbox/runtime,OA_BRIDGE_UPSTREAM_TIMEOUT_SECS=300" \
  --set-secrets "OA_BRIDGE_CONTROL_TOKEN=${control_token_secret}:latest" \
  --min 0 \
  --max 2 \
  --cpu 1 \
  --memory 256Mi \
  --timeout 300

cat <<SUMMARY
Managed-sandbox bridge
  service:      $service
  region:       $region
  image:        $image
  control:      $control_ip:8787
  allowed path: /v1/managed-sandbox/runtime/*
  firewall:     $firewall ($bridge_tag -> $control_tag tcp:8787)
  auth:         application bearer from Secret Manager; Cloud Run IAM is edge-open
  mode apply:   $apply
SUMMARY
