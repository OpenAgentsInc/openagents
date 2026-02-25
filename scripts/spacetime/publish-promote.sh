#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODULE_PATH_DEFAULT="$ROOT_DIR/spacetime/modules/autopilot-sync/spacetimedb"
OUTPUT_ROOT_DEFAULT="$ROOT_DIR/output/spacetime/publish"

required_cmds=(spacetime jq git)
for cmd in "${required_cmds[@]}"; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "missing required command: $cmd" >&2
    exit 1
  fi
done

usage() {
  cat <<'USAGE'
Usage:
  scripts/spacetime/publish-promote.sh publish --env <dev|staging|prod> [--module-path <path>] [--output-dir <dir>] [--server <server>]
  scripts/spacetime/publish-promote.sh promote --from-env <dev|staging|prod> --to-env <dev|staging|prod> [--module-path <path>] [--output-dir <dir>] [--server <server>] [--allow-target-drift]

Required env for each environment:
  OA_SPACETIME_<ENV>_DATABASE

Optional env:
  OA_SPACETIME_<ENV>_SERVER (defaults to maincloud)

Examples:
  scripts/spacetime/publish-promote.sh publish --env dev
  scripts/spacetime/publish-promote.sh promote --from-env staging --to-env prod
USAGE
}

normalize_env() {
  local value="$1"
  printf '%s' "$value" | tr '[:lower:]' '[:upper:]'
}

env_database() {
  local env_upper="$1"
  local key="OA_SPACETIME_${env_upper}_DATABASE"
  local value="${!key:-}"
  if [[ -z "$value" ]]; then
    echo "missing required env: ${key}" >&2
    exit 1
  fi
  printf '%s' "$value"
}

env_server() {
  local env_upper="$1"
  local key="OA_SPACETIME_${env_upper}_SERVER"
  local value="${!key:-maincloud}"
  printf '%s' "$value"
}

verify_schema_contract() {
  local schema_json="$1"

  local required_tables=(
    active_connection
    nostr_presence_claim
    stream_head
    sync_event
    stream_checkpoint
  )

  local required_reducers=(
    init
    client_connected
    client_disconnected
    heartbeat
    request_nostr_presence_challenge
    bind_nostr_presence_identity
    append_sync_event
    ack_stream_checkpoint
  )

  for table in "${required_tables[@]}"; do
    if ! jq -e --arg table "$table" '.tables[]? | select(.name == $table)' "$schema_json" >/dev/null; then
      echo "schema verification failed: missing table '$table'" >&2
      return 1
    fi
  done

  for reducer in "${required_reducers[@]}"; do
    if ! jq -e --arg reducer "$reducer" '.reducers[]? | select(.name == $reducer)' "$schema_json" >/dev/null; then
      echo "schema verification failed: missing reducer '$reducer'" >&2
      return 1
    fi
  done

  return 0
}

schema_hash() {
  local schema_json="$1"
  jq -c '{tables: [.tables[]?.name] | sort, reducers: [.reducers[]?.name] | sort}' "$schema_json" | shasum -a 256 | awk '{print $1}'
}

publish_env() {
  local env_name="$1"
  local module_path="$2"
  local output_dir="$3"
  local server_override="${4:-}"

  local env_upper
  env_upper="$(normalize_env "$env_name")"
  local database
  database="$(env_database "$env_upper")"
  local server
  server="${server_override:-$(env_server "$env_upper")}" 

  if [[ ! -d "$module_path" ]]; then
    echo "module path does not exist: $module_path" >&2
    exit 1
  fi

  mkdir -p "$output_dir"

  local git_sha
  git_sha="$(git -C "$ROOT_DIR" rev-parse --short=12 HEAD)"
  local published_at
  published_at="$(date -u +"%Y%m%dT%H%M%SZ")"
  local version_tag
  version_tag="${git_sha}-${published_at}"

  local pre_schema="$output_dir/pre-schema.json"
  local post_schema="$output_dir/post-schema.json"
  local publish_log="$output_dir/publish.log"
  local report_json="$output_dir/report.json"

  if ! spacetime describe "$database" --server "$server" --json >"$pre_schema" 2>/dev/null; then
    echo '{}' >"$pre_schema"
  fi

  {
    echo "publishing module"
    echo "  env: $env_upper"
    echo "  server: $server"
    echo "  database: $database"
    echo "  module_path: $module_path"
    echo "  version_tag: $version_tag"
  } | tee "$publish_log"

  spacetime publish "$database" --server "$server" --module-path "$module_path" -y | tee -a "$publish_log"

  spacetime describe "$database" --server "$server" --json >"$post_schema"
  verify_schema_contract "$post_schema"

  local pre_hash
  pre_hash="$(schema_hash "$pre_schema")"
  local post_hash
  post_hash="$(schema_hash "$post_schema")"

  cat >"$report_json" <<JSON
{
  "environment": "$env_upper",
  "server": "$server",
  "database": "$database",
  "module_path": "$module_path",
  "version_tag": "$version_tag",
  "published_at_utc": "$published_at",
  "schema_hash_before": "$pre_hash",
  "schema_hash_after": "$post_hash",
  "rollback_target_schema_hash": "$pre_hash"
}
JSON

  echo "publish report: $report_json"
}

