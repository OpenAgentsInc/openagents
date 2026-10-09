#!/bin/sh
# The web smoke suite against staging (or any running site):
#
#   scripts/smoke/staging.sh [BASE_URL] [--no-install] [--only home,docs,...] [--production]
#
# BASE_URL defaults to the staging service
# (https://staging.openagents.com,
# deploy/staging/README.md). One PASS/FAIL/SKIP line per check; exit 1 when
# any check fails. Groups for --only: home, docs, promises, download, agent, github,
# gates, accounts, signed-in, gateway, traces, terminal. The terminal group runs the
# site's hosted installer into a scratch HOME and checks `coder --version`.
# --restart (opt-in) also forces a new revision of the service and checks
# that an account, its session, an API key, a saved provider key and a
# saved own-Claude key survive it (docs/deployment/account-storage.md).
# Signed-in checks use one test account made with the staging operator
# token: SMOKE_SIGNUP_TOKEN, or, when unset, the Secret Manager secret
# openagents-gateway-staging-smoke-signup-token read with the automation
# account (never printed). Without it those checks are skipped.
# --production (openagents.com or its tag URL): one question, no account,
# no sign-in checks; the operator token is not read.
set -eu
case " $* " in *" --production "*) production=1 ;; *) production= ;; esac
if [ -z "$production" ] && [ -z "${SMOKE_SIGNUP_TOKEN:-}" ] && command -v gcloud > /dev/null 2>&1; then
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
