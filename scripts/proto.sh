#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper; holds no logic)
# Standalone schema checks; no old app workspace or runtime dependency.
set -euo pipefail
cd "$(dirname "$0")/.."
exec "${PROTO_PYTHON:-python3}" proto/tools/protocol.py "$@"
