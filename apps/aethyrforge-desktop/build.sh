#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper; build decisions are in aethyrforge-build.coax)
set -euo pipefail
exec "$(dirname "$0")/../../coax/web/tool.sh" "$(dirname "$0")/aethyrforge-build.coax" "$(cd "$(dirname "$0")/../.." && pwd)" "$@"
