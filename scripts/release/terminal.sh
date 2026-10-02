#!/bin/sh
# Build, sign, and publish OpenAgents Terminal (the `openagents` command and
# the `microcoder` engine it runs Coder with) for seven platforms.
#
# The contract this script fills is the one the installers read. Under the
# base URL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents
# they fetch:
#
#   <base>/openagents.<channel>                 a bare version string
#   <base>/openagents-<version>-<platform>      the `openagents` binary
#   <base>/microcoder-<version>-<platform>      the `microcoder` engine
#   <base>/SHA256SUMS-openagents-<version>      "<sha256>  <name>" per line
#   <base>/install.sh, <base>/install.ps1       the installers
#
# The names are derived here once and never spelled twice. Everything lives
# under the bucket's `openagents/` prefix, which nothing else in the bucket
# uses: its root holds the older OpenAgents CLI line
# (`openagents-<version>-<platform>`, `SHA256SUMS-<version>`, and the bare
# `stable` and `rc` pointers), `openagents-coder-api-*`, and Coder
# Terminal's `coder-terminal-*`, and no name here can reach one of them.
#
# Two artifacts ship per platform. Coder runs a turn through `microcoder`,
# found beside the running `openagents` (`coder::task::local::controller`),
# so the installers put both in one directory. Each is a bare executable,
# not an archive, so a person can fetch one with curl and run it.
#
# A channel names the version an installer downloads when nobody names one,
# so that version has to install on every platform. This script moves a
# channel only after reading the bucket back and finding both artifacts and
# both checksum entries for every platform at that version. A run that built
# some platforms can publish them with --allow-partial, and the channel stays
# where it was.
#
# Version grammar:
#   stable  X.Y.Z
#   RC      X.Y.Z-rc.N     N is a decimal integer with no leading zeros
# `1.0.0-rc1` and `1.0.0-rc.01` are refused. A published version is
# immutable: the script refuses to build a version whose checksum file the
# bucket holds, and never replaces an object. A new build takes the next
# rc.N.
#
# The version is the `version` of `crates/openagents-cli`,
# `crates/openagents-terminal`, and `crates/microcoder`, which
# `openagents --version`, the welcome card, and `microcoder --version` print.
# It moves apart from the workspace version, and the script refuses a
# --version that differs from any of those crates' at the commit, so a release
# is cut by committing the bump first.
#
# Every artifact is built from an archive of the commit, extracted into a
# directory of its own, and never from the checkout, so an edit that lands
# while a platform builds does not reach the artifact. After the builds the
# extracted tree is checked against the commit's tree. The manifest
# `dist/releases/openagents/<version>/release-manifest.json` (published
# beside the artifacts as `openagents-<version>.release-manifest.json`) names
# the commit, its tree, each artifact's digest, the toolchain, and each
# macOS artifact's notarization and Gatekeeper verdict.
#
# Each built file is read back with `file` and matched against the platform's
# signature before it is staged: a cross build that quietly produced a host
# binary, or left a stale one at the output path, is refused rather than
# published under the wrong name.
#
# macOS artifacts are signed with the Developer ID Application identity under
# the hardened runtime (`scripts/release/openagents-terminal.entitlements`),
# notarized with `notarytool`, and published only after Gatekeeper accepts
# them. Apple cannot staple a ticket to a bare Mach-O, so Gatekeeper reads
# the ticket from Apple's ticket store, which can serve it minutes after
# `notarytool` answers `Accepted`. The script assesses each notarized artifact
# `OPENAGENTS_RELEASE_ASSESS_DELAY` seconds apart (60 by default), at most
# `OPENAGENTS_RELEASE_ASSESS_ATTEMPTS` times (45 by default), and refuses the
# release when a verdict is still not `accepted`.
#
# Usage:
#   scripts/release/terminal.sh --version 1.0.0-rc.1
#   scripts/release/terminal.sh --version 1.0.0-rc.1 --publish --channel rc
#   scripts/release/terminal.sh --version 1.0.0 --publish --channel stable
#   scripts/release/terminal.sh --version 1.0.0 --point-channel stable
#   scripts/release/terminal.sh --publish-installers
#
# Options:
#   --version X.Y.Z or X.Y.Z-rc.N
#                         Required, except with --publish-installers.
#   --commit REF          The commit to build. Defaults to HEAD.
#   --targets "a b c"     Platforms to attempt. Defaults to all seven.
#   --publish             Upload to the release bucket. Off by default.
#   --channel NAME        Point a channel at this version after publishing.
#                         Moves only when the bucket covers every platform.
#   --allow-partial       Publish although some platforms are missing. The
#                         channel stays where it is.
#   --skip-notarization   Sign macOS artifacts without notarizing them.
#                         Refused with --publish.
#   --point-channel NAME  Point a channel at a version the bucket already
#                         covers on every platform. Builds nothing.
#   --publish-installers  Upload scripts/install/openagents.sh and .ps1 from
#                         the commit as install.sh and install.ps1.
#
# Environment:
#   OPENAGENTS_RELEASES_BUCKET  Default openagentsgemini-cli-releases.
#   CLOUDSDK_CONFIG             The gcloud configuration that can write it.
#   OPENAGENTS_NOTARY_ENV       A file holding ASC_API_KEY_ID,
#                               ASC_API_ISSUER_ID, ASC_API_PRIVATE_KEY_PATH,
#                               and OA_DEVELOPER_ID_APPLICATION (the same file
#                               scripts/desktop/package-macos.sh reads with
#                               --notary-env). NOTARY_KEYCHAIN_PROFILE, a
#                               saved notarytool profile, is used instead
#                               when set.
#   OA_DEVELOPER_ID_APPLICATION The signing identity. Without one, the
#                               keychain's Developer ID Application identity.
#   CARGO_TARGET_DIR            Where the builds land (cross targets under it).
#
# docs/release/terminal.md is the runbook.

