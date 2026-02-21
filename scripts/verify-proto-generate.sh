#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "verify-proto-generate.sh now runs Rust-only proto checks via openagents-proto."
exec "${ROOT_DIR}/scripts/verify-rust-proto-crate.sh"
