#!/usr/bin/env bash
# The OpenAgents Android build entry point; see scripts/build-openagents-android.sh.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec "$root/scripts/build-openagents-android.sh" "$@"
