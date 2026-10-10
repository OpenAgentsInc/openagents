#!/bin/sh
# The staging gateway sidecar: the account service (GitHub sign-in,
# sessions, device sign-in, projects) and the inference gateway in one
# process, as scripts/dev/full-local.sh runs it, on 127.0.0.1:8791.
#
# State lives in $STACK_STATE (/stack), an NFS volume on the account-store
# server (docs/deployment/account-storage.md), so accounts, sessions, API
# keys, sealed GitHub tokens, and saved provider keys outlive the
# instance. Secrets arrive as environment variables from Secret Manager
# and are written to owner-only files in the container's own memory
# (/tmp/private), never to the shared disk; none is printed.
#
# One gateway at a time. The stores assume a single writer (the quota
# ledger holds its lock for the process's life; the provider-key store is
# cached in memory), and a deploy briefly runs the old and the new
# instance side by side. So a starting gateway claims the store
# ($state/handoff/takeover), the running one sees the claim, stops its
# gateway, and marks the store released ($state/handoff/holder), and only
# then does the new one start. A holder that never answers (a lost
# instance) is taken over after 150 seconds. Reads go through `cat`, an
# open(), which NFS revalidates against the server.
#
# GATEWAY_HOLD=serving (production, where a deploy first starts a
# no-traffic candidate): only an instance of the revision that serves
# traffic holds the store. A starting gateway waits, without claiming,
# until Cloud Run's traffic gives its revision (K_REVISION) a share; one
# that handed the store over waits until its revision loses its traffic
# and gets it back (an update-traffic rollback) before claiming again. A
# candidate being smoke-tested therefore never takes the store from the
# revision serving openagents.com. Traffic is read from the Cloud Run API
# with the runtime account's token from the metadata server.
#
# Environment:
#   PUBLIC_ORIGIN               https://... the site's origin (the OAuth callback base)
#   GITHUB_OAUTH_JSON           the staging OAuth App's private file
#   INFERENCE_ADMIN_TOKEN       the /admin/inference bearer
#   SMOKE_SIGNUP_TOKEN          optional: the operator token that makes the smoke
#                               suite's test account (staging only); open sign-up stays off
#   OPENAGENTS_ACCOUNTS_DATABASE_URL  optional: the account database's connection
#                               string; set, the account stores live in Postgres (#11154)
#   INVITE_ONLY_JSON            optional: accounts.invite_only, the GitHub people who
#                               may sign in ({"github": [{"id": N, "login": L, "admin": true}]})
#   GATEWAY_HOLD                optional: `serving` (above)
#   BYOK_KEYRING_JSON           optional: the oa-seal keyring that seals workspaces'
#                               own provider keys (Settings > API keys)
#   INFERENCE_STORE_KEY         optional: the key that seals stored responses
#   VERTEX_SA_JSON              optional: a service-account key for Vertex
#   OPENROUTER_API_KEY, AI_GATEWAY_API_KEY, TYPESAFE_API_KEY: optional upstreams
#   DECISION_PYLONS             optional: comma-separated hex keys of the Pylons whose
#                               beacons POST /v1/systemone sends decisions to (#11225);
#                               default CoderOS-4080's pylon key
#   DECISION_CLEF_URL           optional: our hosted Clef (`/v1/systemone`) after the Pylons
#   DECISION_JEV                ignored since the owner put Jev first again (#11225): the
#                               gateway asks Jev first under TYPESAFE_API_KEY, else the
#                               Secret Manager key openagents-gateway-production-typesafe-key
set -eu
umask 077
state=${STACK_STATE:-/stack}
private=${STACK_PRIVATE:-/tmp/private}
mkdir -p "$state/gateway/attempts" "$state/handoff" "$private"
chmod 700 "$state/gateway" "$state/handoff" "$private"

: "${PUBLIC_ORIGIN:?PUBLIC_ORIGIN is unset}"
: "${GITHUB_OAUTH_JSON:?GITHUB_OAUTH_JSON is unset}"
: "${INFERENCE_ADMIN_TOKEN:?INFERENCE_ADMIN_TOKEN is unset}"
printf '%s' "$GITHUB_OAUTH_JSON" > "$private/github-oauth.json"
unset GITHUB_OAUTH_JSON
if [ -n "${VERTEX_SA_JSON:-}" ]; then
    printf '%s' "$VERTEX_SA_JSON" > "$private/vertex.json"
    unset VERTEX_SA_JSON
    export GOOGLE_APPLICATION_CREDENTIALS="$private/vertex.json"
