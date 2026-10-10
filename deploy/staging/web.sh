# The staging web container's launcher, run by /bin/sh -c in the
# openagents-web image (render.py inlines this file into the service).
# It writes the secrets Cloud Run passes as environment variables into
# owner-only files the site's private-file checks accept, then starts the
# site the way scripts/dev/full-local.sh does, with the account service and
# the inference gateway on the gateway sidecar. Saved own-Claude keys
# (sealed with the keyring in OPENAGENTS_WEB_CLOUD_BYO_KEYS) are kept in
# $WEB_STATE/byo, an NFS volume on the account-store server
# (docs/deployment/account-storage.md), so they outlive the instance.
set -eu
umask 077
p=/tmp/private
byo=${WEB_STATE:-/tmp}/byo
mkdir -p "$p" "$byo"
chmod 700 "$byo"
printf '%s' "$GITHUB_OAUTH_JSON" > "$p/github-oauth.json"
printf '%s' "$CSRF_KEY" > "$p/csrf.key"
unset GITHUB_OAUTH_JSON CSRF_KEY
# INVITE_ONLY_JSON (optional): only these GitHub people may sign in.
invite=""
if [ -n "${INVITE_ONLY_JSON:-}" ]; then
    invite=",\"invite_only\":$INVITE_ONLY_JSON"
fi
printf '{"schema":"openagents.cloud.web-config.v1","public_origin":"%s","account_service":"http://127.0.0.1:8791","csrf_secret":"%s/csrf.key"%s}' "$PUBLIC_ORIGIN" "$p" "$invite" > "$p/cloud.json"
# Agent work (#11162, docs/deployment/agent-work.md): Environments and
# Claude Code runs, for site admins and OPENAGENTS_WEB_AGENT_ACCOUNTS. The
# machines are Boat's (BOAT_API_KEY); the setup agent's model goes through
# the gateway sidecar on the house service key the gateway keeps in
# $STACK_STATE/service.key (mounted read-only); records live in
# $WEB_STATE/environments. Without the key or the service key, the site
# starts without them.
set --
stack=${STACK_STATE:-/stack}
if [ -n "${BOAT_API_KEY:-}" ] && [ -s "$stack/service.key" ]; then
    envs=${WEB_STATE:-/tmp}/environments
    mkdir -p "$envs"
    chmod 700 "$envs"
    cat "$stack/service.key" > "$p/model.key"
    printf '{"schema":"openagents.environment.studio.v1","state":"%s","machines":{"schema":"openagents.environment.owners.v1","provider":"boat","workdir":"/home/user/repo","credential_names":[],"tick_seconds":15},"owner":{"workspace":"openagents-web","principal":"web"},"model":"%s","size":"small","deadline_seconds":7200,"model_api":{"url":"http://127.0.0.1:8791/v1/responses","key_file":"%s/model.key"}}' \
        "$envs" "${ENVIRONMENTS_MODEL:-google/gemini-3.8-flash}" "$p" > "$p/environments.json"
    set -- --environments "$p/environments.json"
fi
exec /usr/local/bin/openagents-web --listen 0.0.0.0:8080 "$@" \
  --public-host "${PUBLIC_ORIGIN#https://}" --public-host "$ALT_HOST" --public-host "$RUN_HOST" \
  --everglade /srv/everglade --components-build /srv/components \
  --cloud-build /srv/cloud --chat-build /srv/chat --bunny /srv/bunny \
  --chat-bucket "$CHAT_BUCKET" \
  --cloud-config "$p/cloud.json" --github-oauth "$p/github-oauth.json" \
  --cloud-byo "$byo" --plan-meter /tmp/plan-meter.sqlite \
  --inference http://127.0.0.1:8791
