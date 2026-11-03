#!/usr/bin/env bash
# Release openagents.com: build origin/main once, put it on staging, check it,
# promote the same bytes to production, check that.
#
#   release.sh                  # origin/main → staging → production
#   release.sh --ref COMMIT     # a specific commit
#   release.sh --staging-only   # stop after staging's checks
#
# One rule decides everything (owner, 2026-11-02): a release stops only for a
# check that FAILS ON STAGING AND PASSES ON PRODUCTION, confirmed by a retry.
# Two checks, each run on both: smoke.sh (HTTP) and goldens.ts --starts (every
# page answers, its engine starts, no errors). Failures production already has
# never stop a release; more of them always do, page by page. After the promote
# the same two checks run on production; a new failure rolls back.
#
# Everything else is a report and never stops a release: tools-check,
# check-visuals, the release metadata check, the golden walk, 60 FPS, every
# world's frame rate, the maker journey, the changed suites (after-release.sh,
# in the background, written to releases/<time>/).
#
# One release at a time across every machine: the lock is an object in the
# production bucket (release-lib.sh). A release waiting for it ships origin/main
# as it is when it gets the lock, and does nothing if production already has it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
ref=origin/main prod=true
while (($#)); do
  case "$1" in
    --ref) ref="$2"; shift 2 ;;
    --staging-only) prod=false; shift ;;
    *) echo "release: unknown option $1 (release.sh [--ref COMMIT] [--staging-only])" >&2; exit 2 ;;
  esac
done
source "$here/release-lib.sh"
source "$here/smoke-compare.sh"
staging_bucket=openagentsgemini-openagents-web-staging
staging=https://staging.openagents.com production=https://openagents.com
now() { [[ -n "${EPOCHREALTIME:-}" ]] && echo "${EPOCHREALTIME//[.,]/}" || echo $(( $(date +%s) * 1000000 )); }
secs() { printf '%d.%d' $(( $1 / 1000000 )) $(( $1 % 1000000 / 100000 )); }
lap() { printf '  %-10s %5s s\n' "$1" "$(secs $(( $(now) - $2 )))"; }
start=$(now)

# A command in its own process group with a time limit; whatever it leaves (a Chrome) goes with it.
group=""
bounded() {
  local limit=$1; shift
  python3 -I -c 'import os, sys; os.setpgid(0, 0); os.execvp(sys.argv[1], sys.argv[1:])' "$@" < /dev/null & local pid=$!
  group=$pid
  ( sleep "$limit" && kill -TERM -- "-$pid" 2>/dev/null && sleep 5 && kill -KILL -- "-$pid" 2>/dev/null ) & local dog=$!
  local status=0; wait "$pid" || status=$?
  kill "$dog" 2>/dev/null; wait "$dog" 2>/dev/null || true
  kill -KILL -- "-$pid" 2>/dev/null || true
  group=""
  return "$status"
}

# The page checks a starts walk failed, without the URL or error detail: "waterlab: no errors".
starts_fails() { sed -n 's/^  FAIL  //p' | sed -E 's# https?://[^ ]*##; s/ \(.*$//' | sort -u; }
starts() { bounded 900 bun "$tree/coax/web/goldens.ts" "$1" --starts 2>&1 || true; }
# new_starts STAGING_OUT BASE_FAILS URL: failures new against BASE, still failing on a second walk.
new_starts() {
  local broke again
  broke=$(new_fails "$(starts_fails <<< "$1")" "$2")
  [[ -n "$broke" ]] || return 0
  echo "  starts     $(wc -l <<< "$broke" | tr -d ' ') new failure(s); walking again to confirm" >&2
  sleep 10
  again=$(starts "$3" | starts_fails)
  comm -12 <(printf '%s\n' "$broke" | sort -u) <(printf '%s\n' "$again" | sed '/^$/d' | sort -u)
}

live() {   # "COMMIT STAMP ..." of the newest release that went live
  local newest
  newest="$(gcloud storage ls "gs://$prod_bucket/releases/" 2>/dev/null | grep -E '/releases/[0-9]{8}T[0-9]{6}Z/$' | sort | tail -1)"
  [[ -n "$newest" ]] && gcloud storage cat "${newest}PASSED" 2>/dev/null | head -1 || true
}

tree=""
cleanup() {
  [[ -z "$group" ]] || kill -KILL -- "-$group" 2>/dev/null || true
  [[ -z "$tree" ]] || git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true
  drop_lock
}
trap cleanup EXIT

take_lock "release $ref"
git -C "$repo" fetch -q origin main
full="$(git -C "$repo" rev-parse "$ref^{commit}")"; commit="${full:0:10}"
passed="$(live)"
if $prod && [[ -n "$passed" ]] && git -C "$repo" merge-base --is-ancestor "$full" "${passed%% *}" 2>/dev/null; then
  echo "release $commit: already live (production runs ${passed%% *}); nothing to do"
  exit 0
fi
tree="$(mktemp -d)/release"
git -C "$repo" worktree add -q --detach "$tree" "$full"
echo "release $commit ($(git -C "$tree" log -1 --format=%s | cut -c1-70))"

t=$(now); compiler "$tree" || exit 1; lap compiler "$t"

# Reports: they never stop a release.
reports="$(mktemp -d)"
compiler_env COAX_RELEASE_COMPILER="$tree/coax/bin/coax" "$tree/scripts/coax-release.sh" --check --binary "$tree/coax/bin/coax" > "$reports/metadata" 2>&1 ||
  echo "  report     the release metadata check fails (not stopping): $(tail -1 "$reports/metadata")"
