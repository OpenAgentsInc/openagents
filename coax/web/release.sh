#!/usr/bin/env bash
# Release openagents.com: build committed code once, put it on staging, smoke
# it, then promote the same bytes to production and smoke that.
#
#   release.sh                     # origin/main → staging → smoke → production → smoke
#   release.sh --ref <commit>      # a specific commit
#   release.sh --staging-only      # stop after staging's smoke
#   release.sh --skip-staging --yes-skip-staging   # emergencies only: straight to production
#   release.sh --full-walk         # compare every page with its golden, unchanged or not
#   release.sh --thorough          # production gets the full walk too (default: the brief check)
#   release.sh --style-check       # staging's painterly realms measured and their sheets written (style-check.ts)
#
# Staging's smoke is the HTTP checks (smoke.sh), then the playtest flow walked
# in headless Chrome against its golden screenshots (goldens.ts): each page
# must start and draw, not only answer 200, and the pages must send every
# event of the analytics contract (analytics-contract.json); a missing one is
# named and nothing is promoted. Then Everglade's frame rate (perf.ts): at
# dpr 1 and 2 it walks four places and casts, and must hold 60 FPS (p95 at
# most 17.5 ms, no frame over 33 ms), or nothing is promoted.
#
# Production, after the promote, in one of two modes (owner decision, 2026-10-29):
#   FAST (the default)  smoke.sh's HTTP checks plus a brief check that each
#                       page answers and its engine starts with no errors
#                       (goldens.ts --starts, seconds; no screenshots).
#   THOROUGH (--thorough) the HTTP checks plus the full walk, as on staging
#                       but without golden comparisons (goldens.ts --walk).
# Either way a failed production check puts the previous pages back on its own.
# The walk runs its pages side by side in a few headless tabs (goldens.ts,
# GOLDENS_POOL). On staging, a page whose HTML and every file it names or
# fetches are byte-identical to what production serves, when production's
# newest release was staged and went live (releases/<time>/PASSED) and the
# page's golden, its numbers and goldens.ts are unchanged since that commit,
# skips its golden comparison: the bytes already passed it. It still opens
# by its address and must start, show no errors and draw. The walk names
# every page it skipped and why; GOLDENS_FULL=1 (or --full-walk) compares
# every page. Production's thorough walk takes every page by its route.
# A page that fails only its golden comparison is held back, not the release
# (2026-10-29, #12605): production keeps that page's current HTML (the files
# it names are named by their content and never removed, so it still
# answers), every other page and all data ship, and the run prints HELD BACK
# with the likeliest breaking commit (golden-pages.ts: the newest since
# production's release touching the page's own inputs, else shared ones such
# as host.js) and lists the pages in releases/<time>/HELD. When that commit is
# the one being released, the rest still ships and the run exits 3. Any
# other failure (a page that doesn't start, errors, draws nothing, the
# analytics contract, the HTTP smoke) still stops the release before
# production. A visual change is its author's to approve, in its own commit:
# look at the shot it printed, then `bun coax/web/goldens.ts
# https://staging.openagents.com --update` and commit coax/web/goldens/ with
# the change; golden-check.sh --commit HASH catches it before the push, in
# about a minute. Never approve someone else's page to get a release through.
# Data needs no release: project pages read their relay live, and
# coax/web/project/publish-data.sh refreshes their Markdown (seconds). A realm not yet
# listed on the galaxy (realms.json listed: false) with no golden doesn't
# block: it must start, show no errors and draw, and its shot waits in
# coax/web/goldens/pending/ ("NEW REALM ...") for approval.
#
# The build comes from a clean worktree of the ref, never from a working
# directory other agents are editing. Production gets the files staging was
# smoked with, copied bucket to bucket, so what passed is what ships. The
# pages production served before are kept under releases/<time>/ in its
# bucket; rollback.sh <time> puts them back. A failed production smoke rolls
# back on its own.
#
# One release at a time on this machine (a lock in $TMPDIR). A release that
# gets the lock when production already runs a commit containing its ref
# (the newest releases/<time>/PASSED) exits 0 and says so. A waiting
# release whose commit is in a newer one also waiting (with the default
# staging-and-production run) leaves it to that one and exits 0.
#
# The compiler: the release uses only a coax compiler whose provenance
# record (coax/provenance.sh) matches the target tree's compiler inputs. It
# keeps checked builds in a cache keyed by that fingerprint
# ($COAX_RELEASE_CACHE, default ~/.cache/coax-release/<fingerprint>/), takes
# the checkout's coax/bin when it matches, and otherwise builds one with
# coax/build.sh (the full bootstrap and all its gates, mutation included:
# ~11-15 min) from the commit's coax/, before queueing for the lock so no
# release waits behind a bootstrap; one builder per fingerprint at a time.
# The cached record is the bootstrap's own, verified again on the release's
# clean tree; nothing is skipped or rewritten. The fingerprint names the C and C++
# compilers and the build environment, so the release fixes them: CC=cc,
# CXX=c++ (COAX_RELEASE_CC / COAX_RELEASE_CXX to choose), and no SDKROOT,
# deployment targets or include/library paths from the caller.
#
# Pinned demos (pin.sh) live in the production bucket under demo/ and
# demo-releases/; a release never writes there, and checks it didn't.
#
# Routing (one load balancer, coaxial-openagents-web-staging):
#   staging.openagents.com → the staging bucket
#   openagents.com         → the production bucket, /api/* to oa-api (the coder
#                            service is retired: docs/2026-10-28-coder-service-deprecation.md)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
ref=origin/main stage=true prod=true full_walk="" thorough=false
while (($#)); do
  case "$1" in
    --ref) ref="$2"; shift 2 ;;
    --staging-only) prod=false; shift ;;
    --skip-staging) stage=false; shift ;;
    --yes-skip-staging) skip_ok=1; shift ;;
    --full-walk) full_walk=1; shift ;;
    --thorough) thorough=true; shift ;;
    --style-check) style_check=1; shift ;;
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

