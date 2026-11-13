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
# build time), is built here. The render packages are taken the same way (each
# source and product by its SHA-256), and a commit without stored pages takes
# its nearest ancestor's build/ folder when the compiler's sources are the same
# (#13154). The build prints each step's time; --build-only prints deploy.sh's too.
#
# One rule decides everything (owner, 2026-11-02): a release stops only for a
# check that FAILS ON STAGING AND PASSES ON PRODUCTION, confirmed by a retry.
# Two checks, each run on both: smoke.sh (HTTP) and goldens.ts --starts (every
# page answers, its engine starts, no errors). Failures production already has
# never stop a release; more of them always do, page by page. After the promote
# the same two checks run on production; a new failure rolls back.
#
# One more check can stop a release, because a release that fails it strands every Everglade
# player (#13387): the page's wire version against the rooms that run now (release-set page-wire,
# its rule in release-set-policy.coax). A room refuses a page whose version it does not speak, the
# page says to reload, and the reload is the same page. So a page ahead of its rooms' wire never
# ships here; it ships with its rooms, as one release set (coax/web/tool.sh release-set ship,
# docs/deployment/release-set.md). The check reads the rooms while the build starts, so a release
# whose wire did not change waits for nothing.
#
# The Everglade drift check is v1's and runs on the frozen tree (everglade/server/legacy/v1.sh drift,
# docs/decisions/0005): no release runs it.
#
# Everything else is a report and never stops a release: tools-check,
# everglade-parity (no zone-local copy of a shared Everglade feature, #13075),
# wire-check (every wire shape registered in proto/registry.json, #13236),
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
# pipeline's full check (coax/remote/after.sh, docs/metaverse/green-main-pipeline.md),
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
# A supervisor (Python, in the group) runs it and stops the whole group when the limit passes or
# when its caller goes, even by SIGKILL, so no compile outlives a stopped release (#13154: a stopped
# dry run left its build running, and the Mac's load passed 200). It has no watchdog of its own to
# leave behind.
group=""
bounded() {
  local limit=$1; shift
  python3 -I -c '
import os, signal, sys, time
limit, caller = float(sys.argv[1]), os.getppid()
os.setpgid(0, 0)
child = os.fork()
if child == 0:
    os.execvp(sys.argv[2], sys.argv[2:])
signal.signal(signal.SIGTERM, signal.SIG_IGN)
deadline = time.monotonic() + limit
while True:
    done, status = os.waitpid(child, os.WNOHANG)
    if done == child:
        code = os.waitstatus_to_exitcode(status)
        sys.exit(code if code >= 0 else 128 - code)
    if os.getppid() != caller or time.monotonic() > deadline:
        why = "its caller is gone" if os.getppid() != caller else "%d s passed" % limit
        print("bounded: stopping %s: %s" % (sys.argv[2], why), file=sys.stderr)
        os.killpg(0, signal.SIGTERM)
        end, done = time.monotonic() + 5, 0
        while time.monotonic() < end and done == 0:
            time.sleep(0.2)
            done, status = os.waitpid(child, os.WNOHANG)
        if done == 0 or os.getppid() != caller:
            os.killpg(0, signal.SIGKILL)   # this supervisor too; the caller kills what is left otherwise
        sys.exit(124)
    time.sleep(0.5)
' "$limit" "$@" < /dev/null & local pid=$!
  group=$pid
  local status=0; wait "$pid" || status=$?
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
# A stopped release takes its build with it: deploy.sh runs in its own process
# group (bounded), which cleanup kills, so no compile outlives the release (#13154).
trap 'exit 143' TERM INT HUP

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

# The page's wire against the rooms it will meet (#13387): staging's for --staging-only, production's
# otherwise. Asked now, in the background, and answered before anything is put on staging.
wire_of() { { git -C "$repo" show "$1:everglade/shared/everglade-wire.coax" 2>/dev/null || true; } | sed -n 's/^WIRE_VERSION :: \([0-9][0-9]*\);.*/\1/p' | head -1; }
wire_page="$(wire_of "$full")"; wire_was=""
[[ -z "$passed_commit" ]] || wire_was="$(wire_of "$passed_commit")"
wire_out="$(mktemp)"; wire_job=""
if ! $build_only; then
  (cd "$tree" && COAX="$tree/coax/bin/coax" coax/web/tool.sh release-set page-wire $($prod || echo --staging) --was "${wire_was:-0}") > "$wire_out" 2>&1 &
  wire_job=$!
fi
# wire_gate: the answer. 1 is the rule's refusal. Any other failure (the tool did not build or run)
# stops only a release whose wire differs from the page production runs: the same wire changes
# nothing a room could refuse.
wire_gate() {
  local status=0
  wait "$wire_job" || status=$?
  if [[ $status -eq 0 ]]; then cat "$wire_out"; return 0; fi
  cat "$wire_out" >&2
  if [[ $status -eq 1 ]]; then
    echo "release $commit: STOPPED. the page is ahead of its rooms' wire version; nothing was put on staging or production" >&2; return 1
  fi
  if [[ -n "$wire_page" && "$wire_page" == "$wire_was" ]]; then
    echo "  report     the wire check did not run (status $status); the page speaks wire $wire_page, as production's page does (not stopping)"; return 0
  fi
  echo "release $commit: STOPPED. the wire check did not run (status $status), and the page speaks wire ${wire_page:-unread} where production's speaks ${wire_was:-unread}; nothing was put on staging or production" >&2; return 1
}

# The hosts' build of this commit, if the shared store has it: the build then
# makes only what it lacks (COAXIAL_PREBUILT). Without one, the nearest
# ancestor's build (#13154): only its build/ folder, and only when the compiler's
# sources (the files coax/provenance.sh names a published compiler by) and the
# build's recipes are the same in both commits: a record names a module's sources,
# not the compiler or the flags that made it. Every module, render package and room
# module there is still taken only if its record names this tree's inputs.
compiler_inputs=(coax/src coax/bootstrap coax/lib/compiler coax/lib/proto coax/build.sh coax/bootstrap-build.sh
  coax/bootstrap-mutation-shards.sh coax/mutation-gate.sh coax/check-proto-cap.sh coax/provenance.sh coax/provenance-bridge.c
  coax/release.coax coax/release-model.coax coax/changelog.coax coax/changelog-model.coax coax/changelog-bridge.c
  coax/lib/crypto/sha256-kernel.coax scripts/coax-release.sh
  coax/web/wasm.sh coax/web/project/build-home.sh coax/web/room metaverse-engine/render/build.coax)
t=$(now)
from="$(cd "$tree" && "$repo/coax/remote/pages.sh" nearest "$full" 2> /dev/null)" || from=""
part=()
if [[ -z "$from" ]]; then
  echo "  prebuilt   none: every module builds here (no stored pages for $commit or its last 200 ancestors)"
elif [[ "$from" != "$full" ]] && ! git -C "$repo" diff --quiet "$from" "$full" -- "${compiler_inputs[@]}"; then
  echo "  prebuilt   none: every module builds here (the compiler or the build changed since ${from:0:10}, the nearest commit with stored pages)"; from=""
elif [[ "$from" != "$full" ]]; then part=(built)
fi
if [[ -z "$from" ]]; then :
elif pages_out="$(cd "$tree" && "$repo/coax/remote/pages.sh" fetch "$from" "$tree" ${part[@]+"${part[@]}"} 2>&1)"; then
  export COAXIAL_PREBUILT=1
  echo "  prebuilt   $(tail -1 <<< "$pages_out" | sed 's/^pages: //')$([[ "$from" == "$full" ]] || echo " ($(git -C "$repo" rev-list --count "$from..$full") commit(s) back: what differs builds here)")"
else
  echo "  prebuilt   none: every module builds here ($(tail -1 <<< "$pages_out" | sed 's/^pages: //'))"
fi
lap pages "$t"
# deploy.sh's own steps (its "  NAME  N s" lines) under the build's time.
deploy_laps() { sed -n 's/^  \([a-z-]*\) *\([0-9.]* s\)$/    \1 \2/p' "$1" | awk '{ printf "    %-8s %7s s\n", $1, $2 }'; }
# Nothing drives the release's own tree (coax drive reads a module's layout and source list from
# build/.drive/), so the modules built here skip those two compiler passes (wasm.sh).
export COAXIAL_DRIVE=0
if $build_only; then
  staged="$(mktemp -d)"
  t=$(now); COAX_VISUALS=report COAXIAL_STAGE_TO="$staged" bounded 3600 "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > "$staged/deploy.log"; lap build "$t"
  deploy_laps "$staged/deploy.log"; rm -f "$staged/deploy.log"
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
(cd "$tree" && bounded 300 coax/web/tool.sh wire-check) > "$reports/wire" 2>&1 ||
  echo "  report     wire-check fails (not stopping): $(grep -m1 '^FAIL' "$reports/wire" || tail -1 "$reports/wire")"
# Before deploy.sh's build rewrites them: are the committed shaders what their Coax writes?
(cd "$tree" && bounded 300 coax/web/tool.sh coax/web/wgsl-module.coax --site --check) > "$reports/wgsl" 2>&1 ||
  echo "  report     committed .wgsl drift from their Coax (not stopping): $(tail -1 "$reports/wgsl")"
"$tree/coax/web/check-visuals.sh" "$tree/apps/openagents-web" > "$reports/visuals" 2>&1 ||
  echo "  report     check-visuals fails (not stopping): $(head -1 "$reports/visuals")"

t=$(now); wire_gate || exit 1; lap wire "$t"

# What production fails today, measured while staging builds.
base="$(mktemp -d)"
( "$tree/coax/web/smoke.sh" "$production" > "$base/smoke" 2>&1 || true
  starts "$production" > "$base/starts" ) &
baseline_job=$!

t=$(now); COAX_VISUALS=report bounded 3600 "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > "$reports/deploy.log"; lap staging "$t"
deploy_laps "$reports/deploy.log"
wait "$baseline_job" || true
base_smoke="$(fails_of < "$base/smoke")"; base_starts="$(starts_fails < "$base/starts")"

t=$(now)
out=$("$tree/coax/web/smoke.sh" "$staging" 2>&1) || true
broke=$(broken_by_release "$out" "$base_smoke" "$tree/coax/web/smoke.sh" "$staging")
[[ -z "$broke" ]] || { echo "release $commit: STOPPED. staging smoke fails where production passes; production is untouched:" >&2; echo "$broke" | sed 's/^/  /' >&2; exit 1; }
lap smoke "$t"

# The Everglade drift check (#12778, coax/web/everglade-drift.coax) plays v1's page against v1's room.
# The room runs Everglade II only (docs/decisions/0005-production-room-runs-everglade-ii.md, #13267), so
# v1's drift runs on the frozen tree (everglade/server/legacy/v1.sh drift), whose result no release changes.
drift_check() { echo "  drift      v1's, on the frozen tree (everglade/server/legacy/v1.sh drift): no release changes it"; }
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
