#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper; holds no logic)
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/nips.py sync "$@"
