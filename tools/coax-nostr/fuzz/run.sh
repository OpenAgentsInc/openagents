#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper; holds no logic)
# Explicit test tooling; no implicit download, compiler bootstrap or app build.
set -euo pipefail
exec python3 "$(cd "$(dirname "$0")" && pwd)/differential.py" "$@"
