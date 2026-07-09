#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-node-status.sh --project PROJECT_ID [--zone us-central1-a] [--node-name oa-node-dev-01]

Prints VM status plus recent redacted systemd logs for openagents-oa-node.
USAGE
}

project_id=""
zone="us-central1-a"
node_name="oa-node-dev-01"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project)
      project_id="${2:-}"
      shift 2
      ;;
    --zone)
      zone="${2:-}"
      shift 2
      ;;
    --node-name)
      node_name="${2:-}"
      shift 2
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

gcloud compute instances describe "$node_name" \
  --project "$project_id" \
  --zone "$zone" \
  --format 'json(name,status,zone,tags.items,serviceAccounts.email,lastStartTimestamp,metadata.items)'

gcloud compute ssh "$node_name" \
  --project "$project_id" \
  --zone "$zone" \
  --tunnel-through-iap \
  --command 'sudo systemctl status openagents-oa-node --no-pager; sudo journalctl -u openagents-oa-node -n 80 --no-pager'
