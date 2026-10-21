#!/bin/sh
# Build, sign, and (only with --publish) upload the Psionic engine,
# `psionic-openai-server`, that `openagents pylon setup` installs.
#
# One build per platform:
#
#   macos-aarch64       Metal; signed with the Developer ID and notarized the
#                       way scripts/release/terminal.sh signs the CLI
#   linux-x86_64-cuda   CUDA 13 kernels (needs nvcc; links libcudart)
#   linux-x86_64-cpu    no CUDA (PSIONIC_CUDA=off links the kernel stubs)
#
# A host builds only its own platforms: a Mac builds macos-aarch64, a Linux
# x86_64 box builds linux-x86_64-cpu and, with nvcc, linux-x86_64-cuda. Each
# build lands in dist/releases/psionic/<version>/<platform>/ with a
# psionic-manifest.json (platform, sha256, size, build commit), and the
# script prints the row for crates/pylon/src/release/psionic_pins.rs.
#
# With --publish, each platform's two files go to the CLI release bucket:
#
#   gs://<bucket>/psionic/<version>/<platform>/psionic-openai-server
#   gs://<bucket>/psionic/<version>/<platform>/psionic-manifest.json
#
# served at https://storage.googleapis.com/<bucket>/psionic/... . Nothing is
# ever replaced: a version whose object already exists is refused. Without
# --publish nothing is uploaded. Only the release agent publishes; see
# "Publishing the Psionic engine" in docs/release/terminal.md.
#
# Usage:
#   scripts/release/psionic.sh --version 0.1.0                  # build + sign here
#   scripts/release/psionic.sh --version 0.1.0 --publish        # release agent only
#
# Options:
#   --version VERSION     The engine release, e.g. 0.1.0. Required.
#   --commit REF          The commit to build (default HEAD).
#   --platforms "A B"     Which of this host's platforms to build (default all
#                         it can build).
#   --publish             Upload to the release bucket. Off by default.
#   --skip-notarization   Sign the macOS build without notarizing it. Refused
#                         with --publish.
#   --unsigned            Do not sign the macOS build at all (a local check).
#                         Refused with --publish.
#
# Environment:
#   OPENAGENTS_RELEASES_BUCKET  Default openagentsgemini-cli-releases.
#   CLOUDSDK_CONFIG             gcloud configuration allowed to write the
#                               bucket (default ~/work/.secrets/gcloud-sa-config
#                               when it exists).
#   OA_DEVELOPER_ID_APPLICATION, NOTARY_KEYCHAIN_PROFILE or ASC_API_KEY_ID /
#   ASC_API_ISSUER_ID / ASC_API_PRIVATE_KEY_PATH (OPENAGENTS_NOTARY_ENV names
#                               a file holding them), as for terminal.sh.
#   CUDAARCHS or PSI_CUDA_ARCH  The CUDA build's GPU architecture (default:
#                               the build host's GPU). Recorded in the manifest.
#   CARGO                       The cargo to run (default cargo).
#   CARGO_TARGET_DIR            Build directory (default crates/psionic/target
#                               of the commit's checkout).

set -eu

script_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

die() {
  echo "$@" >&2
  exit 1
}

server=psionic-openai-server
manifest_name=psionic-manifest.json
prefix=psionic
version=''
commit_ref=HEAD
platforms=''
publish=0
skip_notarization=0
unsigned=0
cargo=${CARGO:-cargo}

bucket=${OPENAGENTS_RELEASES_BUCKET:-openagentsgemini-cli-releases}
default_gcloud_config="$HOME/work/.secrets/gcloud-sa-config"
if [ -z "${CLOUDSDK_CONFIG:-}" ] && [ -d "$default_gcloud_config" ]; then
  CLOUDSDK_CONFIG=$default_gcloud_config
fi
[ -z "${CLOUDSDK_CONFIG:-}" ] || export CLOUDSDK_CONFIG
notary_env=${OPENAGENTS_NOTARY_ENV:-$HOME/work/.secrets/appstoreconnect.env}

usage() {
  sed -n '2,/^set -eu/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || die "--version needs a value"; version=$2; shift 2 ;;
    --commit) [ $# -ge 2 ] || die "--commit needs a value"; commit_ref=$2; shift 2 ;;
    --platforms) [ $# -ge 2 ] || die "--platforms needs a value"; platforms=$2; shift 2 ;;
    --publish) publish=1; shift ;;
    --skip-notarization) skip_notarization=1; shift ;;
    --unsigned) unsigned=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

