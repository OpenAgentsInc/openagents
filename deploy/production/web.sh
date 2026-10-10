# The production web container's launcher (service `coder`,
# openagents.com), run by /bin/sh -c in the openagents-web image with the
# site's own arguments after it ("$@": --listen, --public-host, --chat-bucket,
# ... as the live revision had them). deploy/production/render.py inlines
# this file. Like deploy/staging/web.sh, it writes the secrets Cloud Run
# passes as environment variables into owner-only files in the container's
# own memory, then starts the site with the account service and the
# inference gateway on the `gateway` sidecar (127.0.0.1:8791). Saved
# own-Claude keys (sealed with OPENAGENTS_WEB_CLOUD_BYO_KEYS) are kept in
# $WEB_STATE/byo on the account-store NFS disk
# (docs/deployment/account-storage.md). Paths the site doesn't own still go
# to the coder-serve sidecar (OPENAGENTS_WEB_UPSTREAM). Nothing is printed.
set -eu
umask 077
p=/tmp/private
byo=${WEB_STATE:?WEB_STATE is unset}/byo
mkdir -p "$p" "$byo"
chmod 700 "$byo"
printf '%s' "${GITHUB_OAUTH_JSON:?}" > "$p/github-oauth.json"
printf '%s' "${CSRF_KEY:?}" > "$p/csrf.key"
unset GITHUB_OAUTH_JSON CSRF_KEY
# INVITE_ONLY_JSON: only these GitHub people may sign in (docs/auth/github.md).
invite=""
if [ -n "${INVITE_ONLY_JSON:-}" ]; then
    invite=",\"invite_only\":$INVITE_ONLY_JSON"
fi
printf '{"schema":"openagents.cloud.web-config.v1","public_origin":"%s","account_service":"http://127.0.0.1:8791","csrf_secret":"%s/csrf.key"%s}' \
    "${PUBLIC_ORIGIN:?}" "$p" "$invite" > "$p/cloud.json"
# Agent work (#11162, docs/deployment/agent-work.md): Environments and
# Claude Code runs for the site admin. The machines are Boat's
# (BOAT_API_KEY); the setup agent's model goes through the gateway sidecar
# on the house service key it keeps in $STACK_STATE/service.key (mounted
# read-only); records live in $WEB_STATE/environments. Without the key or
# the service key, the site starts without them.
environments=""
stack=${STACK_STATE:-/stack}
if [ -n "${BOAT_API_KEY:-}" ] && [ -s "$stack/service.key" ]; then
    envs=$WEB_STATE/environments
    mkdir -p "$envs"
    chmod 700 "$envs"
    cat "$stack/service.key" > "$p/model.key"
    printf '{"schema":"openagents.environment.studio.v1","state":"%s","machines":{"schema":"openagents.environment.owners.v1","provider":"boat","workdir":"/home/user/repo","credential_names":[],"tick_seconds":15},"owner":{"workspace":"openagents-web","principal":"web"},"model":"%s","size":"small","deadline_seconds":7200,"model_api":{"url":"http://127.0.0.1:8791/v1/responses","key_file":"%s/model.key"}}' \
        "$envs" "${ENVIRONMENTS_MODEL:-google/gemini-3.8-flash}" "$p" > "$p/environments.json"
    environments="--environments $p/environments.json"
fi
# $environments is one flag and a path without spaces, split on purpose.
# shellcheck disable=SC2086
exec /usr/local/bin/openagents-web "$@" $environments \
    --cloud-config "$p/cloud.json" --github-oauth "$p/github-oauth.json" \
    --cloud-byo "$byo" --plan-meter /tmp/plan-meter.sqlite \
    --inference http://127.0.0.1:8791
