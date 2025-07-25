#!/bin/bash
# Run OpenAgents with debug logging enabled

echo "Starting OpenAgents with debug logging..."
echo "Open browser console (F12) to see client-side logs"
echo "Server logs will appear in this terminal"
echo ""

# Set environment variable for Rust debug logging
export RUST_LOG=debug

# Run the Tauri dev server
bun run tauri dev