# The staging web container's launcher, run by /bin/sh -c in the
# openagents-web image (service.yaml inlines this file; keep them in step).
# It writes the secrets Cloud Run passes as environment variables into
# owner-only files the site's private-file checks accept, then starts the
# site the way scripts/dev/full-local.sh does, with the account service and
# the inference gateway on the gateway sidecar.
set -eu
umask 077
p=/tmp/private
mkdir -p "$p" /tmp/byo
printf '%s' "$GITHUB_OAUTH_JSON" > "$p/github-oauth.json"
printf '%s' "$CSRF_KEY" > "$p/csrf.key"
unset GITHUB_OAUTH_JSON CSRF_KEY
printf '{"schema":"openagents.cloud.web-config.v1","public_origin":"%s","account_service":"http://127.0.0.1:8791","csrf_secret":"%s/csrf.key"}' "$PUBLIC_ORIGIN" "$p" > "$p/cloud.json"
exec /usr/local/bin/openagents-web --listen 0.0.0.0:8080 \
  --public-host "${PUBLIC_ORIGIN#https://}" --public-host "$ALT_HOST" \
  --everglade /srv/everglade --components-build /srv/components \
  --cloud-build /srv/cloud --chat-build /srv/chat --bunny /srv/bunny \
  --chat-bucket "$CHAT_BUCKET" \
  --cloud-config "$p/cloud.json" --github-oauth "$p/github-oauth.json" \
  --cloud-byo /tmp/byo --plan-meter /tmp/plan-meter.sqlite \
  --inference http://127.0.0.1:8791
