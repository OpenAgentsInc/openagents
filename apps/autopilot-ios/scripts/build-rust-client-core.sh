#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT_DIR="${ROOT_DIR}/apps/autopilot-ios/Autopilot/RustCore"
BUILD_DIR="${ROOT_DIR}/target/ios-rust-client-core"

IOS_TARGET="aarch64-apple-ios"
SIM_ARM_TARGET="aarch64-apple-ios-sim"
SIM_X86_TARGET="x86_64-apple-ios"

mkdir -p "${OUT_DIR}" "${BUILD_DIR}"

for target in "${IOS_TARGET}" "${SIM_ARM_TARGET}" "${SIM_X86_TARGET}"; do
  rustup target add "${target}" >/dev/null
  cargo build \
    -p openagents-client-core \
    --release \
    --target "${target}" \
    --target-dir "${BUILD_DIR}"
done

IOS_LIB="${BUILD_DIR}/${IOS_TARGET}/release/libopenagents_client_core.a"
SIM_ARM_LIB="${BUILD_DIR}/${SIM_ARM_TARGET}/release/libopenagents_client_core.a"
SIM_X86_LIB="${BUILD_DIR}/${SIM_X86_TARGET}/release/libopenagents_client_core.a"
SIM_UNIVERSAL_LIB="${OUT_DIR}/libopenagents_client_core_sim.a"
XCFRAMEWORK_PATH="${OUT_DIR}/OpenAgentsClientCore.xcframework"

lipo -create "${SIM_ARM_LIB}" "${SIM_X86_LIB}" -output "${SIM_UNIVERSAL_LIB}"

rm -rf "${XCFRAMEWORK_PATH}"
xcodebuild -create-xcframework \
  -library "${IOS_LIB}" \
  -library "${SIM_UNIVERSAL_LIB}" \
  -output "${XCFRAMEWORK_PATH}"

echo "Built Rust client-core XCFramework: ${XCFRAMEWORK_PATH}"
