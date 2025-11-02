#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

: "${RUN:=all}"

scripts/maestro-prepare.sh

if [[ "$RUN" == "stable" ]]; then
  MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-stable.sh
else
  MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-all.sh
fi
