#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-node-workroom-smoke.sh --local
  scripts/gcp-node-workroom-smoke.sh --project PROJECT_ID [--zone us-central1-a] [--region us-central1] [--node-name oa-node-dev-01] [--env dev] [--image-tag TAG] [--apply]

Runs or prints the first no-wallet workroom smoke:
  - initialize private workroom metadata
  - lifecycle create/start
  - run one bounded command that writes a summary artifact
  - upload/retain the artifact through oa-workroomd
  - submit closeout and lifecycle closeout/archive/destroy receipts

Default GCE mode is dry-run. --local executes the smoke locally against cargo.
USAGE
}

project_id=""
region="us-central1"
zone="us-central1-a"
node_name="oa-node-dev-01"
env_name="dev"
image_tag="local"
apply="false"
local_mode="false"

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
    --node-name)
      node_name="${2:-}"
      shift 2
      ;;
    --env)
      env_name="${2:-}"
      shift 2
      ;;
    --image-tag)
      image_tag="${2:-}"
      shift 2
      ;;
    --apply)
      apply="true"
      shift
      ;;
    --local)
      local_mode="true"
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

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_local() {
  local state_dir artifact_file
  state_dir="$(mktemp -d)"
  artifact_file="${state_dir}/bounded-command-summary.txt"
  trap "rm -rf '$state_dir'" EXIT

  printf 'bounded smoke command completed\nworkroom_wallet_authority=false\n' >"$artifact_file"

  cargo run -p oa-workroomd -- metadata init \
    --workroom workroom.gcp-smoke.local \
    --program program.gcp-smoke.local \
    --repo OpenAgentsInc/openagents \
    --template template.no-wallet-smoke \
    --budget bounded-local-smoke \
    --deadline local \
    --trust-tier public_repo \
    --capability artifacts.write \
    --state-dir "$state_dir" \
    --json >/dev/null
  cargo run -p oa-workroomd -- artifacts policy init --required summary --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- lifecycle create --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- lifecycle start --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- artifacts upload --name summary --file "$artifact_file" --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- closeout submit --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- lifecycle closeout --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- lifecycle archive --state-dir "$state_dir" --json >/dev/null
  cargo run -p oa-workroomd -- lifecycle destroy --state-dir "$state_dir" --json
}

remote_script() {
  local image="${region}-docker.pkg.dev/${project_id}/oa-cloud/oa-workroomd:${image_tag}"
  cat <<REMOTE
set -euo pipefail
state_dir="/var/lib/openagents/workrooms/smoke-${env_name}"
artifact_file="\${state_dir}/bounded-command-summary.txt"
install -d -m 0750 "\${state_dir}"
printf 'bounded smoke command completed\nworkroom_wallet_authority=false\nnode=${node_name}\n' >"\${artifact_file}"
run_workroomd() {
  docker run --rm \
    -e OPENAGENTS_CLOUD_WORKROOM_HOME="\${state_dir}" \
    -v "\${state_dir}:\${state_dir}" \
    "${image}" "\$@"
}
run_workroomd metadata init --workroom "workroom.gcp-smoke.${env_name}" --program "program.gcp-smoke.${env_name}" --repo OpenAgentsInc/openagents --template template.no-wallet-smoke --budget bounded-gcp-smoke --deadline smoke --trust-tier public_repo --capability artifacts.write --state-dir "\${state_dir}" --json
run_workroomd artifacts policy init --required summary --state-dir "\${state_dir}" --json
run_workroomd lifecycle create --state-dir "\${state_dir}" --json
run_workroomd lifecycle start --state-dir "\${state_dir}" --json
run_workroomd artifacts upload --name summary --file "\${artifact_file}" --state-dir "\${state_dir}" --json
run_workroomd closeout submit --state-dir "\${state_dir}" --json
run_workroomd lifecycle closeout --state-dir "\${state_dir}" --json
run_workroomd lifecycle archive --state-dir "\${state_dir}" --json
run_workroomd lifecycle destroy --state-dir "\${state_dir}" --json
journalctl -u openagents-oa-node -n 20 --no-pager || true
REMOTE
}

if [[ "$local_mode" == "true" ]]; then
  run_local
  exit 0
fi

if [[ -z "$project_id" ]]; then
  echo "--project is required unless --local is used" >&2
  usage >&2
  exit 2
fi

script_file="$(mktemp)"
trap 'rm -f "$script_file"' EXIT
remote_script >"$script_file"

if [[ "$apply" == "true" ]]; then
  gcloud compute ssh "$node_name" \
    --project "$project_id" \
    --zone "$zone" \
    --tunnel-through-iap \
    --command "bash -s" <"$script_file"
else
  printf '+ gcloud compute ssh %q --project %q --zone %q --tunnel-through-iap --command %q < %q\n' \
    "$node_name" "$project_id" "$zone" "bash -s" "$script_file"
  sed -n '1,120p' "$script_file"
fi
