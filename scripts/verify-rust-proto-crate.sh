#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

SNAPSHOT_A="${TMP_DIR}/openagents-a.rs"
SNAPSHOT_B="${TMP_DIR}/openagents-b.rs"
TARGET_A="${TMP_DIR}/target-a"
TARGET_B="${TMP_DIR}/target-b"

(
  cd "${ROOT_DIR}"

  OA_PROTO_SNAPSHOT_PATH="${SNAPSHOT_A}" \
  CARGO_TARGET_DIR="${TARGET_A}" \
  cargo build --manifest-path crates/openagents-proto/Cargo.toml --quiet

  OA_PROTO_SNAPSHOT_PATH="${SNAPSHOT_B}" \
  CARGO_TARGET_DIR="${TARGET_B}" \
  cargo build --manifest-path crates/openagents-proto/Cargo.toml --quiet
)

if [[ ! -s "${SNAPSHOT_A}" || ! -s "${SNAPSHOT_B}" ]]; then
  echo "rust proto generation verification failed: missing generated snapshots" >&2
  exit 1
fi

if ! cmp -s "${SNAPSHOT_A}" "${SNAPSHOT_B}"; then
  echo "rust proto generation verification failed: generated output is non-deterministic" >&2
  exit 1
fi

snapshot_hash="$(shasum -a 256 "${SNAPSHOT_A}" | awk '{print $1}')"
echo "rust proto generation verification passed (snapshot_sha256=${snapshot_hash})"
