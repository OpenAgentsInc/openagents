#!/usr/bin/env bash
# Catch a page whose look changed without its golden, and name the commit,
# right after it is pushed rather than at the next release.
#
#   golden-check.sh                    # what origin/main changed since production's release
#   golden-check.sh --commit HASH      # one commit (HASH^..HASH): before or after you push
#   golden-check.sh --since REF [--ref REF]
#
# It finds the pages the range may have changed from the files its commits
# touched (golden-pages.ts: each page's own inputs, and what every page
# shares, such as host.js or the compiler). None: it says so and exits 0 in
# seconds; a task list, a project snapshot or a doc never renders anything.
# Otherwise it builds the ref the way a release does (a clean worktree, the
# release's checked compiler from its cache, deploy.sh staging the exact
# bytes, uploading nothing), serves them on this machine (golden-serve.ts)
# and walks only those pages in headless Chrome against the ref's goldens
# (goldens.ts --only). Each page that fails is named with its likeliest
# breaking commit: the newest in the range touching its own inputs, else the
# newest touching shared ones. Exit 1 when one fails.
#
# A visual change is the author's to approve, in the same commit: look at the
# shot it printed, then `bun coax/web/goldens.ts https://staging.openagents.com
# --update` after a staging-only release (release.sh --staging-only) and
# commit coax/web/goldens/ with the change.
#
# The compiler: it uses the release's cached checked build for the ref's
# compiler inputs (~/.cache/coax-release or release.sh's
# ~/.cache/coax-release-fast) or the checkout's coax/bin when that matches;
# with none it builds one in the ref's tree the way release.sh does
# (COAX_MUTATION=0 coax/build.sh in the release's environment, a few
# minutes, or seconds from a published build) and caches it.
# The release's environment unsets CPATH, C_INCLUDE_PATH, CPLUS_INCLUDE_PATH,
# LIBRARY_PATH and the SDK variables, and coax/provenance.sh's fingerprint
# names them, because each can change what the C and C++ compilers build
# (LIBRARY_PATH can put another libstdc++ or libm in the link). So a
# coax/build.sh run in a shell that sets one (CoderOS sets LIBRARY_PATH) is
# recorded under another fingerprint, and this check doesn't take it; it
# builds its own instead. On coderos-4080 the two builds were the same
# compiler (#12684: every generated C file equal, the binaries apart only in
# their build directory's name), but that holds for that LIBRARY_PATH, not
# for any, so the fingerprint keeps it.
#
# Linux (#12684): headless Chromium there composites in software when
# WebGPU is on the GPU, and a screenshot leaves the WebGPU canvas out
# (#12678). There the pages are walked by golden-frames.coax instead of
# goldens.ts: each page's frame is read back from the GPU (golden-frames.js,
# on the hardware adapter only) and compared in the same 8 px blocks with
# coax/web/goldens/linux/. The Mac walks goldens.ts as before.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
ref=origin/main since=""
while (($#)); do
  case "$1" in
    --ref) ref="$2"; shift 2 ;;
    --since) since="$2"; shift 2 ;;
    --commit) ref="$2"; since="$2^"; shift 2 ;;
    *) echo "golden-check: unknown option $1" >&2; exit 2 ;;
  esac
done
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
git -C "$repo" fetch -q origin main 2>/dev/null || true
full="$(git -C "$repo" rev-parse --verify "$ref^{commit}")"
if [[ -z "$since" ]]; then
  # production's newest release that went live (release.sh's releases/<time>/PASSED)
  bucket=openagentsgemini-openagents-web
  newest="$(gcloud storage ls "gs://$bucket/releases/" 2>/dev/null | grep -E '/releases/[0-9]{8}T[0-9]{6}Z/$' | sort -r | head -5 || true)"
  for r in $newest; do since="$(gcloud storage cat "${r}PASSED" 2>/dev/null | cut -d' ' -f1 || true)"; [[ -z "$since" ]] || break; done
  [[ -n "$since" ]] || { echo "golden-check: no release of production to compare with; pass --since REF" >&2; exit 2; }
fi
base="$(git -C "$repo" rev-parse --verify "$since^{commit}")"
range="$base..$full"
tree="$(mktemp -d)/check" stage="" server=""
cleanup() {
  [[ -z "$server" ]] || kill "$server" 2>/dev/null || true
  git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true
  [[ -z "$stage" ]] || rm -rf "$stage"
}
trap cleanup EXIT
git -C "$repo" worktree add -q --detach "$tree" "$full"
[[ -d "$here/project/node_modules" && ! -e "$tree/coax/web/project/node_modules" ]] && ln -s "$here/project/node_modules" "$tree/coax/web/project/node_modules"
# The ref's pages and inputs, read with this checkout's rules (golden-pages.ts).
pages="$(GOLDEN_PAGES_ROOT="$tree" bun "$here/golden-pages.ts" affected "$range" | tr '\n' ' ')"
count="$(git -C "$repo" rev-list --count "$range")"
if [[ -z "${pages// }" ]]; then
  echo "golden-check ${full:0:10}: the $count commit(s) since ${base:0:10} touch no page's inputs; nothing to render"; exit 0
