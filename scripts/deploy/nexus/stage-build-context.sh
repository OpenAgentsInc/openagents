#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd python3
require_cmd rsync
require_cmd cargo

BUILD_PLAN_HELPER="${SCRIPT_DIR}/materialize-build-plan.py"

CONTEXT_DIR="${1:-$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX")}"
NEXUS_LOCKFILE_PATH="${ROOT_DIR}/apps/nexus-relay/deploy/Cargo.nexus.lock"
REAL_WORKSPACE_PATHS=(
  "apps/nexus-control"
  "apps/nexus-relay"
  "crates/openagents-kernel-core"
  "crates/openagents-kernel-proto"
  "crates/openagents-provider-substrate"
  "crates/psionic-train-contract"
  "crates/openagents-validator-service"
  "crates/nostr/client"
  "crates/nostr/core"
  "third_party/nostr-rs-relay"
  "third_party/rusqlite-0.31.0-libsqlite3-0.30.1"
)

[[ -f "$BUILD_PLAN_HELPER" ]] || die "Missing build-plan helper: ${BUILD_PLAN_HELPER}"

mkdir -p "$CONTEXT_DIR"
find "$CONTEXT_DIR" -mindepth 1 -maxdepth 1 -exec rm -rf {} +

cp "$ROOT_DIR/Cargo.toml" "$CONTEXT_DIR/Cargo.toml"
if [[ -f "$NEXUS_LOCKFILE_PATH" ]]; then
  cp "$NEXUS_LOCKFILE_PATH" "$CONTEXT_DIR/Cargo.lock"
else
  cp "$ROOT_DIR/Cargo.lock" "$CONTEXT_DIR/Cargo.lock"
fi

COPY_PATHS=(
  ".cargo"
  ".dockerignore"
  ".gcloudignore"
  "proto"
  "scripts/dev/protocw"
  "apps/nexus-control"
  "apps/nexus-relay"
  "crates/openagents-kernel-core"
  "crates/openagents-kernel-proto"
  "crates/openagents-provider-substrate"
  "crates/psionic-train-contract"
  "crates/openagents-validator-service"
  "crates/nostr/client"
  "crates/nostr/core"
  "third_party/nostr-rs-relay"
  "third_party/rusqlite-0.31.0-libsqlite3-0.30.1"
)

for relative_path in "${COPY_PATHS[@]}"; do
  if [[ -e "${ROOT_DIR}/${relative_path}" ]]; then
    destination_dir="${CONTEXT_DIR}/$(dirname "${relative_path}")"
    mkdir -p "${destination_dir}"
    rsync -a "${ROOT_DIR}/${relative_path}" "${destination_dir}/"
  fi
done

python3 "$BUILD_PLAN_HELPER" "$ROOT_DIR" "$CONTEXT_DIR" "${REAL_WORKSPACE_PATHS[@]}"

printf '%s\n' "$CONTEXT_DIR"
