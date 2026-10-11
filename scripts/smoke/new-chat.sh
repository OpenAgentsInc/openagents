#!/bin/sh
# Runs scripts/smoke/new-chat.mjs (Playwright, headless Chromium) against
# BASE_URL (default https://staging.openagents.com). Playwright is
# installed once into ~/.cache/openagents-smoke-playwright. Signs in with a
# staging test account made with SMOKE_SIGNUP_TOKEN, read like staging.sh
# does when unset (never printed).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
dir=${PLAYWRIGHT_DIR:-$HOME/.cache/openagents-smoke-playwright}
version=${PLAYWRIGHT_VERSION:-1.61.0}
if [ ! -f "$dir/node_modules/playwright/package.json" ] ||
    ! grep -q "\"version\": \"$version\"" "$dir/node_modules/playwright/package.json"; then
    mkdir -p "$dir"
    (cd "$dir" && npm init -y > /dev/null && npm install --silent "playwright@$version" > /dev/null &&
        npx --yes playwright install chromium > /dev/null)
fi
if [ -z "${SMOKE_SIGNUP_TOKEN:-}" ] && command -v gcloud > /dev/null 2>&1; then
    config=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
    SMOKE_SIGNUP_TOKEN=$(CLOUDSDK_CONFIG=$config gcloud secrets versions access latest \
        --secret openagents-gateway-staging-smoke-signup-token --project openagentsgemini 2> /dev/null || true)
    export SMOKE_SIGNUP_TOKEN
fi
cp "$here/new-chat.mjs" "$dir/new-chat.mjs"
exec node "$dir/new-chat.mjs" "$@"
