#!/usr/bin/env bash
# Signs an update manifest for OpenAgents for Mac and, with --upload,
# publishes it.
#
# The desktop app's updater (`crates/openagents-desktop/src/update.rs`)
# installs only a build that a manifest signed by a compiled-in Ed25519 key
# names, by size and SHA-256. This script makes that manifest from signed,
# notarized, stapled `OpenAgents.app` bundles (the output of
# `scripts/desktop/package-macos.sh`), then signs it with the private key,
# which never enters the repository. Usage:
#
#   scripts/desktop/sign-manifest.sh --version 0.2.0 --app PATH [--app PATH]
#       [--key PEM] [--key-id ID] [--out DIR] [--upload]
#
#   --app PATH   OpenAgents.app, or a zip of it made with
#                `ditto -c -k --keepParent`. Repeat for one build per
#                architecture; the architecture comes from `lipo`.
#   --version    The release version. Each bundle's
#                CFBundleShortVersionString must equal it.
#   --key PEM    The Ed25519 private key. Defaults to
#                $OPENAGENTS_DESKTOP_UPDATE_KEY, then
#                <workspace>/.secrets/openagents-desktop-update-ed25519.pem.
#   --key-id ID  The key's ID in the app's TRUSTED_KEYS
#                (default desktop-update-2026-09).
#   --out DIR    Where the zips and manifest.json go
#                (default target/desktop-update/VERSION).
#   --upload     Copy the zips, then the manifest, to
#                gs://openagentsgemini-oa-updates/desktop/macos/.
#
# Each bundle must pass `codesign --verify --deep --strict` and Gatekeeper's
# notarization check, carry the bundle ID com.openagents.desktop, and all
# must share one Team ID. The script refuses a version not newer than the
# published manifest's.
#
# The signing key. The private key was made once with
# `openssl genpkey -algorithm ed25519` and is kept, mode 600, at
# <workspace>/.secrets/openagents-desktop-update-ed25519.pem on the release
# Mac (the workspace's ignored secrets folder), never in this repository,
# a log, or an issue. Its public half is TRUSTED_KEYS in update.rs, ID
# desktop-update-2026-09, hex b9c688e6f33b77f63f61ce46d1b520b2c5588b79ea4cccafc30b122f211a8b84.
# Keep an offline backup: losing it means shipping a release that trusts a
# new key by hand (a .dmg the person installs), since the app accepts no
# other. To rotate, add the new key to TRUSTED_KEYS, ship that release signed
# by the old key, then sign with the new one.
#
# Hosting. Builds go to gs://openagentsgemini-oa-updates/desktop/macos/VERSION/
# and the envelope to desktop/macos/manifest.json in the same public-read
# bucket (served from https://storage.googleapis.com). The envelope is
# {"key", "payload", "signature"}: the manifest JSON's bytes in base64 and
# an Ed25519 signature over exactly those bytes.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bucket="openagentsgemini-oa-updates"
prefix="desktop/macos"
public_base="https://storage.googleapis.com/$bucket/$prefix"
bundle_id="com.openagents.desktop"
schema="openagents.desktop.update.v1"
# The OpenAgents Developer ID team; update.rs's TEAM_ID refuses any other.
team_id="HQWSG26L43"

version=""
key="${OPENAGENTS_DESKTOP_UPDATE_KEY:-$root/../.secrets/openagents-desktop-update-ed25519.pem}"
key_id="desktop-update-2026-09"
out=""
upload=0
apps=()

die() {
  echo "sign-manifest: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:-}"; shift 2 ;;
    --app) apps+=("${2:-}"); shift 2 ;;
    --key) key="${2:-}"; shift 2 ;;
    --key-id) key_id="${2:-}"; shift 2 ;;
    --out) out="${2:-}"; shift 2 ;;
    --upload) upload=1; shift ;;
    -h | --help) sed -n '2,50p' "$0"; exit 0 ;;
    *) die "unknown argument \`$1\` (see --help)" ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || die "signing checks the bundles with codesign; run it on a Mac"