(cd "$tree" && bounded 300 coax/web/tool.sh tools-check) > "$reports/tools" 2>&1 ||
  echo "  report     tools-check fails (not stopping): $(tail -1 "$reports/tools")"
"$tree/coax/web/check-visuals.sh" "$tree/apps/openagents-web" > "$reports/visuals" 2>&1 ||
  echo "  report     check-visuals fails (not stopping): $(head -1 "$reports/visuals")"

# What production fails today, measured while staging builds.
base="$(mktemp -d)"
( "$tree/coax/web/smoke.sh" "$production" > "$base/smoke" 2>&1 || true
  starts "$production" > "$base/starts" ) &
baseline_job=$!

t=$(now); COAX_VISUALS=report "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > /dev/null; lap staging "$t"
wait "$baseline_job" || true
base_smoke="$(fails_of < "$base/smoke")"; base_starts="$(starts_fails < "$base/starts")"

t=$(now)
out=$("$tree/coax/web/smoke.sh" "$staging" 2>&1) || true
broke=$(broken_by_release "$out" "$base_smoke" "$tree/coax/web/smoke.sh" "$staging")
[[ -z "$broke" ]] || { echo "release $commit: STOPPED. staging smoke fails where production passes; production is untouched:" >&2; echo "$broke" | sed 's/^/  /' >&2; exit 1; }
lap smoke "$t"

t=$(now)
out=$(starts "$staging")
grep -q '^goldens ' <<< "$out" || { echo "release $commit: STOPPED. the staging starts walk did not run:" >&2; tail -5 <<< "$out" >&2; exit 1; }
broke=$(new_starts "$out" "$base_starts" "$staging")
[[ -z "$broke" ]] || { echo "release $commit: STOPPED. pages fail on staging that pass on production; production is untouched:" >&2; echo "$broke" | sed 's/^/  /' >&2; exit 1; }
echo "  starts     staging $(starts_fails <<< "$out" | sed '/^$/d' | wc -l | tr -d ' ') failing check(s), production $(sed '/^$/d' <<< "$base_starts" | wc -l | tr -d ' '); none new"
lap starts "$t"

$prod || { echo "release $commit: staging only, done in $(secs $(( $(now) - start ))) s"; exit 0; }

# Promote: keep production's pages for rollback, copy staging's files, then its pages.
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
demos() { gcloud storage ls -L "gs://$prod_bucket/demo/**" 2>/dev/null | grep -E '^gs://|Hash \(md5\)' || true; }
demos_before="$(demos)"
t=$(now)
gcloud storage cp "gs://$prod_bucket/*.html" "gs://$prod_bucket/*.md" "gs://$prod_bucket/releases/$stamp/" --quiet > /dev/null 2>&1 || true
echo "$commit $stamp" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/COMMIT" --quiet > /dev/null
# What people publish lives only in production: never overwritten from staging.
published='^projects/published(-slugs)?\.txt$|^realms/made-slugs\.txt$|^worlds/made-slugs\.txt$'
for kind in projects:published-slugs realms:made-slugs worlds:made-slugs; do
  for s in $(gcloud storage cat "gs://$staging_bucket/${kind%%:*}/${kind#*:}.txt" 2>/dev/null | grep -E '^[a-z0-9][a-z0-9-]{0,63}$' || true); do
    published+="|^${kind%%:*}/$s(/|\$)"
  done
done
skip="^releases/|^demo/|^demo-releases/|^release\.lock\$|$published"
gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude ".*\.(html|md)\$|$skip" --quiet > /dev/null
gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude "$skip" --quiet > /dev/null
lap promote "$t"
[[ "$(demos)" == "$demos_before" ]] || { echo "release $commit: a pinned demo changed (demo/ in $prod_bucket); rolling back" >&2; "$here/rollback.sh" "$stamp"; exit 1; }

# Production: the same two checks against what it failed before.
t=$(now)
out=$("$tree/coax/web/smoke.sh" "$production" 2>&1) || true
broke=$(broken_by_release "$out" "$base_smoke" "$tree/coax/web/smoke.sh" "$production")
if [[ -z "$broke" ]]; then broke=$(new_starts "$(starts "$production")" "$base_starts" "$production"); fi
if [[ -n "$broke" ]]; then
  echo "release $commit: ROLLED BACK. production fails where it passed before this release:" >&2
  echo "$broke" | sed 's/^/  /' >&2
  "$here/rollback.sh" "$stamp"
  exit 1
fi
lap production "$t"
echo "$full $stamp staged fast" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/PASSED" --quiet > /dev/null ||
  echo "release $commit: couldn't write releases/$stamp/PASSED" >&2
for r in metadata tools visuals; do gcloud storage cp "$reports/$r" "gs://$prod_bucket/releases/$stamp/${r^^}" --quiet > /dev/null 2>&1 || true; done
echo "release $commit: live on openagents.com in $(secs $(( $(now) - start ))) s (rollback: coax/web/rollback.sh $stamp)"
drop_lock

# Reports after the release, in the background: never a stop.
log="$(mktemp -d)/after.log"
nohup "$here/after-release.sh" "$full" "$stamp" "${passed%% *}" > "$log" 2>&1 < /dev/null &
disown
echo "release $commit: reports running after the release ($log, then releases/$stamp/)"
