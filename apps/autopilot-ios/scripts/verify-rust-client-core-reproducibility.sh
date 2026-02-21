#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BUILD_SCRIPT="${SCRIPT_DIR}/build-rust-client-core.sh"

require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}" >&2
    exit 1
  fi
}

require_cmd mktemp
require_cmd diff

if [[ ! -x "${BUILD_SCRIPT}" ]]; then
  echo "Build script is not executable: ${BUILD_SCRIPT}" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d "${ROOT_DIR}/target/ios-rust-repro-XXXXXX")"
trap 'rm -r "${WORK_DIR}"' EXIT

RUN1_OUT="${WORK_DIR}/run1/out"
RUN2_OUT="${WORK_DIR}/run2/out"
RUN1_BUILD="${WORK_DIR}/run1/build"
RUN2_BUILD="${WORK_DIR}/run2/build"

"${BUILD_SCRIPT}" \
  --output-root "${RUN1_OUT}" \
  --build-root "${RUN1_BUILD}" \
  --clean \
  --no-current-symlink

"${BUILD_SCRIPT}" \
  --output-root "${RUN2_OUT}" \
  --build-root "${RUN2_BUILD}" \
  --clean \
  --no-current-symlink

RUN1_VERSION="$(cat "${RUN1_OUT}/LATEST_VERSION")"
RUN2_VERSION="$(cat "${RUN2_OUT}/LATEST_VERSION")"

if [[ "${RUN1_VERSION}" != "${RUN2_VERSION}" ]]; then
  echo "Reproducibility failure: artifact versions differ (${RUN1_VERSION} vs ${RUN2_VERSION})." >&2
  exit 1
fi

RUN1_DIR="${RUN1_OUT}/${RUN1_VERSION}"
RUN2_DIR="${RUN2_OUT}/${RUN2_VERSION}"

diff -u "${RUN1_DIR}/manifest.json" "${RUN2_DIR}/manifest.json" >/dev/null
diff -u "${RUN1_DIR}/manifest.sha256" "${RUN2_DIR}/manifest.sha256" >/dev/null
diff -rq "${RUN1_DIR}" "${RUN2_DIR}" >/dev/null

echo "Reproducibility verified for ${RUN1_VERSION}"
