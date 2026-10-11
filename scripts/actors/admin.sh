#!/bin/sh
# Run actors-admin against the actor store of staging or production
# (#11253, docs/deployment/actors.md), through the Cloud SQL proxy as the
# automation account. The connection string comes from Secret Manager into
# this process's environment only; it is never printed or written to disk.
#
#   scripts/actors/admin.sh staging|production COMMAND [WORKSPACE ...]
#
#   ACTORS_ACCOUNT_ID=acct_... scripts/actors/admin.sh staging list ws_... mac.job
#   ACTORS_ACCOUNT_ID=acct_... scripts/actors/admin.sh staging inspect ws_... mac.job mjob...
#   ACTORS_ACCOUNT_ID=acct_... scripts/actors/admin.sh staging history ws_... mac.job mjob...
#   echo '{"expected_epoch":3,"retry":false,"outcome":{"done":{"summary":"..."}}}' |
#     ACTORS_ACCOUNT_ID=acct_... scripts/actors/admin.sh staging resolve-work ws_... UID ITEM
#
# ACTORS_ADMIN (default: target/debug/actors-admin, built with
# `cargo build -p actors --bin actors-admin`) is the binary; ACTORS_OPERATOR
# names you in the history (default: your git email).
set -eu
env=${1:?staging or production}
shift
case $env in
    staging|production) ;;
    *) echo "Choose staging or production." >&2; exit 2 ;;
esac
root=$(git rev-parse --show-toplevel)
admin=${ACTORS_ADMIN:-${CARGO_TARGET_DIR:-$root/target}/debug/actors-admin}
[ -x "$admin" ] || { echo "Build it first: cargo build -p actors --bin actors-admin" >&2; exit 1; }
sa=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
instance=openagentsgemini:us-central1:openagents-$env-pg
port=${ACTORS_PROXY_PORT:-55499}
token=$(CLOUDSDK_CONFIG=$sa gcloud auth print-access-token)
cloud-sql-proxy "$instance" --port "$port" --token "$token" > /dev/null 2>&1 &
proxy=$!
trap 'kill $proxy 2>/dev/null' EXIT INT TERM
i=0
until nc -z 127.0.0.1 "$port" 2> /dev/null; do
    i=$((i + 1))
    [ $i -lt 50 ] || { echo "The Cloud SQL proxy didn't start." >&2; exit 1; }
    sleep 0.2
done
dsn=$(CLOUDSDK_CONFIG=$sa gcloud secrets versions access latest \
    --secret "openagents-$env-pg-dsn" --project openagentsgemini)
# The secret names the connector's socket; through the proxy it is TCP.
ACTORS_DATABASE_URL=$(printf '%s' "$dsn" | sed -E "s#host=[^ ]+#host=127.0.0.1 port=$port#")
export ACTORS_DATABASE_URL
export ACTORS_OPERATOR=${ACTORS_OPERATOR:-$(git config user.email || echo operator)}
unset dsn token
"$admin" "$@"
