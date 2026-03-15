#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="${REPO_ROOT}/bin"

echo "Building Foundation Models Bridge..."
mkdir -p "$OUTPUT_DIR"

cd "$SCRIPT_DIR"
swift build -c release

BUILT_BINARY="$SCRIPT_DIR/.build/release/foundation-bridge"
if [ -f "$BUILT_BINARY" ]; then
    cp "$BUILT_BINARY" "$OUTPUT_DIR/foundation-bridge"
    chmod +x "$OUTPUT_DIR/foundation-bridge"
    codesign --force --sign - "$OUTPUT_DIR/foundation-bridge"
    echo "Build successful: $OUTPUT_DIR/foundation-bridge"
else
    echo "Build failed: binary not found at $BUILT_BINARY"
    exit 1
fi
