#!/bin/sh
# Build, sign, and publish Coder: one archive per platform holding coder,
# the openagents command, and the microcoder engine Coder runs its turns with.
# Reimplemented from this repository's scripts/release/terminal.sh.
#
# The public contract is under
# https://storage.googleapis.com/openagentsgemini-cli-releases/coder:
#   coder.<channel>                          a version string
#   coder-<version>-<platform>.tar.gz        macOS and Linux: coder (built from
#                                            coder-new), openagents, microcoder
#   coder-<version>-windows-x86_64.zip       coder.exe, openagents.exe,
#                                            microcoder.exe, coder-boundary.exe
#   SHA256SUMS-coder-<version>               the archive digests
#   coder-<version>.release-manifest.json    source, build, and per-binary evidence
#   install.sh, install.ps1                  hosted installers
# The archives hold the executables at their top level under their installed
# names, so a manual install extracts one archive into ~/.openagents/bin.
# Versions up to 1.0.0-rc.5 were published as separate executables
# (<name>-<version>-<platform>[.exe]); the installers still read those.
#
# Builds read an isolated archive of the commit. Published versions are
# immutable. macOS binaries are signed, notarized, and accepted by Gatekeeper
# before they are archived. The channel moves only when all seven platforms
# have their archive and its checksum entry. --allow-partial publishes the
# built artifacts and leaves the channel unchanged.
#
# Usage:
#   scripts/release/coder.sh --version 1.0.0-rc.4
#   scripts/release/coder.sh --version 1.0.0-rc.4 --publish
#   scripts/release/coder.sh --version 1.0.0 --publish --channel stable --publish-installers
#   scripts/release/coder.sh --publish-installers
#
# Options:
#   --version VERSION       X.Y.Z or X.Y.Z-rc.N; required for a build.
#   --commit REF            Source commit; default HEAD.
#   --targets "a b c"       Platforms to build; default all seven.
#   --publish               Upload artifacts; off by default.
#   --channel NAME          Channel to point after publishing; default rc.
#                            stable takes only X.Y.Z, and also moves rc to the
#                            release, so rc never names an older version.
#   --allow-partial         Publish partial artifacts without moving a channel.
#   --skip-notarization     Local builds only; refused with --publish.
#   --point-channel NAME    Move a fully covered published version; no build.
#   --publish-installers    Publish scripts/install/coder.sh and coder.ps1;
#                            with --version and --publish, after the release.
#
# Environment:
#   CODER_RELEASES_BUCKET   Default openagentsgemini-cli-releases.
#   CLOUDSDK_CONFIG         Configuration allowed to write the release bucket.
#   OPENAGENTS_NOTARY_ENV   Signing and notary configuration; default
#                            ~/work/.secrets/appstoreconnect.env.
#   NOTARY_KEYCHAIN_PROFILE Optional saved notarytool profile.
#   OA_DEVELOPER_ID_APPLICATION Optional Developer ID Application identity.
#   CARGO_TARGET_DIR        Persistent build cache outside the checkout.
#   CODER_RELEASE_WINDOWS_TARGET_DIR, CODER_RELEASE_ARM_MUSL_TARGET_DIR,
#   CODER_RELEASE_MAC_INTEL_TARGET_DIR
#                            Optional persistent caches for those cross builds.
#   CODER_RELEASE_GLIBC     GNU Linux compatibility floor; default 2.28.
#   CODER_RELEASE_ASSESS_ATTEMPTS, CODER_RELEASE_ASSESS_DELAY
#                            Gatekeeper retries; defaults 45 and 60 seconds.
#
set -eu

# A test that sources this file for its functions names it in
# CODER_RELEASE_SCRIPT, because `$0` is then the test.
script_dir=$(CDPATH='' cd -- "$(dirname -- "${CODER_RELEASE_SCRIPT:-$0}")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/../.." && pwd)

# platform | rust target triple | builder | expected `file` signature
#
# The two Linux libc flavors are separate platforms because they are
# separate artifacts: the gnu build is dynamically linked against glibc, and
# the musl build is static and names no interpreter.
platform_table='macos-aarch64|aarch64-apple-darwin|cargo|Mach-O 64-bit executable arm64
macos-x86_64|x86_64-apple-darwin|cargo|Mach-O 64-bit executable x86_64
linux-x86_64|x86_64-unknown-linux-gnu|zigbuild|ELF 64-bit LSB*x86-64*dynamically linked
linux-x86_64-musl|x86_64-unknown-linux-musl|zigbuild|ELF 64-bit LSB*x86-64*static
linux-aarch64|aarch64-unknown-linux-gnu|zigbuild|ELF 64-bit LSB*ARM aarch64*dynamically linked
linux-aarch64-musl|aarch64-unknown-linux-musl|zigbuild|ELF 64-bit LSB*ARM aarch64*static
windows-x86_64|x86_64-pc-windows-gnu|zigbuild|PE32+ executable*x86-64'

