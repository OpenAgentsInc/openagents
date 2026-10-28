#!/usr/bin/env bash
# Optional vector-maintenance tooling, not a normal compiler/app build step.
set -euo pipefail
exec python3 "$(dirname "$0")/vectors/run.py" "$@"
