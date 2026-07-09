#!/usr/bin/env bash
set -euo pipefail

# Deploy (or redeploy) the always-on oa-codex-control node on GCE (cloud#95).
#
# The control node is intentionally PERSISTENT. It runs the oa-codex-control
# container under a container-optimized OS VM whose attached service account
# provides in-VM Application Default Credentials (no key files). The live GCE
# per-session provisioner then provisions/tears-down EPHEMERAL worker VMs
# (oa-codex-sess-*) for cloud-gcp runs.
#
# Worker VMs are torn down by the provisioner's release path. This script only
# manages the persistent control instance.
#
# Default mode is dry-run. Pass --apply to execute gcloud commands.

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-codex-control-deploy.sh \
    --project PROJECT_ID \
    --control-token TOKEN \
    [--zone us-central1-a] [--machine-type e2-small] \
    [--instance oa-codex-control-1] \
    [--image-tag cloud95] \
    [--control-source-cidr 1.2.3.4/32] \
    [--apply]

Required:
  --project          GCP project id (e.g. openagentsgemini)
  --control-token    OA_CODEX_CONTROL_TOKEN bearer for the HTTP control API

Notes:
  - The control port (8787) firewall is restricted to --control-source-cidr
    plus the IAP range. Never expose 8787 to 0.0.0.0/0.
  - The instance is labeled openagents-managed=control so it is distinguishable
    from ephemeral oa-codex-sess-* worker VMs.
USAGE
}

project_id=""
zone="us-central1-a"
machine_type="e2-small"
instance="oa-codex-control-1"
image_tag="cloud95"
control_token=""
control_source_cidr=""
apply="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project_id="${2:-}"; shift 2 ;;
    --zone) zone="${2:-}"; shift 2 ;;
    --machine-type) machine_type="${2:-}"; shift 2 ;;
    --instance) instance="${2:-}"; shift 2 ;;
    --image-tag) image_tag="${2:-}"; shift 2 ;;
    --control-token) control_token="${2:-}"; shift 2 ;;
    --control-source-cidr) control_source_cidr="${2:-}"; shift 2 ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$project_id" || -z "$control_token" ]]; then
  echo "--project and --control-token are required" >&2
  usage >&2
  exit 2
fi

region="${zone%-*}"
control_sa="oa-codex-control@${project_id}.iam.gserviceaccount.com"
image="${region}-docker.pkg.dev/${project_id}/oa-cloud/oa-codex-control:${image_tag}"
fw_control="oa-codex-control-port"
network_tag="oa-codex-control"

run() {
  if [[ "$apply" == "true" ]]; then
    "$@"
  else
    printf '+'; printf ' %q' "$@"; printf '\n'
  fi
}

# Container declaration runs the control image. The live GCE provisioner shells
# out to gcloud inside the container using GCE metadata ADC (the instance SA).
# OA_CODEX_GCE_USE_METADATA_ADC=true tells adc_available() to trust the
# metadata-server identity (no key files).
container_env="\
OA_CODEX_CONTROL_BIND=0.0.0.0:8787,\
OA_CODEX_CONTROL_TOKEN=${control_token},\
OA_CODEX_CONTROL_STATE_ROOT=/var/lib/openagents/codex-control,\
OA_CODEX_GCE_PROVISIONER=live,\
OA_CODEX_GCE_PROJECT_ID=${project_id},\
OA_CODEX_GCE_ZONE=${zone},\
OA_CODEX_GCE_MACHINE_TYPE=e2-small,\
OA_CODEX_GCE_USE_METADATA_ADC=true"

# Use the Container-Optimized OS container declaration via metadata so the VM
# runs the image as a managed container with the instance SA's ADC.
container_decl="$(mktemp)"
trap 'rm -f "$container_decl"' EXIT
cat >"$container_decl" <<DECL
spec:
  containers:
    - name: oa-codex-control
      image: ${image}
      stdin: false
      tty: false
      env:
        - name: OA_CODEX_CONTROL_BIND
          value: "0.0.0.0:8787"
        - name: OA_CODEX_CONTROL_TOKEN
          value: "${control_token}"
        - name: OA_CODEX_CONTROL_STATE_ROOT
          value: "/var/lib/openagents/codex-control"
        # Required by Config even for provisioner-only operation. The account
        # subdir(s) are populated only when a real Codex run is enabled.
        - name: OA_CODEX_AUTH_JSON_ROOT
          value: "/var/lib/openagents/codex-accounts"
        # No grant resolver is configured on this node yet, so allow the
        # local-auth path (the daemon still requires a per-account auth.json to
        # actually run Codex; provisioner + control API work without one).
        - name: OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY
          value: "true"
        - name: OA_CODEX_GCE_PROVISIONER
          value: "live"
        - name: OA_CODEX_GCE_PROJECT_ID
          value: "${project_id}"
        - name: OA_CODEX_GCE_ZONE
          value: "${zone}"
        - name: OA_CODEX_GCE_MACHINE_TYPE
          value: "e2-small"
        - name: OA_CODEX_GCE_USE_METADATA_ADC
          value: "true"
  restartPolicy: Always
  # Host networking so the control daemon's :8787 listener is reachable on the
  # VM's network interface (the GCE firewall is the access control boundary).
  hostNetwork: true
DECL

# Control-port firewall: IAP range + optional owner CIDR only. Never 0.0.0.0/0.
source_ranges="35.235.240.0/20"
if [[ -n "$control_source_cidr" ]]; then
  source_ranges="${source_ranges},${control_source_cidr}"
fi

# Idempotent firewall: update sources if the rule already exists, else create.
if [[ "$apply" == "true" ]] && gcloud compute firewall-rules describe "$fw_control" \
     --project "$project_id" >/dev/null 2>&1; then
  run gcloud compute firewall-rules update "$fw_control" \
    --project "$project_id" \
    --rules tcp:8787 \
    --source-ranges "$source_ranges"
else
  run gcloud compute firewall-rules create "$fw_control" \
    --project "$project_id" \
    --direction INGRESS \
    --action ALLOW \
    --rules tcp:8787 \
    --target-tags "$network_tag" \
    --source-ranges "$source_ranges"
fi

# Redeploy-safe: delete an existing instance of the same name before recreate.
if [[ "$apply" == "true" ]] && gcloud compute instances describe "$instance" \
     --project "$project_id" --zone "$zone" >/dev/null 2>&1; then
  run gcloud compute instances delete "$instance" \
    --project "$project_id" --zone "$zone" --quiet
fi

run gcloud compute instances create "$instance" \
  --project "$project_id" \
  --zone "$zone" \
  --machine-type "$machine_type" \
  --service-account "$control_sa" \
  --scopes cloud-platform \
  --image-family cos-stable \
  --image-project cos-cloud \
  --tags "$network_tag" \
  --labels "openagents-managed=control,openagents-component=codex-control" \
  --metadata-from-file "gce-container-declaration=${container_decl}" \
  --metadata "google-logging-enabled=true"

cat <<SUMMARY

oa-codex-control persistent node:
  Project:      ${project_id}
  Instance:     ${instance}
  Zone:         ${zone}
  Machine:      ${machine_type}
  Service acct: ${control_sa}
  Image:        ${image}
  Control port: 8787 (firewall ${fw_control}; sources ${source_ranges})
  Labels:       openagents-managed=control

Mode apply=${apply}

STOP/DESTROY:
  gcloud compute instances stop ${instance} --project ${project_id} --zone ${zone}
  gcloud compute instances delete ${instance} --project ${project_id} --zone ${zone} --quiet
SUMMARY
