#!/usr/bin/env bash
# Bump the fork pin and run the complete local protocol suite.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
exec python3 "$root/scripts/coder-sync/conformance.py" "$@"
