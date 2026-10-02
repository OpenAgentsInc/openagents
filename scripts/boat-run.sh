#!/usr/bin/env bash
# Run a command on a Boat sandbox against this checkout's change.
#
#   scripts/boat-run.sh NAME -- COMMAND...
#   scripts/boat-run.sh NAME --stop    stops the sandbox (free while stopped)
#
# This is `openagents boat run NAME ...` (crates/openagents-cli/src/boat_run.rs):
# one sandbox per NAME (id in ~/.openagents/boat/NAME), the clone reset to
# origin/main, this checkout's diff against origin/main applied through the
# files API, COMMAND run with CARGO_INCREMENTAL=0, output printed as it
# arrives, and COMMAND's exit code returned. Needs BOAT_API_KEY (source
# ~/work/.secrets/boat.env). Nothing is built on this machine. Owner,
# 2026-10-02: builds move off the Mac to Boat.
#
# An installed `openagents` released before the `boat` command falls back to
# scripts/boat-run-legacy.sh, the same behaviour in curl.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bin=${OPENAGENTS_BIN:-openagents}
if [ -z "${OA_BOAT_RUN_LEGACY:-}" ] && "$bin" boat --help >/dev/null 2>&1; then
  exec "$bin" boat run "$@"
fi
exec "$here/boat-run-legacy.sh" "$@"
