#!/usr/bin/env bash
# Firm reason: 4. Published compiler builds: the bootstrap's checked result,
# built once per platform on a release host and fetched by every other
# machine, so a new checkout, worktree or container installs the compiler in
# seconds instead of bootstrapping it (coax/RELEASE.md, Published builds).
#
#   coax/published.sh platform                 # linux-x86_64, darwin-arm64, ...
#   coax/published.sh name ROOT                # <source fingerprint>/<platform>
#   coax/published.sh pack ROOT OUT.tar.gz     # coax/bin's checked build, with its manifest
#   coax/published.sh check DIR ROOT           # an unpacked build: its manifest, names and hashes
#   coax/published.sh fetch ROOT DIR           # download this tree's build into DIR and check it
#   coax/published.sh upload ROOT OUT.tar.gz   # put a packed build in the bucket and its GitHub mirror (never over one)
#   coax/published.sh publish ROOT             # pack coax/bin and upload it, if not there yet
#   coax/published.sh mirror ROOT              # copy this tree's bucket build to the GitHub mirror if it lacks it
#   coax/published.sh failed ROOT NOTE         # a release host's note of why this platform's build failed
#
# A build is named by its source fingerprint (provenance.sh source-fingerprint:
# the compiler's sources and the platform, not the local toolchain) and its
# platform. Linux builds are static (COAX_LDFLAGS=-static), so one runs on any
# Linux of its architecture; macOS builds use the system's libraries.
#
# Every build is also mirrored as a GitHub release asset: release tag
# coax-build-<first 16 of the source fingerprint>, assets <platform>.tar.gz and
# <platform>.tar.gz.sha256. fetch reads the bucket first and the mirror second,
# so a machine whose network can reach GitHub but not storage.googleapis.com
# (cloud agent sessions with an allowlist) still installs in seconds.
#
#   COAX_PUBLISHED_URL     where builds are read (default the public release bucket)
#   COAX_PUBLISHED_BUCKET  where builds are written (gs://..., release hosts only)
#   COAX_PUBLISHED_GITHUB  the mirror's repository (default OpenAgentsInc/openagents; none turns it off)
#   COAX_PUBLISHED_MIRROR_URL  where mirrored builds are read (default github.com/$COAX_PUBLISHED_GITHUB/releases/download)
set -euo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
url_base="${COAX_PUBLISHED_URL:-https://storage.googleapis.com/openagentsgemini-cli-releases/coax/builds}"
bucket="${COAX_PUBLISHED_BUCKET:-gs://openagentsgemini-cli-releases/coax/builds}"
github="${COAX_PUBLISHED_GITHUB:-OpenAgentsInc/openagents}"
mirror_url="${COAX_PUBLISHED_MIRROR_URL:-https://github.com/$github/releases/download}"
fail() { echo "coax published: $*" >&2; exit 1; }
sha() {
  if command -v shasum > /dev/null 2>&1; then shasum -a 256 | awk '{print $1}'
  elif command -v sha256sum > /dev/null 2>&1; then sha256sum | awk '{print $1}'
  else fail 'SHA-256 tool unavailable'; fi
}
# The files of a build: the compiler, the provenance helper, and the C
# artifacts native tests link (build.sh's artifacts list). provenance.record
# is never shipped: it binds a build to one machine's toolchain, so each
# machine writes its own (build.sh from_published).
files=(coax provenance-check check-domains.c check-domains.h lexer.c lexer.h parser.c parser.h checker.c checker.h sources.c sources.h proto_runtime.inc proto_importer.c proto_importer.h mutation-survivors)
platform() {
  local os arch
  case "$(uname -s)" in Linux) os=linux ;; Darwin) os=darwin ;; *) fail "no published builds for $(uname -s)" ;; esac
  case "$(uname -m)" in x86_64|amd64) arch=x86_64 ;; arm64|aarch64) arch=arm64 ;; *) fail "no published builds for $(uname -m)" ;; esac
  printf '%s-%s\n' "$os" "$arch"
}
name() { printf '%s/%s\n' "$("$here/provenance.sh" source-fingerprint "$1")" "$(platform)"; }
# A Linux build must not need any shared library: it has to run on every Linux.
portable() {
  [[ "$(uname -s)" == Linux ]] || return 0
  local f out
  for f in "$@"; do
    out="$(ldd "$f" 2>&1 || true)"
    [[ "$out" == *"not a dynamic executable"* || "$out" == *"statically linked"* ]] || fail "$(basename "$f") links shared libraries; build with COAX_LDFLAGS=-static"
  done
}
manifest_value() { awk -v k="$2" '$1 == k { print $2; exit }' "$1/manifest"; }
check() {
  local dir="$1" root="$2" n expected got
  [[ -f "$dir/manifest" && ! -L "$dir/manifest" ]] || fail 'no manifest'
  [[ "$(head -n 1 "$dir/manifest")" == coax-published-v1 ]] || fail 'unsupported manifest'
  [[ "$(manifest_value "$dir" source)" == "$("$here/provenance.sh" source-fingerprint "$root")" ]] || fail 'built from other compiler sources than this tree'
  [[ "$(manifest_value "$dir" platform)" == "$(platform)" ]] || fail "built for $(manifest_value "$dir" platform), not $(platform)"
  # Exactly the build's files, each with its hash; nothing else in the directory.
  for n in "${files[@]}"; do
    [[ -f "$dir/$n" && ! -L "$dir/$n" ]] || fail "missing or linked file: $n"
    expected="$(awk -v n="$n" '$1 == "file" && $2 == n { print $3; exit }' "$dir/manifest")"
    got="$(sha < "$dir/$n")"
    [[ -n "$expected" && "$expected" == "$got" ]] || fail "$n differs from the manifest"
  done
  [[ "$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')" == "$(( ${#files[@]} + 1 ))" ]] || fail 'unexpected files in the build'
  [[ "$(awk '$1 == "file"' "$dir/manifest" | wc -l | tr -d ' ')" == "${#files[@]}" ]] || fail 'unexpected files in the manifest'
  chmod +x "$dir/coax" "$dir/provenance-check"
  portable "$dir/coax" "$dir/provenance-check"
  "$dir/coax" version > /dev/null 2>&1 || fail 'the compiler does not run here'
}
pack() {
  local root="$1" out="$2" bin work n commit
  root="$(cd "$root" && pwd -P)"; bin="$root/coax/bin"
  # Only a fully checked build of exactly this tree is ever published.
  "$here/provenance.sh" verify "$root" "$bin/coax" "$bin/provenance-check" "$bin/provenance.record" > /dev/null || fail 'coax/bin is not a checked build of this tree; run coax/build.sh'
  [[ -z "$(git -C "$root" status --porcelain --untracked-files=no -- coax scripts/coax-release.sh)" ]] || fail 'commit the compiler sources first'
  portable "$bin/coax" "$bin/provenance-check"
  # A published build gates on every surviving mutant, wherever it is.
  [[ ! -s "$bin/mutation-survivors" ]] || fail 'coax/bin has surviving mutants; publish only a strict build (COAX_MUTATION_FULL=1)'
  work="$(mktemp -d "${TMPDIR:-/tmp}/coax-published.XXXXXXXX")"
  trap 'rm -rf "$work"' RETURN
  commit="$(git -C "$root" rev-parse HEAD)"
  {
    echo coax-published-v1
    echo "source $("$here/provenance.sh" source-fingerprint "$root")"
    echo "platform $(platform)"
    echo "commit $commit"
    echo "version $("$bin/coax" version 2>/dev/null | head -n 1 | tr ' ' '_')"
    echo "cxx $("${CXX:-c++}" --version | head -n 1 | tr ' ' '_')"
    echo "ldflags ${COAX_LDFLAGS:-none}"
    echo "made $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    for n in "${files[@]}"; do
      [[ -f "$bin/$n" ]] || fail "coax/bin lacks $n"
      cp "$bin/$n" "$work/$n"
      echo "file $n $(sha < "$bin/$n")"
    done
  } > "$work/manifest"
  check "$work" "$root"
  tar -C "$work" -czf "$out" manifest "${files[@]}"
  echo "coax published: packed $(name "$root") from $commit into $out"
}
# The GitHub mirror's release tag and asset names for a build name <source>/<platform>.
mirror_tag() { printf 'coax-build-%s\n' "${1:0:16}"; }
mirror_asset() { basename "$1"; }
# Download URL and SHA-256 file into $2/build.tar.gz and $2/sum. Prints why it
# failed: "missing" (the server answered and has no such build) or
# "unreachable" (no answer: the network, a proxy or an allowlist refused it).
get() {
  local base="$1" tmp="$2" code
  code="$(curl -sSL --retry 2 --max-time 120 -o "$tmp/sum" -w '%{http_code}' "$base.sha256" 2> /dev/null || true)"
  case "$code" in 200) ;; 4??) echo missing; return 1 ;; *) echo unreachable; return 1 ;; esac
  curl -fsSL --retry 2 --max-time 600 "$base" -o "$tmp/build.tar.gz" 2> /dev/null || { echo unreachable; return 1; }
}
fetch() {
  local root="$1" dir="$2" path tmp want got bucket_why mirror_why=off why
  path="$(name "$root")"
  command -v curl > /dev/null 2>&1 || fail 'curl unavailable'
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/coax-fetch.XXXXXXXX")"
  trap 'rm -rf "$tmp"' RETURN
  if ! bucket_why="$(get "$url_base/$path.tar.gz" "$tmp")"; then
    [[ "$github" == none ]] || mirror_why="$(get "$mirror_url/$(mirror_tag "$path")/$(mirror_asset "$path").tar.gz" "$tmp")" || true
    if [[ "$mirror_why" != "" ]]; then
      # the release host's note when its last try failed (failed, below), so the failure is seen
      why="$(curl -fsSL --max-time 20 "$url_base/$path.failed" 2> /dev/null | head -n 1 || true)"
      fail "no published build $path (bucket: $bucket_why; GitHub mirror: $mirror_why)${why:+; its release host failed it: $why}"
    fi
  fi
  want="$(awk '{print $1; exit}' "$tmp/sum")"; got="$(sha < "$tmp/build.tar.gz")"
  [[ -n "$want" && "$want" == "$got" ]] || fail "$path: the download differs from its checksum"
  mkdir -p "$dir"
  tar -C "$dir" -xzf "$tmp/build.tar.gz"
  check "$dir" "$root"
}
# The GitHub mirror (gh, authenticated with write access to $github): the
# release coax-build-<source> is made on first use, at the build's commit, as a
# prerelease; an asset already there is never replaced.
mirror_has() {
  gh release view "$(mirror_tag "$1")" --repo "$github" --json assets --jq '.assets[].name' 2> /dev/null | grep -qx "$(mirror_asset "$1").tar.gz"
}
mirror_upload() {
  local path="$1" file="$2" sum="$3" commit="$4" tag asset
  [[ "$github" != none ]] || return 0
  command -v gh > /dev/null 2>&1 || { echo "coax published: gh unavailable; $path not mirrored to GitHub" >&2; return 1; }
  tag="$(mirror_tag "$path")"; asset="$(mirror_asset "$path")"
  if ! gh release view "$tag" --repo "$github" > /dev/null 2>&1; then
    gh release create "$tag" --repo "$github" --target "$commit" --prerelease --title "Coax compiler build ${path%%/*}" \
      --notes "Published Coax compiler builds for source fingerprint ${path%%/*} (coax/published.sh, coax/RELEASE.md#published-builds). Installed by coax/build.sh; not a product release." > /dev/null \
      || { echo "coax published: could not create the GitHub release $tag" >&2; return 1; }
  fi
  if mirror_has "$path"; then echo "coax published: $path is already mirrored"; return 0; fi
  local stage; stage="$(mktemp -d "${TMPDIR:-/tmp}/coax-mirror.XXXXXXXX")"
  cp "$file" "$stage/$asset.tar.gz"; printf '%s  %s.tar.gz\n' "$(awk '{print $1; exit}' "$sum")" "$asset" > "$stage/$asset.tar.gz.sha256"
  gh release upload "$tag" --repo "$github" "$stage/$asset.tar.gz" "$stage/$asset.tar.gz.sha256" > /dev/null \
    || { rm -rf "$stage"; echo "coax published: could not upload $path to the GitHub mirror" >&2; return 1; }
  rm -rf "$stage"
  echo "coax published: mirrored $path to github.com/$github/releases/tag/$tag"
}
upload() {
  local root="$1" file="$2" path sum commit
  path="$(name "$root")"
  command -v gcloud > /dev/null 2>&1 || fail 'gcloud unavailable'
  sum="$(mktemp "${TMPDIR:-/tmp}/coax-sum.XXXXXXXX")"
  printf '%s  %s\n' "$(sha < "$file")" "$(basename "$path").tar.gz" > "$sum"
  # Immutable: a published name is never replaced.
  if gcloud storage ls "$bucket/$path.tar.gz" > /dev/null 2>&1; then echo "coax published: $path is already published"
  else
    gcloud storage cp "$file" "$bucket/$path.tar.gz" --quiet > /dev/null
    gcloud storage cp "$sum" "$bucket/$path.tar.gz.sha256" --quiet > /dev/null
    echo "coax published: uploaded $path"
  fi
  gcloud storage rm "$bucket/$path.failed" --quiet > /dev/null 2>&1 || true
  commit="$(tar -xzOf "$file" manifest | awk '$1 == "commit" { print $2; exit }')"
  mirror_upload "$path" "$file" "$sum" "$commit" || echo "coax published: the bucket has $path; the GitHub mirror does not yet" >&2
  rm -f "$sum"
}
# Why this tree's build failed on this platform's release host (one line first:
# fetch names it to everyone who falls back to a bootstrap), beside where the
# build would be; each failed round replaces it and upload removes it.
failed() {
  local root="$1" note="$2"
  command -v gcloud > /dev/null 2>&1 || fail 'gcloud unavailable'
  gcloud storage cp "$note" "$bucket/$(name "$root").failed" --cache-control=no-store --content-type=text/plain --quiet > /dev/null
}
# The bucket's builds of this tree (every platform: the source fingerprint does
# not depend on the platform), copied to the GitHub mirror where it lacks them.
# A release host runs it each round, which also backfills builds published
# before the mirror existed.
mirror() {
  local root="$1" fp plat path tmp commit copied=0 failed=0
  [[ "$github" != none ]] || return 0
  command -v gh > /dev/null 2>&1 || fail 'gh unavailable'
  fp="$("$here/provenance.sh" source-fingerprint "$root")"
  for plat in linux-x86_64 linux-arm64 darwin-arm64 darwin-x86_64; do
    path="$fp/$plat"
    mirror_has "$path" && continue
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/coax-mirror-get.XXXXXXXX")"
    if get "$url_base/$path.tar.gz" "$tmp" > /dev/null; then
      if [[ "$(awk '{print $1; exit}' "$tmp/sum")" != "$(sha < "$tmp/build.tar.gz")" ]]; then echo "coax published: $path: the bucket's build differs from its checksum" >&2; failed=1
      else
        commit="$(tar -xzOf "$tmp/build.tar.gz" manifest | awk '$1 == "commit" { print $2; exit }')"
        if mirror_upload "$path" "$tmp/build.tar.gz" "$tmp/sum" "$commit"; then copied=$((copied + 1)); else failed=1; fi
      fi
    fi
    rm -rf "$tmp"
  done
  [[ "$failed" == 0 ]] || fail "could not mirror every build of ${fp:0:12}"
  echo "coax published: ${fp:0:12}: $copied build(s) newly mirrored"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  platform) platform ;;
  name) [[ $# == 1 ]] || fail 'usage: published.sh name ROOT'; name "$1" ;;
  pack) [[ $# == 2 ]] || fail 'usage: published.sh pack ROOT OUT.tar.gz'; pack "$1" "$2" ;;
  check) [[ $# == 2 ]] || fail 'usage: published.sh check DIR ROOT'; check "$1" "$2" ;;
  fetch) [[ $# == 2 ]] || fail 'usage: published.sh fetch ROOT DIR'; fetch "$1" "$2" ;;
  upload) [[ $# == 2 ]] || fail 'usage: published.sh upload ROOT OUT.tar.gz'; upload "$1" "$2" ;;
  mirror) [[ $# == 1 ]] || fail 'usage: published.sh mirror ROOT'; mirror "$1" ;;
  failed) [[ $# == 2 ]] || fail 'usage: published.sh failed ROOT NOTE'; failed "$1" "$2" ;;
  publish)
    [[ $# == 1 ]] || fail 'usage: published.sh publish ROOT'
    out="$(mktemp "${TMPDIR:-/tmp}/coax-build.XXXXXXXX").tar.gz"
    pack "$1" "$out"; upload "$1" "$out"; rm -f "$out" ;;
  *) fail 'usage: published.sh platform | name ROOT | pack ROOT OUT | check DIR ROOT | fetch ROOT DIR | upload ROOT OUT | publish ROOT | mirror ROOT | failed ROOT NOTE' ;;
esac
