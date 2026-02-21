#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SQL_FILE="${SCRIPT_DIR}/sql/db-role-isolation.sql"

log() {
  echo "[${SCRIPT_NAME}] $*"
}

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: required command not found: $command_name" >&2
    exit 1
  fi
}

require_command psql

DB_URL="${DB_URL:-${DATABASE_URL:-}}"
RUNTIME_OWNER_ROLE="${RUNTIME_OWNER_ROLE:-oa_runtime_owner}"
RUNTIME_RW_ROLE="${RUNTIME_RW_ROLE:-oa_runtime_rw}"
KHALA_RO_ROLE="${KHALA_RO_ROLE:-oa_khala_ro}"
CONTROL_RW_ROLE="${CONTROL_RW_ROLE:-oa_control_rw}"
DRY_RUN="${DRY_RUN:-0}"

if [[ -z "${DB_URL}" ]]; then
  echo "error: set DB_URL (or DATABASE_URL)" >&2
  exit 1
fi

if [[ ! -f "${SQL_FILE}" ]]; then
  echo "error: SQL file not found: ${SQL_FILE}" >&2
  exit 1
fi

PSQL_CMD=(
  psql
  "${DB_URL}"
  -v "runtime_owner_role=${RUNTIME_OWNER_ROLE}"
  -v "runtime_rw_role=${RUNTIME_RW_ROLE}"
  -v "khala_ro_role=${KHALA_RO_ROLE}"
  -v "control_rw_role=${CONTROL_RW_ROLE}"
  -f "${SQL_FILE}"
)

if [[ "${DRY_RUN}" == "1" ]]; then
  log "DRY_RUN=1; planned command:"
  printf '%q ' "${PSQL_CMD[@]}"
  echo
  exit 0
fi

log "Applying DB role isolation policy"
log "runtime_owner=${RUNTIME_OWNER_ROLE} runtime_rw=${RUNTIME_RW_ROLE} khala_ro=${KHALA_RO_ROLE} control_rw=${CONTROL_RW_ROLE}"
"${PSQL_CMD[@]}"
log "DB role isolation policy applied"
