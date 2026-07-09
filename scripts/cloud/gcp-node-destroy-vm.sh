#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-node-destroy-vm.sh --project PROJECT_ID [--zone us-central1-a] [--node-name oa-node-dev-01] [--apply]

Prints or deletes the managed test GCE VM. Default mode is dry-run.
USAGE
}

project_id=""
zone="us-central1-a"
node_name="oa-node-dev-01"
apply="false"

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

if [[ "$apply" == "true" ]]; then
  gcloud compute instances delete "$node_name" --project "$project_id" --zone "$zone" --quiet
else
  printf '+ gcloud compute instances delete %q --project %q --zone %q --quiet\n' "$node_name" "$project_id" "$zone"
fi
