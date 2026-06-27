#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: setup-pylon.sh [options]

Creates or updates a Google Compute Engine VM and installs the OpenAgents Pylon
headless systemd node through apps/pylon/scripts/install-cloud-node.sh.

Options:
  --instance <name>              VM name. Default: PYLON_GCE_INSTANCE or pylon-gcloud-<timestamp>
  --project <id>                 GCP project. Default: PYLON_GCE_PROJECT or openagentsgemini
  --zone <zone>                  GCP zone. Default: PYLON_GCE_ZONE or us-central1-a
  --machine-type <type>          VM type. Default: PYLON_GCE_MACHINE_TYPE or e2-standard-4
  --subnet <name>                Subnet. Default: PYLON_GCE_SUBNET or oa-lightning-us-central1
  --boot-disk-size <size>        Boot disk size. Default: PYLON_GCE_BOOT_DISK_SIZE or 100GB
  --accelerator <type=count>     Optional GPU accelerator, for example nvidia-l4=1
  --tags <tags>                  Comma-separated network tags. Default: PYLON_GCE_TAGS
                                  or pylon-hosted,openagents-pylon
  --clear-startup-script         Remove an existing VM startup-script metadata value before start
  --ssh-key-file <path>          SSH key for gcloud compute ssh/scp. Default: PYLON_GCE_SSH_KEY_FILE
                                  or ~/.ssh/google_compute_engine when present
  --env-file <path>              Local env file copied over IAP and sourced as root on the VM
  --pylon-home-archive <path>    Optional tar.gz of an isolated PYLON_HOME to restore on the VM
  --supervisor <mode>            Install persistent own-capacity supervisor: none|codex|claude|both.
                                  Default: PYLON_GCE_SUPERVISOR or none
  --with-address                 Give the VM an external IP. Default is no external IP
  --dry-run                      Print actions without creating, copying, SSHing, or installing
  --help                         Show this help

Environment accepted by the remote installer:
  OPENAGENTS_AGENT_TOKEN         owner-granted token, normally supplied by --env-file
  ANTHROPIC_API_KEY              optional owner BYOK Claude Agent key
  PYLON_REF                      default generated as gcloud.<instance>
  PYLON_DISPLAY_NAME             default generated from the instance name
  PYLON_OPENAGENTS_BASE_URL      default https://openagents.com
  PYLON_RESOURCE_MODE            default background_20
  PYLON_ENABLE_CODEX_SUPERVISOR  set by --supervisor codex|both
  PYLON_ENABLE_CLAUDE_SUPERVISOR set by --supervisor claude|both

Security:
  Secrets are copied over IAP/SSH into /root/openagents-pylon.env on the VM and
  are never placed in instance metadata or startup scripts. Pylon-home archives
  are copied over IAP/SSH into /root and extracted locally on the VM.
USAGE
}

dry_run=0
with_address=0
instance="${PYLON_GCE_INSTANCE:-}"
project="${PYLON_GCE_PROJECT:-openagentsgemini}"
zone="${PYLON_GCE_ZONE:-us-central1-a}"
machine_type="${PYLON_GCE_MACHINE_TYPE:-e2-standard-4}"
subnet="${PYLON_GCE_SUBNET:-oa-lightning-us-central1}"
boot_disk_size="${PYLON_GCE_BOOT_DISK_SIZE:-100GB}"
accelerator="${PYLON_GCE_ACCELERATOR:-}"
network_tags="${PYLON_GCE_TAGS:-pylon-hosted,openagents-pylon}"
clear_startup_script="${PYLON_GCE_CLEAR_STARTUP_SCRIPT:-0}"
env_file="${PYLON_GCE_ENV_FILE:-}"
ssh_key_file="${PYLON_GCE_SSH_KEY_FILE:-}"
pylon_home_archive="${PYLON_GCE_PYLON_HOME_ARCHIVE:-}"
supervisor="${PYLON_GCE_SUPERVISOR:-none}"

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
    --accelerator)
      accelerator="${2:-}"
      shift 2
      ;;
    --tags)
      network_tags="${2:-}"
      shift 2
      ;;
    --clear-startup-script)
      clear_startup_script=1
      shift
      ;;
    --ssh-key-file)
      ssh_key_file="${2:-}"
      shift 2
      ;;
    --env-file)
      env_file="${2:-}"
      shift 2
      ;;
    --pylon-home-archive)
      pylon_home_archive="${2:-}"
      shift 2
      ;;
    --supervisor)
      supervisor="${2:-}"
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
  instance="pylon-gcloud-$(date -u +%Y%m%d%H%M%S)"
