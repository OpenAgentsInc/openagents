#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/cloud/provision-managed-sandbox-runtime.sh \
    --project PROJECT --region REGION --zone ZONE \
    --environment staging|production \
    --provider-broker-url HTTPS_URL \
    --guest-image-name IMMUTABLE_NAME --image-tag SOURCE_TAG [--apply]

Creates the dedicated secrets, static internal control address, immutable guest
image, control image/node, least-privilege firewall rules, and Cloud Run bridge
for SBX-09. It does not enable Worker product flags; the live acceptance and
rollback gate own that separate step. Default mode is dry-run.
USAGE
}

project=""
region="us-central1"
zone="us-central1-a"
environment=""
provider_broker_url=""
guest_image_name=""
image_tag=""
apply="false"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project="${2:-}"; shift 2 ;;
    --region) region="${2:-}"; shift 2 ;;
    --zone) zone="${2:-}"; shift 2 ;;
    --environment) environment="${2:-}"; shift 2 ;;
    --provider-broker-url) provider_broker_url="${2:-}"; shift 2 ;;
    --guest-image-name) guest_image_name="${2:-}"; shift 2 ;;
    --image-tag) image_tag="${2:-}"; shift 2 ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done
if [[ -z "$project" || -z "$region" || -z "$zone" || \
      -z "$environment" || -z "$provider_broker_url" || \
      -z "$guest_image_name" || -z "$image_tag" ]]; then
  usage >&2
  exit 2
fi
if [[ "$environment" != "staging" && "$environment" != "production" ]]; then
  echo "environment must be staging or production" >&2
  exit 2
