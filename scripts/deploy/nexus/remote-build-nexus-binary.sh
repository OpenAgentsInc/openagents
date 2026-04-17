#!/usr/bin/env bash
set -euo pipefail

CONTEXT_TARBALL="${1:?usage: remote-build-nexus-binary.sh <context-tarball> <git-sha> <git-short-sha> <git-ref> <build-profile> [clear-caches] }"
GIT_SHA="${2:?usage: remote-build-nexus-binary.sh <context-tarball> <git-sha> <git-short-sha> <git-ref> <build-profile> [clear-caches] }"
GIT_SHORT_SHA="${3:?usage: remote-build-nexus-binary.sh <context-tarball> <git-sha> <git-short-sha> <git-ref> <build-profile> [clear-caches] }"
GIT_REF="${4:?usage: remote-build-nexus-binary.sh <context-tarball> <git-sha> <git-short-sha> <git-ref> <build-profile> [clear-caches] }"
BUILD_PROFILE="${5:?usage: remote-build-nexus-binary.sh <context-tarball> <git-sha> <git-short-sha> <git-ref> <build-profile> [clear-caches] }"
CLEAR_CACHES="${6:-false}"

CACHE_MOUNT_POINT="${NEXUS_BUILDER_CACHE_MOUNT_POINT:-/mnt/disks/nexus-builder-cache}"
BUILDER_USER="${NEXUS_BUILDER_USER:-nexus-builder}"
BUILDER_HOME="/home/${BUILDER_USER}"
CARGO_HOME="${CACHE_MOUNT_POINT}/cargo-home"
RUSTUP_HOME="${CACHE_MOUNT_POINT}/rustup-home"
CARGO_TARGET_DIR="${CACHE_MOUNT_POINT}/target"
SCCACHE_DIR="${CACHE_MOUNT_POINT}/sccache"
SOURCE_ROOT="${CACHE_MOUNT_POINT}/sources/${GIT_SHA}"
ARTIFACT_ROOT="${CACHE_MOUNT_POINT}/artifacts/${GIT_SHA}"
ARTIFACT_TMP_ROOT="${CACHE_MOUNT_POINT}/artifacts/.tmp-${GIT_SHA}-$$"
TIMINGS_DIR="${CACHE_MOUNT_POINT}/timings"
FINAL_BINARY_PATH="${ARTIFACT_ROOT}/nexus-relay"
FINAL_METADATA_PATH="${ARTIFACT_ROOT}/build-metadata.json"

log() {
  printf '[nexus-builder-build] %s\n' "$*" >&2
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    printf '[nexus-builder-build] ERROR: missing command: %s\n' "$cmd" >&2
    exit 1
  fi
}

require_cmd sudo
require_cmd python3
require_cmd sha256sum
require_cmd tar

[[ -r "$CONTEXT_TARBALL" ]] || {
  printf '[nexus-builder-build] ERROR: context tarball not readable: %s\n' "$CONTEXT_TARBALL" >&2
  exit 1
}

sudo install -d -o "$BUILDER_USER" -g "$BUILDER_USER" -m 0755 \
  "$CARGO_HOME" \
  "$RUSTUP_HOME" \
  "$CARGO_TARGET_DIR" \
  "$SCCACHE_DIR" \
  "$TIMINGS_DIR"

