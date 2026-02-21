#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> docs-check"
"$ROOT/scripts/docs-check.mjs"

if [ -d "$ROOT/packages" ]; then
  echo "warning: legacy packages/ directory still exists; Rust-only lane expects it to be removed"
fi

echo "==> rust service checks"
cargo check -p lightning-ops
cargo check -p lightning-wallet-executor

echo "verify: OK"
