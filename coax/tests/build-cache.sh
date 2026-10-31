#!/usr/bin/env bash
# Firm reason: 4. It checks coax/build.sh, the bootstrap that exists before Coax.
#
# Three warm coax/build.sh runs at once, each in a new worktree of HEAD, must
# each install a verified compiler within the limit, and all three compilers
# must be byte-identical. Needs this tree's fingerprint in the checked-build
# cache; run coax/build.sh once first.
#   coax/tests/build-cache.sh [LIMIT_SECONDS]   (default 30)
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd -P)"
limit="${1:-30}"
cache_root="${COAX_RELEASE_CACHE:-$HOME/.cache/coax-release}"
fingerprint="$("$repo/coax/provenance.sh" fingerprint "$repo")"
[[ -d "$cache_root/$fingerprint" ]] || { echo "build-cache: ${fingerprint:0:12} is not cached; run coax/build.sh first" >&2; exit 2; }
[[ -z "$(git -C "$repo" status --porcelain --untracked-files=no -- coax scripts/coax-release.sh)" ]] || { echo 'build-cache: commit coax/ first; the worktrees check out HEAD' >&2; exit 2; }
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-build-cache.XXXXXXXX")"
trees=()
cleanup() { for tree in "${trees[@]}"; do git -C "$repo" worktree remove --force "$tree" 2> /dev/null || true; done; rm -rf "$work"; }
trap cleanup EXIT
for n in 1 2 3; do
  git -C "$repo" worktree add -q --detach "$work/tree$n" HEAD
  trees+=("$work/tree$n")
done
start=$SECONDS pids=()
for n in 1 2 3; do "$work/tree$n/coax/build.sh" > "$work/log$n" 2>&1 & pids+=($!); done
failed=0
for n in 1 2 3; do wait "${pids[$((n - 1))]}" || { echo "build-cache: build $n failed:" >&2; cat "$work/log$n" >&2; failed=1; }; done
elapsed=$((SECONDS - start))
[[ $failed == 0 ]] || exit 1
for n in 1 2 3; do
  "$work/tree$n/coax/provenance.sh" verify "$work/tree$n" "$work/tree$n/coax/bin/coax" "$work/tree$n/coax/bin/provenance-check" "$work/tree$n/coax/bin/provenance.record"
done
cmp "$work/tree1/coax/bin/coax" "$work/tree2/coax/bin/coax"
cmp "$work/tree1/coax/bin/coax" "$work/tree3/coax/bin/coax"
(( elapsed <= limit )) || { echo "build-cache: three warm builds took ${elapsed} s (limit ${limit} s)" >&2; exit 1; }
echo "build-cache: three warm builds in new worktrees: ${elapsed} s, one verified compiler"
