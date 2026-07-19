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
    (--control-token TOKEN | --control-token-secret SECRET_NAME) \
    [--zone us-central1-a] [--machine-type e2-small] \
    [--instance oa-codex-control-1] \
    [--firewall-rule oa-codex-control-port] \
    [--network-tag oa-codex-control] \
    [--image-tag cloud95] \
    [--control-source-cidr 1.2.3.4/32] \
    [--enable-managed-sandbox \
      --managed-sandbox-image-project PROJECT \
      --managed-sandbox-image-name IMAGE \
      --managed-sandbox-image-id IMMUTABLE_ID \
      --managed-sandbox-image-digest sha256:HEX \
      --managed-sandbox-profile-digest sha256:HEX \
      --managed-sandbox-control-internal-ip 10.0.0.10 \
      --managed-sandbox-provider-broker-url https://openagents.com \
      [--managed-sandbox-provider-broker-port 8790] \
      [--managed-sandbox-turn-driver /absolute/path] \
      [--managed-sandbox-io-driver /absolute/path]] \
    [--apply]

Required:
  --project          GCP project id (e.g. openagentsgemini)
  --control-token    OA_CODEX_CONTROL_TOKEN bearer for the HTTP control API
  --control-token-secret  Secret Manager secret read by the keyless control VM

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
firewall_rule="oa-codex-control-port"
network_tag="oa-codex-control"
image_tag="cloud95"
control_token=""
control_token_secret=""
control_source_cidr=""
apply="false"
enable_managed_sandbox="false"
managed_sandbox_image_project=""
managed_sandbox_image_name=""
managed_sandbox_image_id=""
managed_sandbox_image_digest=""
managed_sandbox_profile_digest=""
managed_sandbox_control_internal_ip=""
managed_sandbox_provider_broker_url=""
managed_sandbox_provider_broker_port="8790"
managed_sandbox_turn_driver=""
managed_sandbox_io_driver=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project_id="${2:-}"; shift 2 ;;
    --zone) zone="${2:-}"; shift 2 ;;
    --machine-type) machine_type="${2:-}"; shift 2 ;;
    --instance) instance="${2:-}"; shift 2 ;;
    --firewall-rule) firewall_rule="${2:-}"; shift 2 ;;
    --network-tag) network_tag="${2:-}"; shift 2 ;;
    --image-tag) image_tag="${2:-}"; shift 2 ;;
    --control-token) control_token="${2:-}"; shift 2 ;;
    --control-token-secret) control_token_secret="${2:-}"; shift 2 ;;
    --control-source-cidr) control_source_cidr="${2:-}"; shift 2 ;;
    --enable-managed-sandbox) enable_managed_sandbox="true"; shift ;;
    --managed-sandbox-image-project) managed_sandbox_image_project="${2:-}"; shift 2 ;;
    --managed-sandbox-image-name) managed_sandbox_image_name="${2:-}"; shift 2 ;;
    --managed-sandbox-image-id) managed_sandbox_image_id="${2:-}"; shift 2 ;;
    --managed-sandbox-image-digest) managed_sandbox_image_digest="${2:-}"; shift 2 ;;
    --managed-sandbox-profile-digest) managed_sandbox_profile_digest="${2:-}"; shift 2 ;;
    --managed-sandbox-control-internal-ip) managed_sandbox_control_internal_ip="${2:-}"; shift 2 ;;
    --managed-sandbox-provider-broker-url) managed_sandbox_provider_broker_url="${2:-}"; shift 2 ;;
    --managed-sandbox-provider-broker-port) managed_sandbox_provider_broker_port="${2:-}"; shift 2 ;;
    --managed-sandbox-turn-driver) managed_sandbox_turn_driver="${2:-}"; shift 2 ;;
    --managed-sandbox-io-driver) managed_sandbox_io_driver="${2:-}"; shift 2 ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$project_id" || -z "$instance" || -z "$firewall_rule" || -z "$network_tag" ]] || \
   [[ -z "$control_token" && -z "$control_token_secret" ]] || \
   [[ -n "$control_token" && -n "$control_token_secret" ]]; then
  echo "project, exactly one control token source, instance, firewall rule, and network tag are required" >&2
  usage >&2
  exit 2
fi

