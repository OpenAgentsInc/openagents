#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/collect-mvp-observability.sh --local [--output-dir DIR]
  scripts/collect-mvp-observability.sh --project PROJECT_ID [--zone us-central1-a] [--node-name oa-node-dev-01] [--output-dir DIR] [--apply]

Collects MVP node/workroom/ingress/artifact/receipt observability with one
trace id. GCP mode is dry-run unless --apply is supplied.
USAGE
}

project_id=""
zone="us-central1-a"
node_name="oa-node-dev-01"
output_dir=""
local_mode="false"
apply="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --local)
      local_mode="true"
      shift
      ;;
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
    --output-dir)
      output_dir="${2:-}"
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

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

new_trace_id() {
  printf 'obs.%s.%s' "$(date +%s)" "$$"
}

assert_no_secret_markers() {
  local label="$1"
  local path="$2"
  local lower
  lower="$(tr '[:upper:]' '[:lower:]' <"$path")"
  for marker in \
    "secret-token" \
    "bearer " \
    "authorization:" \
    "api_key=sk-" \
    "x-api-key:" \
    "access_token" \
    "wallet seed" \
    "private key"; do
    if grep -Fq "$marker" <<<"$lower"; then
      echo "${label} leaked marker ${marker} in ${path}" >&2
      exit 1
    fi
  done
}

append_event() {
  local events_file="$1"
  local trace_id="$2"
  local seq="$3"
  local source="$4"
  local kind="$5"
  local state_file="$6"
  printf '{"trace_id":"%s","event_id":"%s.%03d.%s","source":"%s","kind":"%s","state_file":"%s"}\n' \
    "$trace_id" "$trace_id" "$seq" "$kind" "$source" "$kind" "$state_file" >>"$events_file"
}

run_json() {
  local output="$1"
  shift
  "$@" --json >"$output"
}