all_platforms=$(printf '%s\n' "$platform_table" | cut -d'|' -f1 | tr '\n' ' ')

# The glibc the gnu builds link against. zig links against this version's
# symbols, so the artifact runs on any glibc at least this new.
glibc_version=${CODER_RELEASE_GLIBC:-2.28}

case "$(uname -sm)" in
  'Darwin arm64') native_platform=macos-aarch64 ;;
  'Darwin x86_64') native_platform=macos-x86_64 ;;
  'Linux aarch64') native_platform=linux-aarch64 ;;
  'Linux x86_64') native_platform=linux-x86_64 ;;
  *) native_platform='' ;;
esac

# The bucket prefix every name below lives under.
prefix=coder
product=coder
companion=openagents
engine=microcoder
helper=coder-boundary
# The cargo package and binary behind each published name.
product_package=coder-new
product_bin=coder-new
companion_package=openagents-cli
companion_bin=openagents
engine_package=microcoder
engine_bin=microcoder

version=''
commit_ref=HEAD
targets=''
publish=0
channel=rc
allow_partial=0
skip_notarization=0
point_channel_name=''
publish_installers=0

assess_attempts=${CODER_RELEASE_ASSESS_ATTEMPTS:-45}
assess_delay=${CODER_RELEASE_ASSESS_DELAY:-60}
notarized=''
gatekeeper_verdicts=''

bucket=${CODER_RELEASES_BUCKET:-openagentsgemini-cli-releases}
default_gcloud_config="$HOME/work/.secrets/gcloud-sa-config"
if [ "${CODER_RELEASE_LIBRARY:-}" != 1 ] && [ -z "${CLOUDSDK_CONFIG:-}" ] && [ -d "$default_gcloud_config" ]; then
  CLOUDSDK_CONFIG=$default_gcloud_config
fi
[ -z "${CLOUDSDK_CONFIG:-}" ] || export CLOUDSDK_CONFIG
notary_env=${OPENAGENTS_NOTARY_ENV:-$HOME/work/.secrets/appstoreconnect.env}
public_base="https://storage.googleapis.com/$bucket/$prefix"
root="gs://$bucket/$prefix"

entitlements="$script_dir/openagents-terminal.entitlements"

target_dir=${CARGO_TARGET_DIR:-"$HOME/.cache/openagents/target-release-coder"}

die() {
  echo "$@" >&2
  exit 1
}

# The executables required by one platform, including its boundary helper.
products_for() {
  printf '%s %s %s' "$product" "$companion" "$engine"
  case "$1" in windows-*) printf ' %s' "$helper" ;; esac
  printf '\n'
}

binary_for() {
  case "$1" in
    "$product") echo "$product_bin" ;;
    "$companion") echo "$companion_bin" ;;
    "$engine") echo "$engine_bin" ;;
    "$helper") echo "$helper" ;;
    *) die "unknown release executable: $1" ;;
  esac
}

artifact_name() {
  case "$2" in
    windows-*) echo "$1-$version-$2.exe" ;;
    *) echo "$1-$version-$2" ;;
  esac
}

# The one file a platform publishes: every executable of products_for, at
# the archive's top level.
archive_name() {
  case "$1" in
    windows-*) echo "$product-$version-$1.zip" ;;
    *) echo "$product-$version-$1.tar.gz" ;;
  esac
}

# The name an executable is installed under, which is its name in the archive.
installed_name() {
  case "$2" in
    windows-*) echo "$1.exe" ;;
    *) echo "$1" ;;
  esac
}

sums_file_name() {
  echo "SHA256SUMS-$product-$version"
}

channel_name() {
  echo "$product.$1"
}

# Artifact URLs and sums entries use the same names on every platform.
sums_name_for() {
  echo "$1"
}

# The signing identifier per binary, pinned rather than left to the file
# name, which differs per platform.
signing_identifier() {
  case "$1" in
    "$product") echo com.openagents.coder-terminal ;;
    "$companion") echo com.openagents.terminal.openagents ;;
    "$engine") echo com.openagents.terminal.microcoder ;;
  esac
}

gs() {
  gcloud storage "$@"
}

# The digest a sums file records for a name, or nothing.
sums_entry() {
  awk -v name="$2" '$2 == name || $2 == "*" name { print $1; exit }' "$1"
}

# The platforms a version does not cover, as a leading-space list. Covered
# means the bucket holds the platform's archive and the sums file its
# checksum. `$1` is the sums file, `$2` a file of the object names the bucket
# holds.
uncovered_platforms() {
  _gap=''
  for _platform in $all_platforms; do
    _archive=$(archive_name "$_platform")
    _sha=''
    [ -f "$1" ] && _sha=$(sums_entry "$1" "$(sums_name_for "$_archive" "$_platform")")
    if [ -z "$_sha" ] || ! grep -qxF "$_archive" "$2" 2>/dev/null; then
      _gap="$_gap $_platform"
    fi
  done
  printf '%s' "$_gap"
}

