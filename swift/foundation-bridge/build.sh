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

echo "Building Foundation Models Bridge..."
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
        "$INFO_TEMPLATE" > "$APP_CONTENTS_DIR/Info.plist"
    codesign --force --sign - "$OUTPUT_DIR/foundation-bridge"
    codesign --force --sign - --deep "$APP_BUNDLE_DIR"
    echo "Build successful: $OUTPUT_DIR/foundation-bridge"
    echo "Bundle successful: $APP_BUNDLE_DIR"
else
    echo "Build failed: binary not found at $BUILT_BINARY"
    exit 1
fi
