#!/usr/bin/env bash
# Move a never-released desktop upload aside before the real release.
#
# On 2026-09-30 a 1.0.0 build was uploaded to
# gs://openagentsgemini-oa-updates/desktop/{macos,linux}/1.0.0/ (and each
# platform's manifest.json pointed at it), but 1.0.0 was never released.
# The real 1.0.0 goes to the same folders (#11092). This script copies
# each platform's VERSION folder, and the manifest.json beside it, to
# desktop/<os>/LABEL/ (the manifest as previous-manifest.json), checks
# every copy against its original's CRC32C, and only then deletes the
# originals. Run it on release day, right before the publish steps in
# docs/desktop/release.md (Release day): between the delete and the new
# manifest, an installed app's update check finds no update to fetch and
# tries again later.
#
# By default it only prints the commands. Nothing runs without --run.
#
#   scripts/desktop/retire-prerelease.sh                 # print the plan
#   scripts/desktop/retire-prerelease.sh --run           # do it
#
# Options:
#   --version V   The never-released version (default 1.0.0).
#   --label L     Where it goes, desktop/<os>/L/ (default V-rc.0).
#   --os LIST     Platforms, comma-separated (default macos,linux,windows).
#   --run         Run the commands instead of printing them.
#
# gcloud must be signed in with write access to the bucket, for example
# CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config. A platform with
# nothing under desktop/<os>/V/ is skipped (Windows had no 1.0.0 upload;
# its test build is under desktop/windows-test/, which this never touches).
set -euo pipefail

bucket="openagentsgemini-oa-updates"
version="1.0.0"
label=""
platforms="macos,linux,windows"
run=0

while [ $# -gt 0 ]; do
  case "$1" in
    --version) version="${2:?--version needs a value}"; shift 2 ;;
    --label) label="${2:?--label needs a value}"; shift 2 ;;
    --os) platforms="${2:?--os needs a list}"; shift 2 ;;
    --run) run=1; shift ;;
    --dry-run) run=0; shift ;;
    -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
    *) echo "retire-prerelease: unknown option $1" >&2; exit 64 ;;
  esac
done
label="${label:-$version-rc.0}"
case "$label" in "$version"|"") echo "retire-prerelease: the label must differ from the version" >&2; exit 64 ;; esac

# Prints a command; with --run, also runs it.
step() {
  printf '+ %s\n' "$*"
  if [ "$run" = 1 ]; then "$@"; fi
}

# name<TAB>crc32c for every object under a folder, sorted by name.
listing() {
  gcloud storage objects list "gs://$bucket/$1/**" --format="value(name,crc32c_hash)" \
    | sed "s|^$1/||" | sort
}

[ "$run" = 1 ] || echo "# Dry run: printing the commands only. Pass --run to run them."
retired=0
IFS=',' read -r -a list <<<"$platforms"
for os in "${list[@]}"; do
  from="desktop/$os/$version"
  to="desktop/$os/$label"
  echo "# $os: gs://$bucket/$from/ -> gs://$bucket/$to/"
  if ! gcloud storage ls "gs://$bucket/$from/" >/dev/null 2>&1; then
    echo "#   nothing under $from; skipped"
    continue
  fi
  if gcloud storage ls "gs://$bucket/$to/" >/dev/null 2>&1; then
    echo "retire-prerelease: gs://$bucket/$to/ already exists; check it, then remove it or pick another --label" >&2
    exit 1
  fi
  step gcloud storage cp --no-clobber --recursive "gs://$bucket/$from/*" "gs://$bucket/$to/"
  if gcloud storage ls "gs://$bucket/desktop/$os/manifest.json" >/dev/null 2>&1; then
    step gcloud storage cp --no-clobber "gs://$bucket/desktop/$os/manifest.json" \
      "gs://$bucket/$to/previous-manifest.json"
  fi
  if [ "$run" = 1 ]; then
    want="$(listing "$from")"
    got="$(listing "$to" | grep -v '^previous-manifest.json' || true)"
    if [ -z "$want" ] || [ "$want" != "$got" ]; then
      echo "retire-prerelease: the copy in $to does not match $from; nothing deleted" >&2
      diff <(echo "$want") <(echo "$got") >&2 || true
      exit 1
    fi
    echo "#   copy checked: $(echo "$want" | wc -l | tr -d ' ') file(s), same CRC32C"
  else
    echo "#   (with --run: compare every copied file's CRC32C with its original; stop on any difference)"
  fi
  step gcloud storage rm --recursive "gs://$bucket/$from/"
  retired=$((retired + 1))
done

if [ "$run" = 1 ]; then echo "# $retired platform(s) moved to $label."; else echo "# $retired platform(s) would move to $label."; fi
echo "# Next: publish the real $version (docs/desktop/release.md, Release day)."