[[ -n "$version" ]] || die "--version is required"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] || die "\`$version\` is not a semantic version"
[[ ${#apps[@]} -gt 0 ]] || die "at least one --app is required"
[[ "$key_id" =~ ^[A-Za-z0-9._-]+$ ]] || die "\`$key_id\` is not a key ID"
[[ -r "$key" ]] || die "cannot read the signing key at $key"
openssl pkey -in "$key" -noout 2>/dev/null || die "$key is not a private key openssl can read"
out="${out:-$root/target/desktop-update/$version}"
mkdir -p "$out"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Succeeds when semantic version $1 is newer than $2. Prerelease tags
# compare with `sort -V`, which orders rc.2 before rc.10.
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

# Newer than what is published, so a stale run cannot replace a newer
# release (the app would refuse it as a downgrade anyway).
published="$(curl -fsS "$public_base/manifest.json" 2>/dev/null || true)"
if [[ -n "$published" ]]; then
  old="$(printf '%s' "$published" | plutil -extract payload raw -o - - 2>/dev/null |
    base64 -D 2>/dev/null | plutil -extract version raw -o - - 2>/dev/null || true)"
  if [[ -n "$old" ]] && ! newer "$version" "$old"; then
    die "the published manifest is already $old; sign a newer version"
  fi
fi

team=""
artifacts=""
zips=()
for input in "${apps[@]}"; do
  [[ -e "$input" ]] || die "$input does not exist"
  case "$input" in
    *.app)
      app="$input"
      ;;
    *.zip)
      unpack="$work/unpack-${#zips[@]}"
      mkdir -p "$unpack"
      ditto -x -k "$input" "$unpack"
      app="$(find "$unpack" -maxdepth 1 -name '*.app' | head -n 1)"
      [[ -n "$app" ]] || die "$input holds no .app"
      ;;
    *) die "$input is neither an .app nor a .zip" ;;
  esac

  codesign --verify --deep --strict "$app" || die "$app fails codesign --verify"
  assessment="$(spctl --assess --type execute -v "$app" 2>&1 || true)"
  [[ "$assessment" == *"Notarized Developer ID"* ]] ||
    die "$app is not accepted as notarized: $assessment"
  this_team="$(codesign -d --verbose=4 "$app" 2>&1 | sed -n 's/^TeamIdentifier=//p')"
  [[ "$this_team" == "$team_id" ]] || die "$app is signed by team \`$this_team\`, not $team_id"
  [[ -z "$team" || "$team" == "$this_team" ]] || die "the bundles are signed by different teams"
  team="$this_team"
  plist="$app/Contents/Info.plist"
  [[ "$(plutil -extract CFBundleIdentifier raw "$plist")" == "$bundle_id" ]] ||
    die "$app is not $bundle_id"
  [[ "$(plutil -extract CFBundleShortVersionString raw "$plist")" == "$version" ]] ||
    die "$app is not version $version"

  executable="$app/Contents/MacOS/$(plutil -extract CFBundleExecutable raw "$plist")"
  archs="$(lipo -archs "$executable")"
  case "$archs" in
    *arm64*x86_64* | *x86_64*arm64*) arch="universal" ;;
    arm64) arch="arm64" ;;
    x86_64) arch="x86_64" ;;
    *) die "$executable has architectures \`$archs\`" ;;
  esac
  [[ "$artifacts" != *"\"arch\":\"$arch\""* ]] || die "two bundles are $arch"

  zip="$out/OpenAgents-$version-$arch.zip"
  rm -f "$zip"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"
  sha="$(shasum -a 256 "$zip" | cut -d ' ' -f 1)"
  size="$(stat -f %z "$zip")"
  url="$public_base/$version/$(basename "$zip")"
  entry="{\"arch\":\"$arch\",\"url\":\"$url\",\"sha256\":\"$sha\",\"size\":$size}"
  artifacts="${artifacts:+$artifacts,}$entry"
  zips+=("$zip")
done

published_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
payload="$work/payload.json"
printf '{"schema":"%s","bundle_id":"%s","version":"%s","team_id":"%s","published":"%s","artifacts":[%s]}' \
  "$schema" "$bundle_id" "$version" "$team" "$published_at" "$artifacts" >"$payload"

openssl pkeyutl -sign -inkey "$key" -rawin -in "$payload" -out "$work/signature"
openssl pkey -in "$key" -pubout -out "$work/public.pem"
openssl pkeyutl -verify -pubin -inkey "$work/public.pem" -rawin -in "$payload" \
  -sigfile "$work/signature" >/dev/null || die "the new signature does not verify"
public_hex="$(openssl pkey -pubin -in "$work/public.pem" -outform DER | tail -c 32 | xxd -p -c 64)"

printf '{"key":"%s","payload":"%s","signature":"%s"}\n' \
  "$key_id" "$(base64 -i "$payload" | tr -d '\n')" "$(base64 -i "$work/signature" | tr -d '\n')" \
  >"$out/manifest.json"

echo "signed $version for team $team with $key_id (public key $public_hex)"
echo "wrote $out/manifest.json"

if [[ "$upload" == 1 ]]; then
  for zip in "${zips[@]}"; do
    gcloud storage cp "$zip" "gs://$bucket/$prefix/$version/$(basename "$zip")"
  done
  # The manifest last, and never cached, so no app sees it before its zips.
  gcloud storage cp --cache-control="no-cache, max-age=0" \
    "$out/manifest.json" "gs://$bucket/$prefix/manifest.json"
  echo "published $public_base/manifest.json"
fi
