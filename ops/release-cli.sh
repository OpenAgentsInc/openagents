#!/bin/sh
# Build, sign, and publish the OpenAgents CLI for every platform the installer
# knows how to ask for.
#
# The contract this script fills is written down in the installer served at
# https://openagents.com/install.sh. Under the base URL
# https://openagents.com/releases it fetches:
#
#   <base>/<channel>                              a bare version string
#   <base>/openagents-<version>-<platform>        the CLI, no extension
#   <base>/openagents-coder-api-<version>-<platform>
#                                                 the local inference door
#   <base>/SHA256SUMS-<version>                   "<sha256>  <name>" per line
#
# Anything this script emits that disagrees with those four shapes is a broken
# release, so the names are derived here once and never spelled twice.
#
# Producer version grammar (stricter than the installer consumer grammar):
#   stable  X.Y.Z
#   RC      X.Y.Z-rc.N     N is a decimal integer with no leading zeros
# `0.2.0-rc8`, `0.2.0-rc8.12`, `rc.8`, and `rc8` are refused. A published
# <version, platform> pair is immutable; a new build of the same line takes
# the next rc.N.
#
# Usage:
#   ops/release-cli.sh --version 0.1.0-rc.1
#   ops/release-cli.sh --version 0.1.0-rc.1 --publish
#   ops/release-cli.sh --version 0.1.0 --publish --channel stable
#
# Options:
#   --version X.Y.Z or X.Y.Z-rc.N
#                             Required. The version to build and name.
#   --bench                   Build-only for the Gym arena (bench/run-suite.sh).
#                             Accepts the X.Y.Z-bench.<stamp> name the runner
#                             mints and skips publish paths entirely; a bench
#                             artifact is never a release.
#   --targets "a b c"         Platforms to attempt. Defaults to all seven.
#   --publish                 Upload to the release bucket. Off by default.
#                             Runs the Cargo completion gate first unless
#                             OPENAGENTS_CLI_RELEASE_GATE=passed.
#   --channel NAME            Point a channel at this version after publishing.
#   --allow-partial           Publish even though some platforms are missing.
#                             Use this for an Apple-aarch64-only RC.
#   --allow-prerelease-channel  Let a prerelease version claim a channel.
#   --skip-notarization       Build macOS artifacts without Apple notarization.
#   --skip-tests              Skip the Cargo completion gate. Refused with
#                             --publish.

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

# platform | rust target triple | builder | expected `file` signature
#
# The fourth column is the anti-forgery check. Cross-compilation fails in ways
# that still leave a runnable file sitting at the output path -- a stale
# artifact from a previous target, or a host build that silently ignored the
# requested triple -- and a release that ships a darwin binary under a
# linux-x86_64 name is worse than one that admits it built four of five. Every
# artifact is read back and matched against this signature before it is staged.
#
# The two Linux libc flavors are separate platforms because they are separate
# artifacts: the gnu build is dynamically linked and names a glibc loader that
# a musl system does not have, and the musl build is statically linked and
# names no interpreter at all. Their signatures say so -- `dynamically linked`
# against `static-pie linked` -- which is what keeps the pair from being
# published under each other's names.
platform_table='macos-aarch64|aarch64-apple-darwin|cargo|Mach-O 64-bit executable arm64
macos-x86_64|x86_64-apple-darwin|cargo|Mach-O 64-bit executable x86_64
linux-x86_64|x86_64-unknown-linux-gnu|zigbuild|ELF 64-bit LSB*x86-64*dynamically linked
linux-x86_64-musl|x86_64-unknown-linux-musl|zigbuild|ELF 64-bit LSB*x86-64*static
linux-aarch64|aarch64-unknown-linux-gnu|zigbuild|ELF 64-bit LSB*ARM aarch64*dynamically linked
linux-aarch64-musl|aarch64-unknown-linux-musl|zigbuild|ELF 64-bit LSB*ARM aarch64*static
windows-x86_64|x86_64-pc-windows-gnu|zigbuild|PE32+ executable*x86-64'

all_platforms=$(printf '%s\n' "$platform_table" | cut -d'|' -f1 | tr '\n' ' ')

