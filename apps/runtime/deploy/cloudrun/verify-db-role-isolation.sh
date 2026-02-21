#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"

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

query_value() {
  local sql="$1"
  psql "${DB_URL}" -Atqc "$sql"
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  if [[ "${expected}" != "${actual}" ]]; then
    echo "error: ${message} (expected=${expected}, actual=${actual})" >&2
    exit 1
  fi
}

assert_leq() {
  local actual="$1"
  local max="$2"
  local message="$3"
  if (( actual > max )); then
    echo "error: ${message} (actual=${actual}, max=${max})" >&2
    exit 1
  fi
}

require_command psql

DB_URL="${DB_URL:-${DATABASE_URL:-}}"
RUNTIME_OWNER_ROLE="${RUNTIME_OWNER_ROLE:-oa_runtime_owner}"
RUNTIME_RW_ROLE="${RUNTIME_RW_ROLE:-oa_runtime_rw}"
KHALA_RO_ROLE="${KHALA_RO_ROLE:-oa_khala_ro}"
CONTROL_RW_ROLE="${CONTROL_RW_ROLE:-oa_control_rw}"

if [[ -z "${DB_URL}" ]]; then
  echo "error: set DB_URL (or DATABASE_URL)" >&2
  exit 1
fi

ROLE_COUNT="$(query_value "SELECT count(*) FROM pg_roles WHERE rolname IN ('${RUNTIME_OWNER_ROLE}','${RUNTIME_RW_ROLE}','${KHALA_RO_ROLE}','${CONTROL_RW_ROLE}');")"
assert_eq "4" "${ROLE_COUNT}" "required authority-plane roles must exist"

SCHEMA_OWNER="$(query_value "SELECT nspowner::regrole::text FROM pg_namespace WHERE nspname = 'runtime';")"
assert_eq "${RUNTIME_OWNER_ROLE}" "${SCHEMA_OWNER}" "runtime schema owner must match runtime owner role"

CONTROL_RUNTIME_WRITES="$(query_value "SELECT count(*) FROM information_schema.role_table_grants WHERE grantee = '${CONTROL_RW_ROLE}' AND table_schema = 'runtime' AND privilege_type IN ('INSERT','UPDATE','DELETE','TRUNCATE','REFERENCES','TRIGGER');")"
assert_eq "0" "${CONTROL_RUNTIME_WRITES}" "control role must not have runtime write privileges"

KHALA_RUNTIME_WRITES="$(query_value "SELECT count(*) FROM information_schema.role_table_grants WHERE grantee = '${KHALA_RO_ROLE}' AND table_schema = 'runtime' AND privilege_type IN ('INSERT','UPDATE','DELETE','TRUNCATE','REFERENCES','TRIGGER');")"
assert_eq "0" "${KHALA_RUNTIME_WRITES}" "khala role must remain read-only on runtime schema"

RUNTIME_CONTROL_WRITES=0
CONTROL_SCHEMA_EXISTS="$(query_value "SELECT count(*) FROM pg_namespace WHERE nspname = 'control';")"
if [[ "${CONTROL_SCHEMA_EXISTS}" == "1" ]]; then
  RUNTIME_CONTROL_WRITES="$(query_value "SELECT count(*) FROM information_schema.role_table_grants WHERE grantee = '${RUNTIME_RW_ROLE}' AND table_schema = 'control' AND privilege_type IN ('INSERT','UPDATE','DELETE','TRUNCATE','REFERENCES','TRIGGER');")"
  assert_eq "0" "${RUNTIME_CONTROL_WRITES}" "runtime role must not have control schema write privileges"
fi

SYNC_TABLES_PRESENT="$(query_value "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'runtime' AND table_name IN ('sync_stream_events','sync_topic_sequences');")"
if [[ "${SYNC_TABLES_PRESENT}" -gt 0 ]]; then
  KHALA_SYNC_SELECT="$(query_value "SELECT count(*) FROM information_schema.role_table_grants WHERE grantee = '${KHALA_RO_ROLE}' AND table_schema = 'runtime' AND table_name IN ('sync_stream_events','sync_topic_sequences') AND privilege_type = 'SELECT';")"
  assert_leq "${SYNC_TABLES_PRESENT}" "${KHALA_SYNC_SELECT}" "khala role must have SELECT on runtime sync tables"
fi

log "DB role isolation verification passed"
log "runtime_owner=${RUNTIME_OWNER_ROLE} runtime_rw=${RUNTIME_RW_ROLE} khala_ro=${KHALA_RO_ROLE} control_rw=${CONTROL_RW_ROLE}"
