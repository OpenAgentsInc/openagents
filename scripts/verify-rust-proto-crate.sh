#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${OA_PROTO_VERIFY_MODE:-fast}"
TMP_DIR="$(mktemp -d)"
BUF_OUT_DIR="${ROOT_DIR}/target/buf/rust"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

SNAPSHOT_A="${TMP_DIR}/openagents-a.rs"
SNAPSHOT_B="${TMP_DIR}/openagents-b.rs"
TARGET_A="${TMP_DIR}/target-a"
TARGET_B="${TMP_DIR}/target-b"
BUF_SNAPSHOT_A="${TMP_DIR}/buf-rust-a.sha256"
BUF_SNAPSHOT_B="${TMP_DIR}/buf-rust-b.sha256"

require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}" >&2
    exit 1
  fi
}

generate_buf_rust() {
  rm -rf "${BUF_OUT_DIR}"
  (
    cd "${ROOT_DIR}"
    buf generate --template buf.gen.yaml
  )
}

assert_buf_rust_output() {
  if [[ ! -d "${BUF_OUT_DIR}" ]]; then
    echo "rust proto generation verification failed: missing Buf Rust output directory (${BUF_OUT_DIR})" >&2
    exit 1
  fi

  local first_rs
  first_rs="$(find "${BUF_OUT_DIR}" -type f -name '*.rs' | head -n 1 || true)"
  if [[ -z "${first_rs}" ]]; then
    echo "rust proto generation verification failed: Buf Rust output is empty (${BUF_OUT_DIR})" >&2
    exit 1
  fi
}

snapshot_buf_rust_output() {
  local snapshot_path="$1"
  (
    cd "${BUF_OUT_DIR}"
    find . -type f -name '*.rs' | LC_ALL=C sort | while IFS= read -r rel; do
      shasum -a 256 "${rel}"
    done
  ) >"${snapshot_path}"
}

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

require_cmd buf
require_cmd cargo

if [[ "${MODE}" == "strict" ]]; then
  generate_buf_rust
  assert_buf_rust_output
  snapshot_buf_rust_output "${BUF_SNAPSHOT_A}"

  generate_buf_rust
  assert_buf_rust_output
  snapshot_buf_rust_output "${BUF_SNAPSHOT_B}"

  if ! cmp -s "${BUF_SNAPSHOT_A}" "${BUF_SNAPSHOT_B}"; then
    echo "rust proto generation verification failed: Buf Rust output is non-deterministic" >&2
    exit 1
  fi

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
  generate_buf_rust
  assert_buf_rust_output
  snapshot_buf_rust_output "${BUF_SNAPSHOT_A}"

  # Fast mode intentionally reuses the standard cargo target cache.
  build_once "${SNAPSHOT_A}"
  if [[ ! -s "${SNAPSHOT_A}" ]]; then
    echo "rust proto generation verification failed: missing generated snapshot" >&2
    exit 1
  fi
fi

snapshot_hash="$(shasum -a 256 "${SNAPSHOT_A}" | awk '{print $1}')"
buf_snapshot_hash="$(shasum -a 256 "${BUF_SNAPSHOT_A}" | awk '{print $1}')"

(
  cd "${ROOT_DIR}"
  cargo test --manifest-path crates/openagents-proto/Cargo.toml --quiet
)

echo "rust proto generation verification passed (mode=${MODE}, crate_snapshot_sha256=${snapshot_hash}, buf_snapshot_sha256=${buf_snapshot_hash})"
