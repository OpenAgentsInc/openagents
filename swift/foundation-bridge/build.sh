#!/bin/bash
# Build the Foundation Models HTTP Bridge
# Requires: macOS 26+ SDK, Xcode 26+

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="${REPO_ROOT}/bin"

echo "Building Foundation Models Bridge..."
echo "Script directory: $SCRIPT_DIR"
echo "Output directory: $OUTPUT_DIR"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Build with Swift Package Manager
cd "$SCRIPT_DIR"
swift build -c release

# Copy binary to bin directory
BUILT_BINARY="$SCRIPT_DIR/.build/release/foundation-bridge"
if [ -f "$BUILT_BINARY" ]; then
    cp "$BUILT_BINARY" "$OUTPUT_DIR/foundation-bridge"
    chmod +x "$OUTPUT_DIR/foundation-bridge"
    echo ""
    echo "Build successful!"
    echo "Binary location: $OUTPUT_DIR/foundation-bridge"
    echo ""
    echo "Usage:"
    echo "  $OUTPUT_DIR/foundation-bridge [port]"
    echo ""
    echo "Default port: 11435"
else
    echo "Build failed: binary not found at $BUILT_BINARY"
    exit 1
fi
