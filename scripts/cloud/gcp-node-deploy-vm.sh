#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-node-deploy-vm.sh --project PROJECT_ID [--zone us-central1-a] [--region us-central1] [--env dev] [--node-name oa-node-dev-01] [--image-tag TAG] [--apply]

Creates or prints commands for one managed test GCE VM that starts oa-node
under systemd, writes redacted journald logs, and can be inspected or destroyed
with companion scripts.

The default mode is dry-run. Pass --apply to execute gcloud commands.
USAGE
}

project_id=""
region="us-central1"
zone="us-central1-a"
env_name="dev"
node_name="oa-node-dev-01"
image_tag="local"
machine_type="e2-standard-2"
apply="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project)
      project_id="${2:-}"
      shift 2
      ;;
    --region)
      region="${2:-}"
      shift 2
      ;;
    --zone)
      zone="${2:-}"
      shift 2
      ;;
    --env)
      env_name="${2:-}"
      shift 2
      ;;
    --node-name)
      node_name="${2:-}"
      shift 2
      ;;
    --image-tag)
      image_tag="${2:-}"
      shift 2
      ;;
    --machine-type)
      machine_type="${2:-}"
      shift 2
      ;;
    --apply)
      apply="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$project_id" ]]; then
  echo "--project is required" >&2
  usage >&2
  exit 2
fi

network_name="oa-cloud-${env_name}"
subnet_name="oa-cloud-${env_name}-${region}"
node_sa="oa-node-${env_name}@${project_id}.iam.gserviceaccount.com"
state_bucket="${project_id}-oa-node-state-${env_name}"
artifact_repo="${region}-docker.pkg.dev/${project_id}/oa-cloud"
startup_script="$(mktemp)"
trap 'rm -f "$startup_script"' EXIT

cat >"$startup_script" <<STARTUP
#!/usr/bin/env bash
set -euo pipefail

install -d -m 0750 -o root -g root /var/lib/openagents/oa-node
install -d -m 0755 -o root -g root /opt/openagents/bin

cat >/opt/openagents/bin/oa-node <<'NODE'
#!/usr/bin/env bash
set -euo pipefail
exec docker run --rm \
  --name oa-node-status \
  -e OPENAGENTS_CLOUD_NODE_HOME=/var/lib/openagents/oa-node \
  -v /var/lib/openagents/oa-node:/var/lib/openagents/oa-node \
  ${artifact_repo}/oa-node:${image_tag} "\$@"
NODE
chmod 0755 /opt/openagents/bin/oa-node

cat >/etc/systemd/system/openagents-oa-node.service <<'UNIT'
[Unit]
Description=OpenAgents managed Cloud node
After=network-online.target docker.service
Wants=network-online.target docker.service

[Service]
Type=oneshot
RemainAfterExit=true
Environment=OPENAGENTS_CLOUD_NODE_HOME=/var/lib/openagents/oa-node
ExecStartPre=/usr/bin/docker pull ${artifact_repo}/oa-node:${image_tag}
ExecStart=/usr/bin/docker run --rm --name openagents-oa-node-status \\
  -e OPENAGENTS_CLOUD_NODE_HOME=/var/lib/openagents/oa-node \\
  -v /var/lib/openagents/oa-node:/var/lib/openagents/oa-node \\
  ${artifact_repo}/oa-node:${image_tag} status --json
StandardOutput=journal
StandardError=journal
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
UNIT

if command -v gcloud >/dev/null 2>&1; then
  /opt/openagents/bin/oa-node init --org org.openagents.gcp.${env_name} --node-id ${node_name} --json >/var/lib/openagents/oa-node/init.json 2>/var/lib/openagents/oa-node/init.err || true
fi

systemctl daemon-reload
systemctl enable --now openagents-oa-node.service

printf '{"event":"oa_node_startup_complete","node":"%s","state_bucket":"gs://%s"}\n' "${node_name}" "${state_bucket}" | systemd-cat -t openagents-bootstrap
STARTUP

run() {
  if [[ "$apply" == "true" ]]; then
    "$@"
  else
    printf '+'
    printf ' %q' "$@"
    printf '\n'
  fi
}

run gcloud compute instances create "$node_name" \
  --project "$project_id" \
  --zone "$zone" \
  --machine-type "$machine_type" \
  --network-interface "subnet=${subnet_name},no-address" \
  --service-account "$node_sa" \
  --scopes cloud-platform \
  --tags "oa-cloud-node-${env_name}" \
  --image-family cos-stable \
  --image-project cos-cloud \
  --metadata-from-file "startup-script=${startup_script}" \
  --metadata "enable-oslogin=TRUE,openagents-env=${env_name},openagents-state-bucket=${state_bucket}"

cat <<SUMMARY

OpenAgents Cloud node VM ${node_name}:
  Project:        ${project_id}
  Zone:           ${zone}
  Subnet:         ${subnet_name}
  Service acct:   ${node_sa}
  Image:          ${artifact_repo}/oa-node:${image_tag}
  Systemd unit:   openagents-oa-node.service
  State bucket:   gs://${state_bucket}

Mode apply=${apply}
SUMMARY
