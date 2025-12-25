#!/bin/bash
#
# Local inference runner for GPT-OSS or Apple Foundation Models.
# Usage:
#   scripts/local-infer.sh --backend gpt-oss "Hello"
#   scripts/local-infer.sh --backend fm-bridge --tools "Summarize this repo"
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BIN_RELEASE="$PROJECT_ROOT/target/release/local-infer"
BIN_DEBUG="$PROJECT_ROOT/target/debug/local-infer"

if [ -x "$BIN_RELEASE" ]; then
    exec "$BIN_RELEASE" "$@"
elif [ -x "$BIN_DEBUG" ]; then
    exec "$BIN_DEBUG" "$@"
else
    exec cargo run --manifest-path "$PROJECT_ROOT/Cargo.toml" --bin local-infer -- "$@"
fi