# The release compiler: a checked coax build whose provenance record matches
# the target tree's compiler inputs, from the cache, the checkout's coax/bin,
# or bootstrapped once (coax/build.sh, all its gates) and kept. The same
# compilers and environment for the fingerprint, the build and the check.
compiler_env() {
  env -u COAX_HOST_TARGET -u SDKROOT -u MACOSX_DEPLOYMENT_TARGET -u IPHONEOS_DEPLOYMENT_TARGET -u DEVELOPER_DIR \
    -u CPATH -u C_INCLUDE_PATH -u CPLUS_INCLUDE_PATH -u LIBRARY_PATH \
    CC="${COAX_RELEASE_CC:-cc}" CXX="${COAX_RELEASE_CXX:-c++}" "$@"
}
cache_root="${COAX_RELEASE_CACHE:-$HOME/.cache/coax-release}"
# compiler ROOT TARGET: ROOT holds coax/ and scripts/ at the commit; the
# checked build is installed into TARGET/coax/bin (verified against TARGET).
compiler() {
  local root="$1" target="$2" fingerprint cache building builder into
  fingerprint="$(compiler_env "$root/coax/provenance.sh" fingerprint "$root")" || return 1
  cache="$cache_root/$fingerprint"
  install_from() { [[ -d "$1" ]] && compiler_env "$root/coax/provenance.sh" install "$target" "$1" 2> /dev/null; }
  keep() {   # a checked build into the cache, whole or not at all
    into="$(mktemp -d "$cache_root/.keep.XXXXXXXX")"
    cp "$1/coax" "$1/provenance-check" "$1/provenance.record" "$into/"
    compiler_env "$root/coax/provenance.sh" verify "$target" "$into/coax" "$into/provenance-check" "$into/provenance.record" || { rm -rf "$into"; return 1; }
    rm -rf "$cache"; mv "$into" "$cache"
  }
  mkdir -p "$cache_root"
  if install_from "$cache"; then echo "  compiler   cached build ${fingerprint:0:12}"; return; fi
  if install_from "$repo/coax/bin"; then keep "$target/coax/bin"; echo "  compiler   the checkout's build ${fingerprint:0:12}, now cached"; return; fi
  # One builder per fingerprint; one left by a dead builder is taken over.
  building="$cache_root/$fingerprint.building"
  until mkdir "$building" 2>/dev/null; do
    builder="$(cat "$building/pid" 2>/dev/null || true)"
    if [[ -n "$builder" ]] && ! kill -0 "$builder" 2>/dev/null; then rm -rf "$building"; continue; fi
    echo "release: waiting for compiler build ${fingerprint:0:12} (pid ${builder:-?})"; sleep 10
  done
  echo $$ > "$building/pid"
  if ! install_from "$cache"; then
    echo "  compiler   no checked build for these compiler inputs (${fingerprint:0:12}); bootstrapping one (coax/build.sh, about 15 min)"
    if ! compiler_env "$root/coax/build.sh" > "$cache_root/$fingerprint.log" 2>&1 || ! keep "$root/coax/bin" || ! install_from "$cache"; then
      rm -rf "$building"; echo "release: coax/build.sh failed (log: $cache_root/$fingerprint.log)" >&2; return 1
    fi
  fi
  rm -rf "$building"
}

