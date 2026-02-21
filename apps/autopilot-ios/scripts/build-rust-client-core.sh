#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

CRATE_NAME="openagents-client-core"
LIB_BASENAME="openagents_client_core"
IOS_TARGET="aarch64-apple-ios"
SIM_ARM_TARGET="aarch64-apple-ios-sim"
SIM_X86_TARGET="x86_64-apple-ios"
FFI_CONTRACT_VERSION="1"
SWIFT_BRIDGE_FILE="${ROOT_DIR}/apps/autopilot-ios/Autopilot/Autopilot/RustClientCoreBridge.swift"

REQUIRED_SYMBOLS=(
  "oa_client_core_ffi_contract_version"
  "oa_client_core_normalize_email"
  "oa_client_core_normalize_verification_code"
  "oa_client_core_normalize_message_text"
  "oa_client_core_extract_desktop_handshake_ack_id"
  "oa_client_core_parse_khala_frame"
  "oa_client_core_free_string"
)

OUTPUT_ROOT="${ROOT_DIR}/apps/autopilot-ios/Autopilot/RustCore"
BUILD_ROOT="${ROOT_DIR}/target/ios-rust-client-core"
ARTIFACT_VERSION=""
SKIP_RUSTUP_TARGET_ADD=0
CLEAN=0
WRITE_CURRENT_LINK=1

usage() {
  cat <<'USAGE'
Usage: apps/autopilot-ios/scripts/build-rust-client-core.sh [options]

Build deterministic Rust iOS artifacts for openagents-client-core and emit a
versioned artifact manifest with checksums + FFI boundary contract metadata.

Options:
  --output-root <dir>            Output root directory (default: apps/autopilot-ios/Autopilot/RustCore)
  --build-root <dir>             Cargo target root for iOS builds (default: target/ios-rust-client-core)
  --artifact-version <value>     Override generated artifact version stamp
  --skip-rustup-target-add       Skip rustup target add checks
  --clean                        Remove existing version/build dirs before build
  --no-current-symlink           Do not refresh output-root/current symlink
  -h, --help                     Show this help
USAGE
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}" >&2
    exit 1
  fi
}

sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-root)
      OUTPUT_ROOT="$2"
      shift 2
      ;;
    --build-root)
      BUILD_ROOT="$2"
      shift 2
      ;;
    --artifact-version)
      ARTIFACT_VERSION="$2"
      shift 2
      ;;
    --skip-rustup-target-add)
      SKIP_RUSTUP_TARGET_ADD=1
      shift
      ;;
    --clean)
      CLEAN=1
      shift
      ;;
    --no-current-symlink)
      WRITE_CURRENT_LINK=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_cmd cargo
require_cmd rustc
require_cmd xcodebuild
require_cmd lipo
require_cmd shasum
require_cmd strings
require_cmd rg
require_cmd git

CRATE_VERSION="$(cargo pkgid -p "${CRATE_NAME}" --manifest-path "${ROOT_DIR}/Cargo.toml" | sed -E 's|.*#||')"
GIT_COMMIT="$(git -C "${ROOT_DIR}" rev-parse HEAD)"
GIT_SHORT_COMMIT="$(git -C "${ROOT_DIR}" rev-parse --short=12 HEAD)"
RUSTC_VERSION="$(rustc --version | tr -d '\r')"
CARGO_VERSION="$(cargo --version | tr -d '\r')"
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "${ROOT_DIR}" log -1 --format=%ct HEAD)}"

if [[ -z "${ARTIFACT_VERSION}" ]]; then
  ARTIFACT_VERSION="v${CRATE_VERSION}-${GIT_SHORT_COMMIT}"
fi