fi
if [[ ! "$provider_broker_url" =~ ^https:// ]]; then
  echo "provider-broker-url must be HTTPS" >&2
  exit 2
fi

suffix=""
if [[ "$environment" == "staging" ]]; then suffix="-staging"; fi
control_instance="oa-managed-sandbox-control${suffix}-1"
control_address="oa-managed-sandbox-control${suffix}-ip"
control_tag="oa-managed-sandbox-control${suffix}"
control_firewall="oa-managed-sandbox-control${suffix}-port"
guest_broker_firewall="oa-managed-sandbox-guest-to-broker${suffix}"
guest_broker_deny_firewall="oa-managed-sandbox-broker-deny${suffix}"
bridge_service="oa-managed-sandbox-bridge${suffix}"
bridge_firewall="oa-managed-sandbox-bridge-to-control${suffix}"
bridge_tag="oa-managed-sandbox-bridge${suffix}"
control_token_secret="oa-managed-sandbox-control-token${suffix}"
broker_signing_secret="oa-managed-sandbox-broker-signing-key${suffix}"
checkpoint_bucket="${project}-managed-sandbox-checkpoints${suffix}"
control_sa="oa-codex-control@${project}.iam.gserviceaccount.com"
project_number="$(gcloud projects describe "$project" --format='value(projectNumber)')"
runtime_sa="${project_number}-compute@developer.gserviceaccount.com"
revision="$(git rev-parse HEAD)"

run() {
  if [[ "$apply" == "true" ]]; then
    "$@"
  else
    printf '+'; printf ' %q' "$@"; printf '\n'
  fi
}

ensure_secret() {
  local name="$1"
  if gcloud secrets describe "$name" --project "$project" >/dev/null 2>&1; then
    return
  fi
  if [[ "$apply" == "true" ]]; then
    openssl rand -hex 32 | gcloud secrets create "$name" \
      --project "$project" --replication-policy automatic --data-file=- \
      --format='value(name)'
  else
    echo "+ openssl rand -hex 32 | gcloud secrets create $name --data-file=-"
  fi
}

ensure_secret "$control_token_secret"
ensure_secret "$broker_signing_secret"
run gcloud secrets add-iam-policy-binding "$control_token_secret" \
  --project "$project" \
  --member "serviceAccount:${control_sa}" \
  --role roles/secretmanager.secretAccessor \
  --condition=None \
  --format='value(name)'
run gcloud secrets add-iam-policy-binding "$control_token_secret" \
  --project "$project" \
  --member "serviceAccount:${runtime_sa}" \
  --role roles/secretmanager.secretAccessor \
  --condition=None \
  --format='value(name)'
run gcloud secrets add-iam-policy-binding "$broker_signing_secret" \
  --project "$project" \
  --member "serviceAccount:${runtime_sa}" \
  --role roles/secretmanager.secretAccessor \
  --condition=None \
  --format='value(name)'

if ! gcloud storage buckets describe "gs://${checkpoint_bucket}" \
  --project "$project" >/dev/null 2>&1; then
  run gcloud storage buckets create "gs://${checkpoint_bucket}" \
    --project "$project" \
    --location "$region" \
    --uniform-bucket-level-access \
    --soft-delete-duration 0
fi
run gcloud storage buckets add-iam-policy-binding "gs://${checkpoint_bucket}" \
  --member "serviceAccount:${control_sa}" \
  --role roles/storage.objectAdmin

if ! gcloud compute addresses describe "$control_address" \
  --project "$project" --region "$region" >/dev/null 2>&1; then
  run gcloud compute addresses create "$control_address" \
    --project "$project" --region "$region" --subnet default
fi
if [[ "$apply" == "true" ]]; then
  control_ip="$(gcloud compute addresses describe "$control_address" \
    --project "$project" --region "$region" --format='value(address)')"
else
  control_ip="10.128.0.254"
fi

guest_args=(
  --project "$project"
  --zone "$zone"
  --image-name "$guest_image_name"
)
if [[ "$apply" == "true" ]]; then guest_args+=(--apply); fi
scripts/cloud/build-managed-sandbox-guest-image.sh "${guest_args[@]}"

if [[ "$apply" == "true" ]]; then
  image_id="$(gcloud compute images describe "$guest_image_name" \
    --project "$project" --format='value(id)')"
else
  image_id="dry-run-image-id"
fi
image_digest="sha256:$(printf '%s' "${project}|${guest_image_name}|${image_id}" | \
  shasum -a 256 | awk '{print $1}')"
profile_digest="sha256:$(printf '%s' \
  "profile.sbx.gce.e2-small.v1|${image_digest}|${region}|e2-small|gce_vm|broker-only-v1|${control_ip}|900|1000|8790|metadata-v1|169.254.169.254|80|900000|2|40000" | \
  shasum -a 256 | awk '{print $1}')"

control_image="${region}-docker.pkg.dev/${project}/oa-cloud/oa-codex-control:${image_tag}"
if [[ "$apply" == "true" ]]; then
  control_build_id="$(gcloud builds submit . \
    --project "$project" --region "$region" --async --format='value(id)' \
    --config docker/cloud/cloudbuild-oa-codex-control.yaml \
    --substitutions "_IMAGE=${control_image},_REVISION=${revision}")"
  for _ in $(seq 1 240); do
    control_build_status="$(gcloud builds describe "$control_build_id" \
      --project "$project" --region "$region" --format='value(status)')"
    case "$control_build_status" in
      SUCCESS) break ;;
      FAILURE|INTERNAL_ERROR|TIMEOUT|CANCELLED|EXPIRED)
        echo "control image build failed: $control_build_id ($control_build_status)" >&2
        exit 1
        ;;
    esac
    sleep 5
  done
  if [[ "${control_build_status:-}" != "SUCCESS" ]]; then
    echo "control image build did not reach SUCCESS: $control_build_id" >&2
    exit 1
  fi
else
  run gcloud builds submit . \
    --project "$project" --region "$region" --async --format='value(id)' \
    --config docker/cloud/cloudbuild-oa-codex-control.yaml \
    --substitutions "_IMAGE=${control_image},_REVISION=${revision}"
fi

