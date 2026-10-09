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
exec /usr/local/bin/openagents-web --listen 0.0.0.0:8080 \
  --public-host "${PUBLIC_ORIGIN#https://}" --public-host "$ALT_HOST" --public-host "$RUN_HOST" \
  --everglade /srv/everglade --components-build /srv/components \
  --cloud-build /srv/cloud --chat-build /srv/chat --bunny /srv/bunny \
  --chat-bucket "$CHAT_BUCKET" \
  --cloud-config "$p/cloud.json" --github-oauth "$p/github-oauth.json" \
  --cloud-byo "$byo" --plan-meter /tmp/plan-meter.sqlite \
  --inference http://127.0.0.1:8791
