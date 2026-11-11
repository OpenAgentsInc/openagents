#!/usr/bin/env bash
# Release openagents.com: build origin/main-green once, put it on staging,
# check it, promote the same bytes to production, check that.
#
#   release.sh                  # origin/main-green → staging → production
#   release.sh --ref COMMIT     # a specific commit (--ref origin/main: main's head)
#   release.sh --staging-only   # stop after staging's checks
#   release.sh --build-only     # build and stage the release in a scratch folder, then stop:
#                               # no lock, no upload, no checks against a site (a dry run of the build)
#
# The build reuses the hosts' build of the same commit (#13119): pages.sh
# fetches it from the shared store, checked against the commit's tree and its
# archive's SHA-256, and wasm.sh takes each module whose record names the same
# inputs and bytes (COAXIAL_PREBUILT=1). Every module's compile headroom and
# size budget is still checked against those bytes; a module the hosts didn't
# build, or whose sources differ here (the project tables come from Nostr at
# build time), is built here.
#
# One rule decides everything (owner, 2026-11-02): a release stops only for a
# check that FAILS ON STAGING AND PASSES ON PRODUCTION, confirmed by a retry.
# Two checks, each run on both: smoke.sh (HTTP) and goldens.ts --starts (every
# page answers, its engine starts, no errors). Failures production already has
# never stop a release; more of them always do, page by page. After the promote
# the same two checks run on production; a new failure rolls back.
#
# The Everglade drift check holds to the same rule: when the release changes Everglade, a drift
# it fails here, twice, stops the release only if production's commit holds it.
#
# Everything else is a report and never stops a release: tools-check,
# everglade-parity (no zone-local copy of a shared Everglade feature, #13075),
# the committed shaders against their Coax (wgsl-module --site --check),
# check-visuals, the release metadata check, the golden walk, 60 FPS, every
# world's frame rate, the maker journey, the changed suites (after-release.sh,
# in the background, written to releases/<time>/).
#
# One release at a time across every machine: the lock is an object in the
# production bucket (release-lib.sh). A release waiting for it ships origin/main-green
# as it is when it gets the lock, and does nothing if production already has it.
#
# main-green is the newest commit of main that passed the green-main
# pipeline's full check (coax/remote/after.sh, docs/verse/green-main-pipeline.md),
# so a fresh failure on main never blocks a release. With no main-green on the
# remote, the default is origin/main.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
ref="" prod=true build_only=false
while (($#)); do
  case "$1" in
    --ref) ref="$2"; shift 2 ;;
    --staging-only) prod=false; shift ;;
    --build-only) build_only=true; prod=false; shift ;;
    *) echo "release: unknown option $1 (release.sh [--ref COMMIT] [--staging-only] [--build-only])" >&2; exit 2 ;;
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
# 1800 s: on a loaded Mac (agents' builds and recordings) the walk has taken over 900 s, and a cut-off
# walk stops the release with no failure in it.
starts() { bounded 1800 bun "$tree/coax/web/goldens.ts" "$1" --starts ${2:+--only "$2"} 2>&1 || true; }
# new_starts STAGING_OUT BASE_FAILS URL: failures new against BASE, still failing on a second walk.
new_starts() {
  local broke again
  broke=$(new_fails "$(starts_fails <<< "$1")" "$2")
  [[ -n "$broke" ]] || return 0
  # Confirm by walking only the pages that failed, not every page again (#13133).
  local pages; pages=$(sed -n 's/^\([a-z0-9-]*\):.*/\1/p' <<< "$broke" | sort -u | paste -sd, -)
  echo "  starts     $(wc -l <<< "$broke" | tr -d ' ') new failure(s); walking ${pages:-them} again to confirm" >&2
  sleep 10
  again=$(starts "$3" "$pages" | starts_fails)
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

$build_only || take_lock "release ${ref:-origin/main-green}"
git -C "$repo" fetch -q origin main
if [[ -z "$ref" ]]; then
  if git -C "$repo" fetch -q origin +refs/heads/main-green:refs/remotes/origin/main-green 2>/dev/null; then
    ref=origin/main-green
    behind="$(git -C "$repo" rev-list --count origin/main-green..origin/main)"
    echo "release: main-green is $(git -C "$repo" rev-parse --short=10 origin/main-green), $behind commit(s) behind main; those ship once a full check passes on them"
    "$repo/coax/remote/after.sh" health > /dev/null 2>&1 ||
      echo "release: WARNING: the green-main watcher is not running on this computer, so main-green is not moving. Start it: coax/remote/after.sh install"
  else
    ref=origin/main
    echo "release: no main-green on origin; releasing origin/main"
  fi
fi
full="$(git -C "$repo" rev-parse "$ref^{commit}")"; commit="${full:0:10}"
passed="$(live)"
passed_commit="${passed%% *}"
if $prod && [[ -n "$passed" ]] && git -C "$repo" merge-base --is-ancestor "$full" "${passed%% *}" 2>/dev/null; then
  echo "release $commit: already live (production runs ${passed%% *}); nothing to do"
  exit 0
fi
tree="$(mktemp -d)/release"
git -C "$repo" worktree add -q --detach "$tree" "$full"
echo "release $commit ($(git -C "$tree" log -1 --format=%s | cut -c1-70))"

t=$(now); compiler "$tree" || exit 1; lap compiler "$t"

# The hosts' build of this commit, if the shared store has it: wasm.sh then
# builds only the modules it lacks (COAXIAL_PREBUILT).
t=$(now)
if pages_out="$(cd "$tree" && "$repo/coax/remote/pages.sh" fetch "$full" "$tree" 2>&1)"; then
  export COAXIAL_PREBUILT=1
  echo "  prebuilt   $(tail -1 <<< "$pages_out" | sed 's/^pages: //')"
else
  echo "  prebuilt   none: every module builds here ($(tail -1 <<< "$pages_out" | sed 's/^pages: //'))"
fi
lap pages "$t"
if $build_only; then
  staged="$(mktemp -d)"
  t=$(now); COAX_VISUALS=report COAXIAL_STAGE_TO="$staged" "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > /dev/null; lap build "$t"
  echo "release $commit: built and staged in $staged (build only) in $(secs $(( $(now) - start ))) s"
  exit 0
fi

# Reports: they never stop a release.
reports="$(mktemp -d)"
# Check the exact commit serving production. The Coax gate compares retained
# manifests and remeasures production to suppress only pre-existing faults.
t=$(now)
(cd "$tree" && COAX="$tree/coax/bin/coax" coax/web/tool.sh compat-gate "$tree/coax/bin/coax" "$passed_commit" "$reports/compat")
lap compat "$t"

compiler_env COAX_RELEASE_COMPILER="$tree/coax/bin/coax" "$tree/scripts/coax-release.sh" --check --binary "$tree/coax/bin/coax" > "$reports/metadata" 2>&1 ||
  echo "  report     the release metadata check fails (not stopping): $(tail -1 "$reports/metadata")"
(cd "$tree" && bounded 300 coax/web/tool.sh tools-check) > "$reports/tools" 2>&1 ||
  echo "  report     tools-check fails (not stopping): $(tail -1 "$reports/tools")"
(cd "$tree" && bounded 300 coax/web/tool.sh everglade-parity) > "$reports/parity" 2>&1 ||
  echo "  report     everglade-parity fails (not stopping): $(grep -m1 '^FAIL' "$reports/parity" || tail -1 "$reports/parity")"
# Before deploy.sh's build rewrites them: are the committed shaders what their Coax writes?
(cd "$tree" && bounded 300 coax/web/tool.sh coax/web/wgsl-module.coax --site --check) > "$reports/wgsl" 2>&1 ||
  echo "  report     committed .wgsl drift from their Coax (not stopping): $(tail -1 "$reports/wgsl")"
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

# The Everglade drift check (#12778, coax/web/everglade-drift.coax): a release that changes Everglade's
# page, its room or the engine under them plays it on this release's build (deploy.sh built the page
# here). It stops the release when it fails here and holds on production's commit, a drift this
# release causes; when production's commit fails it too, it is a report.
# It runs alongside the staging starts walk (#13133); its stop messages print when both are done.
drift_check() {
everglade='^(apps/openagents-web/everglade|coax/web/room/|coax/web/room-worker\.js|coax/web/host\.js|coax/web/everglade-drift|aethyr/|verse/everglade/|verse/grid\.coax)'
if [[ -n "$passed" ]] && git -C "$tree" diff --name-only "${passed%% *}" "$full" | grep -Eq "$everglade"; then
  t=$(now)
  # A failure is confirmed by a second run before it counts, as the other checks' are: the first run
  # shares the Mac with the starts walk's Chromes, and its position probe then fails by millimetres
  # (#13137). The second waits for the walk to end.
  drift_run() { (cd "$tree" && coax/web/room/build.sh && coax/web/tool.sh everglade-drift) > "$reports/drift" 2>&1; }
  walk_done() { local n=0; while [[ ! -e "$reports/starts-done" ]] && (( n++ < 400 )); do sleep 5; done; }
  if drift_run; then echo "  drift      holds"
  elif cp "$reports/drift" "$reports/drift-first" && walk_done && drift_run; then echo "  drift      holds on a second run, once the starts walk was done (the first failed beside it: $(grep -cE '^DRIFT ' "$reports/drift-first") probe(s))"
  else
    old="$(mktemp -d)/production"
    git -C "$repo" worktree add -q --detach "$old" "${passed%% *}"
    if compiler "$old" > /dev/null &&
       (cd "$old" && coax/web/room/build.sh && apps/openagents-web/build.sh && coax/web/tool.sh everglade-drift) > "$reports/drift-production" 2>&1; then
      git -C "$repo" worktree remove --force "$old"
      echo "release $commit: STOPPED. the Everglade drift check fails here and holds on production's ${passed:0:10}; production is untouched:" >&2
      grep -E '^DRIFT ' "$reports/drift" | head -20 | sed 's/^/  /' >&2
      return 1
    fi
    git -C "$repo" worktree remove --force "$old"
    echo "  report     the Everglade drift check fails here and on production's ${passed:0:10} (not stopping): $(tail -1 "$reports/drift")"
  fi
  lap drift "$t"
fi

}
drift_out="$reports/drift-run"
drift_check > "$drift_out" 2>&1 & drift_job=$!

t=$(now)
out=$(starts "$staging")
: > "$reports/starts-done"
grep -q '^goldens ' <<< "$out" || { kill "$drift_job" 2>/dev/null; echo "release $commit: STOPPED. the staging starts walk did not finish within 1800 s, or did not run (Mac load: $(uptime | sed 's/.*averages*: //')):" >&2; tail -5 <<< "$out" >&2; exit 1; }
broke=$(new_starts "$out" "$base_starts" "$staging")
[[ -z "$broke" ]] || { kill "$drift_job" 2>/dev/null; echo "release $commit: STOPPED. pages fail on staging that pass on production; production is untouched:" >&2; echo "$broke" | sed 's/^/  /' >&2; exit 1; }
echo "  starts     staging $(starts_fails <<< "$out" | sed '/^$/d' | wc -l | tr -d ' ') failing check(s), production $(sed '/^$/d' <<< "$base_starts" | wc -l | tr -d ' '); none new"
lap starts "$t"

if wait "$drift_job"; then cat "$drift_out"; else cat "$drift_out" >&2; exit 1; fi

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
gcloud storage cp "$reports/compat/"*.surface "gs://$prod_bucket/releases/$stamp/compat/" --quiet > /dev/null
for r in metadata tools visuals; do gcloud storage cp "$reports/$r" "gs://$prod_bucket/releases/$stamp/${r^^}" --quiet > /dev/null 2>&1 || true; done
echo "release $commit: live on openagents.com in $(secs $(( $(now) - start ))) s (rollback: coax/web/rollback.sh $stamp)"
drop_lock

# Reports after the release, in the background: never a stop.
log="$(mktemp -d)/after.log"
nohup "$here/after-release.sh" "$full" "$stamp" "${passed%% *}" > "$log" 2>&1 < /dev/null &
disown
echo "release $commit: reports running after the release ($log, then releases/$stamp/)"
