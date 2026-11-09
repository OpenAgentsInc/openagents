#!/usr/bin/env bash
# Firm reason: 5. One page build per commit, shared by every agent and the
# release (docs/verse/remote-builds.md, #13119): git, gcloud, tar and
# coax/remote/remote.sh.
#
#   coax/remote/pages.sh COMMIT [DIR]   COMMIT's built pages into DIR (default this checkout's
#                                       root): apps/openagents-web/build/ (every module, the
#                                       render packages) and the build's other untracked files.
#                                       From the shared store, or built once on a host and
#                                       stored, then fetched. Tracked files never change.
#   coax/remote/pages.sh fetch COMMIT DIR
#                                       only from the store (release.sh): exit 1 when COMMIT
#                                       has none
#   coax/remote/pages.sh build COMMIT   build and store COMMIT's pages unless stored (after.sh)
#   coax/remote/pages.sh has COMMIT     exit 0 when the store has COMMIT's pages
#
# The store: gs://$COAX_SHARED_CACHE/pages/COMMIT/ (pages.tar.gz and MANIFEST,
# which names the commit, its tree and the archive's SHA-256; a fetch checks
# both). A build runs apps/openagents-web/build.sh in release mode on a build
# host, whose wasm.sh takes every unit another host built from the shared
# cache. While one caller builds a commit (pages/COMMIT/BUILDING), the others
# wait for its MANIFEST instead of building it again.
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
bucket="${COAX_SHARED_CACHE:-openagentsgemini-coax-build-cache}"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
say() { echo "pages: $*" >&2; }
[[ "$bucket" != 0 ]] || { say "COAX_SHARED_CACHE=0: no shared store"; exit 1; }

resolve() {   # rev: the full commit
  git rev-parse --verify -q "$1^{commit}" 2> /dev/null || { git fetch -q origin "$1" 2> /dev/null && git rev-parse --verify -q "$1^{commit}"; }
}
has() { gcloud storage ls "gs://$bucket/pages/$1/MANIFEST" > /dev/null 2>&1; }

fetch() {   # commit dir: 0 fetched and checked, 1 none (or it doesn't check)
  local commit="$1" dir="$2" tmp started=$SECONDS
  tmp="$(mktemp -d)"
  gcloud storage cp "gs://$bucket/pages/$commit/MANIFEST" "$tmp/MANIFEST" --quiet > /dev/null 2>&1 || { rm -rf "$tmp"; say "none stored for ${commit:0:10}"; return 1; }
  local tree want sum
  tree="$(git rev-parse "$commit^{tree}")"
  want="$(awk '$1 == "tar" { print $2 }' "$tmp/MANIFEST")"
  if [[ "$(sed -n 1p "$tmp/MANIFEST")" != coax-pages-1 || "$(sed -n 2p "$tmp/MANIFEST")" != "commit $commit" ||
        "$(sed -n 3p "$tmp/MANIFEST")" != "tree $tree" || -z "$want" ]]; then
    say "the stored MANIFEST for ${commit:0:10} doesn't name its tree $tree; not using it"; rm -rf "$tmp"; return 1
  fi
  gcloud storage cp "gs://$bucket/pages/$commit/pages.tar.gz" "$tmp/pages.tar.gz" --quiet > /dev/null 2>&1 || { rm -rf "$tmp"; say "could not download ${commit:0:10}'s pages"; return 1; }
  sum="$(shasum -a 256 < "$tmp/pages.tar.gz" | cut -c1-64)"
  [[ "$sum" == "$want" ]] || { say "${commit:0:10}'s pages.tar.gz has SHA-256 $sum, its MANIFEST says $want; not using it"; rm -rf "$tmp"; return 1; }
  mkdir -p "$dir" && tar -xzf "$tmp/pages.tar.gz" -C "$dir" || { rm -rf "$tmp"; say "could not unpack into $dir"; return 1; }
  say "${commit:0:10}: $(awk '$1 == "files" { print $2 }' "$tmp/MANIFEST") files ($(( $(wc -c < "$tmp/pages.tar.gz") / 1000000 )) MB, built on $(awk '$1 == "host" { print $2 " at " $3 }' "$tmp/MANIFEST")) into $dir in $((SECONDS - started)) s"
  rm -rf "$tmp"
}

build() {   # commit: stored when this returns 0
  local commit="$1" lock="gs://$bucket/pages/$1/BUILDING" since
  has "$commit" && return 0
  while ! echo "$(date +%s) $(hostname -s) $$" | gcloud storage cp - "$lock" --if-generation-match=0 --quiet > /dev/null 2>&1; do
    # Another caller builds it: wait for its MANIFEST, or take over a build
    # that has run for 40 minutes (its caller is gone).
    since="$(gcloud storage cat "$lock" 2> /dev/null | awk '{ print $1 }')"
    if [[ -z "$since" ]]; then continue; fi
    if (( $(date +%s) - since > 2400 )); then gcloud storage rm "$lock" --quiet > /dev/null 2>&1; continue; fi
    say "another caller is building ${commit:0:10} (for $(( $(date +%s) - since )) s); waiting for it"
    while (( $(date +%s) - since <= 2400 )); do
      sleep 15
      has "$commit" && return 0
      gcloud storage ls "$lock" > /dev/null 2>&1 || break
    done
    has "$commit" && return 0
  done
  say "building ${commit:0:10} on a build host (once; every caller after this fetches it)"
  local status
  # COAX_PAGES_TRY keeps the host from replaying an earlier run of this job.
  COAX_PAGES_TRY="$(date +%s)" "$here/remote.sh" run --commit "$commit" --no-fallback -- \
    bash -c 'COAXIAL_MODE=release apps/openagents-web/build.sh && "$COAX_SHARED_CACHE_TOOL" publish-pages "$1" "$COAX_JOB_MARKER"' pages "$commit" >&2
  status=$?
  gcloud storage rm "$lock" --quiet > /dev/null 2>&1
  (( status == 0 )) || { say "the build of ${commit:0:10} failed (status $status)"; return 1; }
  has "$commit" || { say "the build of ${commit:0:10} passed but stored nothing"; return 1; }
}

case "${1:-}" in
  fetch) c="$(resolve "$2")" || { say "no commit $2"; exit 2; }; fetch "$c" "$3" ;;
  build) c="$(resolve "$2")" || { say "no commit $2"; exit 2; }; build "$c" ;;
  has) c="$(resolve "$2")" || exit 2; has "$c" ;;
  ""|-h|--help) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
  *)
    c="$(resolve "$1")" || { say "no commit $1"; exit 2; }
    dir="${2:-$(git rev-parse --show-toplevel)}"
    has "$c" || build "$c" || exit 1
    fetch "$c" "$dir" ;;
esac