[ -n "$version" ] || die "--version is required"
printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$' ||
  die "--version must look like 0.1.0 or 0.1.0-rc.1, not $version"
if [ "$publish" = 1 ] && [ "$skip_notarization" = 1 ]; then
  die "--skip-notarization is refused with --publish: Gatekeeper refuses a macOS engine that is not notarized"
fi
if [ "$publish" = 1 ] && [ "$unsigned" = 1 ]; then
  die "--unsigned is refused with --publish"
fi

find_nvcc() {
  for candidate in "${NVCC:-}" /opt/cuda/bin/nvcc /usr/local/cuda/bin/nvcc "$(command -v nvcc 2>/dev/null || true)"; do
    [ -n "$candidate" ] && [ -x "$candidate" ] && { echo "$candidate"; return 0; }
  done
  return 1
}

# What this host can build.
case "$(uname -s) $(uname -m)" in
  'Darwin arm64') buildable=macos-aarch64 ;;
  'Linux x86_64')
    if find_nvcc >/dev/null; then
      buildable='linux-x86_64-cuda linux-x86_64-cpu'
    else
      buildable=linux-x86_64-cpu
    fi
    ;;
  *) buildable='' ;;
esac
[ -n "$buildable" ] || die "this host ($(uname -s) $(uname -m)) builds no engine platform; use an Apple Silicon Mac or a Linux x86_64 box"
[ -n "$platforms" ] || platforms=$buildable
for platform in $platforms; do
  case "$platform" in
    macos-aarch64|linux-x86_64-cuda|linux-x86_64-cpu) ;;
    *) die "unknown platform: $platform (known: macos-aarch64 linux-x86_64-cuda linux-x86_64-cpu)" ;;
  esac
  case " $buildable " in
    *" $platform "*) ;;
    *) die "this host cannot build $platform (it builds: $buildable)" ;;
  esac
done

sha=$(git -C "$repo_root" rev-parse --verify "$commit_ref^{commit}") || die "no commit $commit_ref"
short=$(printf '%s' "$sha" | cut -c1-10)
public_base="https://storage.googleapis.com/$bucket/$prefix/$version"
root="gs://$bucket/$prefix/$version"

gs() {
  gcloud storage "$@"
}

if [ "$publish" = 1 ]; then
  command -v gcloud >/dev/null 2>&1 || die "gcloud is required to publish"
  for platform in $platforms; do
    for name in "$server" "$manifest_name"; do
      if gs ls "$root/$platform/$name" >/dev/null 2>&1; then
        die "refusing to build: $root/$platform/$name already exists; take the next version"
      fi
    done
  done
fi

for command_name in "$cargo" git shasum; do
  command -v "$command_name" >/dev/null 2>&1 || die "$command_name is required to build the engine"
done

dist="$repo_root/dist/releases/$prefix/$version"
mkdir -p "$dist"
source=$(mktemp -d "${TMPDIR:-/tmp}/psionic-release-source.XXXXXX")
trap 'rm -rf "$source"' EXIT
echo "Psionic engine $version from $short ($(git -C "$repo_root" log -1 --format=%s "$sha"))"
# The Psionic workspace links crates/stage-link by path, and that crate takes
# its package fields from the root manifest.
git -C "$repo_root" archive --format=tar "$sha" crates/psionic crates/stage-link Cargo.toml \
  rust-toolchain.toml | (cd "$source" && tar -xf -)
target_dir=${CARGO_TARGET_DIR:-"$source/crates/psionic/target"}

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

