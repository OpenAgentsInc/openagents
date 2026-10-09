#!/usr/bin/env bash
# Signs the Windows release and, with --upload, publishes it.
#
# The Windows app's updater (`crates/openagents-desktop/src/update.rs`,
# `update::windows`) installs only an MSI that a manifest signed by a
# compiled-in Ed25519 key names, by size and SHA-256. This script checks
# the packages scripts/desktop/package-windows.sh (or package-windows.ps1)
# wrote, writes that manifest, and signs SHA256SUMS with the same key so a
# person can check a download by hand. Usage:
#
#   scripts/desktop/sign-manifest-windows.sh --version 1.0.0 --dir DIR
#       [--key PEM] [--key-id ID] [--prefix PATH] [--upload]
#
#   --dir DIR     The build output: OpenAgents-VERSION-x64.msi,
#                 OpenAgents-VERSION-windows-x64.zip, SHA256SUMS, and
#                 BUILDINFO when there is one. The signed files are
#                 written here too.
#   --version     The release version.
#   --key PEM     The Ed25519 private key. Defaults to
#                 $OPENAGENTS_DESKTOP_UPDATE_KEY, then
#                 <workspace>/.secrets/openagents-desktop-update-ed25519.pem
#                 (the same key as the Mac and Linux releases).
#   --key-id ID   The key's ID in the app's TRUSTED_KEYS
#                 (default desktop-update-2026-09).
#   --prefix P    The bucket folder (default desktop/windows). A test of
#                 the updater uses another,
#                 such as desktop/windows-test, and points
#                 the app at it with OPENAGENTS_UPDATE_MANIFEST_URL.
#   --upload      Copy the packages and signatures to
#                 gs://openagentsgemini-oa-updates/PREFIX/VERSION/, then the
#                 manifest to PREFIX/manifest.json.
#
# Unsigned packages may be released (owner, 2026-10-09): Windows runs them
# after SmartScreen's "More info, Run anyway". The update manifest is still
# signed with our own key, so updates stay verified. This script says
# whether the packages carry an Authenticode signature (BUILDINFO
# "signed yes", written by package-windows.sh).
#
# Published, per release, under PREFIX/VERSION/: the MSI, the .zip,
# SHA256SUMS, SHA256SUMS.sig (the raw 64-byte Ed25519 signature over
# SHA256SUMS), openagents-desktop-update.pub.pem, BUILDINFO, and
# manifest.json (a copy). To check a download by hand:
#
#   openssl pkeyutl -verify -pubin -inkey openagents-desktop-update.pub.pem \
#     -rawin -in SHA256SUMS -sigfile SHA256SUMS.sig
#   sha256sum -c SHA256SUMS
#
# The envelope is the Mac's: {"key", "payload", "signature"}. The payload
# names "platform": "windows" and each artifact's "format" (msi, zip), so
# only a Windows app accepts it. The script refuses a version not newer
# than the one published at PREFIX. Runs on macOS or Linux with OpenSSL 3.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bucket="openagentsgemini-oa-updates"
prefix="desktop/windows"
bundle_id="com.openagents.desktop"
schema="openagents.desktop.update.v1"

version=""
dir=""
key="${OPENAGENTS_DESKTOP_UPDATE_KEY:-$root/../.secrets/openagents-desktop-update-ed25519.pem}"
key_id="desktop-update-2026-09"
upload=0

die() {
  echo "sign-manifest-windows: $*" >&2
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
[[ "$prefix" =~ ^desktop/windows(-[a-z0-9-]+)?$ ]] || die "\`$prefix\` is not desktop/windows or desktop/windows-NAME"
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

msi="$dir/OpenAgents-$version-x64.msi"
zipfile="$dir/OpenAgents-$version-windows-x64.zip"
sums="$dir/SHA256SUMS"
for file in "$msi" "$zipfile" "$sums"; do
  [[ -f "$file" ]] || die "missing $file"
done

# SHA256SUMS must describe exactly these files.
while read -r sum name; do
  [[ -f "$dir/$name" ]] || die "SHA256SUMS names $name, which is not in $dir"
  [[ "$(sha256 "$dir/$name")" == "$sum" ]] || die "$name does not match SHA256SUMS"
done <"$sums"
for file in "$msi" "$zipfile"; do
  grep -q " $(basename "$file")\$" "$sums" || die "SHA256SUMS does not list $(basename "$file")"
done

# The MSI: a compound file, as update::windows::check_msi requires. The
# .zip: a zip archive.
[[ "$(od -An -tx1 -N8 "$msi" | tr -d ' \n')" == d0cf11e0a1b11ae1 ]] || die "$msi is not an MSI"
[[ "$(od -An -tx1 -N4 "$zipfile" | tr -d ' \n')" == 504b0304 ]] || die "$zipfile is not a zip archive"

# Unsigned packages are allowed in a release; say which this is.
if grep -qx 'signed yes' "$dir/BUILDINFO" 2>/dev/null; then
  echo "packages are Authenticode-signed"
else
  echo "packages are not Authenticode-signed: SmartScreen will ask people to confirm (More info, Run anyway)" >&2
fi

entry() {
  # $1: the file, $2: its format
  printf '{"arch":"x86_64","url":"%s","sha256":"%s","size":%s,"format":"%s"}' \
    "$public_base/$version/$(basename "$1")" "$(sha256 "$1")" "$(size "$1")" "$2"
}
published_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
payload="$work/payload.json"
printf '{"schema":"%s","bundle_id":"%s","version":"%s","platform":"windows","published":"%s","artifacts":[%s,%s]}' \
  "$schema" "$bundle_id" "$version" "$published_at" \
  "$(entry "$msi" msi)" "$(entry "$zipfile" zip)" >"$payload"

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

echo "signed Windows $version with $key_id (public key $public_hex)"
echo "wrote $dir/manifest.json and $dir/SHA256SUMS.sig"

if [[ "$upload" == 1 ]]; then
  for file in "$msi" "$zipfile" "$sums" "$dir/SHA256SUMS.sig" \
    "$dir/openagents-desktop-update.pub.pem" "$dir/manifest.json" "$dir/BUILDINFO"; do
    [[ -f "$file" ]] || continue
    gcloud storage cp "$file" "gs://$bucket/$prefix/$version/$(basename "$file")"
  done
  # The manifest last, and never cached, so no app sees it before its files.
  gcloud storage cp --cache-control="no-cache, max-age=0" \
    "$dir/manifest.json" "gs://$bucket/$prefix/manifest.json"
  echo "published $public_base/manifest.json"
fi