promote_env() {
  local from_env="$1"
  local to_env="$2"
  local module_path="$3"
  local output_dir="$4"
  local server_override="${5:-}"
  local allow_target_drift="$6"

  local from_upper
  from_upper="$(normalize_env "$from_env")"
  local to_upper
  to_upper="$(normalize_env "$to_env")"

  local from_database
  from_database="$(env_database "$from_upper")"
  local to_database
  to_database="$(env_database "$to_upper")"

  local from_server
  from_server="${server_override:-$(env_server "$from_upper")}" 
  local to_server
  to_server="${server_override:-$(env_server "$to_upper")}" 

  mkdir -p "$output_dir"

  local source_schema="$output_dir/source-schema.json"
  local target_pre_schema="$output_dir/target-pre-schema.json"
  spacetime describe "$from_database" --server "$from_server" --json >"$source_schema"
  if ! spacetime describe "$to_database" --server "$to_server" --json >"$target_pre_schema" 2>/dev/null; then
    echo '{}' >"$target_pre_schema"
  fi

  verify_schema_contract "$source_schema"

  local source_hash
  source_hash="$(schema_hash "$source_schema")"
  local target_pre_hash
  target_pre_hash="$(schema_hash "$target_pre_schema")"

  if [[ "$allow_target_drift" != "true" && "$target_pre_hash" != "$source_hash" && "$target_pre_hash" != "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356" ]]; then
    echo "target schema drift detected before promote (source=$source_hash target=$target_pre_hash). rerun with --allow-target-drift if intentional." >&2
    exit 1
  fi

  publish_env "$to_env" "$module_path" "$output_dir/publish" "$to_server"

  local target_post_schema="$output_dir/target-post-schema.json"
  spacetime describe "$to_database" --server "$to_server" --json >"$target_post_schema"
  verify_schema_contract "$target_post_schema"

  local target_post_hash
  target_post_hash="$(schema_hash "$target_post_schema")"

  cat >"$output_dir/promote-report.json" <<JSON
{
  "from_environment": "$from_upper",
  "to_environment": "$to_upper",
  "source_database": "$from_database",
  "target_database": "$to_database",
  "source_schema_hash": "$source_hash",
  "target_schema_hash_before": "$target_pre_hash",
  "target_schema_hash_after": "$target_post_hash",
  "allow_target_drift": $allow_target_drift
}
JSON

  echo "promote report: $output_dir/promote-report.json"
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

mode="$1"
shift

module_path="$MODULE_PATH_DEFAULT"
output_dir="$OUTPUT_ROOT_DEFAULT/$(date -u +"%Y%m%dT%H%M%SZ")"
server_override=""

case "$mode" in
  publish)
    env_name=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --env)
          env_name="$2"
          shift 2
          ;;
        --module-path)
          module_path="$2"
          shift 2
          ;;
        --output-dir)
          output_dir="$2"
          shift 2
          ;;
        --server)
          server_override="$2"
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

    if [[ -z "$env_name" ]]; then
      echo "publish requires --env" >&2
      exit 2
    fi

    publish_env "$env_name" "$module_path" "$output_dir" "$server_override"
    ;;
  promote)
    from_env=""
    to_env=""
    allow_target_drift="false"

    while [[ $# -gt 0 ]]; do
      case "$1" in
        --from-env)
          from_env="$2"
          shift 2
          ;;
        --to-env)
          to_env="$2"
          shift 2
          ;;
        --module-path)
          module_path="$2"
          shift 2
          ;;
        --output-dir)
          output_dir="$2"
          shift 2
          ;;
        --server)
          server_override="$2"
          shift 2
          ;;
        --allow-target-drift)
          allow_target_drift="true"
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

    if [[ -z "$from_env" || -z "$to_env" ]]; then
      echo "promote requires --from-env and --to-env" >&2
      exit 2
    fi

    promote_env "$from_env" "$to_env" "$module_path" "$output_dir" "$server_override" "$allow_target_drift"
    ;;
  -h|--help)
    usage
    ;;
  *)
    echo "unknown mode: $mode" >&2
    usage >&2
    exit 2
    ;;
esac
