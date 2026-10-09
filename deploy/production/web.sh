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
exec /usr/local/bin/openagents-web "$@" \
    --cloud-config "$p/cloud.json" --github-oauth "$p/github-oauth.json" \
    --cloud-byo "$byo" --plan-meter /tmp/plan-meter.sqlite \
    --inference http://127.0.0.1:8791