# Production's newest release, when it went live: "<commit> <time> staged|unstaged fast|thorough"
# (releases/<time>/PASSED, written after production's walk and smoke). A
# newer release that didn't pass, or a rollback, has none: then nothing.
live() {
  local newest
  newest="$(gcloud storage ls "gs://$prod_bucket/releases/" 2>/dev/null | grep -E '/releases/[0-9]{8}T[0-9]{6}Z/$' | sort | tail -1)"
  [[ -n "$newest" ]] && gcloud storage cat "${newest}PASSED" 2>/dev/null | head -1 || true
}

# One release at a time on this machine: a second one waits, since both
# rewrite the staging bucket and promote from it. A lock left by a dead
# release is taken over. Waiting releases list themselves in $queue
# (<time>.<pid>: the commit they'd ship); a default release whose commit is
# in another waiting one's (an equal one: the one that came first) exits in
# its favour. Only the lock decides who runs.
lock="${TMPDIR:-/tmp}/coaxial-release.lock" queue="${TMPDIR:-/tmp}/coaxial-release.queue"
tree="" held=false mine=""
cleanup() {
  [[ -z "$mine" ]] || rm -f "$mine"
  [[ -z "$tree" ]] || git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true
  ! $held || rm -rf "$lock"
}
trap cleanup EXIT
if $stage && $prod; then
  git -C "$repo" fetch -q origin main 2>/dev/null || true
  if wanted="$(git -C "$repo" rev-parse --verify -q "$ref^{commit}")"; then
    mkdir -p "$queue"; mine="$queue/$(now).$$"; echo "$wanted" > "$mine"
  fi
fi
# Make sure the compiler for the commit is built before queueing, so a
# bootstrap never holds the lock (and the releases behind it) for minutes.
if [[ -n "${wanted:-}" ]] || wanted="$(git -C "$repo" rev-parse --verify -q "$ref^{commit}")"; then
  inputs="$(mktemp -d)"
  git -C "$repo" archive "$wanted" coax scripts/coax-release.sh | tar -x -C "$inputs"
  compiler "$inputs" "$inputs" || true   # checked again on the release's own tree below
  rm -rf "$inputs"