version=''
targets=''
publish=0
channel=''
allow_partial=0
allow_prerelease_channel=0
skip_notarization=0
skip_tests=0

bucket=${OPENAGENTS_RELEASES_BUCKET:-openagentsgemini-cli-releases}
gcloud_config=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
notary_env=${OPENAGENTS_NOTARY_ENV:-/Users/christopherdavid/work/.secrets/appstoreconnect.env}

# The code signing identifier is pinned rather than derived from the file name.
# codesign defaults the identifier to the basename, which would give the same
# build a different identity depending on where it was staged.
signing_identifier='com.openagents.cli'
api_signing_identifier='com.openagents.coder-api'

# Honour an operator-set target dir so a shared cache can sit outside a
# disposable worktree. Cargo already reads CARGO_TARGET_DIR; the paths below
# must agree with it.
target_dir=${CARGO_TARGET_DIR:-"$repo_root/target"}

staged_name() {
  echo "$1-$version-$2"
}

sums_name_for() {
  case "$2" in
    windows-*) echo "$1.exe" ;;
    *) echo "$1" ;;
  esac
}

die() {
  echo "$@" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) version=${2:-}; shift 2 ;;
    --targets) targets=${2:-}; shift 2 ;;
    --channel) channel=${2:-}; shift 2 ;;
    --publish) publish=1; shift ;;
    --allow-partial) allow_partial=1; shift ;;
    --allow-prerelease-channel) allow_prerelease_channel=1; shift ;;
    --skip-notarization) skip_notarization=1; shift ;;
    --skip-tests) skip_tests=1; shift ;;
    --bench) bench=1; skip_tests=1; skip_notarization=1; shift ;;
    -h | --help) sed -n '2,44p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
done

[ -n "$version" ] || die "--version is required"

# Producer grammar. Stable is X.Y.Z. An RC is X.Y.Z-rc.N only: the hyphen,
# the literal `rc`, a dot, and a decimal integer with no leading zeros.
# The installer still accepts a broader suffix so already-published names
# such as 0.2.0-rc7 remain fetchable; this script will not mint another.
# A bench build names the arena artifact instead (X.Y.Z-bench.<stamp>):
# working-tree provenance in the name, and never a release.
if [ "${bench:-0}" = 1 ]; then
  printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+-bench\.[0-9]+$' ||
    die "invalid bench version: $version (expected X.Y.Z-bench.<stamp>)"
  [ "$publish" = 0 ] || die "--bench is refused with --publish; a bench artifact is not a release"
else
  printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-rc\.(0|[1-9][0-9]*))?$' ||
    die "invalid version: $version (expected X.Y.Z or X.Y.Z-rc.N)"
fi

case "$version" in
  *-*) prerelease=1 ;;
  *) prerelease=0 ;;
esac

if [ "$publish" = 1 ] && [ "$skip_tests" = 1 ]; then
  die "--skip-tests is refused with --publish; run the Cargo completion gate, or set OPENAGENTS_CLI_RELEASE_GATE=passed after it has passed"
fi

