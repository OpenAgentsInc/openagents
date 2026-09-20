#!/usr/bin/env bash
# Fetch and verify pinned public Kev artifacts. Python is infrastructure only.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$here/fetch-kev-artifacts.py" "$@"
