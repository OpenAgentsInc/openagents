#!/usr/bin/env bash
# Signs the Linux release and, with --upload, publishes it.
#
# The Linux app's updater (`crates/openagents-desktop/src/update.rs`,
# `update::linux`) installs only a file that a manifest signed by a
# compiled-in Ed25519 key names, by size and SHA-256; Linux packages carry
# no code signature, so this manifest is the signature. This script checks
# the packages that scripts/desktop/build-linux-release.sh (or
# package-linux.sh) wrote, writes that manifest, and signs SHA256SUMS with
# the same key so a person can check a download by hand. Usage:
#
#   scripts/desktop/sign-manifest-linux.sh --version 1.0.0 --dir DIR
#       [--key PEM] [--key-id ID] [--prefix PATH] [--upload]
#
#   --dir DIR     The build output: OpenAgents-VERSION-x86_64.AppImage,
#                 openagents_VERSION_amd64.deb, the .tar.gz, SHA256SUMS,
#                 and BUILDINFO when there is one. The signed files are
#                 written here too.
#   --version     The release version; the .deb's Version must equal it.
#   --key PEM     The Ed25519 private key. Defaults to
#                 $OPENAGENTS_DESKTOP_UPDATE_KEY, then
#                 <workspace>/.secrets/openagents-desktop-update-ed25519.pem
#                 (the same key and place as sign-manifest.sh for the Mac).
#   --key-id ID   The key's ID in the app's TRUSTED_KEYS
#                 (default desktop-update-2026-09).
#   --prefix P    The bucket folder (default desktop/linux). A test of the
#                 updater uses another, such as desktop/linux-test, and
#                 points the app at it with OPENAGENTS_UPDATE_MANIFEST_URL.
#   --upload      Copy the packages and signatures to
#                 gs://openagentsgemini-oa-updates/PREFIX/VERSION/, then the
#                 manifest to PREFIX/manifest.json.
#
# Published, per release, under PREFIX/VERSION/: the AppImage, the .deb,
# the .tar.gz, SHA256SUMS, SHA256SUMS.sig (the raw 64-byte Ed25519
# signature over SHA256SUMS), openagents-desktop-update.pub.pem (the public
# key, for convenience; the one to trust is TRUSTED_KEYS in update.rs and
# docs/desktop/release.md), BUILDINFO, and manifest.json (a copy). To check
# a download by hand:
#
#   openssl pkeyutl -verify -pubin -inkey openagents-desktop-update.pub.pem \
#     -rawin -in SHA256SUMS -sigfile SHA256SUMS.sig
#   sha256sum -c SHA256SUMS
#
# The envelope is the Mac's: {"key", "payload", "signature"}, the payload
# the manifest JSON's bytes in base64 and an Ed25519 signature over exactly
# those bytes. The payload names "platform": "linux" and each artifact's
# "format", so a Mac app refuses it and a Linux app refuses the Mac's. The
# script refuses a version not newer than the one published at PREFIX.
# Runs on macOS or Linux with OpenSSL 3.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bucket="openagentsgemini-oa-updates"
prefix="desktop/linux"
bundle_id="com.openagents.desktop"
schema="openagents.desktop.update.v1"

version=""
dir=""
key="${OPENAGENTS_DESKTOP_UPDATE_KEY:-$root/../.secrets/openagents-desktop-update-ed25519.pem}"
key_id="desktop-update-2026-09"
upload=0

