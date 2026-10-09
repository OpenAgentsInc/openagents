#!/bin/sh
# The web smoke suite against staging (or any running site):
#
#   scripts/smoke/staging.sh [BASE_URL] [--no-install] [--only home,docs,...]
#
# BASE_URL defaults to the staging service
# (https://openagents-web-1-staging-157437760789.us-central1.run.app,
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
exec python3 -I "$(dirname "$0")/web.py" "$@"
