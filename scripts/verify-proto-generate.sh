#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "verify-proto-generate.sh runs Rust-only proto checks (buf Rust template + openagents-proto crate)."
exec "${ROOT_DIR}/scripts/verify-rust-proto-crate.sh"
