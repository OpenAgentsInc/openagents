#!/bin/bash
# Build and serve the Coder web app

set -e

# Check for wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack not found"
    echo "Install it with: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
    exit 1
fi

# Check for wasm32 target
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "Adding wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

echo "Building Coder for web (WASM)..."
wasm-pack build --target web --out-dir pkg --features web

echo ""
echo "✅ Build complete!"
echo "🌐 Starting local server on http://localhost:8080"
echo "Press Ctrl+C to stop"
echo ""
python3 -m http.server 8080