control_args=(
  --project "$project"
  --zone "$zone"
  --instance "$control_instance"
  --firewall-rule "$control_firewall"
  --network-tag "$control_tag"
  --image-tag "$image_tag"
  --control-token-secret "$control_token_secret"
  --enable-managed-sandbox
  --managed-sandbox-image-project "$project"
  --managed-sandbox-image-name "$guest_image_name"
  --managed-sandbox-image-id "$image_id"
  --managed-sandbox-image-digest "$image_digest"
  --managed-sandbox-profile-digest "$profile_digest"
  --managed-sandbox-control-internal-ip "$control_ip"
  --managed-sandbox-provider-broker-url "$provider_broker_url"
  --managed-sandbox-provider-broker-port 8790
  --managed-sandbox-turn-driver /usr/local/bin/managed-sandbox-turn-driver.mjs
  --managed-sandbox-io-driver /usr/local/bin/managed-sandbox-io-driver.mjs
  --managed-sandbox-phase2-driver /usr/local/bin/managed-sandbox-phase2-driver.mjs
  --managed-sandbox-phase2-bucket "$checkpoint_bucket"
)
if [[ "$apply" == "true" ]]; then control_args+=(--apply); fi
bash scripts/cloud/gcp-codex-control-deploy.sh "${control_args[@]}"

if [[ "$apply" == "true" ]] && gcloud compute firewall-rules describe "$guest_broker_firewall" \
  --project "$project" >/dev/null 2>&1; then
  run gcloud compute firewall-rules update "$guest_broker_firewall" \
    --project "$project" --rules tcp:8790 --priority 900 \
    --source-tags oa-managed-sandbox-guest --target-tags "$control_tag"
else
  run gcloud compute firewall-rules create "$guest_broker_firewall" \
    --project "$project" --direction INGRESS --action ALLOW --rules tcp:8790 \
    --priority 900 \
    --source-tags oa-managed-sandbox-guest --target-tags "$control_tag"
fi
if [[ "$apply" == "true" ]] && gcloud compute firewall-rules describe "$guest_broker_deny_firewall" \
  --project "$project" >/dev/null 2>&1; then
  run gcloud compute firewall-rules update "$guest_broker_deny_firewall" \
    --project "$project" --rules tcp:8790 --priority 1000 \
    --source-ranges 0.0.0.0/0 --target-tags "$control_tag"
else
  run gcloud compute firewall-rules create "$guest_broker_deny_firewall" \
    --project "$project" --direction INGRESS --action DENY --rules tcp:8790 \
    --priority 1000 --source-ranges 0.0.0.0/0 --target-tags "$control_tag"
fi

bridge_args=(
  --project "$project"
  --region "$region"
  --control-ip "$control_ip"
  --control-token-secret "$control_token_secret"
  --image-tag "$image_tag"
  --control-tag "$control_tag"
  --service "$bridge_service"
  --firewall "$bridge_firewall"
  --bridge-tag "$bridge_tag"
  --service-account "$runtime_sa"
)
if [[ "$apply" == "true" ]]; then bridge_args+=(--apply); fi
scripts/cloud/deploy-managed-sandbox-bridge.sh "${bridge_args[@]}"

if [[ "$apply" == "true" ]]; then
  bridge_url="$(gcloud run services describe "$bridge_service" \
    --project "$project" --region "$region" --format='value(status.url)')"
else
  bridge_url="https://oa-managed-sandbox-bridge.example.invalid"
fi
cat <<SUMMARY
Managed-sandbox runtime binding
  environment: $environment
  sourceRevision: $revision
  controlInstance: $control_instance
  controlIp: $control_ip
  bridgeUrl: $bridge_url
  guestImageName: $guest_image_name
  guestImageId: $image_id
  guestImageDigest: $image_digest
  profileDigest: $profile_digest
  checkpointBucket: $checkpoint_bucket
  networkPolicyRef: network-policy-ref://openagents/managed-sandbox/broker-only-v1
  Worker enablement remains separate and default-off.
SUMMARY
