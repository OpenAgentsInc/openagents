#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${OA_PROTO_VERIFY_MODE:-fast}"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

SNAPSHOT_A="${TMP_DIR}/openagents-a.rs"
SNAPSHOT_B="${TMP_DIR}/openagents-b.rs"
TARGET_A="${TMP_DIR}/target-a"
TARGET_B="${TMP_DIR}/target-b"

build_once() {
  local snapshot_path="$1"
  local target_dir="${2:-}"
  (
    cd "${ROOT_DIR}"
    if [[ -n "${target_dir}" ]]; then
      OA_PROTO_SNAPSHOT_PATH="${snapshot_path}" \
        CARGO_TARGET_DIR="${target_dir}" \
        cargo build --manifest-path crates/openagents-proto/Cargo.toml --quiet
    else
      OA_PROTO_SNAPSHOT_PATH="${snapshot_path}" \
        cargo build --manifest-path crates/openagents-proto/Cargo.toml --quiet
    fi
  )
}

if [[ "${MODE}" == "strict" ]]; then
  build_once "${SNAPSHOT_A}" "${TARGET_A}"
  build_once "${SNAPSHOT_B}" "${TARGET_B}"

  if [[ ! -s "${SNAPSHOT_A}" || ! -s "${SNAPSHOT_B}" ]]; then
    echo "rust proto generation verification failed: missing generated snapshots" >&2
    exit 1
  fi

  if ! cmp -s "${SNAPSHOT_A}" "${SNAPSHOT_B}"; then
    echo "rust proto generation verification failed: generated output is non-deterministic" >&2
    exit 1
  fi
else
  # Fast mode intentionally reuses the standard cargo target cache.
  build_once "${SNAPSHOT_A}"
  if [[ ! -s "${SNAPSHOT_A}" ]]; then
    echo "rust proto generation verification failed: missing generated snapshot" >&2
    exit 1
  fi
fi

snapshot_hash="$(shasum -a 256 "${SNAPSHOT_A}" | awk '{print $1}')"

(
  cd "${ROOT_DIR}"
  cargo test --manifest-path crates/openagents-proto/Cargo.toml --quiet
)

echo "rust proto generation verification passed (mode=${MODE}, snapshot_sha256=${snapshot_hash})"