# Source, lockfile, and changelog must already name this version. A binary
# published as 0.2.0-rc.10 whose crate still says 0.2.0-rc7 is a release
# nobody can rebuild from the tree that claims to have produced it. These
# checks run before any build so a bad name never writes artifacts.
crate_manifest="$repo_root/crates/openagents-cli/Cargo.toml"
[ -f "$crate_manifest" ] || die "missing $crate_manifest"
crate_version=$(awk '
  /^\[package\]/ { in_pkg = 1; next }
  /^\[/ { in_pkg = 0 }
  in_pkg && $1 == "version" && $2 == "=" {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' "$crate_manifest")
[ -n "$crate_version" ] || die "could not read version from $crate_manifest"
# A bench build names the working tree, not a release: the version is
# provenance in the artifact name, and the digest the adapter records is the
# real identity. Skip the source-agreement and changelog gates that only
# make sense for something a person could install.
if [ "${bench:-0}" != 1 ]; then
  [ "$crate_version" = "$version" ] ||
    die "crates/openagents-cli/Cargo.toml version is $crate_version, not $version"
fi

lockfile="$repo_root/Cargo.lock"
[ -f "$lockfile" ] || die "missing $lockfile"
lock_version=$(awk '
  $0 == "name = \"openagents-cli\"" {
    getline
    if ($1 == "version" && $2 == "=") {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  }
' "$lockfile")
[ -n "$lock_version" ] || die "could not read openagents-cli version from $lockfile"
if [ "${bench:-0}" != 1 ]; then
  [ "$lock_version" = "$version" ] ||
    die "Cargo.lock package openagents-cli version is $lock_version, not $version"
fi

changelog="$repo_root/docs/changelog/UNRELEASED.md"
if [ -f "$changelog" ] && [ "${bench:-0}" != 1 ]; then
  if ! grep -oE '[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9._]+)?' "$changelog" | grep -qx "$version"; then
    die "docs/changelog/UNRELEASED.md does not name release $version"
  fi
fi

[ -n "$targets" ] || targets=$all_platforms

for command_name in cargo file shasum; do
  command -v "$command_name" >/dev/null 2>&1 ||
    die "$command_name is required to build a release"
done

# A publish is a release of this tree. The operator must have run
# `cargo fmt --all -- --check` then `cargo test --workspace`. The script
# re-runs that pair unless OPENAGENTS_CLI_RELEASE_GATE=passed records that
# this session already did. --skip-tests cannot waive it on --publish.
if [ "$publish" = 1 ]; then
  if [ "${OPENAGENTS_CLI_RELEASE_GATE:-}" = "passed" ]; then
    echo "Cargo completion gate: OPENAGENTS_CLI_RELEASE_GATE=passed"
  else
    echo "Running Cargo completion gate before publish"
    (cd "$repo_root" && cargo fmt --all -- --check) ||
      die "cargo fmt --all -- --check failed; refuse publish"
    (cd "$repo_root" && cargo test --workspace) ||
      die "cargo test --workspace failed; refuse publish"
  fi

  echo "Checking release source parity"
  "$repo_root/scripts/release-source-parity.sh"
fi

dist="$repo_root/dist/releases/$version"
rm -rf "$dist"
mkdir -p "$dist"

built=''
missing=''
manifest_entries=''

notary_loaded=0
load_notary_env() {
  [ "$notary_loaded" = 0 ] || return 0
  [ -f "$notary_env" ] || return 1
  # The App Store Connect key id, issuer, and private key path are read from the
  # operator's secret file at call time and passed to notarytool per invocation.
  # `notarytool store-credentials` would copy the same private key into the
  # login keychain, leaving a second durable copy of a credential that already
  # exists on disk; passing it per call leaves nothing new behind.
  set -a
  # shellcheck disable=SC1090
  . "$notary_env"
  set +a
  notary_loaded=1
  return 0
}

# Sign, notarize, and verify one macOS artifact.
#
# Apple cannot staple a notarization ticket to a bare Mach-O executable -- only
# a container such as a .zip, .dmg, or .pkg carries the stapled ticket -- and
# `xcrun stapler staple` on a bare binary fails with error 73. The installer
# downloads a single executable and marks it executable, so a stapled container
# would mean changing a landed contract to buy something the install path does
# not use: curl sets no com.apple.quarantine attribute, so Gatekeeper is not
# consulted on a piped install at all. The artifact therefore ships bare, and
# `spctl --assess -t install` confirms Apple's notary service recognizes it
# online as "Notarized Developer ID" for the paths where quarantine does apply.
sign_and_notarize() {
  artifact=$1
  platform=$2
  identifier=$3
  entitlements=${4:-}

  command -v codesign >/dev/null 2>&1 || die "codesign is required for $platform"

  load_notary_env ||
    die "missing $notary_env; macOS artifacts must be signed. Pass --skip-notarization to build unsigned."

  [ -n "${OA_DEVELOPER_ID_APPLICATION:-}" ] ||
    die "OA_DEVELOPER_ID_APPLICATION is not set in $notary_env"

  echo "  signing $(basename "$artifact") as $identifier"
  # Hardened runtime (`--options runtime`) is required by notarization. The
  # CLI embeds a wasm JIT: wasmtime mmaps executable pages for Cranelift at
  # every plugin invoke (`oa plugin run`, the coder capability search, the
  # /resume foreign-session scanner). Under hardened runtime without the JIT
  # entitlements, the kernel kills those pages on first execute with
  # `EXC_BAD_ACCESS / SIGKILL (Code Signature Invalid)` — termination
  # CODESIGNING "Invalid Page" — which took down every 0.2.0-rc binary the
  # moment a capability ran (issues #329, #330). These entitlements are what
  # notarized JIT apps ship (Electron, Firefox, wasmtime's own release docs).
  # openagents-coder-api does not JIT, so it signs with hardened runtime only.
  codesign_log="$artifact.codesign.log"
  if [ -n "$entitlements" ]; then
    [ -f "$entitlements" ] || die "missing $entitlements; hardened runtime needs the JIT entitlements"
    codesign --force --timestamp --options runtime \
      --entitlements "$entitlements" \
      --identifier "$identifier" \
      --sign "$OA_DEVELOPER_ID_APPLICATION" \
      "$artifact" >"$codesign_log" 2>&1 ||
      { cat "$codesign_log" >&2; die "codesign failed for $artifact"; }
  else
    codesign --force --timestamp --options runtime \
      --identifier "$identifier" \
      --sign "$OA_DEVELOPER_ID_APPLICATION" \
      "$artifact" >"$codesign_log" 2>&1 ||
      { cat "$codesign_log" >&2; die "codesign failed for $artifact"; }
  fi

  codesign --verify --strict "$artifact" ||
    die "signature does not verify for $platform"

  if [ "$skip_notarization" = 1 ]; then
    echo "  skipping notarization for $platform (--skip-notarization)"
    notary_status='skipped'
    notary_submission=''
    return 0
  fi

  command -v xcrun >/dev/null 2>&1 || die "xcrun is required to notarize $platform"

  submission_zip="$artifact.notarize.zip"
  rm -f "$submission_zip"
  /usr/bin/ditto -c -k --keepParent "$artifact" "$submission_zip"

  echo "  notarizing $platform"
  notary_log="$artifact.notary.log"
  xcrun notarytool submit "$submission_zip" \
    --key "$ASC_API_PRIVATE_KEY_PATH" \
    --key-id "$ASC_API_KEY_ID" \
    --issuer "$ASC_API_ISSUER_ID" \
    --wait --timeout 30m >"$notary_log" 2>&1 ||
    { cat "$notary_log" >&2; die "notarization failed for $platform"; }

  notary_submission=$(awk '/^  id: /{print $2; exit}' "$notary_log")
  notary_status=$(awk '/^  status: /{print $2; exit}' "$notary_log")
  rm -f "$submission_zip"

  [ "$notary_status" = "Accepted" ] ||
    { cat "$notary_log" >&2; die "notarization for $platform came back $notary_status"; }

  echo "  notarized $platform ($notary_submission, $notary_status)"

  # Evidence, not ceremony: this is the assessment an operator would run by hand
  # to answer "will Gatekeeper accept this".
  spctl --assess -vv -t install "$artifact" 2>&1 | sed 's/^/    /'
}

echo "Building OpenAgents CLI $version"
echo

for platform in $targets; do
  row=$(printf '%s\n' "$platform_table" | grep "^$platform|") ||
    die "unknown platform: $platform (known: $all_platforms)"

  triple=$(printf '%s' "$row" | cut -d'|' -f2)
  builder=$(printf '%s' "$row" | cut -d'|' -f3)
  expected=$(printf '%s' "$row" | cut -d'|' -f4)

  echo "$platform ($triple, $builder)"

  if ! rustup target list --installed 2>/dev/null | grep -qx "$triple"; then
    echo "  SKIP: rust target $triple is not installed (rustup target add $triple)"
    missing="$missing $platform"
    continue
  fi

  if [ "$builder" = zigbuild ] && ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "  SKIP: cargo-zigbuild is not installed and $triple cannot be built natively here"
    missing="$missing $platform"
    continue
  fi

  # Every artifact is rebuilt from source into its own target directory. Nothing
  # is copied forward from a previous run, so a build that fails cannot leave a
  # stale binary behind for the verification step to bless.
  # The shipped pair is the OpenAgents CLI and the local inference door it
  # starts under `coder --dev`. Both use the same target triple. A platform
  # that produces only one of them is not a platform this release covers.
  case "$triple" in
    *windows*)
      cli_output="$target_dir/$triple/release/openagents.exe"
      api_output="$target_dir/$triple/release/openagents-coder-api.exe"
      ;;
    *)
      cli_output="$target_dir/$triple/release/openagents"
      api_output="$target_dir/$triple/release/openagents-coder-api"
      ;;
  esac
  rm -f "$cli_output" "$api_output"

  build_log="$dist/$platform.build.log"
  if [ "$builder" = zigbuild ]; then
    build_command='cargo zigbuild'
  else
    build_command='cargo build'
  fi

  # The release version is threaded into the build rather than left to the
  # crate manifest. `oa --version` is what `oa update` compares against the
  # channel pointer, so a binary published as 0.1.0-rc.2 that reports 0.1.0
  # would make every update either a no-op or a reinstall depending on which
  # way the comparison fell. `build.rs` declares the dependency on this
  # variable, so changing it rebuilds.
  if ! (cd "$repo_root" && OPENAGENTS_CLI_RELEASE_VERSION="$version" \
    $build_command --release -p openagents-cli -p openagents-coder-api --target "$triple") \
    >"$build_log" 2>&1; then
    echo "  SKIP: build failed (see $build_log)"
    tail -5 "$build_log" | sed 's/^/    /'
    missing="$missing $platform"
    continue
  fi

  [ -f "$cli_output" ] || { echo "  SKIP: build reported success but produced no CLI binary"; missing="$missing $platform"; continue; }
  [ -f "$api_output" ] || { echo "  SKIP: build reported success but produced no openagents-coder-api binary"; missing="$missing $platform"; continue; }

  # The forgery check. `file` reads the actual Mach-O/ELF/PE header rather than
  # trusting the path the compiler was asked to write to. Both binaries of a
  # platform must match that platform.
  signature=$(file -b "$cli_output")
  api_signature=$(file -b "$api_output")
  refused=0
  # shellcheck disable=SC2254
  case "$signature" in
    $expected*) ;;
    *)
      echo "  REFUSED: $cli_output is '$signature', expected '$expected'"
      refused=1
      ;;
  esac
  # shellcheck disable=SC2254
  case "$api_signature" in
    $expected*) ;;
    *)
      echo "  REFUSED: $api_output is '$api_signature', expected '$expected'"
      refused=1
      ;;
  esac
  if [ "$refused" = 1 ]; then
    echo "  Refusing to publish a binary under a platform name it does not match."
    missing="$missing $platform"
    continue
  fi

  # The installer never appends an extension to the artifact URL, on any
  # platform: it computes artifact_base before it branches on windows and never
  # revisits it. The staged file name matches that URL exactly.
  cli_artifact="$dist/$(staged_name openagents "$platform")"
  api_artifact="$dist/$(staged_name openagents-coder-api "$platform")"
  cp "$cli_output" "$cli_artifact"
  cp "$api_output" "$api_artifact"
  chmod +x "$cli_artifact" "$api_artifact"

  notary_status='not-applicable'
  notary_submission=''
  api_notary_status='not-applicable'
  api_notary_submission=''
  case "$platform" in
    macos-*)
      sign_and_notarize "$cli_artifact" "$platform" "$signing_identifier" \
        "$repo_root/ops/macos-entitlements.plist"
      cli_notary_status=$notary_status
      cli_notary_submission=$notary_submission
      sign_and_notarize "$api_artifact" "$platform" "$api_signing_identifier"
      api_notary_status=$notary_status
      api_notary_submission=$notary_submission
      notary_status=$cli_notary_status
      notary_submission=$cli_notary_submission
      ;;
  esac

  sha=$(shasum -a 256 "$cli_artifact" | awk '{print $1}')
  size=$(wc -c <"$cli_artifact" | tr -d ' ')
  api_sha=$(shasum -a 256 "$api_artifact" | awk '{print $1}')
  api_size=$(wc -c <"$api_artifact" | tr -d ' ')
  echo "  ok  CLI $signature"
  echo "      sha256 $sha  ($size bytes)"
  echo "  ok  coder-api $api_signature"
  echo "      sha256 $api_sha  ($api_size bytes)"

  built="$built $platform"
  manifest_entries="$manifest_entries
    {\"platform\": \"$platform\", \"target\": \"$triple\", \"builder\": \"$builder\", \"sha256\": \"$sha\", \"bytes\": $size, \"notarization\": \"$notary_status\", \"notarization_submission\": \"$notary_submission\", \"coder_api_sha256\": \"$api_sha\", \"coder_api_bytes\": $api_size, \"coder_api_notarization\": \"$api_notary_status\", \"coder_api_notarization_submission\": \"$api_notary_submission\"},"
  echo
done

[ -n "$built" ] || die "no platform built; nothing to publish"

# The sums file the installer reads. Its lookup is
#   awk '$2 == name || $2 == "*" name'
# over "<sha256>  <name>", and for windows it appends ".exe" to the name it
# looks up even though it downloaded a URL without one. That asymmetry lives in
# a landed installer, so the sums file reproduces it exactly: the artifact keeps
# its extensionless name and the sums entry carries the .exe the installer will
# search for. Diverging here would checksum-fail every Windows install.
# Each platform contributes two names: the CLI and openagents-coder-api.
sums="$dist/SHA256SUMS-$version"
: >"$sums"
for platform in $built; do
  for kind in openagents openagents-coder-api; do
    artifact=$(staged_name "$kind" "$platform")
    sums_name=$(sums_name_for "$artifact" "$platform")
    sha=$(shasum -a 256 "$dist/$artifact" | awk '{print $1}')
    printf '%s  %s\n' "$sha" "$sums_name" >>"$sums"
  done
done

# The commit alone is not a claim about what was built. A dirty worktree
# produces artifacts that no commit describes, and a manifest naming a commit
# it does not match is worse than one that admits the gap: someone checking out
# that sha later would build something else and have no way to know. Rehearsals
# are routinely built dirty; releases should not be.
if [ -n "$(git -C "$repo_root" status --porcelain 2>/dev/null)" ]; then
  git_clean=false
else
  git_clean=true
fi

cat >"$dist/release-manifest.json" <<EOF
{
  "schema": "openagents.cli-release.v1",
  "version": "$version",
  "built_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "git_sha": "$(git -C "$repo_root" rev-parse --verify HEAD)",
  "git_clean": $git_clean,
  "host": "$(uname -sm)",
  "artifacts": [$(printf '%s' "$manifest_entries" | sed '$ s/,$//')
  ]
}
EOF

echo "Built:  $(printf '%s' "$built" | tr -s ' ')"
if [ -n "$missing" ]; then
  echo "Missing:$missing"
fi
echo "Staged in $dist"
echo

if [ -n "$missing" ] && [ "$allow_partial" = 0 ]; then
  echo "This release is missing:$missing" >&2
  echo "" >&2
  echo "A channel that points at a version some platforms cannot install is a" >&2
  echo "broken channel, and the installer reports a bare download failure rather" >&2
  echo "than 'unsupported platform'. Fix the toolchain, restrict --targets to" >&2
  echo "what you meant to ship, or pass --allow-partial to publish anyway." >&2
  exit 1
fi

# The check above only sees platforms that were attempted. Narrowing --targets
# makes a one-platform build look complete, and a release that "built
# everything it was asked for" can still be a release that six of seven
# platforms cannot install. That is fine for a rehearsal and fatal for a
# channel, because a channel is the thing readers resolve without naming a
# version. So coverage is judged against the whole platform table, not against
# the request, at the moment a channel is about to be claimed.
if [ -n "$channel" ] && [ "$allow_partial" = 0 ] && [ "$publish" = 0 ]; then
  uncovered=''
  for platform in $all_platforms; do
    case " $built " in
      *" $platform "*) ;;
      *) uncovered="$uncovered $platform" ;;
    esac
  done

  if [ -n "$uncovered" ]; then
    echo "Refusing to point '$channel' at $version." >&2
    echo "" >&2
    echo "This release does not cover:$uncovered" >&2
    echo "" >&2
    echo "Readers who resolve '$channel' on those platforms would get a bare" >&2
    echo "download failure. Build every platform, or pass --allow-partial if a" >&2
    echo "channel that only some platforms can follow is what you mean." >&2
    exit 1
  fi