if [[ "$CLEAR_CACHES" == "true" ]]; then
  log "Clearing persistent caches before build"
  sudo rm -rf "${CARGO_HOME}"/* "${CARGO_TARGET_DIR}"/* "${SCCACHE_DIR}"/*
fi

sudo rm -rf "$SOURCE_ROOT" "$ARTIFACT_TMP_ROOT"
sudo install -d -o "$BUILDER_USER" -g "$BUILDER_USER" -m 0755 "$SOURCE_ROOT" "$ARTIFACT_TMP_ROOT"

SOURCE_ARCHIVE_SHA256="$(sha256sum "$CONTEXT_TARBALL" | awk '{print $1}')"
SOURCE_ARCHIVE_SIZE_BYTES="$(stat -c '%s' "$CONTEXT_TARBALL")"

sudo -u "$BUILDER_USER" tar -xzf "$CONTEXT_TARBALL" -C "$SOURCE_ROOT"

BUILD_STARTED_UNIX_MS="$(date +%s%3N)"
sudo -u "$BUILDER_USER" bash -lc "
  set -euo pipefail
  export HOME='${BUILDER_HOME}'
  export PATH='${CARGO_HOME}/bin':\$PATH
  export CARGO_HOME='${CARGO_HOME}'
  export RUSTUP_HOME='${RUSTUP_HOME}'
  export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}'
  export SCCACHE_DIR='${SCCACHE_DIR}'
  export RUSTC_WRAPPER='/usr/local/bin/sccache'
  cd '${SOURCE_ROOT}'

  fetch_started_ms=\$(date +%s%3N)
  cargo fetch --locked >/dev/null
  fetch_finished_ms=\$(date +%s%3N)
  build_started_ms=\$(date +%s%3N)
  cargo build --locked --profile '${BUILD_PROFILE}' -p nexus-relay >/dev/null
  build_finished_ms=\$(date +%s%3N)

  printf '%s\n' \"\$fetch_started_ms \$fetch_finished_ms \$build_started_ms \$build_finished_ms\" > '${ARTIFACT_TMP_ROOT}/timing.raw'
  cp '${CARGO_TARGET_DIR}/${BUILD_PROFILE}/nexus-relay' '${ARTIFACT_TMP_ROOT}/nexus-relay'
  sha256sum '${ARTIFACT_TMP_ROOT}/nexus-relay' | awk '{print \$1}' > '${ARTIFACT_TMP_ROOT}/nexus-relay.sha256'
  rustc --version > '${ARTIFACT_TMP_ROOT}/rustc-version.txt'
  cargo --version > '${ARTIFACT_TMP_ROOT}/cargo-version.txt'
  sccache --version > '${ARTIFACT_TMP_ROOT}/sccache-version.txt'
  sccache --show-stats > '${ARTIFACT_TMP_ROOT}/sccache-stats.txt' || true
"
BUILD_FINISHED_UNIX_MS="$(date +%s%3N)"

read -r FETCH_STARTED_MS FETCH_FINISHED_MS BUILD_COMPILE_STARTED_MS BUILD_COMPILE_FINISHED_MS <"${ARTIFACT_TMP_ROOT}/timing.raw"
BINARY_SHA256="$(tr -d '[:space:]' <"${ARTIFACT_TMP_ROOT}/nexus-relay.sha256")"
BINARY_SIZE_BYTES="$(stat -c '%s' "${ARTIFACT_TMP_ROOT}/nexus-relay")"
HOSTNAME_VALUE="$(hostname)"
TIMING_STAMP="$(date -u +%Y%m%d-%H%M%S)"
TIMING_RECEIPT_PATH="${TIMINGS_DIR}/${TIMING_STAMP}-${GIT_SHORT_SHA}.json"

sudo rm -rf "$ARTIFACT_ROOT"
sudo mv "$ARTIFACT_TMP_ROOT" "$ARTIFACT_ROOT"

if [[ "$CLEAR_CACHES" == "true" ]]; then
  CLEAR_CACHES_JSON="True"
else
  CLEAR_CACHES_JSON="False"
fi

sudo -u "$BUILDER_USER" python3 - "$FINAL_METADATA_PATH" "$TIMING_RECEIPT_PATH" <<PY
import json
from pathlib import Path

metadata_path = Path("${FINAL_METADATA_PATH}")
timing_receipt_path = Path("${TIMING_RECEIPT_PATH}")
payload = {
    "built_at": "${TIMING_STAMP}",
    "builder_hostname": "${HOSTNAME_VALUE}",
    "builder_user": "${BUILDER_USER}",
    "git_sha": "${GIT_SHA}",
    "git_short_sha": "${GIT_SHORT_SHA}",
    "git_ref": "${GIT_REF}",
    "build_profile": "${BUILD_PROFILE}",
    "clear_caches": ${CLEAR_CACHES_JSON},
    "source_archive_sha256": "${SOURCE_ARCHIVE_SHA256}",
    "source_archive_size_bytes": int("${SOURCE_ARCHIVE_SIZE_BYTES}"),
    "source_root": "${SOURCE_ROOT}",
    "artifact_root": "${ARTIFACT_ROOT}",
    "binary_path": "${FINAL_BINARY_PATH}",
    "binary_sha256": "${BINARY_SHA256}",
    "binary_size_bytes": int("${BINARY_SIZE_BYTES}"),
    "timing": {
        "build_started_unix_ms": int("${BUILD_STARTED_UNIX_MS}"),
        "build_finished_unix_ms": int("${BUILD_FINISHED_UNIX_MS}"),
        "total_duration_ms": int("${BUILD_FINISHED_UNIX_MS}") - int("${BUILD_STARTED_UNIX_MS}"),
        "fetch_started_unix_ms": int("${FETCH_STARTED_MS}"),
        "fetch_finished_unix_ms": int("${FETCH_FINISHED_MS}"),
        "fetch_duration_ms": int("${FETCH_FINISHED_MS}") - int("${FETCH_STARTED_MS}"),
        "compile_started_unix_ms": int("${BUILD_COMPILE_STARTED_MS}"),
        "compile_finished_unix_ms": int("${BUILD_COMPILE_FINISHED_MS}"),
        "compile_duration_ms": int("${BUILD_COMPILE_FINISHED_MS}") - int("${BUILD_COMPILE_STARTED_MS}"),
    },
    "cache_layout": {
        "cargo_home": "${CARGO_HOME}",
        "rustup_home": "${RUSTUP_HOME}",
        "cargo_target_dir": "${CARGO_TARGET_DIR}",
        "sccache_dir": "${SCCACHE_DIR}",
    },
    "tool_versions": {
        "rustc": Path("${ARTIFACT_ROOT}/rustc-version.txt").read_text().strip(),
        "cargo": Path("${ARTIFACT_ROOT}/cargo-version.txt").read_text().strip(),
        "sccache": Path("${ARTIFACT_ROOT}/sccache-version.txt").read_text().strip(),
    },
}
metadata_path.write_text(json.dumps(payload, indent=2) + "\n")
timing_receipt_path.write_text(json.dumps(payload, indent=2) + "\n")
PY

sudo chown "$BUILDER_USER:$BUILDER_USER" "$FINAL_METADATA_PATH" "$TIMING_RECEIPT_PATH"
sudo cat "$FINAL_METADATA_PATH"
