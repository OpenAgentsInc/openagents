#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="${REPO_ROOT}/bin"
APP_BUNDLE_DIR="${OUTPUT_DIR}/FoundationBridge.app"
APP_CONTENTS_DIR="${APP_BUNDLE_DIR}/Contents"
APP_MACOS_DIR="${APP_CONTENTS_DIR}/MacOS"
APP_RESOURCES_DIR="${APP_CONTENTS_DIR}/Resources"
INFO_TEMPLATE="${SCRIPT_DIR}/FoundationBridge-Info.plist.in"
MARKETING_VERSION="${OPENAGENTS_APPLE_FM_BRIDGE_VERSION:-1.0.0}"
BUILD_VERSION="${OPENAGENTS_APPLE_FM_BRIDGE_BUILD_VERSION:-$(date -u +%Y%m%d%H%M%S)}"
SIGN_IDENTITY_OVERRIDE="${OPENAGENTS_APPLE_FM_BRIDGE_SIGN_IDENTITY:-}"
BUILD_MACHINE_OS_BUILD="$(sw_vers -buildVersion 2>/dev/null || echo unknown)"
DT_PLATFORM_NAME="macosx"
DT_PLATFORM_VERSION="$(xcrun --sdk macosx --show-sdk-version)"
DT_PLATFORM_BUILD="$(xcrun --show-sdk-build-version --sdk macosx)"
DT_SDK_NAME="macosx${DT_PLATFORM_VERSION}"
DT_SDK_BUILD="${DT_PLATFORM_BUILD}"
XCODE_VERSION="$(xcodebuild -version | awk '/^Xcode / { print $2; exit }')"
DT_XCODE_BUILD="$(xcodebuild -version | awk 'NR == 2 { print $3; exit }')"
DT_COMPILER="$(swiftc --version 2>/dev/null | head -n 1 | tr '\t' ' ')"

encode_xcode_version() {
    local version="$1"
    local major=0
    local minor=0
    local patch=0
    IFS='.' read -r major minor patch <<< "$version"
    major="${major:-0}"
    minor="${minor:-0}"
    patch="${patch:-0}"
    printf '%d' "$((major * 100 + minor * 10 + patch))"
}

select_codesign_identity() {
    if [ -n "$SIGN_IDENTITY_OVERRIDE" ]; then
        printf '%s' "$SIGN_IDENTITY_OVERRIDE"
        return 0
    fi

    local identities
    identities="$(security find-identity -v -p codesigning 2>/dev/null || true)"
    local preferred
    preferred="$(printf '%s\n' "$identities" | sed -n 's/.*"\(Apple Development:[^"]*\)".*/\1/p' | head -n 1)"
    if [ -z "$preferred" ]; then
        preferred="$(printf '%s\n' "$identities" | sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p' | head -n 1)"
    fi
    if [ -z "$preferred" ]; then
        preferred="$(printf '%s\n' "$identities" | sed -n 's/.*"\(Apple Distribution:[^"]*\)".*/\1/p' | head -n 1)"
    fi
    if [ -n "$preferred" ]; then
        printf '%s' "$preferred"
        return 0
    fi

    printf '%s' "-"
}

DT_XCODE="$(encode_xcode_version "$XCODE_VERSION")"
SIGN_IDENTITY="$(select_codesign_identity)"

echo "Building Foundation Models Bridge..."
echo "Using codesign identity: $SIGN_IDENTITY"
mkdir -p "$OUTPUT_DIR"

cd "$SCRIPT_DIR"
swift build -c release

BUILT_BINARY="$SCRIPT_DIR/.build/release/foundation-bridge"
if [ -f "$BUILT_BINARY" ]; then
    cp "$BUILT_BINARY" "$OUTPUT_DIR/foundation-bridge"
    chmod +x "$OUTPUT_DIR/foundation-bridge"
    rm -rf "$APP_BUNDLE_DIR"
    mkdir -p "$APP_MACOS_DIR" "$APP_RESOURCES_DIR"
    cp "$BUILT_BINARY" "$APP_MACOS_DIR/foundation-bridge"
    chmod +x "$APP_MACOS_DIR/foundation-bridge"
    sed \
        -e "s/@MARKETING_VERSION@/${MARKETING_VERSION}/g" \
        -e "s/@BUILD_VERSION@/${BUILD_VERSION}/g" \
        -e "s/@BUILD_MACHINE_OS_BUILD@/${BUILD_MACHINE_OS_BUILD}/g" \
        -e "s/@DT_COMPILER@/${DT_COMPILER//\//\\/}/g" \
        -e "s/@DT_PLATFORM_BUILD@/${DT_PLATFORM_BUILD}/g" \
        -e "s/@DT_PLATFORM_NAME@/${DT_PLATFORM_NAME}/g" \
        -e "s/@DT_PLATFORM_VERSION@/${DT_PLATFORM_VERSION}/g" \
        -e "s/@DT_SDK_BUILD@/${DT_SDK_BUILD}/g" \
        -e "s/@DT_SDK_NAME@/${DT_SDK_NAME}/g" \
        -e "s/@DT_XCODE@/${DT_XCODE}/g" \
        -e "s/@DT_XCODE_BUILD@/${DT_XCODE_BUILD}/g" \
        "$INFO_TEMPLATE" > "$APP_CONTENTS_DIR/Info.plist"
    codesign --force --sign "$SIGN_IDENTITY" "$OUTPUT_DIR/foundation-bridge"
    codesign --force --sign "$SIGN_IDENTITY" --deep "$APP_BUNDLE_DIR"
    echo "Build successful: $OUTPUT_DIR/foundation-bridge"
    echo "Bundle successful: $APP_BUNDLE_DIR"
else
    echo "Build failed: binary not found at $BUILT_BINARY"
    exit 1
fi
