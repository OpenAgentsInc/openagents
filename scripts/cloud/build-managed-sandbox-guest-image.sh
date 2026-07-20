#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/cloud/build-managed-sandbox-guest-image.sh \
    --project PROJECT --zone ZONE --image-name NAME [--apply]

Builds the immutable SBX-09 GCE guest image with Node.js 24,
@openai/codex-sdk@0.144.3, @anthropic-ai/claude-agent-sdk@0.3.172, and the
OpenAgents guest driver. Default mode is dry-run.
USAGE
}

project=""
zone="us-central1-a"
image_name=""
apply="false"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project="${2:-}"; shift 2 ;;
    --zone) zone="${2:-}"; shift 2 ;;
    --image-name) image_name="${2:-}"; shift 2 ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done
if [[ -z "$project" || -z "$zone" || -z "$image_name" ]]; then
  usage >&2
  exit 2
fi
if [[ ! "$image_name" =~ ^[a-z]([-a-z0-9]{0,61}[a-z0-9])?$ ]]; then
  echo "image name is not a valid immutable GCE image name" >&2
  exit 2
fi

revision="$(git rev-parse HEAD)"
stamp="$(date -u +%Y%m%d%H%M%S)-$$"
builder="oa-msb-image-builder-${stamp}"
setup_file="$(mktemp)"
cleanup() {
  rm -f "$setup_file"
  if [[ "$apply" == "true" ]]; then
    gcloud compute instances delete "$builder" \
      --project "$project" --zone "$zone" --quiet >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

cat >"$setup_file" <<'SETUP'
#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends bubblewrap ca-certificates curl git iptables openssh-server python3 xz-utils
curl -fsSL https://deb.nodesource.com/setup_24.x | bash -
apt-get install -y --no-install-recommends nodejs
id openagents >/dev/null 2>&1 || useradd --create-home --shell /bin/bash openagents
install -d -o openagents -g openagents -m 0700 /workspace /var/lib/openagents/managed-sandbox-turns
install -d -o root -g root -m 0755 /opt/openagents-managed-sandbox
cd /opt/openagents-managed-sandbox
npm init -y >/dev/null
npm install --omit=dev --save-exact \
  @openai/codex-sdk@0.144.3 \
  @anthropic-ai/claude-agent-sdk@0.3.172
install -o root -g root -m 0755 /tmp/managed-sandbox-guest-turn.mjs \
  /opt/openagents-managed-sandbox/managed-sandbox-guest-turn.mjs
install -o root -g root -m 0755 /tmp/managed-sandbox-guest-io.py \
  /opt/openagents-managed-sandbox/managed-sandbox-guest-io.py
rm -f /tmp/managed-sandbox-guest-turn.mjs /tmp/managed-sandbox-guest-io.py
cat >/etc/systemd/system/openagents-managed-sandbox-hostkeys.service <<'UNIT'
[Unit]
Description=Generate per-guest OpenSSH host keys
After=local-fs.target
Before=ssh.service

[Service]
Type=oneshot
ExecStart=/usr/bin/ssh-keygen -A
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
UNIT
systemctl enable openagents-managed-sandbox-hostkeys.service
cat >/etc/systemd/system/openagents-managed-sandbox-metadata-guard.service <<'UNIT'
[Unit]
Description=Block managed-sandbox workload access to GCE metadata
After=network-online.target
Before=google-startup-scripts.service ssh.service

[Service]
Type=oneshot
ExecStart=/bin/sh -c '/usr/sbin/iptables -C OUTPUT -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT 2>/dev/null || /usr/sbin/iptables -I OUTPUT 1 -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT'
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
UNIT
systemctl enable openagents-managed-sandbox-metadata-guard.service
printf '%s\n' \
  'PasswordAuthentication no' \
  'PermitRootLogin no' \
  'AllowUsers openagents' \
  >/etc/ssh/sshd_config.d/90-openagents-managed-sandbox.conf
npm cache clean --force >/dev/null 2>&1 || true
apt-get clean
rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
rm -f /etc/ssh/ssh_host_* /etc/machine-id
truncate -s 0 /var/log/wtmp /var/log/btmp /var/log/lastlog 2>/dev/null || true
sync
SETUP

if [[ "$apply" != "true" ]]; then
  cat <<SUMMARY
Managed-sandbox guest image dry run
  project:  $project
  zone:     $zone
  image:    $image_name
  revision: $revision
  builder:  $builder
  SDKs:     codex 0.144.3; claude-agent 0.3.172
SUMMARY
  exit 0
fi

if gcloud compute images describe "$image_name" --project "$project" >/dev/null 2>&1; then
  read -r existing_revision existing_status existing_id < <(
    gcloud compute images describe "$image_name" \
      --project "$project" \
      --format='value(labels.openagents-source-revision,status,id)'
  )
  if [[ "$existing_revision" != "$revision" || "$existing_status" != "READY" || \
        -z "$existing_id" ]]; then
    echo "immutable image exists but does not match this source revision in READY state: $image_name" >&2
    exit 2
  fi
  existing_digest="$(printf '%s' "${project}|${image_name}|${existing_id}" | \
    shasum -a 256 | awk '{print $1}')"
  cat <<SUMMARY
Managed-sandbox guest image already admitted
  project:       $project
  imageName:     $image_name
  imageId:       $existing_id
  imageDigest:   sha256:$existing_digest
  sourceRevision:$revision
  SDKs:          codex 0.144.3; claude-agent 0.3.172
SUMMARY
  exit 0
fi

gcloud compute instances create "$builder" \
  --project "$project" \
  --zone "$zone" \
  --machine-type e2-standard-2 \
  --image-family debian-12 \
  --image-project debian-cloud \
  --boot-disk-size 20GB \
  --no-service-account \
  --no-scopes \
  --labels "openagents-managed=image-builder,openagents-component=managed-sandbox-guest"

for _ in $(seq 1 30); do
  if gcloud compute ssh "openagents@${builder}" \
    --project "$project" --zone "$zone" --quiet \
    --command 'true' >/dev/null 2>&1; then
    break
  fi
  sleep 5
done

gcloud compute scp \
  scripts/cloud/managed-sandbox-guest-turn.mjs \
  "openagents@${builder}:/tmp/managed-sandbox-guest-turn.mjs" \
  --project "$project" --zone "$zone" --quiet
gcloud compute scp \
  scripts/cloud/managed-sandbox-guest-io.py \
  "openagents@${builder}:/tmp/managed-sandbox-guest-io.py" \
  --project "$project" --zone "$zone" --quiet
gcloud compute scp "$setup_file" "openagents@${builder}:/tmp/setup.sh" \
  --project "$project" --zone "$zone" --quiet
gcloud compute ssh "openagents@${builder}" \
  --project "$project" --zone "$zone" --quiet \
  --command 'sudo bash /tmp/setup.sh'
gcloud compute instances stop "$builder" --project "$project" --zone "$zone" --quiet
gcloud compute images create "$image_name" \
  --project "$project" \
  --source-disk "$builder" \
  --source-disk-zone "$zone" \
  --family oa-managed-sandbox-guest-v1 \
  --labels "openagents-managed=managed-sandbox-image,openagents-contract=managed-sandbox-v1,openagents-source-revision=${revision}"

image_id="$(gcloud compute images describe "$image_name" \
  --project "$project" --format='value(id)')"
image_digest="$(printf '%s' "${project}|${image_name}|${image_id}" | shasum -a 256 | awk '{print $1}')"
cat <<SUMMARY
Managed-sandbox guest image built
  project:       $project
  imageName:     $image_name
  imageId:       $image_id
  imageDigest:   sha256:$image_digest
  sourceRevision:$revision
  SDKs:          codex 0.144.3; claude-agent 0.3.172
SUMMARY