# The object names the bucket holds for this version, one per line. The
# listing is a prefix match; `uncovered_platforms` matches whole names.
bucket_objects() {
  for _name in "$product" "$companion" "$engine" "$helper"; do
    gs ls "$root/$_name-$version-*" 2>/dev/null || :
  done | sed 's|.*/||'
}

# Packs one platform's staged executables (`$dist/<artifact_name>`) into
# `$dist/<archive_name>` under their installed names.
make_archive() {
  _platform=$1
  _bundle="$dist/.bundle-$_platform"
  rm -rf "$_bundle"
  mkdir -p "$_bundle"
  _files=''
  for _name in $(products_for "$_platform"); do
    _file=$(installed_name "$_name" "$_platform")
    cp -p "$dist/$(artifact_name "$_name" "$_platform")" "$_bundle/$_file"
    chmod 755 "$_bundle/$_file"
    _files="$_files $_file"
  done
  _archive="$dist/$(archive_name "$_platform")"
  rm -f "$_archive"
  case "$_platform" in
    # shellcheck disable=SC2086
    windows-*) (cd "$_bundle" && zip -q -X "$_archive" $_files) ;;
    # COPYFILE_DISABLE keeps macOS tar from adding AppleDouble (._) entries.
    # shellcheck disable=SC2086
    *) (cd "$_bundle" && COPYFILE_DISABLE=1 tar -czf "$_archive" $_files) ;;
  esac
  rm -rf "$_bundle"
}

published_version() {
  gs cat "$root/$(channel_name "$1")" 2>/dev/null | tr -d '[:space:]'
}

refuse_uncovered_channel() {
  _gap=$(uncovered_platforms "$2" "$3")
  [ -n "$_gap" ] || return 0
  echo "" >&2
  echo "Refusing to point '$1' at $version." >&2
  echo "These platforms have no artifact or no checksum entry for $version:$_gap" >&2
  _standing=$(published_version "$1")
  echo "Channel '$(channel_name "$1")' still points at ${_standing:-nothing}." >&2
  exit 1
}

point_channel() {
  _pointer=$(mktemp)
  printf '%s\n' "$version" >"$_pointer"
  gs cp "$_pointer" "$root/$(channel_name "$1")" \
    --content-type=text/plain --cache-control='public, max-age=60' --quiet
  rm -f "$_pointer"
  _read=$(curl -fsSL "$public_base/$(channel_name "$1")?nocache=$$" 2>/dev/null | tr -d '[:space:]' || :)
  echo "Channel '$(channel_name "$1")' now points at $version (public read: ${_read:-unreadable})"
}

check_version() {
  printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-rc\.(0|[1-9][0-9]*))?$' ||
    die "invalid version: $version (expected X.Y.Z or X.Y.Z-rc.N)"
}

check_channel() {
  printf '%s' "$1" | grep -Eq '^[A-Za-z0-9_-]+$' || die "invalid channel: $1"
}

# stable names only a release, never a candidate.
check_stable_version() {
  if [ "$1" = stable ]; then
    case "$version" in
      *-*) die "the stable channel takes a release (X.Y.Z), not $version" ;;
    esac
  fi
  return 0
}

# One Gatekeeper assessment: `spctl --assess -vv -t install`, the one a
# person's browser download is held to. Prints the verdict and the source on
# two lines; returns 0 only for `accepted` from `Notarized Developer ID`.
assess_once() {
  _assessment=$(spctl --assess -vv -t install "$1" 2>&1) || true
  _verdict=$(printf '%s\n' "$_assessment" | sed -n '1s/.*: //p')
  _source=$(printf '%s\n' "$_assessment" | sed -n 's/^source=//p' | head -1)
  printf '%s\n%s\n' "${_verdict:-no verdict}" "${_source:-no source}"
  [ "$_verdict" = accepted ] && [ "$_source" = 'Notarized Developer ID' ]
}

gatekeeper_verdict_for() {
  printf '%s\n' "$gatekeeper_verdicts" | grep "^$1|" | cut -d'|' -f2-3 | tr '|' ' ' | head -1
}

# Wait until Gatekeeper accepts every notarized artifact; refuse the release
# when one is still refused at the bound. `$1` is a list of staged file
# names.
wait_for_gatekeeper() {
  pending=$1
  [ -n "$pending" ] || return 0
  command -v spctl >/dev/null 2>&1 || die "spctl is required to assess a notarized artifact"
  echo "Gatekeeper assessment of$pending"
  started=$(date +%s)
  round=0
  while :; do
    round=$((round + 1))
    still=''
    for name in $pending; do
      status=0
      reading=$(assess_once "$dist/$name") || status=$?
      verdict=$(printf '%s\n' "$reading" | sed -n 1p)
      origin=$(printf '%s\n' "$reading" | sed -n 2p)
      gatekeeper_verdicts=$( (printf '%s\n' "$gatekeeper_verdicts" | grep -v "^$name|" || true)
        printf '%s|%s|%s\n' "$name" "$verdict" "$origin")
      if [ "$status" = 0 ]; then
        echo "  $name: accepted ($origin) on assessment $round, $(( $(date +%s) - started ))s in"
      else
        echo "  $name: $verdict ($origin) on assessment $round of $assess_attempts"
        still="$still $name"
      fi
    done
    pending=$still
    [ -n "$pending" ] || return 0
    if [ "$round" -ge "$assess_attempts" ]; then
      die "Refusing to publish: Gatekeeper still refuses$pending after $round assessments. Raise CODER_RELEASE_ASSESS_ATTEMPTS and build $version again; nothing was published."
    fi
    echo "  the ticket has not reached the store Gatekeeper reads; next assessment in ${assess_delay}s"
    sleep "$assess_delay"
  done
}

