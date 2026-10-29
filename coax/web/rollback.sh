#!/usr/bin/env bash
# Put back the pages production served before a release (release.sh keeps
# them under releases/<time>/ in the production bucket). The files those
# pages name are content-named and never deleted, so the pages alone are
# the whole rollback.
#   rollback.sh             # list the kept releases
#   rollback.sh <time>      # restore the pages kept at <time>
set -euo pipefail
bucket=openagentsgemini-openagents-web
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
if [[ -z "${1:-}" ]]; then gcloud storage ls "gs://$bucket/releases/"; exit 0; fi
kept="gs://$bucket/releases/$1"
pages=$(gcloud storage ls "$kept/" | grep -E '\.(html|md)$' || true)
[[ -n "$pages" ]] || { echo "rollback: no pages kept at $1 (the first release has nothing before it)" >&2; exit 1; }
for p in $pages; do
  gcloud storage cp "$p" "gs://$bucket/$(basename "$p")" --cache-control "no-cache" --quiet > /dev/null
done
# Production no longer runs the newest release that passed: a newer entry
# without PASSED says so, so the next release ships and walks in full.
echo "rollback to $1" | gcloud storage cp - "gs://$bucket/releases/$(date -u +%Y%m%dT%H%M%SZ)/ROLLBACK" --quiet > /dev/null ||
  echo "rollback: couldn't mark the rollback under releases/; until a release passes, release.sh may take the rolled-back commit as live" >&2
echo "rollback: production pages restored from $1"