fi

if [[ -z "$project" || -z "$zone" || -z "$machine_type" || -z "$boot_disk_size" ]]; then
  usage >&2
  exit 2
fi

if [[ -n "$env_file" && ! -f "$env_file" ]]; then
  echo "env file not found: $env_file" >&2
  exit 2
fi

if [[ -n "$pylon_home_archive" && ! -f "$pylon_home_archive" ]]; then
  echo "pylon home archive not found: $pylon_home_archive" >&2
  exit 2
fi

case "$supervisor" in
  none|codex|claude|both) ;;
  *)
    echo "--supervisor must be one of none|codex|claude|both" >&2
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
  local attempts="${PYLON_GCE_SSH_ATTEMPTS:-8}"
  local delay_seconds="${PYLON_GCE_SSH_RETRY_SECONDS:-15}"
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

pylon_ref="${PYLON_REF:-gcloud.${instance}}"
display_name="${PYLON_DISPLAY_NAME:-GCloud Pylon ${instance}}"
base_url="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"
resource_mode="${PYLON_RESOURCE_MODE:-background_20}"
assignment_worker="${PYLON_ENABLE_ASSIGNMENT_WORKER:-1}"
enable_codex_supervisor=0
enable_claude_supervisor=0
if [[ "$supervisor" == "codex" || "$supervisor" == "both" ]]; then
  enable_codex_supervisor=1
fi
if [[ "$supervisor" == "claude" || "$supervisor" == "both" ]]; then
  enable_claude_supervisor=1
fi
remote_env="/root/openagents-pylon.env"
remote_pylon_home_archive="/root/openagents-pylon-home.tar.gz"
tmp_env="$(mktemp -t openagents-pylon-gcloud-env.XXXXXX)"
trap 'rm -f "$tmp_env"' EXIT
chmod 0600 "$tmp_env"

{
  if [[ -n "$env_file" ]]; then
    sed '/^[[:space:]]*#/d;/^[[:space:]]*$/d' "$env_file"
  fi
  printf 'PYLON_REF=%q\n' "$pylon_ref"
  printf 'PYLON_DISPLAY_NAME=%q\n' "$display_name"
  printf 'PYLON_OPENAGENTS_BASE_URL=%q\n' "$base_url"
  printf 'PYLON_RESOURCE_MODE=%q\n' "$resource_mode"
  printf 'PYLON_ENABLE_ASSIGNMENT_WORKER=%q\n' "$assignment_worker"
  printf 'PYLON_ENABLE_CODEX_SUPERVISOR=%q\n' "$enable_codex_supervisor"
  printf 'PYLON_ENABLE_CLAUDE_SUPERVISOR=%q\n' "$enable_claude_supervisor"
  if [[ -n "$pylon_home_archive" ]]; then
    printf 'PYLON_HOME_ARCHIVE=%q\n' "$remote_pylon_home_archive"
  fi
} >"$tmp_env"

if [[ "$dry_run" == "1" ]]; then
  echo "dry-run supervisor=${supervisor} codex=${enable_codex_supervisor} claude=${enable_claude_supervisor}"
  echo "dry-run PYLON_ENABLE_CODEX_SUPERVISOR=${enable_codex_supervisor}"
  echo "dry-run PYLON_ENABLE_CLAUDE_SUPERVISOR=${enable_claude_supervisor}"
  if [[ "$enable_codex_supervisor" == "1" ]]; then
    echo "dry-run installs openagents-codex-supervisor"
  fi
  if [[ "$enable_claude_supervisor" == "1" ]]; then
    echo "dry-run installs openagents-claude-supervisor"
  fi
  if [[ -n "$pylon_home_archive" ]]; then
    echo "dry-run pylon-home-archive=<redacted-local-path> remote=${remote_pylon_home_archive}"
  fi
fi

create_args=(
  compute instances create "$instance"
  --project "$project"
  --zone "$zone"
  --machine-type "$machine_type"
  --image-family ubuntu-2404-lts-amd64
  --image-project ubuntu-os-cloud
  --boot-disk-size "$boot_disk_size"
  --boot-disk-type pd-balanced
  --tags "$network_tags"
  --scopes https://www.googleapis.com/auth/cloud-platform
)

if [[ -n "$subnet" ]]; then
  create_args+=(--subnet "$subnet")
fi

if [[ "$with_address" == "0" ]]; then
  create_args+=(--no-address)
fi