VERSION_DIR="${OUTPUT_ROOT}/${ARTIFACT_VERSION}"
BUILD_DIR="${BUILD_ROOT}/${ARTIFACT_VERSION}"
HEADERS_DIR="${VERSION_DIR}/Headers"
XCFRAMEWORK_PATH="${VERSION_DIR}/OpenAgentsClientCore.xcframework"
SIM_UNIVERSAL_LIB="${VERSION_DIR}/lib${LIB_BASENAME}_sim.a"
FFI_CONTRACT_PATH="${VERSION_DIR}/ffi-contract.json"
MANIFEST_PATH="${VERSION_DIR}/manifest.json"
MANIFEST_SHA_PATH="${VERSION_DIR}/manifest.sha256"
LATEST_VERSION_FILE="${OUTPUT_ROOT}/LATEST_VERSION"
CURRENT_LINK="${OUTPUT_ROOT}/current"

if [[ "${CLEAN}" -eq 1 ]]; then
  if [[ -d "${VERSION_DIR}" ]]; then
    rm -r "${VERSION_DIR}"
  fi
  if [[ -d "${BUILD_DIR}" ]]; then
    rm -r "${BUILD_DIR}"
  fi
fi

mkdir -p "${VERSION_DIR}" "${BUILD_DIR}" "${HEADERS_DIR}"

if [[ "${SKIP_RUSTUP_TARGET_ADD}" -eq 0 ]]; then
  rustup target add "${IOS_TARGET}" "${SIM_ARM_TARGET}" "${SIM_X86_TARGET}" >/dev/null
fi

EXISTING_RUSTFLAGS="${RUSTFLAGS:-}"
if [[ -n "${EXISTING_RUSTFLAGS}" ]]; then
  export RUSTFLAGS="--remap-path-prefix=${ROOT_DIR}=. ${EXISTING_RUSTFLAGS}"
else
  export RUSTFLAGS="--remap-path-prefix=${ROOT_DIR}=."
fi
export SOURCE_DATE_EPOCH
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_RELEASE_DEBUG=0

if [[ ! -f "${SWIFT_BRIDGE_FILE}" ]]; then
  echo "Missing Swift bridge file: ${SWIFT_BRIDGE_FILE}" >&2
  exit 1
fi

SWIFT_EXPECTED_CONTRACT_VERSION="$(rg -o 'expectedFFIContractVersion:[[:space:]]*UInt32[[:space:]]*=[[:space:]]*[0-9]+' "${SWIFT_BRIDGE_FILE}" | sed -E 's|.*=[[:space:]]*([0-9]+)$|\1|' | head -n1)"
if [[ -z "${SWIFT_EXPECTED_CONTRACT_VERSION}" ]]; then
  echo "Unable to determine Swift expected FFI contract version." >&2
  exit 1
fi

if [[ "${SWIFT_EXPECTED_CONTRACT_VERSION}" != "${FFI_CONTRACT_VERSION}" ]]; then
  echo "Swift bridge expects FFI contract ${SWIFT_EXPECTED_CONTRACT_VERSION}, but build script is packaging ${FFI_CONTRACT_VERSION}." >&2
  exit 1
fi

for symbol in "${REQUIRED_SYMBOLS[@]}"; do
  if ! rg -Fq "\"${symbol}\"" "${SWIFT_BRIDGE_FILE}"; then
    echo "Swift bridge is missing required symbol reference: ${symbol}" >&2
    exit 1
  fi
done

for target in "${IOS_TARGET}" "${SIM_ARM_TARGET}" "${SIM_X86_TARGET}"; do
  cargo build \
    --manifest-path "${ROOT_DIR}/Cargo.toml" \
    -p "${CRATE_NAME}" \
    --locked \
    --release \
    --target "${target}" \
    --target-dir "${BUILD_DIR}"
done

IOS_LIB="${BUILD_DIR}/${IOS_TARGET}/release/lib${LIB_BASENAME}.a"
SIM_ARM_LIB="${BUILD_DIR}/${SIM_ARM_TARGET}/release/lib${LIB_BASENAME}.a"
SIM_X86_LIB="${BUILD_DIR}/${SIM_X86_TARGET}/release/lib${LIB_BASENAME}.a"

if [[ ! -f "${IOS_LIB}" || ! -f "${SIM_ARM_LIB}" || ! -f "${SIM_X86_LIB}" ]]; then
  echo "Missing required build artifacts for iOS packaging." >&2
  exit 1
