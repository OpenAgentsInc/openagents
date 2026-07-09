#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: setup-gce-host.sh [options]

Creates or updates the public, non-secret GCE host substrate for OpenAgents
Agent Computers. This arms only the nested-virtualization host shape; the
in-repo crates/oa-codex-control provisioner owns Firecracker lifecycle and secrets.

Options:
  --instance <name>              VM name. Default: AGENT_COMPUTER_GCE_INSTANCE or agent-computer-gce-<timestamp>
  --project <id>                 GCP project. Default: AGENT_COMPUTER_GCE_PROJECT or openagentsgemini
  --zone <zone>                  GCP zone. Default: AGENT_COMPUTER_GCE_ZONE or us-central1-a
  --machine-type <type>          VM type. Default: AGENT_COMPUTER_GCE_MACHINE_TYPE or n2-standard-4
  --subnet <name>                Subnet. Default: AGENT_COMPUTER_GCE_SUBNET or oa-lightning-us-central1
  --boot-disk-size <size>        Boot disk size. Default: AGENT_COMPUTER_GCE_BOOT_DISK_SIZE or 200GB
  --tags <tags>                  Comma-separated network tags. Default: AGENT_COMPUTER_GCE_TAGS
                                  or agent-computer-host,openagents-agent-computer
  --ssh-key-file <path>          SSH key for gcloud compute ssh. Default: AGENT_COMPUTER_GCE_SSH_KEY_FILE
                                  or ~/.ssh/google_compute_engine when present
  --with-address                 Give the VM an external IP. Default is no external IP
  --dry-run                      Print actions without creating, SSHing, or installing
  --help                         Show this help

Security:
  This script accepts no env file and no bearer token. Control-plane tokens,
  SCM credentials, user repo material, kernels/rootfs paths, and capability
  broker internals stay in Secret Manager and the openagents monorepo crates.
USAGE
}

dry_run=0
with_address=0
instance="${AGENT_COMPUTER_GCE_INSTANCE:-}"
project="${AGENT_COMPUTER_GCE_PROJECT:-openagentsgemini}"
zone="${AGENT_COMPUTER_GCE_ZONE:-us-central1-a}"
machine_type="${AGENT_COMPUTER_GCE_MACHINE_TYPE:-n2-standard-4}"
subnet="${AGENT_COMPUTER_GCE_SUBNET:-oa-lightning-us-central1}"
boot_disk_size="${AGENT_COMPUTER_GCE_BOOT_DISK_SIZE:-200GB}"
network_tags="${AGENT_COMPUTER_GCE_TAGS:-agent-computer-host,openagents-agent-computer}"
ssh_key_file="${AGENT_COMPUTER_GCE_SSH_KEY_FILE:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --instance)
      instance="${2:-}"
      shift 2
      ;;
    --project)
      project="${2:-}"
      shift 2
      ;;
    --zone)
      zone="${2:-}"
      shift 2
      ;;
    --machine-type)
      machine_type="${2:-}"
      shift 2
      ;;
    --subnet)
      subnet="${2:-}"
      shift 2
      ;;
    --boot-disk-size)
      boot_disk_size="${2:-}"
      shift 2
      ;;
    --tags)
      network_tags="${2:-}"
      shift 2
      ;;
    --ssh-key-file)
      ssh_key_file="${2:-}"
      shift 2
      ;;
    --with-address)
      with_address=1
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$instance" ]]; then
  instance="agent-computer-gce-$(date -u +%Y%m%d%H%M%S)"
fi

if [[ -z "$project" || -z "$zone" || -z "$machine_type" || -z "$boot_disk_size" ]]; then
  usage >&2
  exit 2
fi

case "$machine_type" in
  n2-*|n1-*) ;;
  *)
    echo "machine type must be n2-* or n1-* for nested virtualization: $machine_type" >&2
    exit 2
    ;;
esac

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 127
  fi
}

if [[ -z "$ssh_key_file" && -f "${HOME}/.ssh/google_compute_engine" ]]; then
  ssh_key_file="${HOME}/.ssh/google_compute_engine"
fi

quote_cmd() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
}

run() {
  if [[ "$dry_run" == "1" ]]; then
    quote_cmd "$@"
  else
    "$@"
  fi
}

run_with_retries() {
  local attempts="${AGENT_COMPUTER_GCE_SSH_ATTEMPTS:-8}"
  local delay_seconds="${AGENT_COMPUTER_GCE_SSH_RETRY_SECONDS:-15}"
  if [[ "$dry_run" == "1" ]]; then
    run "$@"
    return
  fi

  local attempt=1
  while true; do
    if "$@"; then
      return
    fi
    if [[ "$attempt" -ge "$attempts" ]]; then
      return 1
    fi
    echo "command failed; retrying in ${delay_seconds}s (${attempt}/${attempts}): $*" >&2
    sleep "$delay_seconds"
    attempt=$((attempt + 1))
  done
}

if [[ "$dry_run" != "1" ]]; then
  require_cmd gcloud
fi

create_args=(
  compute instances create "$instance"
  --project "$project"
  --zone "$zone"
  --machine-type "$machine_type"
  --enable-nested-virtualization
  --image-family ubuntu-2404-lts-amd64
  --image-project ubuntu-os-cloud
  --boot-disk-size "$boot_disk_size"
  --boot-disk-type pd-balanced
  --tags "$network_tags"
  --scopes https://www.googleapis.com/auth/cloud-platform
)

if [[ "$machine_type" == n1-* ]]; then
  create_args+=(--min-cpu-platform "Intel Haswell")
fi

if [[ -n "$subnet" ]]; then
  create_args+=(--subnet "$subnet")
fi

if [[ "$with_address" == "1" ]]; then
  create_args+=(--address "")
else
  create_args+=(--no-address)
fi

ssh_args=(compute ssh "$instance" --project "$project" --zone "$zone" --tunnel-through-iap)
if [[ -n "$ssh_key_file" ]]; then
  ssh_args+=(--ssh-key-file "$ssh_key_file")
fi

remote_verify='set -euo pipefail; test -c /dev/kvm; sudo install -d -m 0700 /var/lib/openagents/agent-computers; sudo apt-get update; sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl jq iproute2 iptables nftables; echo agent-computer-host-ready'

run gcloud "${create_args[@]}"
run_with_retries gcloud "${ssh_args[@]}" --command "$remote_verify"