if [[ -n "$managed_sandbox_turn_driver" && "$managed_sandbox_turn_driver" != /* ]]; then
  echo "managed-sandbox turn driver must be an absolute container path" >&2
  exit 2
fi
if [[ -n "$managed_sandbox_turn_driver" && "$enable_managed_sandbox" != "true" ]]; then
  echo "managed-sandbox turn driver requires --enable-managed-sandbox" >&2
  exit 2
fi
if [[ -n "$managed_sandbox_io_driver" && "$managed_sandbox_io_driver" != /* ]]; then
  echo "managed-sandbox I/O driver must be an absolute container path" >&2
  exit 2
fi
if [[ -n "$managed_sandbox_io_driver" && "$enable_managed_sandbox" != "true" ]]; then
  echo "managed-sandbox I/O driver requires --enable-managed-sandbox" >&2
  exit 2
fi

if [[ "$enable_managed_sandbox" == "true" ]]; then
  for value in \
    "$managed_sandbox_image_project" \
    "$managed_sandbox_image_name" \
    "$managed_sandbox_image_id" \
    "$managed_sandbox_image_digest" \
    "$managed_sandbox_profile_digest" \
    "$managed_sandbox_control_internal_ip" \
    "$managed_sandbox_provider_broker_url"; do
    if [[ -z "$value" ]]; then
      echo "managed-sandbox image project/name/id/digest and profile digest are required when enabled" >&2
      exit 2
    fi
  done
  if [[ ! "$managed_sandbox_image_digest" =~ ^sha256:[0-9a-fA-F]{64}$ ]] || \
     [[ ! "$managed_sandbox_profile_digest" =~ ^sha256:[0-9a-fA-F]{64}$ ]]; then
    echo "managed-sandbox image and profile digests must be sha256 refs" >&2
    exit 2
  fi
  if [[ ! "$managed_sandbox_control_internal_ip" =~ ^10\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]] || \
     [[ ! "$managed_sandbox_provider_broker_port" =~ ^[0-9]{2,5}$ ]] || \
     [[ ! "$managed_sandbox_provider_broker_url" =~ ^https:// ]]; then
    echo "managed-sandbox control IP, broker port, or HTTPS broker URL is invalid" >&2
    exit 2
  fi
fi

region="${zone%-*}"
control_sa="oa-codex-control@${project_id}.iam.gserviceaccount.com"
image="${region}-docker.pkg.dev/${project_id}/oa-cloud/oa-codex-control:${image_tag}"
fw_control="$firewall_rule"
fw_control_deny="${firewall_rule}-deny"

run() {
  if [[ "$apply" == "true" ]]; then
    "$@"
  else
    printf '+'; printf ' %q' "$@"; printf '\n'
  fi
}

# Use the Container-Optimized OS container declaration via metadata so the VM
# runs the image as a managed container with the instance SA's ADC.
container_decl="$(mktemp)"
startup_script="$(mktemp)"
trap 'rm -f "$container_decl" "$startup_script"' EXIT
cat >"$startup_script" <<'STARTUP'
#!/bin/sh
set -eu
install -d -m 0700 /var/lib/openagents
STARTUP
managed_sandbox_env_yaml=""
managed_sandbox_turn_driver_env_yaml=""
managed_sandbox_io_driver_env_yaml=""
control_token_env_yaml=""
if [[ -n "$control_token_secret" ]]; then
  control_token_env_yaml="$(cat <<ENVYAML
        - name: OA_CODEX_CONTROL_TOKEN_SECRET
          value: "${control_token_secret}"
ENVYAML
)"
else
  control_token_env_yaml="$(cat <<ENVYAML
        - name: OA_CODEX_CONTROL_TOKEN
          value: "${control_token}"
ENVYAML
)"
fi
if [[ -n "$managed_sandbox_turn_driver" ]]; then
  managed_sandbox_turn_driver_env_yaml="$(cat <<ENVYAML
        - name: OA_MANAGED_SANDBOX_TURN_DRIVER
          value: "${managed_sandbox_turn_driver}"
ENVYAML
)"
fi
if [[ -n "$managed_sandbox_io_driver" ]]; then
  managed_sandbox_io_driver_env_yaml="$(cat <<ENVYAML
        - name: OA_MANAGED_SANDBOX_IO_DRIVER
          value: "${managed_sandbox_io_driver}"
ENVYAML
)"
fi
if [[ "$enable_managed_sandbox" == "true" ]]; then
  managed_sandbox_env_yaml="$(cat <<ENVYAML
        - name: OA_MANAGED_SANDBOX_PROVISIONER
          value: "live_gce"
        - name: OA_MANAGED_SANDBOX_PROJECT_ID
          value: "${project_id}"
        - name: OA_MANAGED_SANDBOX_ZONE
          value: "${zone}"
        - name: OA_MANAGED_SANDBOX_REGION
          value: "${region}"
        - name: OA_MANAGED_SANDBOX_MACHINE_CLASS
          value: "e2-small"
        - name: OA_MANAGED_SANDBOX_IMAGE_PROJECT
          value: "${managed_sandbox_image_project}"
        - name: OA_MANAGED_SANDBOX_IMAGE_NAME
          value: "${managed_sandbox_image_name}"
        - name: OA_MANAGED_SANDBOX_IMAGE_ID
          value: "${managed_sandbox_image_id}"
        - name: OA_MANAGED_SANDBOX_IMAGE_DIGEST
          value: "${managed_sandbox_image_digest}"
        - name: OA_MANAGED_SANDBOX_NETWORK
          value: "default"
        - name: OA_MANAGED_SANDBOX_PROFILE_REF
          value: "profile-ref://openagents/managed-sandbox/gce-e2-small-v1"
        - name: OA_MANAGED_SANDBOX_PROFILE_DIGEST
          value: "${managed_sandbox_profile_digest}"
        - name: OA_MANAGED_SANDBOX_PROVISIONER_REF
          value: "provisioner-ref://openagents/oa-codex-control/gce-v1"
        - name: OA_MANAGED_SANDBOX_NETWORK_POLICY_REF
          value: "network-policy-ref://openagents/managed-sandbox/broker-only-v1"
        - name: OA_MANAGED_SANDBOX_CONTROL_IDENTITY_REF
          value: "identity-ref://openagents/managed-sandbox/control"
        - name: OA_MANAGED_SANDBOX_CONTROL_INTERNAL_IP
          value: "${managed_sandbox_control_internal_ip}"
        - name: OA_MANAGED_SANDBOX_CONTROL_SERVICE_ACCOUNT
          value: "${control_sa}"
        - name: OA_MANAGED_SANDBOX_PROVIDER_BROKER_URL
          value: "${managed_sandbox_provider_broker_url}"
        - name: OA_MANAGED_SANDBOX_PROVIDER_BROKER_PORT
          value: "${managed_sandbox_provider_broker_port}"
${managed_sandbox_turn_driver_env_yaml}
${managed_sandbox_io_driver_env_yaml}
ENVYAML
)"
fi
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
${control_token_env_yaml}
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
${managed_sandbox_env_yaml}
      volumeMounts:
        - name: control-state
          mountPath: /var/lib/openagents
  restartPolicy: Always
  volumes:
    - name: control-state
      hostPath:
        path: /var/lib/openagents
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
    --priority 900 \
    --target-tags "$network_tag" \
    --source-ranges "$source_ranges"
else
  run gcloud compute firewall-rules create "$fw_control" \
    --project "$project_id" \
    --direction INGRESS \
    --action ALLOW \
    --rules tcp:8787 \
    --priority 900 \
    --target-tags "$network_tag" \
    --source-ranges "$source_ranges"
fi
if [[ "$apply" == "true" ]] && gcloud compute firewall-rules describe "$fw_control_deny" \
     --project "$project_id" >/dev/null 2>&1; then
  run gcloud compute firewall-rules update "$fw_control_deny" \
    --project "$project_id" \
    --rules tcp:8787 \
    --priority 1000 \
    --target-tags "$network_tag" \
    --source-ranges 0.0.0.0/0
else
  run gcloud compute firewall-rules create "$fw_control_deny" \
    --project "$project_id" \
    --direction INGRESS \
    --action DENY \
    --rules tcp:8787 \
    --priority 1000 \
    --target-tags "$network_tag" \
    --source-ranges 0.0.0.0/0
fi

# Redeploy-safe: delete an existing instance of the same name before recreate.
if [[ "$apply" == "true" ]] && gcloud compute instances describe "$instance" \
     --project "$project_id" --zone "$zone" >/dev/null 2>&1; then
  if ! gcloud compute instances delete "$instance" \
    --project "$project_id" --zone "$zone" --quiet; then
    # A concurrent cleanup can win after describe. Treat that race as clean
    # only when a second observation confirms the instance is absent.
    if gcloud compute instances describe "$instance" \
         --project "$project_id" --zone "$zone" >/dev/null 2>&1; then
      echo "failed to replace existing control instance" >&2
      exit 1
    fi
  fi
fi

private_network_args=()
if [[ -n "$managed_sandbox_control_internal_ip" ]]; then
  private_network_args=(
    --private-network-ip "$managed_sandbox_control_internal_ip"
    --no-address
  )
fi

run gcloud compute instances create "$instance" \
  --project "$project_id" \
  --zone "$zone" \
  --machine-type "$machine_type" \
  --service-account "$control_sa" \
  --scopes cloud-platform \
  --image-family cos-stable \
  --image-project cos-cloud \
  "${private_network_args[@]}" \
  --tags "$network_tag" \
  --labels "openagents-managed=control,openagents-component=codex-control" \
  --metadata-from-file "gce-container-declaration=${container_decl},startup-script=${startup_script}" \
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
  Managed SBX:  ${enable_managed_sandbox} (live_gce, keyless control identity, guest identity none)

Mode apply=${apply}

STOP/DESTROY:
  gcloud compute instances stop ${instance} --project ${project_id} --zone ${zone}
  gcloud compute instances delete ${instance} --project ${project_id} --zone ${zone} --quiet
SUMMARY