json_string() {
  printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"
}

toolchain_json() {
  _first=1
  printf '{'
  for _tool in rustc cargo cargo-zigbuild zig; do
    case "$_tool" in
      zig) _line=$(zig version 2>/dev/null || :) ;;
      cargo-zigbuild) _line=$(cargo-zigbuild -V 2>/dev/null | head -1 || :) ;;
      *) _line=$("$_tool" --version 2>/dev/null | head -1 || :) ;;
    esac
    [ -n "$_line" ] || continue
    [ "$_first" = 1 ] || printf ', '
    _first=0
    printf '%s: %s' "$(json_string "$_tool")" "$(json_string "$_line")"
  done
  printf '}'
}

# Checks that `$3` still holds the tree of commit `$2` in repository `$1`,
# ignoring what the commit's own .gitignore covers (build output).
source_matches() {
  _want=$(git -C "$1" rev-parse "$2^{tree}")
  _gitdir=$(git -C "$1" rev-parse --absolute-git-dir)
  _index=$(mktemp "${TMPDIR:-/tmp}/coder-release-index.XXXXXX")
  rm -f "$_index"
  (cd "$3" && GIT_DIR=$_gitdir GIT_WORK_TREE=$3 GIT_INDEX_FILE=$_index \
    git -c core.excludesFile=/dev/null add -A -- . 2>/dev/null)
  # Tracked files under an ignore rule are part of the commit too.
  git -C "$1" ls-files -ci --exclude-standard 2>/dev/null |
    while IFS= read -r _path; do
      if [ -e "$3/$_path" ] || [ -L "$3/$_path" ]; then
        (cd "$3" && GIT_DIR=$_gitdir GIT_WORK_TREE=$3 GIT_INDEX_FILE=$_index \
          git -c core.excludesFile=/dev/null add -f -- "$_path" 2>/dev/null || :)
      fi
    done
  _got=$(GIT_DIR=$_gitdir GIT_INDEX_FILE=$_index git write-tree)
  if [ "$_got" = "$_want" ]; then
    rm -f "$_index"
    return 0
  fi
  GIT_DIR=$_gitdir GIT_INDEX_FILE=$_index git diff --cached --name-only "$2" -- | head -20
  rm -f "$_index"
  return 1
}

# `scripts/test-release-coder.sh` sources the functions above and stops
# here.
if [ "${CODER_RELEASE_LIBRARY:-}" = 1 ]; then
  return 0
fi

while [ $# -gt 0 ]; do
  case "$1" in
    --version) version=${2:-}; shift 2 ;;
    --commit) commit_ref=${2:-}; shift 2 ;;
    --targets) targets=${2:-}; shift 2 ;;
    --channel) channel=${2:-}; shift 2 ;;
    --publish) publish=1; shift ;;
    --allow-partial) allow_partial=1; shift ;;
    --skip-notarization) skip_notarization=1; shift ;;
    --point-channel) point_channel_name=${2:-}; shift 2 ;;
    --publish-installers) publish_installers=1; shift ;;
    -h | --help) sed -n '2,/^set -eu/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
done

check_channel "$channel"
[ -z "$point_channel_name" ] || check_channel "$point_channel_name"

if [ "$publish" = 1 ] && [ "$skip_notarization" = 1 ]; then
  die "--skip-notarization is refused with --publish: Gatekeeper refuses a macOS artifact that is not notarized"
fi

if [ -n "$point_channel_name" ]; then
  [ -n "$version" ] || die "--point-channel needs --version"
  check_version
  check_stable_version "$point_channel_name"
  command -v gcloud >/dev/null 2>&1 || die "gcloud is required to point a channel"
  work=$(mktemp -d "${TMPDIR:-/tmp}/coder-point-channel.XXXXXX")
  gs cp "$root/$(sums_file_name)" "$work/sums" --quiet >/dev/null 2>&1 || : >"$work/sums"
  bucket_objects >"$work/objects"
  refuse_uncovered_channel "$point_channel_name" "$work/sums" "$work/objects"
  point_channel "$point_channel_name"
  rm -rf "$work"
  exit 0
fi

sha=$(git -C "$repo_root" rev-parse --verify --quiet "$commit_ref^{commit}") ||
  die "no commit named $commit_ref"