fi
until mkdir "$lock" 2>/dev/null; do
  holder="$(cat "$lock/pid" 2>/dev/null || true)"
  if [[ -n "$holder" ]] && ! kill -0 "$holder" 2>/dev/null; then rm -rf "$lock"; continue; fi
  # A lock whose taker died before writing its pid would block every release: after a minute it is stale.
  if [[ -z "$holder" ]] && [[ -n "$(find "$lock" -maxdepth 0 -mmin +1 2>/dev/null)" ]]; then rmdir "$lock" 2>/dev/null || true; continue; fi
  if [[ -n "$mine" ]]; then
    for other in "$queue"/*.*; do
      [[ -f "$other" && "$other" != "$mine" ]] || continue
      kill -0 "${other##*.}" 2>/dev/null || { rm -f "$other"; continue; }
      theirs="$(cat "$other" 2>/dev/null || true)"
      [[ "$theirs" =~ ^[0-9a-f]{40}$ ]] || continue
      if [[ "$theirs" == "$wanted" && "${other##*/}" < "${mine##*/}" ]] ||
         { [[ "$theirs" != "$wanted" ]] && git -C "$repo" merge-base --is-ancestor "$wanted" "$theirs" 2>/dev/null; }; then
        echo "release ${wanted:0:10}: release ${other##*.}, waiting too, ships ${theirs:0:10}, which contains it; leaving it to that one"
        exit 0
      fi
    done
  fi
  echo "release: waiting for release ${holder:-?} to finish"; sleep 5
done
held=true
echo $$ > "$lock/pid"
[[ -z "$mine" ]] || { rm -f "$mine"; mine=""; }

git -C "$repo" fetch -q origin main
commit="$(git -C "$repo" rev-parse --short "$ref")"
full="$(git -C "$repo" rev-parse "$commit^{commit}")"
passed="$(live)"
if $prod && [[ -n "$passed" ]] && git -C "$repo" merge-base --is-ancestor "$full" "${passed%% *}" 2>/dev/null; then
  echo "release $commit: already live: production runs ${passed%% *} (released $(cut -d' ' -f2 <<< "$passed")), which contains it; nothing to do"
  exit 0
fi
tree="$(mktemp -d)/release"
git -C "$repo" worktree add -q --detach "$tree" "$commit"
echo "release $commit ($(git -C "$tree" log -1 --format=%s | cut -c1-70))"
# Use only a snapshot whose checked build record matches this target tree.
# Input/binary consistency is not a signature against a hostile local writer.
t=$(now)
compiler "$tree" "$tree" || exit 1
lap compiler "$t"
compiler_env COAX_RELEASE_COMPILER="$tree/coax/bin/coax" "$tree/scripts/coax-release.sh" --check --binary "$tree/coax/bin/coax"

if $stage; then
  t=$(now); "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > /dev/null; lap staging "$t"
  t=$(now); "$tree/coax/web/smoke.sh" https://staging.openagents.com; lap smoke "$t"
  if [[ -f "$tree/coax/web/tests/home_live_browser.ts" ]]; then
    t=$(now); bun "$tree/coax/web/tests/home_live_browser.ts" https://staging.openagents.com; lap "live home" "$t"
  fi
  t=$(now)
  # Pages unchanged since production's newest live release, if it was staged, skip their golden comparison.
  unchanged=()
  [[ -n "$full_walk" || "$passed" != *" staged "* ]] || unchanged=(--unchanged-against https://openagents.com --since "${passed%% *}")
  report="$(mktemp -d)/goldens.json"
  if ! GOLDENS_FULL="${full_walk:-${GOLDENS_FULL:-}}" GOLDENS_PENDING="$repo/coax/web/goldens/pending" bun "$tree/coax/web/goldens.ts" https://staging.openagents.com ${unchanged[@]+"${unchanged[@]}"} --report "$report"; then
    # Only golden comparisons failed: those pages are held back, the rest ships.
    # Anything else (a page that doesn't start, errors, draws nothing, a missing analytics event) stops the release.
    held_names="$(bun -e 'const r = JSON.parse(await Bun.file(process.argv[1]).text()); if (r.other.length || !r.golden.length) process.exit(1); console.log(r.golden.map((g) => g.name).join(" "))' "$report" 2>/dev/null)" ||
      { echo "release $commit: staging's pages don't start, draw or send the analytics contract; production is untouched" >&2; exit 1; }
    hold="$(cd "$tree/coax/web" && bun golden-pages.ts objects $held_names)"
    # The likeliest break: the newest commit since production's release that touched the page's own inputs, else shared ones.
    range="$full~50..$full"; [[ -z "$passed" ]] || range="${passed%% *}..$full"
    while IFS=$'\t' read -r name hash author why subject; do
      echo "HELD BACK $name: it doesn't match its golden; production keeps its current page, everything else ships"
      if [[ -n "$hash" ]]; then
        echo "  likeliest break: ${hash:0:10} ($author, $why inputs): $subject"
        [[ "$hash" != "$full" ]] || held_mine+="$name "
      else
        echo "  $subject"
      fi
    done < <(cd "$tree/coax/web" && bun golden-pages.ts blame "$range" $held_names)
    echo "  the commit's author fixes the page or approves its look: bun coax/web/goldens.ts https://staging.openagents.com --update, look at the shot, commit coax/web/goldens/"
  fi
  lap goldens "$t"
  # --style-check: the painterly realms measured on staging and their side-by-side sheets written
  # (style-check.ts; art-styles.md 15). A report for now, not a gate: Everglade doesn't pass it yet
  # (eg-paint-review), and it takes a few minutes, so it runs when asked.
  if [[ -n "${style_check:-}" ]]; then
    t=$(now)
    bun "$tree/coax/web/style-check.ts" https://staging.openagents.com --out "$repo/coax/web/style" || echo "  style check: views out of range (a report; nothing is held back)"
    lap style "$t"
  fi
  # Everglade must hold 60 FPS at dpr 1 and 2 (perf.ts: p95 <= 17.5 ms, no frame over 33 ms, walking and casting).
  t=$(now)
  bun "$tree/coax/web/perf.ts" https://staging.openagents.com ||
    { echo "release $commit: Everglade misses 60 FPS on staging (perf.ts above); production is untouched" >&2; exit 1; }
  lap perf "$t"
fi
$prod || { echo "release $commit: staging only, done in $(secs $(( $(now) - start ))) s${held_names:+; would hold back $held_names}"; exit 0; }
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
  # Projects published from a drop on staging (coax/web/api/drops.c) stay on staging: production has its own.
  published='^projects/published(-slugs)?\.txt$'
  for s in $(gcloud storage cat "gs://$staging_bucket/projects/published-slugs.txt" 2>/dev/null | grep -E '^[a-z0-9][a-z0-9-]{0,63}$' || true); do
    published+="|^projects/$s(/|\$)"
  done
  # So do realms made at /realms/new on staging (coax/web/api/realms.c).
  published+='|^realms/made-slugs\.txt$'
  for s in $(gcloud storage cat "gs://$staging_bucket/realms/made-slugs.txt" 2>/dev/null | grep -E '^[a-z0-9][a-z0-9-]{0,63}$' || true); do
    published+="|^realms/$s(/|\$)"
  done
  # Held-back pages keep production's current bytes: their HTML isn't copied.
  # Their files are named by their content and never removed, so they still answer.
  [[ -z "${hold:-}" ]] || published+="|$hold"
  gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude ".*\.(html|md)\$|^releases/|^demo/|^demo-releases/|$published" --quiet > /dev/null
  wait $backup
  gcloud storage rsync -r "gs://$staging_bucket" "gs://$prod_bucket" --exclude "^releases/|^demo/|^demo-releases/|$published" --quiet > /dev/null
  [[ -z "${held_names:-}" ]] || echo "$held_names" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/HELD" --quiet > /dev/null || true
  lap promote "$t"
else
  echo "$commit $stamp" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/COMMIT" --quiet > /dev/null
fi

[[ "$(demos)" == "$demos_before" ]] || { echo "release $commit: a pinned demo changed (demo/ in $prod_bucket); it must not" >&2; exit 1; }
t=$(now)
via=""
[[ "$(dig +short openagents.com A | sort | head -1)" == "$lb_ip" ]] || via="--via $lb_ip"
walked=true check=--starts
! $thorough || check=--walk
[[ -n "$via" ]] || bun "$tree/coax/web/goldens.ts" https://openagents.com "$check" || walked=false
if ! $walked || ! "$tree/coax/web/smoke.sh" https://openagents.com $via; then
  echo "release $commit: production smoke failed; rolling back to the pages from before" >&2
  "$here/rollback.sh" "$stamp"
  exit 1
fi
if [[ -f "$tree/coax/web/tests/home_live_browser.ts" ]] && ! bun "$tree/coax/web/tests/home_live_browser.ts" https://openagents.com; then
  echo "release $commit: production live homepage failed; rolling back" >&2
  "$here/rollback.sh" "$stamp"
  exit 1
fi
lap "prod ${check#--}" "$t"
# This release went live after its walk and smoke: the next one may build on it.
echo "$full $stamp $($stage && echo staged || echo unstaged) $($thorough && echo thorough || echo fast)" | gcloud storage cp - "gs://$prod_bucket/releases/$stamp/PASSED" --quiet > /dev/null ||
  echo "release $commit: couldn't write releases/$stamp/PASSED; the next release walks in full" >&2
echo "release $commit: live on openagents.com in $(secs $(( $(now) - start ))) s (rollback: coax/web/rollback.sh $stamp)${held_names:+; held back: $held_names}"
# The commit released broke a page itself: the rest is live, but say so with a failure.
if [[ -n "${held_mine:-}" ]]; then
  echo "release $commit: this commit changed how ${held_mine% } look(s) without updating the golden(s); those pages were held back" >&2
  exit 3
fi
