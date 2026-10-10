#!/bin/sh
# The web smoke suite against staging (or any running site):
#
#   scripts/smoke/staging.sh [BASE_URL] [--no-install] [--only home,docs,...] [--production] [--candidate]
#
# BASE_URL defaults to the staging service
# (https://staging.openagents.com,
# deploy/staging/README.md). One PASS/FAIL/SKIP line per check; exit 1 when
# any check fails. Groups for --only: home, docs, promises, download, agent, github,
# gates, alias, accounts, signed-in, environments, gateway, traces, terminal.
# --environment-run OWNER/REPO also sets up one environment end to end on
# real Boat machines as the agent-work test account (#11162). --invite-only
# also checks that /login says sign-in is invite-only. The terminal group runs the
# site's hosted installer into a scratch HOME and checks `coder --version`.
# --restart (opt-in) also forces a new revision of the service and checks
# that an account, its session, an API key, a saved provider key and a
# saved own-Claude key survive it (docs/deployment/account-storage.md).
# Signed-in checks use one test account made with the staging operator
# token: SMOKE_SIGNUP_TOKEN, or, when unset, the Secret Manager secret
# openagents-gateway-staging-smoke-signup-token read with the automation
# account (never printed). Without it those checks are skipped.
# --production (openagents.com or its tag URL): one question, no test
# account; the operator token is not read. --candidate (a no-traffic
# production candidate, implies --production): the gateway checks, which
# wait for traffic (#11154), print WAIT instead of failing.
set -eu
case " $* " in *" --production "* | *" --candidate "*) production=1 ;; *) production= ;; esac
if [ -z "$production" ] && [ -z "${SMOKE_SIGNUP_TOKEN:-}" ] && command -v gcloud > /dev/null 2>&1; then
    config=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
    SMOKE_SIGNUP_TOKEN=$(CLOUDSDK_CONFIG=$config gcloud secrets versions access latest \
        --secret openagents-gateway-staging-smoke-signup-token --project openagentsgemini 2> /dev/null || true)
    export SMOKE_SIGNUP_TOKEN
fi
# The environments group signs in as the fixed agent-work test account
# (#11162) with its key: SMOKE_AGENT_KEY, or the staging-only secret
# openagents-web-1-staging-agent-smoke-key (never printed).
if [ -z "$production" ] && [ -z "${SMOKE_AGENT_KEY:-}" ] && command -v gcloud > /dev/null 2>&1; then
    config=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
    SMOKE_AGENT_KEY=$(CLOUDSDK_CONFIG=$config gcloud secrets versions access latest \
        --secret openagents-web-1-staging-agent-smoke-key --project openagentsgemini 2> /dev/null || true)
    export SMOKE_AGENT_KEY
fi
# macOS's /usr/bin/python3 links LibreSSL 2.8, whose TLS drops connections
# to Google's front end on custom domains; prefer a current Python.
py=python3
for candidate in python3.13 python3.12 python3.11; do
    if command -v "$candidate" > /dev/null 2>&1; then py=$candidate; break; fi
done
exec "$py" -I "$(dirname "$0")/web.py" "$@"
