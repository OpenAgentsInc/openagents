#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
SERVICE_DIR="${ROOT_DIR}/apps/openagents.com"
WORK_DIR="${WORK_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-store-migrate}"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
BACKUP_DIR="${WORK_DIR}/backups/${TIMESTAMP}"
MANIFEST_PATH="${WORK_DIR}/manifests/${TIMESTAMP}.json"

AUTH_STORE_PATH="${AUTH_STORE_PATH:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-auth-store.json}"
CODEX_THREAD_STORE_PATH="${CODEX_THREAD_STORE_PATH:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-codex-thread-store.json}"
DOMAIN_STORE_PATH="${DOMAIN_STORE_PATH:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-domain-store.json}"

mkdir -p "${BACKUP_DIR}" "$(dirname "${MANIFEST_PATH}")"

cargo run \
  --manifest-path "${SERVICE_DIR}/Cargo.toml" \
  --bin rust_store_migrate \
  -- \
  --auth-store "${AUTH_STORE_PATH}" \
  --codex-thread-store "${CODEX_THREAD_STORE_PATH}" \
  --domain-store "${DOMAIN_STORE_PATH}" \
  --backup-dir "${BACKUP_DIR}" \
  --manifest "${MANIFEST_PATH}"

echo "[rust-store-backfill] manifest: ${MANIFEST_PATH}"
echo "[rust-store-backfill] backups: ${BACKUP_DIR}"
