#!/bin/bash
# OANIX Boot Demo
#
# Builds and runs OANIX to demonstrate environment discovery.
#
# Usage:
#   ./scripts/oanix-demo.sh          # Build and run
#   ./scripts/oanix-demo.sh --release # Release build

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Determine build profile
if [[ "$1" == "--release" ]]; then
    PROFILE="release"
    BUILD_FLAGS="--release"
else
    PROFILE="debug"
    BUILD_FLAGS=""
fi

echo "Building OANIX ($PROFILE)..."
cargo build -p oanix $BUILD_FLAGS

echo ""
echo "Running OANIX boot sequence..."
echo ""

./target/$PROFILE/oanix
