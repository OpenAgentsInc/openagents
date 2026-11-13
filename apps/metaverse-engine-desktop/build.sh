#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper; build decisions are in app-build.coax)
set -euo pipefail
exec "$(dirname "$0")/../../coax/web/tool.sh" "$(dirname "$0")/app-build.coax" "$(cd "$(dirname "$0")/../.." && pwd)" "$@"
