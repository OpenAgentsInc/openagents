#!/bin/sh
# The web smoke suite against staging (or any running site):
#
#   scripts/smoke/staging.sh [BASE_URL] [--no-install] [--only home,docs,...]
#
# BASE_URL defaults to the staging service
# (https://staging.openagents.com,
# deploy/staging/README.md). One PASS/FAIL/SKIP line per check; exit 1 when
# any check fails. Groups for --only: home, docs, download, agent, github,
# gates, accounts, signed-in, gateway, traces, terminal. The terminal group runs the
# site's hosted installer into a scratch HOME and checks `coder --version`.
# Signed-in checks use one test account made with the staging operator
# token: SMOKE_SIGNUP_TOKEN, or, when unset, the Secret Manager secret
# openagents-gateway-staging-smoke-signup-token read with the automation
# account (never printed). Without it those checks are skipped.
set -eu
if [ -z "${SMOKE_SIGNUP_TOKEN:-}" ] && command -v gcloud > /dev/null 2>&1; then
    config=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
    SMOKE_SIGNUP_TOKEN=$(CLOUDSDK_CONFIG=$config gcloud secrets versions access latest \
        --secret openagents-gateway-staging-smoke-signup-token --project openagentsgemini 2> /dev/null || true)
    export SMOKE_SIGNUP_TOKEN
fi
# macOS's /usr/bin/python3 links LibreSSL 2.8, whose TLS drops connections
# to Google's front end on custom domains; prefer a current Python.
py=python3
for candidate in python3.13 python3.12 python3.11; do
    if command -v "$candidate" > /dev/null 2>&1; then py=$candidate; break; fi
done
exec "$py" -I "$(dirname "$0")/web.py" "$@"
