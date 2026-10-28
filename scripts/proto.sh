#!/usr/bin/env bash
# Standalone schema checks; no old app workspace or runtime dependency.
set -euo pipefail
cd "$(dirname "$0")/.."
exec "${PROTO_PYTHON:-python3}" proto/tools/protocol.py "$@"
