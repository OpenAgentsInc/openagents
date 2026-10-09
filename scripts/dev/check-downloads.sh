#!/usr/bin/env bash
# Check every download link on /download, read-only (#11103).
#
# Reads the live page, takes every https link in its main content and every
# address in its install commands, and requests each one with HEAD
# (following redirects; a server that refuses HEAD gets a one-byte GET).
# Then checks the desktop release's files, which the page lists only once
# the release is out (DESKTOP_RELEASED in
# crates/openagents-web/src/pages/download.rs), and prints the version each
# platform's update manifest names. Downloads nothing beyond one byte and
# changes nothing.
#
#   scripts/dev/check-downloads.sh                       # production
#   scripts/dev/check-downloads.sh --site http://127.0.0.1:8080
#   scripts/dev/check-downloads.sh --desktop-version 1.0.0
#
# Exits 1 when any link fails.
set -uo pipefail

site="https://openagents.com"
desktop_version="1.0.0"
updates="https://storage.googleapis.com/openagentsgemini-oa-updates/desktop"

while [ $# -gt 0 ]; do
  case "$1" in
    --site) site="${2:?--site needs a URL}"; shift 2 ;;
    --desktop-version) desktop_version="${2:?--desktop-version needs a value}"; shift 2 ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "check-downloads: unknown option $1" >&2; exit 64 ;;
  esac
done

page="$(curl -fsSL "$site/download")" || { echo "check-downloads: could not load $site/download" >&2; exit 1; }
main="$(printf '%s' "$page" | tr '\n' ' ' | sed -e 's/.*<main//' -e 's/<\/main>.*//')"

# Links in the main content, then addresses inside commands.
links="$(printf '%s' "$main" | grep -o 'href="https://[^"]*"' | sed -e 's/^href="//' -e 's/"$//' | sed 's/&amp;/\&/g')"
commands="$(printf '%s' "$main" | grep -o '<code>[^<]*</code>' | grep -o 'https://[^ |<]*')"

desktop="$updates/macos/$desktop_version/OpenAgents-$desktop_version.dmg
$updates/linux/$desktop_version/OpenAgents-$desktop_version-x86_64.AppImage
$updates/linux/$desktop_version/openagents_${desktop_version}_amd64.deb
$updates/windows/$desktop_version/OpenAgents-$desktop_version-x64.msi
$updates/windows/$desktop_version/OpenAgents-$desktop_version-windows-x64.zip"

failed=0
# status URL: the final HTTP status after redirects.
status() {
  local code
  code="$(curl -sIL -o /dev/null -w '%{http_code}' --max-time 30 "$1")"
  case "$code" in
    200) ;;
    *) code="$(curl -sL -o /dev/null -r 0-0 -w '%{http_code}' --max-time 30 "$1")"
       [ "$code" = 206 ] && code=200 ;;
  esac
  echo "$code"
}

check() {
  local heading="$1" list="$2" url code
  echo "$heading"
  if [ -z "$list" ]; then echo "  (none)"; return; fi
  while IFS= read -r url; do
    [ -n "$url" ] || continue
    code="$(status "$url")"
    # A TestFlight link answers 200 even when it takes nobody.
    if [ "$code" = 200 ] && case "$url" in https://testflight.apple.com/join/*) true ;; *) false ;; esac \
      && curl -sL --max-time 30 "$url" | grep -qi "not accepting any new testers\|isn&#39;t accepting any new testers\|isn't accepting any new testers\|This beta is full"; then
      code="200, but the beta is not taking new testers"
    fi
    if [ "$code" = 200 ]; then
      echo "  ok   $code $url"
    else
      echo "  FAIL $code $url"
      failed=$((failed + 1))
    fi
  done <<<"$(printf '%s\n' "$list" | awk '!seen[$0]++')"
}

check "On $site/download:" "$links
$commands"
check "Desktop $desktop_version files (shown on the page once released):" "$desktop"

echo "Desktop update manifests:"
for os in macos linux windows; do
  # The signed envelope's payload is base64 JSON naming the version.
  version="$(curl -fsL --max-time 30 "$updates/$os/manifest.json" 2>/dev/null | python3 -c '
import base64, json, sys
try:
    m = json.load(sys.stdin)
    p = json.loads(base64.b64decode(m["payload"]))
    print(p.get("version", "?"), "published", p.get("published", "?"))
except Exception:
    pass
')"
  echo "  $os: ${version:-no manifest}"
done

if [ "$failed" -gt 0 ]; then
  echo "$failed link(s) failed."
  exit 1
fi
echo "Every link answered 200."