fi

if [ "$publish" = 0 ]; then
  echo "Not publishing (--publish was not passed)."
  exit 0
fi

command -v gcloud >/dev/null 2>&1 || die "gcloud is required to publish"
CLOUDSDK_CONFIG=$gcloud_config
export CLOUDSDK_CONFIG

# A release can be published in stages. Preserve checksum entries from earlier
# stages, and treat every published <version, platform> pair as immutable. A
# signed macOS build is not byte-for-byte reproducible: rebuilding it changes
# its signature. Replacing that artifact before replacing its sums file creates
# a window where every installer rejects it, while replacing the sums file
# first creates the same window in reverse. Refusing the replacement removes
# both races. Later stages may only add platforms.
published_sums="$dist/.published-SHA256SUMS-$version"
merged_sums="$dist/.merged-SHA256SUMS-$version"
if gcloud storage cp "gs://$bucket/SHA256SUMS-$version" "$published_sums" --quiet \
  >/dev/null 2>&1; then
  :
else
  : >"$published_sums"
fi

: >"$merged_sums"
covered=''
for platform in $all_platforms; do
  platform_sha=''
  for kind in openagents openagents-coder-api; do
    artifact=$(staged_name "$kind" "$platform")
    sums_name=$(sums_name_for "$artifact" "$platform")
    sha=''
    case " $built " in
      *" $platform "*) sha=$(shasum -a 256 "$dist/$artifact" | awk '{print $1}') ;;
      *)
        sha=$(awk -v name="$sums_name" '$2 == name || $2 == "*" name { print $1; exit }' "$published_sums")
        ;;
    esac
    if [ -n "$sha" ]; then
      printf '%s  %s\n' "$sha" "$sums_name" >>"$merged_sums"
      if [ "$kind" = openagents ]; then
        platform_sha=$sha
      fi
    fi
  done
  if [ -n "$platform_sha" ]; then
    covered="$covered $platform"
  fi