short=$(git -C "$repo_root" rev-parse --short=10 "$sha")

publish_installers_from_commit() {
  command -v gcloud >/dev/null 2>&1 || die "gcloud is required to publish"
  staged=$(mktemp -d "${TMPDIR:-/tmp}/coder-installers.XXXXXX")
  for pair in coder.sh:install.sh coder.ps1:install.ps1; do
    source_name=${pair%%:*}
    published=${pair#*:}
    git -C "$repo_root" show "$sha:scripts/install/$source_name" >"$staged/$published" 2>/dev/null ||
      die "missing scripts/install/$source_name at $short"
    echo "Publishing $published from $short"
    gs cp "$staged/$published" "$root/$published" \
      --content-type=text/plain --cache-control='public, max-age=60' --quiet
  done
  rm -rf "$staged"
  echo "Install with: curl -fsSL $public_base/install.sh | sh"
}

if [ "$publish_installers" = 1 ] && { [ -z "$version" ] || [ "$publish" = 0 ]; }; then
  publish_installers_from_commit
  exit 0
fi

[ -n "$version" ] || die "--version is required"
check_version
[ "$publish" = 0 ] || [ -z "$channel" ] || [ "$allow_partial" = 1 ] || check_stable_version "$channel"

echo "$short $(git -C "$repo_root" log -1 --format=%s "$sha")"

command -v gcloud >/dev/null 2>&1 || die "gcloud is required (to check the version is unpublished)"
if gs ls "$root/$(sums_file_name)" >/dev/null 2>&1; then
  die "refusing to build: $version is already published ($(sums_file_name) is in $root); take the next version"
fi
# The listing is a prefix match (`1.0.0-*` also lists `1.0.0-rc.1-*`), so
# each name this version would publish is matched whole.
existing=$(bucket_objects)
for platform in $all_platforms; do
  for name in $(archive_name "$platform") $(for executable in $(products_for "$platform"); do artifact_name "$executable" "$platform"; done); do
    if printf '%s\n' "$existing" | grep -qxF "$name"; then
      die "refusing to build: $root/$name already exists; take the next version"
    fi
  done
done

# The version the binaries print is their crates' own, so a --version that
# differs would publish artifacts named for one release that call themselves
# another.
for manifest in crates/coder-new/Cargo.toml crates/openagents-cli/Cargo.toml crates/microcoder/Cargo.toml; do
  crate_version=$(git -C "$repo_root" show "$sha:$manifest" 2>/dev/null |
    sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)
  [ "$crate_version" = "$version" ] ||
    die "$manifest says version ${crate_version:-(workspace)} at $short, and this is $version; commit the bundled version bump first"
done

[ -n "$targets" ] || targets=$all_platforms

for command_name in cargo file shasum git tar zip; do
  command -v "$command_name" >/dev/null 2>&1 || die "$command_name is required to build a release"
done

dist="$repo_root/dist/releases/$prefix/$version"
rm -rf "$dist"
mkdir -p "$dist"

source=$(mktemp -d "${TMPDIR:-/tmp}/coder-terminal-source.XXXXXX")
trap 'rm -rf "$source"' EXIT
echo "Extracting $short into $source"
git -C "$repo_root" archive --format=tar "$sha" | (cd "$source" && tar -xf -)
entitlements="$source/scripts/release/openagents-terminal.entitlements"

# Signing material, read once and only when a macOS platform builds.
signing_loaded=0
load_signing() {
  [ "$signing_loaded" = 0 ] || return 0
  if [ -f "$notary_env" ]; then
    set -a
    # shellcheck disable=SC1090
    . "$notary_env"
    set +a
  fi
  if [ -z "${OA_DEVELOPER_ID_APPLICATION:-}" ]; then
    OA_DEVELOPER_ID_APPLICATION=$(security find-identity -v -p codesigning 2>/dev/null |
      sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)
  fi
  [ -n "${OA_DEVELOPER_ID_APPLICATION:-}" ] ||
    die "no Developer ID Application identity: set OA_DEVELOPER_ID_APPLICATION or install the certificate"
  if [ "$skip_notarization" = 0 ] && [ -z "${NOTARY_KEYCHAIN_PROFILE:-}" ]; then
    [ -n "${ASC_API_KEY_ID:-}" ] && [ -n "${ASC_API_ISSUER_ID:-}" ] && [ -n "${ASC_API_PRIVATE_KEY_PATH:-}" ] ||
      die "notarization needs NOTARY_KEYCHAIN_PROFILE or ASC_API_KEY_ID, ASC_API_ISSUER_ID and ASC_API_PRIVATE_KEY_PATH (OPENAGENTS_NOTARY_ENV names a file)"
  fi
  signing_loaded=1
}

notarize_zip() {
  if [ -n "${NOTARY_KEYCHAIN_PROFILE:-}" ]; then
    xcrun notarytool submit "$1" --keychain-profile "$NOTARY_KEYCHAIN_PROFILE" --wait --timeout 30m
  else
    xcrun notarytool submit "$1" --key "$ASC_API_PRIVATE_KEY_PATH" \
      --key-id "$ASC_API_KEY_ID" --issuer "$ASC_API_ISSUER_ID" --wait --timeout 30m
  fi
}

# Sign each macOS artifact of a platform and notarize them in one
# submission. Sets `notary_status` and `notary_submission`.
sign_and_notarize() {
  platform=$1
  shift
  load_signing
  for artifact in "$@"; do
    case "$(basename "$artifact")" in
      "$engine"-*) identifier=$(signing_identifier "$engine") ;;
      "$companion"-*) identifier=$(signing_identifier "$companion") ;;
      *) identifier=$(signing_identifier "$product") ;;
    esac
    echo "  signing $(basename "$artifact") as $identifier"
    codesign --force --timestamp --options runtime --entitlements "$entitlements" \
      --identifier "$identifier" --sign "$OA_DEVELOPER_ID_APPLICATION" "$artifact" \
      >"$artifact.codesign.log" 2>&1 ||
      { cat "$artifact.codesign.log" >&2; die "codesign failed for $artifact"; }
    rm -f "$artifact.codesign.log"
    codesign --verify --strict "$artifact" || die "signature does not verify: $artifact"
  done
  if [ "$skip_notarization" = 1 ]; then
    notary_status=skipped
    notary_submission=''
    return 0
  fi
  submission="$dist/.notarize-$platform"
  rm -rf "$submission" "$submission.zip"
  mkdir -p "$submission"
  cp "$@" "$submission/"
  /usr/bin/ditto -c -k --keepParent "$submission" "$submission.zip"
  echo "  notarizing $platform"
  notary_log="$dist/.notary-$platform.log"
  notarize_zip "$submission.zip" >"$notary_log" 2>&1 ||
    { cat "$notary_log" >&2; die "notarization failed for $platform"; }
  notary_submission=$(awk '/^  id: /{print $2; exit}' "$notary_log")
  notary_status=$(awk '/^  status: /{print $2; exit}' "$notary_log")
  rm -rf "$submission" "$submission.zip"
  [ "$notary_status" = Accepted ] ||
    { cat "$notary_log" >&2; die "notarization for $platform came back $notary_status"; }
  echo "  notarized $platform ($notary_submission, $notary_status)"
  for artifact in "$@"; do
    notarized="$notarized $(basename "$artifact")"
  done
}

