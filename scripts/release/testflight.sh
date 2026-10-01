#!/usr/bin/env bash
# Ship the OpenAgents iOS app to TestFlight from this checkout (#10118):
# the build number, the signed archive, App Store Connect's validation or
# the upload, and processing, with a few plain progress lines. Coder runs
# it when asked from the phone; it works the same from a terminal.
#
#   scripts/release/testflight.sh start [--validate-only] [--build N]
#       Starts the release in the background and returns at once.
#   scripts/release/testflight.sh wait [--minutes M]
#       Prints the progress, waiting up to M minutes (default 4, under
#       Coder's five-minute limit on one command) for the release to end.
#       Exit 0: it finished. 1: it failed (the reason is the last line).
#       3: still running, with where it is; run wait again.
#   scripts/release/testflight.sh run [--validate-only] [--build N]
#       The whole release in the foreground (20 minutes or more).
#
# --validate-only archives and validates with App Store Connect without
# uploading anything, under the next unused build number (a dry run).
# Without it the release uploads build N (default: CURRENT_PROJECT_VERSION
# in bins/openagents-ios/host/project.yml, which must be clean, committed,
# and higher than every build App Store Connect has for the version), then
# waits until App Store Connect says it is VALID and in Internal Testers.
#
# The App Store Connect key: ASC_API_KEY_ID, ASC_API_ISSUER_ID, and
# ASC_API_PRIVATE_KEY_PATH, or the env file OPENAGENTS_ASC_ENV names
# (default ~/work/.secrets/appstoreconnect.env). It is never printed.
# Signing uses the login keychain's Apple Distribution identity and the
# "OpenAgents App Store" profile. Each release keeps its archive, logs,
# and progress under OPENAGENTS_SHIP_DIR (default
# $CARGO_TARGET_DIR/openagents-ios-ship).
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
self="$root/scripts/release/testflight.sh"
# A host started by launchd has a short PATH; the build needs cargo,
# xcodegen, and Xcode's tools.
for dir in "$HOME/.cargo/bin" /opt/homebrew/bin /usr/local/bin; do
  case ":$PATH:" in *":$dir:"*) ;; *) [ -d "$dir" ] && PATH="$PATH:$dir" ;; esac
