#!/usr/bin/env bash
# Carries the demo's recorded data over (pt-import-demo-data): copies the
# bucket aethyr-demo-data to a temporary folder (read only: the demo's bucket
# is never written, moved or deleted), then runs import_demo.py into the
# staging or production database through a Cloud SQL proxy on this machine.
# Safe to run again at any time, just before the switch-over too: it adds only
# what the demo recorded since.
#
#   coax/web/api/import_demo.sh --staging      oa_analytics_staging
#   coax/web/api/import_demo.sh --production   oa_analytics
#
# The database must have migration 0009 (deploy.sh applies it on start).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
case "${1:-}" in
  --staging) secret=oa-analytics-staging-database-url ;;
  --production) secret=oa-analytics-database-url ;;
  *) echo "usage: import_demo.sh --staging|--production" >&2; exit 2 ;;
esac
project=openagentsgemini
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
work="$(mktemp -d)"; chmod 700 "$work"
trap 'kill ${proxy:-} 2>/dev/null || true; rm -rf "$work"' EXIT

gcloud storage rsync -r gs://aethyr-demo-data "$work/data" --quiet >/dev/null 2>"$work/rsync.log" \
  || { cat "$work/rsync.log" >&2; exit 1; }

port=$((55800 + RANDOM % 200))
cloud-sql-proxy "$project:us-central1:openagents-production-pg" --port "$port" \
  --token "$(gcloud auth print-access-token)" >"$work/proxy.log" 2>&1 & proxy=$!
for _ in $(seq 30); do grep -qs "ready for new connections" "$work/proxy.log" && break; sleep 0.5; done
# The secret is a libpq conninfo for the Cloud Run socket; here it goes through the proxy.
url="$(gcloud secrets versions access latest --secret "$secret" --project "$project")"
# The password goes in the environment, not on a command line.
PGPASSWORD="$(printf '%s' "$url" | sed -nE 's/.*password=([^ ]+).*/\1/p')"; export PGPASSWORD
conn="$(printf '%s' "$url" | sed -E "s#host=[^ ]+#host=127.0.0.1 port=$port#; s#password=[^ ]+##")"
python3 -I "$here/import_demo.py" "$work/data" --db "$conn" --psql "$(command -v psql)"