echo "Building Coder $version"
echo

built=''
missing=''
manifest_rows=''

for platform in $targets; do
  row=$(printf '%s\n' "$platform_table" | grep "^$platform|") ||
    die "unknown platform: $platform (known: $all_platforms)"
  triple=$(printf '%s' "$row" | cut -d'|' -f2)
  builder=$(printf '%s' "$row" | cut -d'|' -f3)
  expected=$(printf '%s' "$row" | cut -d'|' -f4)

  echo "$platform ($triple, $builder)"

  if ! (cd "$source" && rustup target list --installed 2>/dev/null) | grep -qx "$triple"; then
    echo "  SKIP: rust target $triple is not installed (rustup target add $triple)"
    missing="$missing $platform"
    continue
  fi
  if [ "$builder" = zigbuild ] && ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "  SKIP: cargo-zigbuild is not installed"
    missing="$missing $platform"
    continue
  fi

  case "$triple" in
    *windows*) exe=.exe ;;
    *) exe='' ;;
  esac
  zig_target=$triple
  case "$triple" in
    *-linux-gnu) zig_target="$triple.$glibc_version" ;;
  esac
  platform_target_dir=$target_dir
  case "$platform" in
    macos-x86_64) platform_target_dir=${CODER_RELEASE_MAC_INTEL_TARGET_DIR:-$target_dir} ;;
    windows-x86_64) platform_target_dir=${CODER_RELEASE_WINDOWS_TARGET_DIR:-$target_dir} ;;
    linux-aarch64-musl) platform_target_dir=${CODER_RELEASE_ARM_MUSL_TARGET_DIR:-$target_dir} ;;
  esac
  out_dir="$platform_target_dir/$triple/release"
  for name in $(products_for "$platform"); do
    rm -f "$out_dir/$(binary_for "$name")$exe"
  done

  build_log="$dist/$platform.build.log"
  if [ "$builder" = zigbuild ]; then
    set -- cargo zigbuild --target "$zig_target"
  else
    set -- cargo build --target "$triple"
  fi
  set -- "$@" --release --locked -p "$product_package" --bin "$product_bin" \
    -p "$companion_package" --bin "$companion_bin" -p "$engine_package" --bin "$engine_bin"
  case "$platform" in windows-*) set -- "$@" -p "$helper" --bin "$helper" ;; esac
  if ! (cd "$source" && CODER_BUILD_COMMIT="$sha" CODER_BUILD_DIRTY=clean \
    OPENAGENTS_RELEASE=1 \
    MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}" \
    CARGO_TARGET_DIR="$platform_target_dir" \
    "$@") >"$build_log" 2>&1; then
    echo "  SKIP: build failed (see $build_log)"
    grep -E '^error' "$build_log" | head -5 | sed 's/^/    /'
    tail -3 "$build_log" | sed 's/^/    /'
    missing="$missing $platform"
    continue
  fi

  refused=0
  staged=''
  for name in $(products_for "$platform"); do
    output="$out_dir/$(binary_for "$name")$exe"
    if [ ! -f "$output" ]; then
      echo "  REFUSED: the build reported success and produced no $output"
      refused=1
      break
    fi
    signature=$(file -b "$output")
    # shellcheck disable=SC2254
    case "$signature" in
      $expected*) ;;
      *)
        echo "  REFUSED: $output is '$signature', expected '$expected'"
        refused=1
        break
        ;;
    esac
    artifact="$dist/$(artifact_name "$name" "$platform")"
    cp "$output" "$artifact"
    chmod +x "$artifact"
    staged="$staged $artifact"
  done
  if [ "$refused" = 1 ]; then
    missing="$missing $platform"
    continue
  fi

  # Version and demo checks run without loading the operator's credentials.
  if [ "$platform" = "$native_platform" ]; then
    for name in "$product" "$companion" "$engine"; do
      said=$("$dist/$(artifact_name "$name" "$platform")" --version 2>&1) ||
        die "$(artifact_name "$name" "$platform") --version failed: $said"
      case "$said" in
        *"$version"*) echo "      says: $said" ;;
        *) die "$(artifact_name "$name" "$platform") --version does not name $version" ;;
      esac
    done
    smoke_home=$(mktemp -d "${TMPDIR:-/tmp}/coder-release-smoke.XXXXXX")
    native_coder="$dist/$(artifact_name "$product" "$platform")"
    HOME="$smoke_home" "$native_coder" --snapshot >"$dist/live-$platform.svg" ||
      die "$(artifact_name "$product" "$platform") cannot render its live view"
    grep -q '<svg' "$dist/live-$platform.svg" || die "the live view did not produce an SVG"
    HOME="$smoke_home" "$native_coder" --help >"$dist/help-$platform.txt"
    grep -q '/resume' "$dist/help-$platform.txt" || die "the release does not expose /resume"
    if grep -q -- '--demo\|/demo' "$dist/help-$platform.txt"; then
      die "the release exposes demo mode"
    fi
    if HOME="$smoke_home" "$native_coder" --demo --snapshot >"$dist/demo-$platform.txt" 2>&1; then
      die "the release accepts demo mode"
    fi
    grep -q 'only in local development builds' "$dist/demo-$platform.txt" ||
      die "the release did not explain why demo mode is unavailable"
    rm -rf "$smoke_home"
    "$dist/$(artifact_name "$engine" "$platform")" repository --help >/dev/null 2>&1 ||
      die "$(artifact_name "$engine" "$platform") does not run"
  fi

  notary_status=not-applicable
  notary_submission=''
  case "$platform" in
    macos-*)
      # shellcheck disable=SC2086
      sign_and_notarize "$platform" $staged
      ;;
  esac

  for artifact in $staged; do
    name=$(basename "$artifact")
    artifact_sha=$(shasum -a 256 "$artifact" | awk '{print $1}')
    size=$(wc -c <"$artifact" | tr -d ' ')
    echo "  ok  $name  $artifact_sha  ($size bytes)"
    manifest_rows="$manifest_rows
