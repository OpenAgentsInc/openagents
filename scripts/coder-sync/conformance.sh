#!/usr/bin/env bash
# Run the complete local protocol suite: the server against the sync clients and
# headless engine from apps/openagents in this checkout.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
exec python3 "$root/scripts/coder-sync/conformance.py" "$@"
