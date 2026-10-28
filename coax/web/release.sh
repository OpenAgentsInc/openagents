#!/usr/bin/env bash
# Release openagents.com: build committed code once, put it on staging, smoke
# it, then promote the same bytes to production and smoke that.
#
#   release.sh                     # origin/main → staging → smoke → production → smoke
#   release.sh --ref <commit>      # a specific commit
#   release.sh --staging-only      # stop after staging's smoke
#   release.sh --skip-staging --yes-skip-staging   # emergencies only: straight to production
#
# The build comes from a clean worktree of the ref, never from a working
# directory other agents are editing. Production gets the files staging was
# smoked with, copied bucket to bucket, so what passed is what ships. The
# pages production served before are kept under releases/<time>/ in its
# bucket; rollback.sh <time> puts them back. A failed production smoke rolls
# back on its own.
#
# Routing (one load balancer, coaxial-openagents-web-staging):
#   staging.openagents.com → the staging bucket
#   openagents.com         → the production bucket for / and the Coax files
#                            (/build/*, /coaxial/*), the coder service for the rest
#                            until it is retired (docs/2026-10-28-coder-service-deprecation.md)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
ref=origin/main stage=true prod=true
while (($#)); do
  case "$1" in
    --ref) ref="$2"; shift 2 ;;
    --staging-only) prod=false; shift ;;
    --skip-staging) stage=false; shift ;;
    --yes-skip-staging) skip_ok=1; shift ;;
    *) echo "release: unknown option $1" >&2; exit 2 ;;
  esac
done
if ! $stage && [[ -z "${skip_ok:-}" ]]; then
  echo "release: --skip-staging ships untested bytes; add --yes-skip-staging if you mean it" >&2; exit 2
fi
project=openagentsgemini
staging_bucket=$project-openagents-web-staging prod_bucket=$project-openagents-web
lb_ip=136.69.24.60
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
now() { python3 -c 'import time; print(time.time())'; }
lap() { printf '  %-10s %5.1f s\n' "$1" "$(python3 -c "print($(now) - $2)")"; }
start=$(now)

git -C "$repo" fetch -q origin main
commit="$(git -C "$repo" rev-parse --short "$ref")"
tree="$(mktemp -d)/release"; trap 'git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true' EXIT
git -C "$repo" worktree add -q --detach "$tree" "$commit"
echo "release $commit ($(git -C "$tree" log -1 --format=%s | cut -c1-70))"
# The compiler is built per worktree; reuse this checkout's if the tree has none.
[[ -x "$tree/coax/bin/coax" ]] || { mkdir -p "$tree/coax/bin"; cp "$repo/coax/bin/coax" "$tree/coax/bin/"; }

if $stage; then
  t=$(now); "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > /dev/null; lap staging "$t"
  t=$(now); "$here/smoke.sh" https://staging.openagents.com; lap smoke "$t"
fi
$prod || { echo "release $commit: staging only, done in $(python3 -c "print(round($(now) - $start, 1))") s"; exit 0; }
if ! $stage; then
  t=$(now); "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web > /dev/null; lap build "$t"
fi

# Promote: keep production's current pages, copy files first (content-named,
# nothing points at them yet), then the pages that name them.
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
if $stage; then
  t=$(now)
  gcloud storage cp "gs://$prod_bucket/*.html" "gs://$prod_bucket/*.md" "gs://$prod_bucket/releases/$stamp/" --quiet > /dev/null 2>&1 || true
  gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude '.*\.(html|md)$|^releases/' --quiet > /dev/null
  gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude '^releases/' --quiet > /dev/null
  lap promote "$t"
fi
echo "$commit $stamp" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/COMMIT" --quiet > /dev/null

t=$(now)
via=""
[[ "$(dig +short openagents.com A | sort | head -1)" == "$lb_ip" ]] || via="--via $lb_ip"
if ! "$here/smoke.sh" https://openagents.com $via --coder; then
  echo "release $commit: production smoke failed; rolling back to the pages from before" >&2
  "$here/rollback.sh" "$stamp"
  exit 1
fi
lap smoke "$t"
echo "release $commit: live on openagents.com in $(python3 -c "print(round($(now) - $start, 1))") s (rollback: coax/web/rollback.sh $stamp)"