done
export PATH
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}"
[[ "$CARGO_TARGET_DIR" == /* ]] || CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
ship_root="${OPENAGENTS_SHIP_DIR:-$CARGO_TARGET_DIR/openagents-ios-ship}"
# One release at a time per checkout.
key="$(printf '%s' "$root" | shasum | cut -c1-12)"
state="$ship_root/$key"
project="$root/bins/openagents-ios/host/project.yml"

die() { echo "$*" >&2; exit 64; }
say() { echo "$*" | tee -a "$state/progress.log"; }

command="${1:-}"
[ $# -gt 0 ] && shift
validate_only=0
build=""
minutes=4
while [ $# -gt 0 ]; do
  case "$1" in
    --validate-only) validate_only=1; shift ;;
    --build) build="${2:?--build needs a number}"; shift 2 ;;
    --minutes) minutes="${2:?--minutes needs a number}"; shift 2 ;;
    *) die "unknown option $1 (see the top of $self)" ;;
  esac
done

running() {
  [ -f "$state/pid" ] && kill -0 "$(cat "$state/pid")" 2>/dev/null && [ "$(cat "$state/status" 2>/dev/null)" = running ]
}

load_key() {
  if [ -z "${ASC_API_KEY_ID:-}" ] || [ -z "${ASC_API_ISSUER_ID:-}" ] || [ -z "${ASC_API_PRIVATE_KEY_PATH:-}" ]; then
    local env_file="${OPENAGENTS_ASC_ENV:-$HOME/work/.secrets/appstoreconnect.env}"
    [ -f "$env_file" ] || { echo "No App Store Connect key: set OPENAGENTS_ASC_ENV or the ASC_API_* variables."; return 1; }
    set -a
    # shellcheck disable=SC1090
    . "$env_file" >/dev/null 2>&1
    set +a
  fi
  [ -n "${ASC_API_PRIVATE_KEY_PATH:-}" ] && [ -f "$ASC_API_PRIVATE_KEY_PATH" ] \
    || { echo "The App Store Connect key file is missing."; return 1; }
  export ASC_API_KEY_ID ASC_API_ISSUER_ID ASC_API_PRIVATE_KEY_PATH
}

setting() { sed -n "s/^ *$1: *//p" "$project" | head -1 | tr -d '"'; }

# The reason a step failed, from its log: the first error line.
reason() {
  local line
  line="$(grep -E -m1 '(^|[^a-z])(error|ERROR)[: ]|\*\* ARCHIVE FAILED|EXPORT FAILED|Validation failed|failed with' "$1" 2>/dev/null | cut -c1-240)"
  [ -n "$line" ] || line="$(tail -1 "$1" 2>/dev/null | cut -c1-240)"
  echo "$line"
}

fail() { # step log
  say "Failed while $1: $(reason "$2") (log: $2)"
  echo failed > "$state/status"
  exit 1
}

release() {
  mkdir -p "$state"
  : > "$state/progress.log"
  echo running > "$state/status"
  echo $$ > "$state/pid"
  local started version built next
  started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  load_key > "$state/key.log" 2>&1 || fail "reading the App Store Connect key" "$state/key.log"
  version="$(setting MARKETING_VERSION)"
  built="$(setting CURRENT_PROJECT_VERSION)"
  [ -n "$version" ] && [ -n "$built" ] || { echo "no version in $project" > "$state/setup.log"; fail "reading the project" "$state/setup.log"; }
  /usr/bin/python3 "$root/scripts/release/asc.py" next-build --version "$version" --at-least "$built" \
    > "$state/next.txt" 2> "$state/asc.log" || fail "reading App Store Connect" "$state/asc.log"
  next="$(cat "$state/next.txt")"
  if [ "$validate_only" = 1 ]; then
    build="${build:-$next}"
  else
    build="${build:-$built}"
    if [ "$build" != "$built" ]; then
      echo "project.yml builds $built, not $build: raise CURRENT_PROJECT_VERSION to $build and commit it first" > "$state/setup.log"
      fail "checking the build number" "$state/setup.log"
    fi
    if [ "$build" -lt "$next" ]; then
      echo "App Store Connect already has build $((next - 1)) for $version: raise CURRENT_PROJECT_VERSION to $next" > "$state/setup.log"
      fail "checking the build number" "$state/setup.log"
    fi
    if [ -n "$(git -C "$root" status --porcelain=v1 --untracked-files=no)" ]; then
      git -C "$root" status --short --untracked-files=no > "$state/setup.log"
      echo "the checkout has uncommitted changes: commit them first" >> "$state/setup.log"
      fail "checking the checkout" "$state/setup.log"
    fi
  fi
  echo "$build" > "$state/build"
  git -C "$root" rev-parse HEAD > "$state/commit"
  local output="$state/ios"
  say "Archiving build $build of $version from $(cut -c1-10 "$state/commit"). This takes a while."
  OPENAGENTS_IOS_BUILD_NUMBER="$build" OPENAGENTS_IOS_OUTPUT="$output" \
    "$root/bins/openagents-ios/build.sh" archive > "$state/archive.log" 2>&1 \
    || fail "archiving" "$state/archive.log"
  if [ "$validate_only" = 1 ]; then
    say "Archived. Validating with App Store Connect (nothing is uploaded)."
    OPENAGENTS_IOS_OUTPUT="$output" "$root/bins/openagents-ios/build.sh" validate > "$state/validate.log" 2>&1 \
      || fail "validating" "$state/validate.log"
    say "Done: build $build archived and validated. Nothing was uploaded."
  else
    say "Archived. Uploading build $build to TestFlight."
    OPENAGENTS_IOS_OUTPUT="$output" "$root/bins/openagents-ios/build.sh" upload > "$state/upload.log" 2>&1 \
      || fail "uploading" "$state/upload.log"
    say "Uploaded. Waiting for App Store Connect to process build $build."
    /usr/bin/python3 "$root/scripts/release/asc.py" wait-valid --build "$build" --version "$version" \
      --after "$started" --timeout 5400 > "$state/processing.log" 2>&1 \
      || fail "processing" "$state/processing.log"
    tail -1 "$state/processing.log" > "$state/result.json"
    say "Done: build $build is on TestFlight for Internal Testers."
  fi
  echo done > "$state/status"
}

# Where a running release is, in a few words, from its newest log.
where() {
  local log
  log="$(ls -t "$state"/archive.log "$state"/validate.log "$state"/upload.log "$state"/processing.log 2>/dev/null | head -1)"
  case "$log" in
    */archive.log)
      if grep -q 'ARCHIVE SUCCEEDED' "$log"; then echo "archived"
      elif grep -qE 'CompileSwift|SwiftCompile|SwiftDriver' "$log"; then echo "compiling the app"
      elif grep -q 'xcodegen\|Command line invocation' "$log"; then echo "starting Xcode"
      else echo "building the Rust library ($(grep -c 'Compiling ' "$log") crates compiled)"
      fi ;;
    */validate.log) echo "validating" ;;
    */upload.log) echo "uploading" ;;
    */processing.log) echo "App Store Connect: $(tail -1 "$log")" ;;
    *) echo "starting" ;;
  esac
}

case "$command" in
  run)
    running && die "A release is already running here; run: $self wait"
    release ;;
  start)
    running && die "A release is already running here; run: $self wait"
    mkdir -p "$state"
    : > "$state/progress.log"
    echo running > "$state/status"
    rm -f "$state/pid"
    args=(run)
    [ "$validate_only" = 1 ] && args+=(--validate-only)
    [ -n "$build" ] && args+=(--build "$build")
    # Its own session, so it outlives the shell that started it.
    nohup /usr/bin/python3 -c 'import os, sys; os.setsid(); os.execv(sys.argv[1], sys.argv[1:])' \
      "$self" "${args[@]}" </dev/null > "$state/job.log" 2>&1 &
    for _ in $(seq 1 20); do [ -s "$state/progress.log" ] || [ -f "$state/pid" ] && break; sleep 0.5; done
    echo "Started. Run: $self wait" ;;
  wait)
    [ -d "$state" ] || die "No release has started here."
    deadline=$(( $(date +%s) + minutes * 60 ))
    while running && [ "$(date +%s)" -lt "$deadline" ]; do sleep 10; done
    cat "$state/progress.log"
    case "$(cat "$state/status" 2>/dev/null)" in
      done) exit 0 ;;
      failed) exit 1 ;;
      running)
        if running || [ ! -f "$state/pid" ]; then
          echo "Still running: $(where), $(( ( $(date +%s) - $(stat -f %m "$state/status") ) / 60 )) min so far. Run: $self wait"
          exit 3
        fi
        echo "The release stopped without finishing (see $state/job.log)."
        exit 1 ;;
      *) exit 1 ;;
    esac ;;
  *) die "usage: $self start|wait|run [--validate-only] [--build N] [--minutes M]" ;;
esac