$name|$platform|$triple|$builder|$artifact_sha|$size|$notary_status|$notary_submission"
  done
  built="$built $platform"
  echo
done

[ -n "$built" ] || die "no platform built; nothing to publish"

if ! differs=$(source_matches "$repo_root" "$sha" "$source"); then
  printf 'the source moved while the build ran; these paths differ from %s:\n%s\n' "$short" "$differs" >&2
  die "refusing to stage artifacts no commit describes"
fi

wait_for_gatekeeper "$notarized"

# Each platform publishes one archive of its signed, checked executables.
archive_rows=''
for platform in $built; do
  make_archive "$platform"
  archive=$(archive_name "$platform")
  archive_sha=$(shasum -a 256 "$dist/$archive" | awk '{print $1}')
  size=$(wc -c <"$dist/$archive" | tr -d ' ')
  echo "  ok  $archive  $archive_sha  ($size bytes)"
  archive_rows="$archive_rows
$archive|$platform|$archive_sha|$size"
done

# The native archive must extract to executables that still run.
case " $built " in
  *" $native_platform "*)
    unpacked=$(mktemp -d "${TMPDIR:-/tmp}/coder-release-archive.XXXXXX")
    tar -xzf "$dist/$(archive_name "$native_platform")" -C "$unpacked"
    for name in $(products_for "$native_platform"); do
      said=$("$unpacked/$(installed_name "$name" "$native_platform")" --version 2>&1) ||
        die "$name from $(archive_name "$native_platform") does not run: $said"
      case "$said" in
        "$name $version"*) ;;
        *) die "$name from $(archive_name "$native_platform") says '$said', not $version" ;;
      esac
    done
    rm -rf "$unpacked"
    echo "  $(archive_name "$native_platform") extracts and runs"
    ;;
