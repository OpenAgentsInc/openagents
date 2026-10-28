#!/usr/bin/env bash
# Release openagents.com: build committed code once, put it on staging, smoke
# it, then promote the same bytes to production and smoke that.
#
#   release.sh                     # origin/main → staging → smoke → production → smoke
#   release.sh --ref <commit>      # a specific commit
#   release.sh --staging-only      # stop after staging's smoke
#   release.sh --skip-staging --yes-skip-staging   # emergencies only: straight to production
#
# Staging's smoke is the HTTP checks (smoke.sh), then the playtest flow walked
# in headless Chrome against its golden screenshots (goldens.ts): each page
# must start and draw, not only answer 200. Production gets the walk too.
# A visual change fails the goldens until approved: look at the shot it
# printed, then `bun coax/web/goldens.ts https://staging.openagents.com
# --update` and commit coax/web/goldens/ with the change.
#
# The build comes from a clean worktree of the ref, never from a working
# directory other agents are editing. Production gets the files staging was
# smoked with, copied bucket to bucket, so what passed is what ships. The
# pages production served before are kept under releases/<time>/ in its
# bucket; rollback.sh <time> puts them back. A failed production smoke rolls
# back on its own.
#
# Pinned demos (pin.sh) live in the production bucket under demo/ and
# demo-releases/; a release never writes there, and checks it didn't.
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
now() { [[ -n "${EPOCHREALTIME:-}" ]] && echo "${EPOCHREALTIME//[.,]/}" || echo $(( $(date +%s) * 1000000 )); }   # microseconds
secs() { printf '%d.%d' $(( $1 / 1000000 )) $(( $1 % 1000000 / 100000 )); }
lap() { printf '  %-10s %5s s\n' "$1" "$(secs $(( $(now) - $2 )))"; }
start=$(now)

# One release at a time on this machine: a second one waits, since both
# rewrite the staging bucket and promote from it. A lock left by a dead
# release is taken over.
lock="${TMPDIR:-/tmp}/coaxial-release.lock"
until mkdir "$lock" 2>/dev/null; do
  holder="$(cat "$lock/pid" 2>/dev/null || true)"
  if [[ -n "$holder" ]] && ! kill -0 "$holder" 2>/dev/null; then rm -rf "$lock"; continue; fi
  echo "release: waiting for release ${holder:-?} to finish"; sleep 5
done
echo $$ > "$lock/pid"
tree=""
cleanup() { [[ -z "$tree" ]] || git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true; rm -rf "$lock"; }
trap cleanup EXIT

git -C "$repo" fetch -q origin main
commit="$(git -C "$repo" rev-parse --short "$ref")"
tree="$(mktemp -d)/release"
git -C "$repo" worktree add -q --detach "$tree" "$commit"
echo "release $commit ($(git -C "$tree" log -1 --format=%s | cut -c1-70))"
# The compiler is built per worktree; reuse this checkout's if the tree has none.
[[ -x "$tree/coax/bin/coax" ]] || { mkdir -p "$tree/coax/bin"; cp "$repo/coax/bin/coax" "$tree/coax/bin/"; }

if $stage; then
  t=$(now); "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > /dev/null; lap staging "$t"
  t=$(now); "$tree/coax/web/smoke.sh" https://staging.openagents.com; lap smoke "$t"
  t=$(now)
  bun "$tree/coax/web/goldens.ts" https://staging.openagents.com ||
    { echo "release $commit: staging's pages don't start, draw or match their goldens; production is untouched" >&2; exit 1; }
  lap goldens "$t"
fi
$prod || { echo "release $commit: staging only, done in $(secs $(( $(now) - start ))) s"; exit 0; }
if ! $stage; then
  t=$(now); "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web > /dev/null; lap build "$t"
fi

# Promote: keep production's current pages, copy files first (content-named,
# nothing points at them yet), then the pages that name them.
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
demos() { gcloud storage ls -L "gs://$prod_bucket/demo/**" 2>/dev/null | grep -E '^gs://|Hash \(md5\)' || true; }
demos_before="$(demos)"
if $stage; then
  # The backup and the files copy touch different objects, so they run
  # together; the pages wait for both.
  t=$(now)
  { gcloud storage cp "gs://$prod_bucket/*.html" "gs://$prod_bucket/*.md" "gs://$prod_bucket/releases/$stamp/" --quiet > /dev/null 2>&1 || true
    echo "$commit $stamp" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/COMMIT" --quiet > /dev/null; } &
  backup=$!
  gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude '.*\.(html|md)$|^releases/|^demo/|^demo-releases/' --quiet > /dev/null
  wait $backup
  gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude '^releases/|^demo/|^demo-releases/' --quiet > /dev/null
  lap promote "$t"
else
  echo "$commit $stamp" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/COMMIT" --quiet > /dev/null
fi

[[ "$(demos)" == "$demos_before" ]] || { echo "release $commit: a pinned demo changed (demo/ in $prod_bucket); it must not" >&2; exit 1; }
t=$(now)
via=""
[[ "$(dig +short openagents.com A | sort | head -1)" == "$lb_ip" ]] || via="--via $lb_ip"
walked=true
[[ -n "$via" ]] || bun "$tree/coax/web/goldens.ts" https://openagents.com --walk || walked=false
if ! $walked || ! "$tree/coax/web/smoke.sh" https://openagents.com $via --coder; then
  echo "release $commit: production smoke failed; rolling back to the pages from before" >&2
  "$here/rollback.sh" "$stamp"
  exit 1
fi
lap smoke "$t"
echo "release $commit: live on openagents.com in $(secs $(( $(now) - start ))) s (rollback: coax/web/rollback.sh $stamp)"
