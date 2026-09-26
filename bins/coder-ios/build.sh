#!/usr/bin/env bash
# Keep the Coder app's existing build entry point.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec "$root/scripts/build-coder-mobile.sh" "$@"