fi
byok=""
if [ -n "${BYOK_KEYRING_JSON:-}" ]; then
    printf '%s' "$BYOK_KEYRING_JSON" > "$private/byok-keyring.json"
    unset BYOK_KEYRING_JSON
    byok=",
    \"byok\": {\"keyring\": \"$private/byok-keyring.json\"}"
fi

# --- the handoff --------------------------------------------------------
meta() {
    curl -fsS -H 'Metadata-Flavor: Google' \
        "http://metadata.google.internal/computeMetadata/v1/$1" 2> /dev/null
}
instance=$(meta instance/id || hostname)
hold=${GATEWAY_HOLD:-}
# Whether Cloud Run sends this revision a share of the traffic.
serving() {
    token=$(meta instance/service-accounts/default/token |
        sed -n 's/.*"access_token" *: *"\([^"]*\)".*/\1/p')
    project=$(meta project/project-id)
    region=$(meta instance/region | sed 's#.*/##')
    [ -n "$token" ] && [ -n "$project" ] && [ -n "$region" ] || return 1
    curl -fsS -H "Authorization: Bearer $token" \
        "https://run.googleapis.com/v2/projects/$project/locations/$region/services/${K_SERVICE:?}" \
        2> /dev/null | tr -d ' \n' |
        grep -q "\"revision\":\"${K_REVISION:?}\",\"percent\":[1-9]"
}
# Wait until serving() is $1 (0: serving, 1: not), polling every 5 s.
until_serving() {
    while :; do
        if serving; then now=0; else now=1; fi
        [ "$now" = "$1" ] && return 0
        sleep 5 &
        wait $! || true
    done
}
if [ "$hold" = serving ]; then
    trap 'exit 0' TERM INT
    echo "gateway: waiting for this revision to serve traffic before taking the store" >&2
    until_serving 0
    trap - TERM INT
fi
put() {
    printf '%s\n' "$2" > "$1.$$.tmp"
    mv "$1.$$.tmp" "$1"
}
get() {
    cat "$1" 2> /dev/null || true
}
idle() {
    trap 'exit 0' TERM INT
    while :; do
        sleep 3600 &
        wait $! || true
    done
}
# After handing the store over: idle, or with GATEWAY_HOLD=serving wait
# for this revision to lose its traffic and get it back, then return to
# claim the store again.
rejoin() {
    [ "$hold" = serving ] || idle
    trap 'exit 0' TERM INT
    until_serving 1
    echo "gateway: this revision no longer serves traffic; waiting to get it back" >&2
    until_serving 0
    echo "gateway: this revision serves traffic again; taking the store back" >&2
    trap - TERM INT
}
# Claim the store, run the gateway, and return only after handing the
# store over and rejoining (GATEWAY_HOLD=serving).
take() {
me="$instance.$(date +%s).$$"
put "$state/handoff/takeover" "$me"
waited=0
while [ -e "$state/handoff/holder" ]; do
    holder=$(get "$state/handoff/holder")
    case "$holder" in
        released | "$instance".*) break ;;
    esac
    claim=$(get "$state/handoff/takeover")
    if [ -n "$claim" ] && [ "$claim" != "$me" ]; then
        echo "gateway: a newer instance claimed the store first; not starting" >&2
        rejoin
        return 0
    fi
    if [ "$waited" -ge 150 ]; then
        echo "gateway: the previous holder never released the store; taking it over" >&2
        break
    fi
    sleep 1
    waited=$((waited + 1))
done
put "$state/handoff/holder" "$me"
# No other gateway runs now, so any lock file on the share is stale.
find "$state/gateway" -name '*.lock' -type f -delete
echo "gateway: holding the store after ${waited}s"

# One registry: `house` is the service tenant the chat worker calls
# inference as (its key goes to $state/service.key for the worker
# sidecar); `signup` holds new accounts' personal workspaces.
if [ ! -s "$state/service.key" ]; then
    bootstrap_registry --registry "$state/gateway/registry" \
        --tenant house --also-tenant signup --door local-kev --model kev-latest \
        --signature "sha256:0000000000000000000000000000000000000000000000000000000000000000" \
        > "$state/service.key.tmp"
    mv "$state/service.key.tmp" "$state/service.key"
fi
chmod 600 "$state/service.key"