fi
echo "golden-check ${full:0:10}: $count commit(s) since ${base:0:10} may change: $pages"
[[ -f "$tree/coax/web/golden-pages.ts" ]] || { echo "golden-check ${full:0:10}: its walk predates goldens.ts --only; check a newer commit" >&2; exit 2; }
[[ "$(uname -s)" == Darwin || -f "$tree/coax/web/golden-frames.coax" ]] || { echo "golden-check ${full:0:10}: it predates Linux goldens (golden-frames.coax, #12684); check a newer commit or run it on the Mac" >&2; exit 2; }

# The release's compiler for the ref's inputs (release.sh: the same compilers and environment).
compiler_env() {
  env -u COAX_HOST_TARGET -u SDKROOT -u MACOSX_DEPLOYMENT_TARGET -u IPHONEOS_DEPLOYMENT_TARGET -u DEVELOPER_DIR \
    -u CPATH -u C_INCLUDE_PATH -u CPLUS_INCLUDE_PATH -u LIBRARY_PATH \
    CC="${COAX_RELEASE_CC:-cc}" CXX="${COAX_RELEASE_CXX:-c++}" "$@"
}
fingerprint="$(compiler_env "$tree/coax/provenance.sh" fingerprint "$tree")"
cache_root="${COAX_RELEASE_CACHE:-$HOME/.cache/coax-release}"
install_from() { [[ -d "$1" ]] && compiler_env "$tree/coax/provenance.sh" install "$tree" "$1" 2> /dev/null; }
if ! install_from "$cache_root/$fingerprint" && ! install_from "$HOME/.cache/coax-release-fast/$fingerprint" && ! install_from "$repo/coax/bin"; then
  echo "golden-check ${full:0:10}: no checked compiler for its compiler inputs (${fingerprint:0:12}); building one (coax/build.sh, no mutation gates, in the release's environment)"
  t0=$SECONDS
  mkdir -p "$cache_root"
  if ! compiler_env COAX_RELEASE_CACHE="$cache_root" COAX_MUTATION=0 "$tree/coax/build.sh" > "$cache_root/$fingerprint.log" 2>&1 || ! install_from "$cache_root/$fingerprint"; then
    tail -5 "$cache_root/$fingerprint.log" >&2
    echo "golden-check: the compiler for ${full:0:10} did not build (log: $cache_root/$fingerprint.log)" >&2
    exit 2
  fi
  echo "  compiler   $(( SECONDS - t0 )) s"
fi

t0=$SECONDS
stage="$(mktemp -d)"
COAXIAL_STAGE_TO="$stage" "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web-staging > "$stage/build.log" 2>&1 ||
  { tail -20 "$stage/build.log" >&2; echo "golden-check ${full:0:10}: the build failed" >&2; exit 1; }
echo "  build      $(( SECONDS - t0 )) s"
port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')"
bun "$here/golden-serve.ts" "$stage/site" "$port" > "$stage/serve.log" 2>&1 & server=$!
for _ in $(seq 50); do curl -sf -o /dev/null "http://127.0.0.1:$port/verse" && break; sleep 0.1; done
report="$stage/goldens.json"
t0=$SECONDS
if [[ "$(uname -s)" == Darwin ]]; then
  walk() { GOLDENS_PENDING="$stage/pending" bun "$tree/coax/web/goldens.ts" "http://golden.localhost:$port" --only "$(echo $pages | tr ' ' ',')" --report "$report"; }
else
  # the ref's pages (this checkout's rules, as above) as golden-frames.coax's table, walked with the ref's tool
  GOLDEN_PAGES_ROOT="$tree" bun "$here/golden-pages.ts" table $pages > "$stage/pages.tsv"
  # One Chrome, one page at a time (about 7 s a page): three side by side on one GPU starved
  # Everglade's scene and moved Drop's frame (#12684).
  walk() { COAX="$tree/coax/bin/coax" "$tree/coax/web/tool.sh" "$tree/coax/web/golden-frames.coax" "http://golden.localhost:$port" "$stage/pages.tsv" \
    --dir "$tree/coax/web/goldens/linux" --pending "$stage/pending" --report "$report" --glue "$tree/coax/web/golden-frames.js" --require-hardware; }
fi
if walk; then
  echo "golden-check ${full:0:10}: every page the range may change matches its golden ($(( SECONDS - t0 )) s)"; exit 0
fi
[[ -f "$report" ]] || { echo "golden-check ${full:0:10}: the walk could not run" >&2; exit 2; }
# The failing pages: those whose golden failed, and those named first in any other failure.
failing="$(bun -e 'const r = JSON.parse(await Bun.file(process.argv[1]).text()); const n = new Set(r.golden.map((g) => g.name)); for (const o of r.other) { const m = /^([a-z0-9.-]+):/.exec(o); if (m && process.argv[2].split(" ").includes(m[1])) n.add(m[1]) } console.log([...n].join(" "))' "$report" "$pages")"
while IFS=$'\t' read -r name hash author why subject; do
  [[ -n "$name" ]] || continue
  if [[ -n "$hash" ]]; then echo "FLAGGED $name: ${hash:0:10} ($author, $why inputs): $subject"
  else echo "FLAGGED $name: $subject"; fi
done < <(GOLDEN_PAGES_ROOT="$tree" bun "$here/golden-pages.ts" blame "$range" $failing)
echo "golden-check ${full:0:10}: a page changed without its golden; its commit's author fixes it or approves the look (see the top of coax/web/golden-check.sh)" >&2
exit 1