esac

entries=''
while IFS='|' read -r name platform triple builder artifact_sha size notary_status notary_submission; do
  [ -n "$name" ] || continue
  gatekeeper=$(gatekeeper_verdict_for "$name")
  if [ -z "$gatekeeper" ]; then
    case "$platform" in
      macos-*) gatekeeper=not-assessed ;;
      *) gatekeeper=not-applicable ;;
    esac
  fi
  entries="$entries
    {\"name\": \"$name\", \"platform\": \"$platform\", \"target\": \"$triple\", \"builder\": \"$builder\", \"sha256\": \"$artifact_sha\", \"bytes\": $size, \"notarization\": \"$notary_status\", \"notarization_submission\": \"$notary_submission\", \"gatekeeper\": \"$gatekeeper\"},"
done <<EOF
$manifest_rows
EOF

archive_entries=''
while IFS='|' read -r name platform archive_sha size; do
  [ -n "$name" ] || continue
  contains=''
  for executable in $(products_for "$platform"); do
    contains="$contains${contains:+, }\"$(installed_name "$executable" "$platform")\""
  done
  archive_entries="$archive_entries
    {\"name\": \"$name\", \"platform\": \"$platform\", \"sha256\": \"$archive_sha\", \"bytes\": $size, \"contains\": [$contains]},"
done <<EOF
$archive_rows
EOF

sums="$dist/$(sums_file_name)"
: >"$sums"
for platform in $all_platforms; do
  case " $built " in *" $platform "*) ;; *) continue ;; esac
  archive=$(archive_name "$platform")
  printf '%s  %s\n' "$(shasum -a 256 "$dist/$archive" | awk '{print $1}')" \
    "$(sums_name_for "$archive" "$platform")" >>"$sums"
done

cat >"$dist/release-manifest.json" <<EOF
{
  "schema": "openagents.release-manifest.v1",
  "surface": "coder-terminal",
  "product": "$product",
  "version": "$version",
  "built_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "git_sha": "$sha",
  "git_tree": "$(git -C "$repo_root" rev-parse "$sha^{tree}")",
  "source": "an archive of the commit, built apart from the checkout",
  "host": $(json_string "$(uname -sm)"),
  "toolchain": $(toolchain_json),
  "base_url": "$public_base",
  "archives": [$(printf '%s' "$archive_entries" | sed '$ s/,$//')
  ],
  "artifacts": [$(printf '%s' "$entries" | sed '$ s/,$//')
  ]
}
EOF

echo "Built:  $(printf '%s' "$built" | tr -s ' ')"
[ -z "$missing" ] || echo "Missing:$missing"
echo "Staged in $dist"
echo

if [ -n "$missing" ] && [ "$allow_partial" = 0 ]; then
  echo "This release is missing:$missing" >&2
  echo "A channel that points at a version some platforms cannot install is a broken channel." >&2
  echo "Fix the build, restrict --targets, or pass --allow-partial (the channel then stays where it is)." >&2
  exit 1
fi

if [ "$publish" = 0 ]; then
  echo "Not publishing (--publish was not passed)."
  exit 0
fi

echo "Publishing to $root"
for platform in $built; do
  archive=$(archive_name "$platform")
  case "$archive" in
    *.zip) content_type=application/zip ;;
    *) content_type=application/gzip ;;
  esac
  # `--no-clobber` keeps a published object immutable even if another
  # publisher raced this one past the check above.
  gs cp --no-clobber "$dist/$archive" "$root/$archive" \
    --content-type="$content_type" --quiet
done
gs cp --no-clobber "$sums" "$root/$(sums_file_name)" --content-type=text/plain --quiet
gs cp --no-clobber "$dist/release-manifest.json" "$root/$product-$version.release-manifest.json" \
  --content-type=application/json --quiet

# Read back what was published, through the public URL an installer uses.
readback="$dist/.readback-sums"
curl -fsSL "$public_base/$(sums_file_name)" -o "$readback" ||
  die "published, but $public_base/$(sums_file_name) does not read back"
cmp -s "$readback" "$sums" || die "published, but the public sums file differs from the staged one"
rm -f "$readback"
echo "Published $version"

[ "$allow_partial" = 0 ] || { echo "No channel updated (--allow-partial was passed)."; exit 0; }
[ -n "$channel" ] || { echo "No channel updated (--channel was empty)."; exit 0; }

objects="$dist/.objects"
bucket_objects >"$objects"
refuse_uncovered_channel "$channel" "$sums" "$objects"
rm -f "$objects"
point_channel "$channel"
if [ "$channel" = stable ]; then
  point_channel rc
fi
if [ "$publish_installers" = 1 ]; then
  publish_installers_from_commit
fi
