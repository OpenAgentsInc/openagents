#!/usr/bin/env bash
# Isolated, short-lived GCE host for the SPACE spike (#11728).
set -euo pipefail

PROJECT=${SPACE_GCP_PROJECT:-openagentsgemini}
ZONE=${SPACE_GCP_ZONE:-us-central1-a}
REGION=${ZONE%-*}
NAME=oa-space-spike-11728
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
g() { gcloud --project "$PROJECT" --quiet "$@"; }
ssh_host() { g compute ssh "space-admin@$NAME" --zone "$ZONE" --tunnel-through-iap "$@"; }

owned() {
  local label
  label=$(g compute "$1" describe "$NAME" "${@:2}" --format='value(labels.openagents-managed)')
  [[ "$label" == space-spike ]] || { echo "Refusing to change an unrecognized SPACE host." >&2; exit 1; }
}

case ${1:-help} in
  create)
    # Fail before creating anything if the configured login cannot reach GCP.
    g compute zones describe "$ZONE" --format='value(name)' >/dev/null
    if g compute instances describe "$NAME" --zone "$ZONE" >/dev/null 2>&1; then
      owned instances --zone "$ZONE"
      echo "SPACE spike host already exists."
      exit 0
    fi
    # These names belong solely to this issue. Do not reuse production networks.
    if ! g compute networks describe "$NAME" >/dev/null 2>&1; then
      g compute networks create "$NAME" --subnet-mode=custom --description='SPACE spike #11728'
    fi
    [[ $(g compute networks describe "$NAME" --format='value(description)') == 'SPACE spike #11728' ]] || { echo 'Network ownership check failed.' >&2; exit 1; }
    if ! g compute networks subnets describe "$NAME" --region "$REGION" >/dev/null 2>&1; then
      g compute networks subnets create "$NAME" --network "$NAME" --region "$REGION" --range=10.217.28.0/24 --enable-private-ip-google-access
    fi
    if ! g compute firewall-rules describe "$NAME-iap" >/dev/null 2>&1; then
      g compute firewall-rules create "$NAME-iap" --network "$NAME" --allow=tcp:22 --source-ranges=35.235.240.0/20 --target-tags="$NAME" --description='SPACE spike #11728'
    fi
    if ! g compute routers describe "$NAME" --region "$REGION" >/dev/null 2>&1; then
      g compute routers create "$NAME" --network "$NAME" --region "$REGION"
    fi
    if ! g compute routers nats describe "$NAME" --router "$NAME" --region "$REGION" >/dev/null 2>&1; then
      g compute routers nats create "$NAME" --router "$NAME" --region "$REGION" --auto-allocate-nat-external-ips --nat-custom-subnet-ip-ranges="$NAME"
    fi
    g compute instances create "$NAME" --zone "$ZONE" --machine-type=n2-standard-8 \
      --min-cpu-platform='Intel Cascade Lake' --enable-nested-virtualization \
      --image-family=ubuntu-2404-lts-amd64 --image-project=ubuntu-os-cloud \
      --boot-disk-size=100GB --boot-disk-type=pd-balanced --local-ssd=interface=NVME \
      --network="$NAME" --subnet="$NAME" --no-address --no-service-account --no-scopes \
      --tags="$NAME" --labels=openagents-managed=space-spike,issue=11728 \
      --metadata=block-project-ssh-keys=TRUE,enable-oslogin=FALSE \
      --max-run-duration=6h --instance-termination-action=DELETE
    ;;
  install)
    owned instances --zone "$ZONE"
    g compute scp "$ROOT/deploy/space-host/install.sh" "space-admin@$NAME:space-install.sh" --zone "$ZONE" --tunnel-through-iap
    # The log stays on the spike's own local SSD. Poll it with `host.sh status`.
    ssh_host --command 'sudo systemd-run --unit=space-spike-install --collect --property=RuntimeMaxSec=18000 /bin/bash "$HOME/space-install.sh"'
    ;;
  status)
    owned instances --zone "$ZONE"
    ssh_host --command 'sudo journalctl -u space-spike-install --no-pager -n 35'
    ;;
  runtime)
    owned instances --zone "$ZONE"
    image=${SPACE_RUNTIME_IMAGE:-oa-coder-main-20261015}
    g compute disks create "$NAME-runtime" --zone "$ZONE" --image="$image" --type=pd-balanced --labels=openagents-managed=space-spike,issue=11728
    runtime_cleanup() {
      g compute instances detach-disk "$NAME" --zone "$ZONE" --disk="$NAME-runtime" || true
      g compute disks delete "$NAME-runtime" --zone "$ZONE"
    }
    trap runtime_cleanup EXIT
    g compute instances attach-disk "$NAME" --zone "$ZONE" --disk="$NAME-runtime" --device-name=space-runtime --mode=ro
    g compute scp "$ROOT/deploy/space-host/export-runtime.sh" "space-admin@$NAME:space-export-runtime.sh" --zone "$ZONE" --tunnel-through-iap
    ssh_host --command 'sudo /bin/bash "$HOME/space-export-runtime.sh"'
    ;;
  template)
    owned instances --zone "$ZONE"
    g compute scp "$ROOT/deploy/space-host/runtime-template.sh" "space-admin@$NAME:space-runtime-template.sh" --zone "$ZONE" --tunnel-through-iap
    ssh_host --command 'sudo install -o space-spike -g space-spike -m 0755 "$HOME/space-runtime-template.sh" /var/lib/space-spike/home/runtime-template.sh; sudo systemd-run --unit=space-spike-template --collect --property=RuntimeMaxSec=3600 /usr/sbin/runuser -l space-spike -c "bash ~/runtime-template.sh > ~/template.log 2>&1"'
    ;;
  ssh)
    shift
    owned instances --zone "$ZONE"
    ssh_host "$@"
    ;;
  destroy)
    if g compute instances describe "$NAME" --zone "$ZONE" >/dev/null 2>&1; then
      owned instances --zone "$ZONE"
      g compute instances delete "$NAME" --zone "$ZONE" --delete-disks=all
    fi
    if g compute disks describe "$NAME-runtime" --zone "$ZONE" >/dev/null 2>&1; then
      label=$(g compute disks describe "$NAME-runtime" --zone "$ZONE" --format='value(labels.openagents-managed)')
      [[ "$label" == space-spike ]] || { echo 'Runtime disk ownership check failed.' >&2; exit 1; }
      g compute disks delete "$NAME-runtime" --zone "$ZONE"
    fi
    # Check the issue description before deleting the dedicated network.
    description=$(g compute networks describe "$NAME" --format='value(description)' 2>/dev/null || true)
    [[ -z "$description" || "$description" == 'SPACE spike #11728' ]] || { echo 'Network ownership check failed.' >&2; exit 1; }
    if [[ -n "$description" ]]; then
      if g compute routers describe "$NAME" --region "$REGION" >/dev/null 2>&1; then
        g compute routers delete "$NAME" --region "$REGION"
      fi
      if g compute firewall-rules describe "$NAME-iap" >/dev/null 2>&1; then g compute firewall-rules delete "$NAME-iap"; fi
      if g compute networks subnets describe "$NAME" --region "$REGION" >/dev/null 2>&1; then g compute networks subnets delete "$NAME" --region "$REGION"; fi
      g compute networks delete "$NAME"
    fi
    g compute instances list --filter='labels.openagents-managed=space-spike' --format='table(name,status)'
    g compute disks list --filter="name~^$NAME" --format='table(name,zone,sizeGb)'
    ;;
  *)
    echo 'Usage: deploy/space-host/host.sh create|install|status|runtime|template|ssh|destroy'
    ;;
esac