set -eu

# A test that sources this file for its functions names it in
# OPENAGENTS_RELEASE_SCRIPT, because `$0` is then the test.
script_dir=$(CDPATH='' cd -- "$(dirname -- "${OPENAGENTS_RELEASE_SCRIPT:-$0}")" && pwd)
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
glibc_version=${OPENAGENTS_RELEASE_GLIBC:-2.28}

case "$(uname -sm)" in
  'Darwin arm64') native_platform=macos-aarch64 ;;
  'Darwin x86_64') native_platform=macos-x86_64 ;;
  'Linux aarch64') native_platform=linux-aarch64 ;;
  'Linux x86_64') native_platform=linux-x86_64 ;;
  *) native_platform='' ;;
esac

# The bucket prefix every name below lives under.
prefix=openagents
product=openagents
engine=microcoder
# The cargo package and binary behind each published name.
product_package=openagents-cli
product_bin=openagents
engine_package=microcoder
engine_bin=microcoder

version=''
commit_ref=HEAD
targets=''
publish=0
channel=''
allow_partial=0
skip_notarization=0
point_channel_name=''
publish_installers=0

assess_attempts=${OPENAGENTS_RELEASE_ASSESS_ATTEMPTS:-45}
assess_delay=${OPENAGENTS_RELEASE_ASSESS_DELAY:-60}
notarized=''
gatekeeper_verdicts=''

bucket=${OPENAGENTS_RELEASES_BUCKET:-openagentsgemini-cli-releases}
default_gcloud_config="$HOME/work/.secrets/gcloud-sa-config"
if [ -z "${CLOUDSDK_CONFIG:-}" ] && [ -d "$default_gcloud_config" ]; then
  CLOUDSDK_CONFIG=$default_gcloud_config
fi
[ -z "${CLOUDSDK_CONFIG:-}" ] || export CLOUDSDK_CONFIG
notary_env=${OPENAGENTS_NOTARY_ENV:-$HOME/work/.secrets/appstoreconnect.env}
public_base="https://storage.googleapis.com/$bucket/$prefix"
root="gs://$bucket/$prefix"

entitlements="$script_dir/openagents-terminal.entitlements"

target_dir=${CARGO_TARGET_DIR:-"$repo_root/target"}

die() {
  echo "$@" >&2
  exit 1
}

# The published name of an artifact: `$1` is the name (`$product` or
# `$engine`), `$2` the platform.
artifact_name() {
  echo "$1-$version-$2"
}

sums_file_name() {
  echo "SHA256SUMS-$product-$version"
}

channel_name() {
  echo "$product.$1"
}

# The sums file names a Windows artifact with `.exe` although its URL
# carries none; the installer looks it up under that name.
sums_name_for() {
  case "$2" in
    windows-*) echo "$1.exe" ;;
    *) echo "$1" ;;
  esac
}

