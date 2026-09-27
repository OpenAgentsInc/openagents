#!/usr/bin/env bash
# Preserve the Coder Android build entry point.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec "$root/scripts/build-coder-android.sh" "$@"