done

if [ -n "$channel" ] && [ "$allow_partial" = 0 ]; then
  uncovered=''
  for platform in $all_platforms; do
    case " $covered " in
      *" $platform "*) ;;
      *) uncovered="$uncovered $platform" ;;
    esac
  done
  if [ -n "$uncovered" ]; then
    die "refusing to point '$channel' at $version; the published and newly built artifacts do not cover:$uncovered"
  fi
fi

echo "Publishing to gs://$bucket"
for platform in $built; do
  for kind in openagents openagents-coder-api; do
    artifact=$(staged_name "$kind" "$platform")
    sums_name=$(sums_name_for "$artifact" "$platform")
    local_sha=$(shasum -a 256 "$dist/$artifact" | awk '{print $1}')
    published_sha=$(awk -v name="$sums_name" '$2 == name || $2 == "*" name { print $1; exit }' "$published_sums")
    if [ -n "$published_sha" ]; then
      if [ "$published_sha" != "$local_sha" ]; then
        die "refusing to replace immutable $artifact: published sha256 $published_sha, rebuilt sha256 $local_sha"
      fi
      echo "  keeping published $artifact ($local_sha)"
      continue
    fi
    gcloud storage cp "$dist/$artifact" "gs://$bucket/$artifact" \
      --content-type=application/octet-stream --quiet
  done
done
cp "$merged_sums" "$sums"
gcloud storage cp "$sums" "gs://$bucket/SHA256SUMS-$version" \
  --content-type=text/plain --quiet

echo "Published $version"

[ -n "$channel" ] || { echo "No channel updated (--channel was not passed)."; exit 0; }

if [ "$prerelease" = 1 ] && [ "$allow_prerelease_channel" = 0 ]; then
  die "refusing to point '$channel' at prerelease $version; pass --allow-prerelease-channel if that is the intent"
fi

# The pointer file is the whole body, no trailing newline beyond the one the
# installer strips with tr -d '[:space:]'.
pointer=$(mktemp)
printf '%s\n' "$version" >"$pointer"
gcloud storage cp "$pointer" "gs://$bucket/$channel" \
  --content-type=text/plain --cache-control='public, max-age=60' --quiet
rm -f "$pointer"

echo "Channel '$channel' now points at $version"