# The signing identifier per binary, pinned rather than left to the file
# name, which differs per platform.
signing_identifier() {
  case "$1" in
    "$product") echo com.openagents.terminal.openagents ;;
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
# means the bucket holds both artifacts and the sums file names both. `$1` is
# the sums file, `$2` a file of the object names the bucket holds.
uncovered_platforms() {
  _gap=''
  for _platform in $all_platforms; do
    for _name in "$product" "$engine"; do
      _artifact=$(artifact_name "$_name" "$_platform")
      _sha=''
      [ -f "$1" ] && _sha=$(sums_entry "$1" "$(sums_name_for "$_artifact" "$_platform")")
      if [ -z "$_sha" ] || ! grep -qxF "$_artifact" "$2" 2>/dev/null; then
        _gap="$_gap $_platform"
        break
      fi
    done
  done
  printf '%s' "$_gap"
}

# The object names the bucket holds for this version, one per line. The
# listing is a prefix match; `uncovered_platforms` matches whole names.
bucket_objects() {
  {
    gs ls "$root/$product-$version-*" 2>/dev/null || :
    gs ls "$root/$engine-$version-*" 2>/dev/null || :
  } | sed 's|.*/||'
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
      die "Refusing to publish: Gatekeeper still refuses$pending after $round assessments. Raise OPENAGENTS_RELEASE_ASSESS_ATTEMPTS and build $version again; nothing was published."
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
  _index=$(mktemp "${TMPDIR:-/tmp}/openagents-release-index.XXXXXX")
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

# `scripts/test-release-terminal.sh` sources the functions above and stops
# here.
if [ "${OPENAGENTS_RELEASE_LIBRARY:-}" = 1 ]; then
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
    -h | --help) sed -n '2,104p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
done

if [ "$publish" = 1 ] && [ "$skip_notarization" = 1 ]; then
  die "--skip-notarization is refused with --publish: Gatekeeper refuses a macOS artifact that is not notarized"
fi

if [ -n "$point_channel_name" ]; then
  [ -n "$version" ] || die "--point-channel needs --version"
  check_version
  command -v gcloud >/dev/null 2>&1 || die "gcloud is required to point a channel"
  work=$(mktemp -d "${TMPDIR:-/tmp}/openagents-point-channel.XXXXXX")
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

if [ "$publish_installers" = 1 ]; then
  command -v gcloud >/dev/null 2>&1 || die "gcloud is required to publish"
  staged=$(mktemp -d "${TMPDIR:-/tmp}/openagents-installers.XXXXXX")
  for pair in openagents.sh:install.sh openagents.ps1:install.ps1; do
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
  exit 0
fi

[ -n "$version" ] || die "--version is required"
check_version

echo "$short $(git -C "$repo_root" log -1 --format=%s "$sha")"

command -v gcloud >/dev/null 2>&1 || die "gcloud is required (to check the version is unpublished)"
if gs ls "$root/$(sums_file_name)" >/dev/null 2>&1; then
  die "refusing to build: $version is already published ($(sums_file_name) is in $root); take the next version"
fi
# The listing is a prefix match (`1.0.0-*` also lists `1.0.0-rc.1-*`), so
# each name this version would publish is matched whole.
existing=$(bucket_objects)
for platform in $all_platforms; do
  for name in "$product" "$engine"; do
    if printf '%s\n' "$existing" | grep -qxF "$(artifact_name "$name" "$platform")"; then
      die "refusing to build: $root/$(artifact_name "$name" "$platform") already exists; take the next version"
    fi
  done
done

# The version the binaries print is their crates' own, so a --version that
# differs would publish artifacts named for one release that call themselves
# another.
for manifest in crates/openagents-cli/Cargo.toml crates/openagents-terminal/Cargo.toml crates/microcoder/Cargo.toml; do
  crate_version=$(git -C "$repo_root" show "$sha:$manifest" 2>/dev/null |
    sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)
  [ "$crate_version" = "$version" ] ||
    die "$manifest says version ${crate_version:-(workspace)} at $short, and this is $version; commit the bump first (docs/release/terminal.md)"
done

[ -n "$targets" ] || targets=$all_platforms

for command_name in cargo file shasum git; do
  command -v "$command_name" >/dev/null 2>&1 || die "$command_name is required to build a release"