if [[ -n "$accelerator" ]]; then
  accelerator_type="${accelerator%%=*}"
  accelerator_count="${accelerator#*=}"
  if [[ "$accelerator_type" == "$accelerator" || -z "$accelerator_type" || -z "$accelerator_count" ]]; then
    echo "--accelerator must use type=count, for example nvidia-l4=1" >&2
    exit 2
  fi
  create_args+=(--accelerator "type=${accelerator_type},count=${accelerator_count}" --maintenance-policy TERMINATE)
fi

if [[ "$dry_run" == "1" ]]; then
  run gcloud "${create_args[@]}" --quiet
elif gcloud compute instances describe "$instance" --project "$project" --zone "$zone" >/dev/null 2>&1; then
  echo "instance exists: ${instance}"
  if [[ -n "$network_tags" ]]; then
    run gcloud compute instances add-tags "$instance" --project "$project" --zone "$zone" --tags "$network_tags" --quiet
  fi
  if [[ "$clear_startup_script" == "1" ]]; then
    run gcloud compute instances remove-metadata "$instance" --project "$project" --zone "$zone" --keys startup-script --quiet || true
  fi
  run gcloud compute instances start "$instance" --project "$project" --zone "$zone" --quiet
else
  run gcloud "${create_args[@]}" --quiet
fi

ssh_common=(
  --project "$project"
  --zone "$zone"
  --tunnel-through-iap
)
if [[ -n "$ssh_key_file" ]]; then
  ssh_common+=(--ssh-key-file "$ssh_key_file")
fi

run_with_retries gcloud compute scp "$tmp_env" "${instance}:/tmp/openagents-pylon.env" "${ssh_common[@]}" --quiet
if [[ -n "$pylon_home_archive" ]]; then
  run_with_retries gcloud compute scp "$pylon_home_archive" "${instance}:/tmp/openagents-pylon-home.tar.gz" "${ssh_common[@]}" --quiet
fi

remote_install='
set -euo pipefail
sudo -n true
if [[ -f /tmp/openagents-pylon.env ]]; then
  sudo -n install -m 0600 -o root -g root /tmp/openagents-pylon.env /root/openagents-pylon.env
  sudo -n rm -f /tmp/openagents-pylon.env
elif [[ ! -f /root/openagents-pylon.env ]]; then
  echo "missing /tmp/openagents-pylon.env and /root/openagents-pylon.env" >&2
  exit 1
fi
if [[ -f /tmp/openagents-pylon-home.tar.gz ]]; then
  sudo -n install -m 0600 -o root -g root /tmp/openagents-pylon-home.tar.gz /root/openagents-pylon-home.tar.gz
  sudo -n rm -f /tmp/openagents-pylon-home.tar.gz
fi
sudo -n apt-get update
sudo -n apt-get install -y ca-certificates curl git tar unzip
if ! command -v bun >/dev/null 2>&1; then
  sudo -n mkdir -p /opt/bun
  curl -fsSL https://bun.sh/install | sudo -n BUN_INSTALL=/opt/bun bash
  sudo -n ln -sf /opt/bun/bin/bun /usr/local/bin/bun
fi
if [[ ! -d /opt/openagents-pylon/.git ]]; then
  sudo -n git clone --depth 1 https://github.com/OpenAgentsInc/openagents /opt/openagents-pylon
else
  sudo -n git config --global --add safe.directory /opt/openagents-pylon
  sudo -n git -C /opt/openagents-pylon fetch --depth 1 origin main
  sudo -n git -C /opt/openagents-pylon reset --hard origin/main
fi
sudo -n bash -lc "set -a; source /root/openagents-pylon.env; set +a; /opt/openagents-pylon/apps/pylon/scripts/install-cloud-node.sh"
sudo -n systemctl --no-pager --full status openagents-pylon
'

run_with_retries gcloud compute ssh "$instance" "${ssh_common[@]}" --command "$remote_install" --quiet

remote_verify='
set -euo pipefail
sudo -n systemctl is-active --quiet openagents-pylon
sudo -n -u pylon env PYLON_HOME=/var/lib/openagents-pylon PYLON_OPENAGENTS_BASE_URL=https://openagents.com bun /opt/openagents-pylon/apps/pylon/src/index.ts status --json
'

run_with_retries gcloud compute ssh "$instance" "${ssh_common[@]}" --command "$remote_verify" --quiet

cat <<EOF
GCloud Pylon setup finished.

Instance: ${instance}
Project:  ${project}
Zone:     ${zone}
Pylon ref: ${pylon_ref}

Inspect:
  gcloud compute ssh ${instance} --project ${project} --zone ${zone} --tunnel-through-iap --command 'sudo systemctl --no-pager --full status openagents-pylon'
EOF
