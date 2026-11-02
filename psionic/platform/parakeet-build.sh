#!/usr/bin/env bash
# Firm reason: 5 (only execs the Coax build tool; no build logic lives here).
exec "$(dirname "$0")/../../coax/web/tool.sh" "$(dirname "$0")/parakeet-build.coax" "$@"