die() {
  echo "sign-manifest-linux: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:-}"; shift 2 ;;
    --dir) dir="${2:-}"; shift 2 ;;
    --key) key="${2:-}"; shift 2 ;;
    --key-id) key_id="${2:-}"; shift 2 ;;
    --prefix) prefix="${2:-}"; shift 2 ;;
    --upload) upload=1; shift ;;
    -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument \`$1\` (see --help)" ;;
  esac
done

[[ -n "$version" ]] || die "--version is required"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] || die "\`$version\` is not a semantic version"
[[ -n "$dir" && -d "$dir" ]] || die "--dir must be the build output folder"
[[ "$key_id" =~ ^[A-Za-z0-9._-]+$ ]] || die "\`$key_id\` is not a key ID"
[[ "$prefix" =~ ^desktop/linux(-[a-z0-9-]+)?$ ]] || die "\`$prefix\` is not desktop/linux or desktop/linux-NAME"
[[ -r "$key" ]] || die "cannot read the signing key at $key"
openssl pkey -in "$key" -noout 2>/dev/null || die "$key is not a private key openssl can read"
dir="$(cd "$dir" && pwd)"
public_base="https://storage.googleapis.com/$bucket/$prefix"

sha256() { openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1; }
size() { wc -c <"$1" | tr -d ' '; }
b64() { openssl base64 -A -in "$1"; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Succeeds when semantic version $1 is newer than $2 (as sign-manifest.sh).
newer() {
  local a_core="${1%%-*}" b_core="${2%%-*}" a_pre="" b_pre="" i a_part b_part
  [[ "$1" == *-* ]] && a_pre="${1#*-}"
  [[ "$2" == *-* ]] && b_pre="${2#*-}"
  IFS=. read -r -a a_parts <<<"$a_core"
  IFS=. read -r -a b_parts <<<"$b_core"
  for i in 0 1 2; do
    a_part="${a_parts[$i]:-0}" b_part="${b_parts[$i]:-0}"
    ((10#$a_part > 10#$b_part)) && return 0
    ((10#$a_part < 10#$b_part)) && return 1
  done
  [[ -z "$a_pre" && -n "$b_pre" ]] && return 0
  [[ -z "$a_pre" || -z "$b_pre" || "$a_pre" == "$b_pre" ]] && return 1
  [[ "$(printf '%s\n%s\n' "$a_pre" "$b_pre" | sort -V | tail -n 1)" == "$a_pre" ]]
}

published="$(curl -fsS "$public_base/manifest.json" 2>/dev/null || true)"
if [[ -n "$published" ]]; then
  old="$(printf '%s' "$published" | sed -n 's/.*"payload":"\([^"]*\)".*/\1/p' |
    openssl base64 -d -A 2>/dev/null | sed -n 's/.*"version":"\([^"]*\)".*/\1/p' || true)"
  if [[ -n "$old" ]] && ! newer "$version" "$old"; then
    die "the manifest at $prefix is already $old; sign a newer version"
  fi
fi

appimage="$dir/OpenAgents-$version-x86_64.AppImage"
deb="$dir/openagents_${version}_amd64.deb"
tarball="$dir/openagents-$version-linux-x86_64.tar.gz"
sums="$dir/SHA256SUMS"
for file in "$appimage" "$deb" "$tarball" "$sums"; do
  [[ -f "$file" ]] || die "missing $file"
done

# SHA256SUMS must describe exactly these files.
while read -r sum name; do
  [[ -f "$dir/$name" ]] || die "SHA256SUMS names $name, which is not in $dir"
  [[ "$(sha256 "$dir/$name")" == "$sum" ]] || die "$name does not match SHA256SUMS"
done <"$sums"
for file in "$appimage" "$deb" "$tarball"; do
  grep -q " $(basename "$file")\$" "$sums" || die "SHA256SUMS does not list $(basename "$file")"
done

# The AppImage: an x86-64 ELF with the type-2 AppImage magic.
header="$(od -An -tx1 -N20 "$appimage" | tr -d ' \n')"
[[ "${header:0:8}" == 7f454c46 ]] || die "$appimage is not an ELF file"
[[ "${header:16:6}" == 414902 ]] || die "$appimage has no type-2 AppImage magic"
[[ "${header:36:4}" == 3e00 ]] || die "$appimage is not x86-64"

# The .deb: our package, this version, amd64.
control_member="$(ar t "$deb" | grep '^control\.tar' | head -n 1)"
[[ -n "$control_member" ]] || die "$deb has no control archive"
(cd "$work" && ar x "$deb" "$control_member")
mkdir -p "$work/control"
tar -xf "$work/$control_member" -C "$work/control"
field() { sed -n "s/^$1: *//p" "$work/control/control" | head -n 1; }
[[ "$(field Package)" == openagents ]] || die "$deb is not the openagents package"
[[ "$(field Version)" == "$version" ]] || die "$deb is version $(field Version), not $version"
[[ "$(field Architecture)" == amd64 ]] || die "$deb is not amd64"

entry() {
  # $1: the file, $2: its format
  printf '{"arch":"x86_64","url":"%s","sha256":"%s","size":%s,"format":"%s"}' \
    "$public_base/$version/$(basename "$1")" "$(sha256 "$1")" "$(size "$1")" "$2"
}
published_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
payload="$work/payload.json"
printf '{"schema":"%s","bundle_id":"%s","version":"%s","platform":"linux","published":"%s","artifacts":[%s,%s]}' \
  "$schema" "$bundle_id" "$version" "$published_at" \
  "$(entry "$appimage" appimage)" "$(entry "$deb" deb)" >"$payload"

openssl pkey -in "$key" -pubout -out "$work/public.pem"
sign() {
  # $1: the file, $2: the signature to write; checked before it is kept
  openssl pkeyutl -sign -inkey "$key" -rawin -in "$1" -out "$2.part"
  openssl pkeyutl -verify -pubin -inkey "$work/public.pem" -rawin -in "$1" \
    -sigfile "$2.part" >/dev/null || die "the new signature over $1 does not verify"
  mv "$2.part" "$2"
}
sign "$payload" "$work/signature"
sign "$sums" "$dir/SHA256SUMS.sig"
cp "$work/public.pem" "$dir/openagents-desktop-update.pub.pem"
public_hex="$(openssl pkey -pubin -in "$work/public.pem" -outform DER | tail -c 32 | od -An -tx1 | tr -d ' \n')"

printf '{"key":"%s","payload":"%s","signature":"%s"}\n' \
  "$key_id" "$(b64 "$payload")" "$(b64 "$work/signature")" >"$dir/manifest.json"

echo "signed Linux $version with $key_id (public key $public_hex)"
echo "wrote $dir/manifest.json and $dir/SHA256SUMS.sig"

if [[ "$upload" == 1 ]]; then
  for file in "$appimage" "$deb" "$tarball" "$sums" "$dir/SHA256SUMS.sig" \
    "$dir/openagents-desktop-update.pub.pem" "$dir/manifest.json" "$dir/BUILDINFO"; do
    [[ -f "$file" ]] || continue
    gcloud storage cp "$file" "gs://$bucket/$prefix/$version/$(basename "$file")"
  done
  # The manifest last, and never cached, so no app sees it before its files.
  gcloud storage cp --cache-control="no-cache, max-age=0" \
    "$dir/manifest.json" "gs://$bucket/$prefix/manifest.json"
  echo "published $public_base/manifest.json"
fi