# Sign `$1` with the hardened runtime and notarize it. Sets `notary`.
sign_and_notarize() {
  artifact=$1
  if [ "$unsigned" = 1 ]; then
    notary=unsigned
    return 0
  fi
  load_signing
  echo "  signing as com.openagents.psionic.openai-server"
  codesign --force --timestamp --options runtime \
    --identifier com.openagents.psionic.openai-server \
    --sign "$OA_DEVELOPER_ID_APPLICATION" "$artifact" >"$artifact.codesign.log" 2>&1 ||
    { cat "$artifact.codesign.log" >&2; die "codesign failed for $artifact"; }
  rm -f "$artifact.codesign.log"
  codesign --verify --strict "$artifact" || die "signature does not verify: $artifact"
  if [ "$skip_notarization" = 1 ]; then
    notary=skipped
    return 0
  fi
  submission="$dist/.notarize"
  rm -rf "$submission" "$submission.zip"
  mkdir -p "$submission"
  cp "$artifact" "$submission/"
  /usr/bin/ditto -c -k --keepParent "$submission" "$submission.zip"
  echo "  notarizing"
  log="$dist/.notary.log"
  notarize_zip "$submission.zip" >"$log" 2>&1 || { cat "$log" >&2; die "notarization failed"; }
  status=$(awk '/^  status: /{print $2; exit}' "$log")
  id=$(awk '/^  id: /{print $2; exit}' "$log")
  rm -rf "$submission" "$submission.zip"
  [ "$status" = Accepted ] || { cat "$log" >&2; die "notarization came back $status"; }
  rm -f "$log"
  notary="accepted $id"
  echo "  notarized ($id)"
}

pins=''
for platform in $platforms; do
  echo
  echo "$platform"
  cuda_arch=''
  case "$platform" in
    linux-x86_64-cpu) build_env='PSIONIC_CUDA=off' ;;
    linux-x86_64-cuda)
      build_env='PSIONIC_CUDA=on'
      cuda_arch=${CUDAARCHS:-${PSI_CUDA_ARCH:-$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader,nounits 2>/dev/null | head -1 || true)}}
      [ -n "$cuda_arch" ] || die "set CUDAARCHS (for example 89) for the CUDA build: there is no GPU here to read it from"
      echo "  CUDA architecture $cuda_arch (a card of another architecture needs its own build)"
      ;;
    *) build_env='PSIONIC_CUDA=on' ;;
  esac
  (cd "$source" && env $build_env CARGO_TARGET_DIR="$target_dir" "$cargo" build --release \
    --manifest-path crates/psionic/Cargo.toml -p psionic-serve --bin "$server") ||
    die "the $platform build failed"
  out="$dist/$platform"
  rm -rf "$out"
  mkdir -p "$out"
  cp "$target_dir/release/$server" "$out/$server"
  chmod 755 "$out/$server"
  notary=''
  case "$platform" in
    macos-*) sign_and_notarize "$out/$server" ;;
  esac
  digest=$(shasum -a 256 "$out/$server" | awk '{print $1}')
  size=$(wc -c <"$out/$server" | tr -d ' ')
  url="$public_base/$platform/$server"
  cat >"$out/$manifest_name" <<EOF
{
  "schema": "openagents.psionic-engine.v1",
  "version": "$version",
  "platform": "$platform",
  "file": "$server",
  "url": "$url",
  "sha256": "$digest",
  "size": $size,
  "build_commit": "$sha",
  "cuda_arch": "$cuda_arch",
  "notarization": "$notary"
}
EOF
  echo "  $out/$server ($size bytes, sha256 $digest)"
  pins="$pins    PsionicPin {
        platform: \"$platform\",
        url: Some(\"$url\"),
        sha256: Some(\"$digest\"),
        size: Some($size),
        build_commit: Some(\"$sha\"),
    },
"
done

if [ "$publish" = 1 ]; then
  echo
  for platform in $platforms; do
    for name in "$server" "$manifest_name"; do
      case "$name" in
        *.json) type=application/json ;;
        *) type=application/octet-stream ;;
      esac
      echo "Publishing $root/$platform/$name"
      gs cp --no-clobber "$dist/$platform/$name" "$root/$platform/$name" \
        --content-type="$type" --cache-control='public, max-age=31536000, immutable' --quiet
    done
    # Read the engine back: the bucket must hold exactly the bytes built.
    published=$(gs objects describe "$root/$platform/$server" --format='value(size)' 2>/dev/null || true)
    [ "$published" = "$(wc -c <"$dist/$platform/$server" | tr -d ' ')" ] ||
      die "$root/$platform/$server reads back as ${published:-nothing}; check the bucket before pinning it"
  done
fi

echo
if [ "$publish" = 1 ]; then
  echo "Published. Pin these rows in crates/pylon/src/release/psionic_pins.rs:"
else
  echo "Built; nothing was uploaded (--publish uploads). Once published, the rows for crates/pylon/src/release/psionic_pins.rs are:"
fi
printf '%s' "$pins"