fi

lipo -create "${SIM_ARM_LIB}" "${SIM_X86_LIB}" -output "${SIM_UNIVERSAL_LIB}"

cat > "${HEADERS_DIR}/openagents_client_core.h" <<'HEADER'
#ifndef OPENAGENTS_CLIENT_CORE_H
#define OPENAGENTS_CLIENT_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

uint32_t oa_client_core_ffi_contract_version(void);
char *oa_client_core_normalize_email(const char *input);
char *oa_client_core_normalize_verification_code(const char *input);
char *oa_client_core_normalize_message_text(const char *input);
char *oa_client_core_extract_desktop_handshake_ack_id(const char *payload_json);
char *oa_client_core_parse_khala_frame(const char *raw_frame);
void oa_client_core_free_string(char *raw);

#ifdef __cplusplus
}
#endif

#endif /* OPENAGENTS_CLIENT_CORE_H */
HEADER

cat > "${HEADERS_DIR}/module.modulemap" <<'MODULEMAP'
module OpenAgentsClientCore {
  header "openagents_client_core.h"
  export *
}
MODULEMAP

if [[ -d "${XCFRAMEWORK_PATH}" ]]; then
  rm -r "${XCFRAMEWORK_PATH}"
fi

xcodebuild -create-xcframework \
  -library "${IOS_LIB}" -headers "${HEADERS_DIR}" \
  -library "${SIM_UNIVERSAL_LIB}" -headers "${HEADERS_DIR}" \
  -output "${XCFRAMEWORK_PATH}" >/dev/null

DEVICE_XC_LIB="$(find "${XCFRAMEWORK_PATH}" -type f -name "lib${LIB_BASENAME}.a" | head -n1)"
SIM_XC_LIB="$(find "${XCFRAMEWORK_PATH}" -type f -name "lib${LIB_BASENAME}_sim.a" | head -n1)"
XC_INFO_PLIST="${XCFRAMEWORK_PATH}/Info.plist"

if [[ -z "${DEVICE_XC_LIB}" || -z "${SIM_XC_LIB}" || ! -f "${XC_INFO_PLIST}" ]]; then
  echo "XCFramework output missing expected libraries or metadata." >&2
  exit 1
fi

DEVICE_SYMBOLS_FILE="${VERSION_DIR}/.symbols-device.txt"
SIM_SYMBOLS_FILE="${VERSION_DIR}/.symbols-sim.txt"
strings "${DEVICE_XC_LIB}" > "${DEVICE_SYMBOLS_FILE}"
strings "${SIM_XC_LIB}" > "${SIM_SYMBOLS_FILE}"

for symbol in "${REQUIRED_SYMBOLS[@]}"; do
  if ! rg -Fq "${symbol}" "${DEVICE_SYMBOLS_FILE}"; then
    echo "Missing required symbol '${symbol}' in ${DEVICE_XC_LIB}" >&2
    exit 1
  fi
  if ! rg -Fq "${symbol}" "${SIM_SYMBOLS_FILE}"; then
    echo "Missing required symbol '${symbol}' in ${SIM_XC_LIB}" >&2
    exit 1
  fi
done

rm "${DEVICE_SYMBOLS_FILE}" "${SIM_SYMBOLS_FILE}"

DEVICE_XC_LIB_REL="${DEVICE_XC_LIB#${VERSION_DIR}/}"
SIM_XC_LIB_REL="${SIM_XC_LIB#${VERSION_DIR}/}"
SIM_UNIVERSAL_LIB_REL="${SIM_UNIVERSAL_LIB#${VERSION_DIR}/}"
HEADER_REL="${HEADERS_DIR#${VERSION_DIR}/}/openagents_client_core.h"
MODULEMAP_REL="${HEADERS_DIR#${VERSION_DIR}/}/module.modulemap"
XC_INFO_PLIST_REL="${XC_INFO_PLIST#${VERSION_DIR}/}"