operator=""
if [ -n "${SMOKE_SIGNUP_TOKEN:-}" ]; then
    operator='"operator_signup_token_env": "SMOKE_SIGNUP_TOKEN",'
fi
invite=""
if [ -n "${INVITE_ONLY_JSON:-}" ]; then
    invite="\"invite_only\": $INVITE_ONLY_JSON,"
fi
# With the account database's connection string (Secret Manager, through
# the Cloud SQL connector's socket), accounts, sessions, API keys, GitHub
# access and provider keys live in Postgres (#11154,
# docs/data/schema.md). The first start moves the files in and leaves
# postgres-import.json beside them; the files stay as the rollback.
database=""
if [ -n "${OPENAGENTS_ACCOUNTS_DATABASE_URL:-}" ]; then
    database='"store": "postgres", "import_files": true,'
fi
# Decisions (#11225): POST /v1/systemone asks Jev (TypeSafe) first under the
# house key, then a connected Pylon over Nostr (NIP-DEC to the beacons of
# DECISION_PYLONS), then our hosted Clef, then Gemini on Vertex AI with
# structured output. `jev` below is kept for older binaries and ignored.
pylons=$(printf '%s' "${DECISION_PYLONS:-95bc752118e119f852d73741e5f49438cf5e9dce4f3185591014cbb7c311eb32}" |
    tr -d ' ' | sed 's/,*$//; s/,/","/g')
clef=""
if [ -n "${DECISION_CLEF_URL:-}" ]; then
    clef="\"clef_url\": \"$DECISION_CLEF_URL\","
fi
jev=false
[ "${DECISION_JEV:-}" = on ] && jev=true
cat > "$private/gateway.json" << EOF
{
  "v": "openagents.gateway.v1",
  "listen": "0.0.0.0:8791",
  "registry": "$state/gateway/registry",
  "accounts": {
    "signup_tenant": "signup",
    $operator
    $invite
    $database
    "github": {
      "credentials": "$private/github-oauth.json",
      "redirect_url": "$PUBLIC_ORIGIN/auth/github/callback"
    }
  },
  "inference": {
    "admin_token_env": "INFERENCE_ADMIN_TOKEN",
    "own_coders": {"web": "http://127.0.0.1:8080", "token_file": "$state/own-runs.key"},
    "service_tenants": ["house"],
    "journal": "$state/gateway/attempts",
    "public": {"free_tier": {"requests_per_day": 20, "models": ["google/gemini-2.5-flash-lite"]}},
    "accounts": [
      {"id": "google-credit", "upstream": "vertex", "granted": 30000000000, "balance": 30000000000, "basis": "prepaid"},
      {"id": "zai-credit", "upstream": "zai", "granted": 100000000, "balance": 100000000, "basis": "prepaid"},
      {"id": "pro-free-capacity", "upstream": "pro", "granted": 0, "balance": 0, "basis": "free_capacity"},
      {"id": "openrouter", "upstream": "openrouter", "granted": 0, "balance": 0, "basis": "pay_as_you_go"},
      {"id": "vercel", "upstream": "vercel", "granted": 0, "balance": 0, "basis": "pay_as_you_go"}
    ]$byok
  },
  "decisions": {
    $clef
    "pylons": ["$pylons"],
    "jev": $jev
  }
}
EOF

cd "$state/gateway"
gateway --config "$private/gateway.json" &
child=$!
yielded="$private/yielded"
rm -f "$yielded"
# The watcher: when another instance claims the store, stop the gateway.
(
    while sleep 2; do
        claim=$(get "$state/handoff/takeover")
        if [ -n "$claim" ] && [ "$claim" != "$me" ]; then
            echo "gateway: a new instance claimed the store; handing it over" >&2
            : > "$yielded"
            kill -TERM "$child" 2> /dev/null || true
            exit 0
        fi
    done
) &
watcher=$!
release() {
    if [ "$(get "$state/handoff/holder")" = "$me" ]; then
        put "$state/handoff/holder" released
    fi
}
trap 'kill "$watcher" "$child" 2> /dev/null || true; wait "$child" 2> /dev/null || true; release; exit 0' TERM INT
status=0
wait "$child" || status=$?
kill "$watcher" 2> /dev/null || true
if [ -e "$yielded" ]; then
    release
    rejoin
    return 0
fi
# The gateway stopped on its own: exit, and Cloud Run restarts this
# container, which takes the store straight back (same instance).
exit "$status"
}
while :; do
    take
done
