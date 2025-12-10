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

# Create wrapper script in bin directory that calls the built binary
# This avoids macOS code signing issues when copying the binary
BUILT_BINARY="$SCRIPT_DIR/.build/release/foundation-bridge"
if [ -f "$BUILT_BINARY" ]; then
    # Create wrapper script that executes from build directory
    cat > "$OUTPUT_DIR/foundation-bridge" << 'EOF'
#!/bin/bash
# Wrapper script for foundation-bridge
# Executes from build directory to preserve code signature/entitlements
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$REPO_ROOT/swift/foundation-bridge/.build/release/foundation-bridge"

if [ ! -f "$BINARY" ]; then
    echo "Error: foundation-bridge binary not found at $BINARY"
    echo "Run: bun run bridge:build"
    exit 1
fi

exec "$BINARY" "$@"
EOF
    chmod +x "$OUTPUT_DIR/foundation-bridge"
    echo ""
    echo "Build successful!"
    echo "Binary location: $OUTPUT_DIR/foundation-bridge (wrapper script)"
    echo "Actual binary: $BUILT_BINARY"
    echo ""
    echo "Usage:"
    echo "  $OUTPUT_DIR/foundation-bridge [port]"
    echo "  bun run bridge"
    echo ""
    echo "Default port: 11435"
else
    echo "Build failed: binary not found at $BUILT_BINARY"
    exit 1
fi