cat > "${FFI_CONTRACT_PATH}" <<EOF
{
  "schema_version": 1,
  "ffi_contract_version": ${FFI_CONTRACT_VERSION},
  "api": {
    "header": "${HEADER_REL}",
    "modulemap": "${MODULEMAP_REL}",
    "required_symbols": [
      "oa_client_core_ffi_contract_version",
      "oa_client_core_normalize_email",
      "oa_client_core_normalize_verification_code",
      "oa_client_core_normalize_message_text",
      "oa_client_core_extract_desktop_handshake_ack_id",
      "oa_client_core_parse_khala_frame",
      "oa_client_core_free_string"
    ]
  },
  "threading": {
    "model": "C-ABI functions are re-entrant and safe to call from any app-managed thread.",
    "restrictions": [
      "Callers must not mutate freed pointers.",
      "Callers must pass UTF-8 C strings or null pointers."
    ]
  },
  "memory_ownership": {
    "inputs": "Caller retains ownership of input C strings.",
    "outputs": "Returned pointers are owned by Rust and must be freed with oa_client_core_free_string.",
    "null_semantics": "Null output indicates invalid input or decode/parse failure."
  },
  "error_mapping": {
    "invalid_input": "null pointer return",
    "parse_failure": "null pointer return",
    "normalization_failure": "null pointer return"
  }
}
EOF

cat > "${MANIFEST_PATH}" <<EOF
{
  "schema_version": 1,
  "artifact_version": "${ARTIFACT_VERSION}",
  "crate": {
    "name": "${CRATE_NAME}",
    "version": "${CRATE_VERSION}"
  },
  "ffi_contract_version": ${FFI_CONTRACT_VERSION},
  "swift_expected_ffi_contract_version": ${SWIFT_EXPECTED_CONTRACT_VERSION},
  "git_commit": "${GIT_COMMIT}",
  "build_toolchain": {
    "rustc": "${RUSTC_VERSION}",
    "cargo": "${CARGO_VERSION}",
    "source_date_epoch": "${SOURCE_DATE_EPOCH}"
  },
  "targets": [
    "${IOS_TARGET}",
    "${SIM_ARM_TARGET}",
    "${SIM_X86_TARGET}"
  ],
  "artifacts": [
    {
      "path": "${DEVICE_XC_LIB_REL}",
      "sha256": "$(sha256 "${DEVICE_XC_LIB}")"
    },
    {
      "path": "${SIM_XC_LIB_REL}",
      "sha256": "$(sha256 "${SIM_XC_LIB}")"
    },
    {
      "path": "${SIM_UNIVERSAL_LIB_REL}",
      "sha256": "$(sha256 "${SIM_UNIVERSAL_LIB}")"
    },
    {
      "path": "${HEADER_REL}",
      "sha256": "$(sha256 "${HEADERS_DIR}/openagents_client_core.h")"
    },
    {
      "path": "${MODULEMAP_REL}",
      "sha256": "$(sha256 "${HEADERS_DIR}/module.modulemap")"
    },
    {
      "path": "${XC_INFO_PLIST_REL}",
      "sha256": "$(sha256 "${XC_INFO_PLIST}")"
    },
    {
      "path": "ffi-contract.json",
      "sha256": "$(sha256 "${FFI_CONTRACT_PATH}")"
    }
  ]
}
EOF

echo "$(sha256 "${MANIFEST_PATH}")  manifest.json" > "${MANIFEST_SHA_PATH}"
echo "${ARTIFACT_VERSION}" > "${LATEST_VERSION_FILE}"

if [[ "${WRITE_CURRENT_LINK}" -eq 1 ]]; then
  if [[ -L "${CURRENT_LINK}" || -f "${CURRENT_LINK}" ]]; then
    rm "${CURRENT_LINK}"
  elif [[ -d "${CURRENT_LINK}" ]]; then
    rm -r "${CURRENT_LINK}"
  fi
  ln -s "${ARTIFACT_VERSION}" "${CURRENT_LINK}"
fi

echo "Built Rust client-core artifact version: ${ARTIFACT_VERSION}"
echo "Artifact directory: ${VERSION_DIR}"
echo "Manifest: ${MANIFEST_PATH}"