run_local() {
  local trace_id state_dir artifact_file events_file redaction_log cleanup_output_dir
  trace_id="$(new_trace_id)"
  if [[ -z "$output_dir" ]]; then
    output_dir="$(mktemp -d)"
    cleanup_output_dir="true"
  else
    mkdir -p "$output_dir"
    cleanup_output_dir="false"
  fi
  state_dir="${output_dir}/state"
  artifact_file="${output_dir}/summary.txt"
  events_file="${output_dir}/mvp-observability-events.jsonl"
  redaction_log="${output_dir}/redaction-proof.txt"
  mkdir -p "$state_dir"
  : >"$events_file"
  printf 'observability smoke artifact\ntrace_id=%s\nworkroom_wallet_authority=false\n' "$trace_id" >"$artifact_file"

  run_json "${output_dir}/node-init.json" \
    cargo run -q -p oa-node -- init \
      --org org.openagents.observability \
      --node-id "oa-node-observability-${trace_id}" \
      --signing-key-ref gcp-secret://oa-node-observability-signing-key \
      --state-dir "$state_dir/node"
  append_event "$events_file" "$trace_id" 1 node node.init "node-init.json"

  run_json "${output_dir}/node-health.json" \
    cargo run -q -p oa-node -- admin health append \
      --severity info \
      --code observability_smoke \
      --detail "trace ${trace_id} node health event" \
      --state-dir "$state_dir/node"
  append_event "$events_file" "$trace_id" 2 node node.health "node-health.json"

  run_json "${output_dir}/node-status.json" \
    cargo run -q -p oa-node -- status --state-dir "$state_dir/node"
  append_event "$events_file" "$trace_id" 3 node node.status "node-status.json"

  run_json "${output_dir}/workroom-metadata-init.json" \
    cargo run -q -p oa-workroomd -- metadata init \
      --workroom "workroom.observability.${trace_id}" \
      --program "program.observability.${trace_id}" \
      --repo OpenAgentsInc/openagents \
      --template template.observability \
      --budget bounded-observability \
      --deadline smoke \
      --trust-tier public_repo \
      --capability artifacts.write \
      --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 4 workroom workroom.metadata "workroom-metadata-init.json"

  run_json "${output_dir}/ingress-set.json" \
    cargo run -q -p oa-workroomd -- ingress set \
      --visibility collaborators \
      --preview-url "https://preview.example.invalid/${trace_id}" \
      --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 5 ingress ingress.set "ingress-set.json"

  run_json "${output_dir}/ingress-status.json" \
    cargo run -q -p oa-workroomd -- ingress status --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 6 ingress ingress.status "ingress-status.json"

  run_json "${output_dir}/artifact-policy.json" \
    cargo run -q -p oa-workroomd -- artifacts policy init --required summary --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 7 artifact artifact.policy "artifact-policy.json"

  run_json "${output_dir}/lifecycle-create.json" \
    cargo run -q -p oa-workroomd -- lifecycle create --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 8 receipt lifecycle.create "lifecycle-create.json"

  run_json "${output_dir}/lifecycle-start.json" \
    cargo run -q -p oa-workroomd -- lifecycle start --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 9 receipt lifecycle.start "lifecycle-start.json"

  run_json "${output_dir}/artifact-upload.json" \
    cargo run -q -p oa-workroomd -- artifacts upload --name summary --file "$artifact_file" --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 10 artifact artifact.upload "artifact-upload.json"

  run_json "${output_dir}/artifact-status.json" \
    cargo run -q -p oa-workroomd -- artifacts status --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 11 artifact artifact.status "artifact-status.json"

  run_json "${output_dir}/closeout-submit.json" \
    cargo run -q -p oa-workroomd -- closeout submit --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 12 receipt closeout.submit "closeout-submit.json"

  run_json "${output_dir}/lifecycle-closeout.json" \
    cargo run -q -p oa-workroomd -- lifecycle closeout --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 13 receipt lifecycle.closeout "lifecycle-closeout.json"

  run_json "${output_dir}/workroom-metadata-get.json" \
    cargo run -q -p oa-workroomd -- metadata get --state-dir "$state_dir/workroom"
  append_event "$events_file" "$trace_id" 14 workroom workroom.metadata_get "workroom-metadata-get.json"

  scripts/verify-redacted-config.sh >"$redaction_log"
  append_event "$events_file" "$trace_id" 15 node redaction.verified "redaction-proof.txt"

  for required in node workroom ingress artifact receipt; do
    grep -q "\"source\":\"${required}\"" "$events_file" || {
      echo "missing observability source: ${required}" >&2
      exit 1
    }
  done

  find "$output_dir" -type f -print0 |
    while IFS= read -r -d '' file; do
      assert_no_secret_markers "observability bundle" "$file"
    done

  if [[ "$cleanup_output_dir" == "true" ]]; then
    cat "$events_file"
    rm -rf "$output_dir"
  else
    echo "observability bundle: $output_dir"
    echo "trace_id: $trace_id"
  fi
}

gcp_script() {
  cat <<'REMOTE'
set -euo pipefail
echo "== openagents-oa-node journald =="
journalctl -u openagents-oa-node -n 80 --no-pager || true
echo "== workroom receipt tails =="
find /var/lib/openagents/workrooms -maxdepth 3 -type f \( -name '*receipts.jsonl' -o -name 'artifact-state.json' -o -name 'ingress-state.json' \) -print -exec tail -n 20 {} \; 2>/dev/null || true
REMOTE
}

run_gcp() {
  if [[ -z "$project_id" ]]; then
    echo "--project is required unless --local is used" >&2
    usage >&2
    exit 2
  fi
  local script_file logging_filter
  script_file="$(mktemp)"
  trap "rm -f '$script_file'" EXIT
  gcp_script >"$script_file"
  logging_filter='(textPayload:"oa_node_startup_complete" OR textPayload:"openagents-oa-node" OR labels.instance_name="'${node_name}'")'

  if [[ "$apply" == "true" ]]; then
    gcloud compute ssh "$node_name" \
      --project "$project_id" \
      --zone "$zone" \
      --tunnel-through-iap \
      --command "bash -s" <"$script_file"
    gcloud logging read "$logging_filter" \
      --project "$project_id" \
      --limit 50 \
      --format json
  else
    printf '+ gcloud compute ssh %q --project %q --zone %q --tunnel-through-iap --command %q < %q\n' \
      "$node_name" "$project_id" "$zone" "bash -s" "$script_file"
    sed -n '1,120p' "$script_file"
    printf '+ gcloud logging read %q --project %q --limit 50 --format json\n' "$logging_filter" "$project_id"
  fi
}

if [[ "$local_mode" == "true" ]]; then
  run_local
else
  run_gcp
fi