done

dist="$repo_root/dist/releases/$prefix/$version"
rm -rf "$dist"
mkdir -p "$dist"

source=$(mktemp -d "${TMPDIR:-/tmp}/openagents-terminal-source.XXXXXX")
trap 'rm -rf "$source"' EXIT
echo "Extracting $short into $source"
git -C "$repo_root" archive --format=tar "$sha" | (cd "$source" && tar -xf -)

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

# Sign both macOS artifacts of a platform and notarize them in one
# submission. Sets `notary_status` and `notary_submission`.
sign_and_notarize() {
  platform=$1
  shift
  load_signing
  for artifact in "$@"; do
    case "$(basename "$artifact")" in
      "$engine"-*) identifier=$(signing_identifier "$engine") ;;
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

echo "Building OpenAgents Terminal $version"
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
  out_dir="$target_dir/$triple/release"
  rm -f "$out_dir/$product_bin$exe" "$out_dir/$engine_bin$exe"

  build_log="$dist/$platform.build.log"
  if [ "$builder" = zigbuild ]; then
    set -- cargo zigbuild --target "$zig_target"
  else
    set -- cargo build --target "$triple"
  fi
  if ! (cd "$source" && CODER_BUILD_COMMIT="$sha" CODER_BUILD_DIRTY=clean \
    MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}" \
    CARGO_TARGET_DIR="$target_dir" \
    "$@" --release --locked -p "$product_package" --bin "$product_bin" \
    -p "$engine_package" --bin "$engine_bin") >"$build_log" 2>&1; then
    echo "  SKIP: build failed (see $build_log)"
    grep -E '^error' "$build_log" | head -5 | sed 's/^/    /'
    tail -3 "$build_log" | sed 's/^/    /'
    missing="$missing $platform"
    continue
  fi

  refused=0
  staged=''
  for name in "$product" "$engine"; do
    case "$name" in
      "$product") output="$out_dir/$product_bin$exe" ;;
      *) output="$out_dir/$engine_bin$exe" ;;
    esac
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

  # The artifacts this machine can run are asked who they are.
  if [ "$platform" = "$native_platform" ]; then
    said=$("$dist/$(artifact_name "$product" "$platform")" --version 2>&1) ||
      die "$(artifact_name "$product" "$platform") --version failed: $said"
    case "$said" in
      *"$version"*) echo "      says: $said" ;;
      *) die "$(artifact_name "$product" "$platform") --version says '$said', which does not name $version" ;;
    esac
    "$dist/$(artifact_name "$engine" "$platform")" repository --help >/dev/null 2>&1 ||
      die "$(artifact_name "$engine" "$platform") does not run"
    echo "      the engine runs"
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

sums="$dist/$(sums_file_name)"
: >"$sums"
for platform in $all_platforms; do
  case " $built " in *" $platform "*) ;; *) continue ;; esac
  for name in "$product" "$engine"; do
    artifact=$(artifact_name "$name" "$platform")
    printf '%s  %s\n' "$(shasum -a 256 "$dist/$artifact" | awk '{print $1}')" \
      "$(sums_name_for "$artifact" "$platform")" >>"$sums"
  done
done

cat >"$dist/release-manifest.json" <<EOF
{
  "schema": "openagents.release-manifest.v1",
  "surface": "terminal",
  "product": "$product",
  "version": "$version",
  "built_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "git_sha": "$sha",
  "git_tree": "$(git -C "$repo_root" rev-parse "$sha^{tree}")",
  "source": "an archive of the commit, built apart from the checkout",
  "host": $(json_string "$(uname -sm)"),
  "toolchain": $(toolchain_json),
  "base_url": "$public_base",
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
  for name in "$product" "$engine"; do
    artifact=$(artifact_name "$name" "$platform")
    # `--no-clobber` keeps a published object immutable even if another
    # publisher raced this one past the check above.
    gs cp --no-clobber "$dist/$artifact" "$root/$artifact" \
      --content-type=application/octet-stream --quiet
  done
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

[ -n "$channel" ] || { echo "No channel updated (--channel was not passed)."; exit 0; }

objects="$dist/.objects"
bucket_objects >"$objects"
refuse_uncovered_channel "$channel" "$sums" "$objects"
rm -f "$objects"
point_channel "$channel"
