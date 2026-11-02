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
#   coax/published.sh upload ROOT OUT.tar.gz   # put a packed build in the bucket (never over one)
#   coax/published.sh publish ROOT             # pack coax/bin and upload it, if not there yet
#
# A build is named by its source fingerprint (provenance.sh source-fingerprint:
# the compiler's sources and the platform, not the local toolchain) and its
# platform. Linux builds are static (COAX_LDFLAGS=-static), so one runs on any
# Linux of its architecture; macOS builds use the system's libraries.
#
#   COAX_PUBLISHED_URL     where builds are read (default the public release bucket)
#   COAX_PUBLISHED_BUCKET  where builds are written (gs://..., release hosts only)
set -euo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
url_base="${COAX_PUBLISHED_URL:-https://storage.googleapis.com/openagentsgemini-cli-releases/coax/builds}"
bucket="${COAX_PUBLISHED_BUCKET:-gs://openagentsgemini-cli-releases/coax/builds}"
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
fetch() {
  local root="$1" dir="$2" path tmp want got
  path="$(name "$root")"
  command -v curl > /dev/null 2>&1 || fail 'curl unavailable'
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/coax-fetch.XXXXXXXX")"
  trap 'rm -rf "$tmp"' RETURN
  curl -fsSL --retry 2 --max-time 120 "$url_base/$path.tar.gz.sha256" -o "$tmp/sum" 2> /dev/null || fail "no published build $path"
  curl -fsSL --retry 2 --max-time 600 "$url_base/$path.tar.gz" -o "$tmp/build.tar.gz" || fail "could not download $path"
  want="$(awk '{print $1; exit}' "$tmp/sum")"; got="$(sha < "$tmp/build.tar.gz")"
  [[ -n "$want" && "$want" == "$got" ]] || fail "$path: the download differs from its checksum"
  mkdir -p "$dir"
  tar -C "$dir" -xzf "$tmp/build.tar.gz"
  check "$dir" "$root"
}
upload() {
  local root="$1" file="$2" path sum
  path="$(name "$root")"
  command -v gcloud > /dev/null 2>&1 || fail 'gcloud unavailable'
  # Immutable: a published name is never replaced.
  if gcloud storage ls "$bucket/$path.tar.gz" > /dev/null 2>&1; then echo "coax published: $path is already published"; return 0; fi
  sum="$(mktemp "${TMPDIR:-/tmp}/coax-sum.XXXXXXXX")"
  printf '%s  %s\n' "$(sha < "$file")" "$(basename "$path").tar.gz" > "$sum"
  gcloud storage cp "$file" "$bucket/$path.tar.gz" --quiet > /dev/null
  gcloud storage cp "$sum" "$bucket/$path.tar.gz.sha256" --quiet > /dev/null
  rm -f "$sum"
  echo "coax published: uploaded $path"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  platform) platform ;;
  name) [[ $# == 1 ]] || fail 'usage: published.sh name ROOT'; name "$1" ;;
  pack) [[ $# == 2 ]] || fail 'usage: published.sh pack ROOT OUT.tar.gz'; pack "$1" "$2" ;;
  check) [[ $# == 2 ]] || fail 'usage: published.sh check DIR ROOT'; check "$1" "$2" ;;
  fetch) [[ $# == 2 ]] || fail 'usage: published.sh fetch ROOT DIR'; fetch "$1" "$2" ;;
  upload) [[ $# == 2 ]] || fail 'usage: published.sh upload ROOT OUT.tar.gz'; upload "$1" "$2" ;;
  publish)
    [[ $# == 1 ]] || fail 'usage: published.sh publish ROOT'
    out="$(mktemp "${TMPDIR:-/tmp}/coax-build.XXXXXXXX").tar.gz"
    pack "$1" "$out"; upload "$1" "$out"; rm -f "$out" ;;
  *) fail 'usage: published.sh platform | name ROOT | pack ROOT OUT | check DIR ROOT | fetch ROOT DIR | upload ROOT OUT | publish ROOT' ;;
esac
