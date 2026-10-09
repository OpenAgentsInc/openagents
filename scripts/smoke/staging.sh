#!/bin/sh
# The web smoke suite against staging (or any running site):
#
#   scripts/smoke/staging.sh [BASE_URL] [--no-install] [--only home,docs,...]
#
# BASE_URL defaults to the staging service
# (https://openagents-web-1-staging-157437760789.us-central1.run.app,
# deploy/staging/README.md). One PASS/FAIL/SKIP line per check; exit 1 when
# any check fails. Groups for --only: home, docs, download, agent, github,
# gates, signed-in, gateway, traces, terminal. The terminal group runs the
# site's hosted installer into a scratch HOME and checks `coder --version`.
set -eu
exec python3 -I "$(dirname "$0")/web.py" "$@"
