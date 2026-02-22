#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
SERVICE_DIR="${ROOT_DIR}/apps/openagents.com/service"
FIXTURE_PATH="${FIXTURE_PATH:-${ROOT_DIR}/apps/openagents.com/docs/parity-fixtures/baseline/shared-seed-state.json}"

AUTH_STORE_PATH="${AUTH_STORE_PATH:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-auth-store.json}"
CODEX_THREAD_STORE_PATH="${CODEX_THREAD_STORE_PATH:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-codex-thread-store.json}"
DOMAIN_STORE_PATH="${DOMAIN_STORE_PATH:-${ROOT_DIR}/apps/openagents.com/storage/app/rust-domain-store.json}"

MANIFEST_DIR="${MANIFEST_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/parity-fixtures/seed-manifests}"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
MANIFEST_PATH="${MANIFEST_DIR}/rust-seed-${TIMESTAMP}.json"

mkdir -p "${MANIFEST_DIR}"

cargo run \
  --manifest-path "${SERVICE_DIR}/Cargo.toml" \
  --bin seed_parity_fixtures \
  -- \
  --fixture "${FIXTURE_PATH}" \
  --auth-store "${AUTH_STORE_PATH}" \
  --codex-thread-store "${CODEX_THREAD_STORE_PATH}" \
  --domain-store "${DOMAIN_STORE_PATH}" \
  --manifest "${MANIFEST_PATH}"

echo "[parity-seed] rust manifest: ${MANIFEST_PATH}"
