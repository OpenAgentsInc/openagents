#!/bin/sh
# Records the V1 launch film's screen footage (#11432) from the real
# product, running locally: docs/verse/film/cinematic.md.
#
#   scripts/film/footage/record-all.sh [--no-build]
#
# Unless --no-build, the stack's binaries and Coder are rebuilt first, even
# when the stack already runs, and a running stack whose binaries are now
# newer than it is restarted. FILM_IOS=0 skips the iPhone app shot (it
# builds bins/openagents-ios for a simulator, the first time a while).
#
# What runs: the whole 1.0 web stack on this Mac (scripts/dev/full-local.sh:
# the gateway as account service and inference gateway, the chat worker,
# the website on 127.0.0.1:4301), in its own state folder so your other
# local stack is untouched. A fresh test account is made through the
# account service's own sign-up (no GitHub), and then:
#
#   1. a few real chats on the website, so the sidebar isn't empty;
#   2. web-chat.mp4       a question typed on the web, the real answer;
#   3. phone-approve.mp4  `coder login` in a real terminal, approved from a
#                         phone-sized browser at /device;
#   4. terminal-coder.mp4 Coder fixing a real repository's failing tests,
#      web-sync.mp4       and the same chat appearing on the website;
#   5. phone-app.mp4      the OpenAgents iPhone app (a simulator debug
#                         build) on Running while Coder works the same task:
#                         the card appears, works, and turns to Done.
#
# Model answers are real (the chat worker and Coder call real models
# through the local gateway). Long waits are shortened in the edit; nothing
# on screen is typed in after the fact.
#
# Output: $FILM_OUT (default ~/work/film-out/footage). Scratch: $FILM_WORK
# (default ~/work/film-out/.work/<time>). Needs Google Chrome, ffmpeg,
# node 22+, python3, and the keys full-local.sh reads from ~/work/.secrets.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
out=${FILM_OUT:-$HOME/work/film-out/footage}
state=${FILM_LOCAL:-$HOME/.openagents/film-local}
work=${FILM_WORK:-$HOME/work/film-out/.work/$(date +%Y%m%d-%H%M%S)}
cache=${FILM_NODE_CACHE:-$HOME/work/film-out/.cache/node}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/work/openagents-target-film}
export FILM_NODE_CACHE="$cache"

mkdir -p "$out" "$work"
chmod 700 "$work"

# xterm.js for the terminal (headless, to answer the session's terminal
# queries; and the browser build, to draw it). Not in git.
if [ ! -f "$cache/node_modules/@xterm/xterm/lib/xterm.js" ] || [ ! -d "$cache/node_modules/@xterm/headless" ]; then
    mkdir -p "$cache"
    (cd "$cache" && { [ -f package.json ] || npm init -y > /dev/null; } \
        && npm install --silent @xterm/xterm@5.5.0 @xterm/headless@5.5.0)
fi

build=1
for arg in "$@"; do [ "$arg" = --no-build ] && build=0; done
stack() { FULL_LOCAL="$state" FULL_LOCAL_ENVIRONMENTS=0 "$root/scripts/dev/full-local.sh" "$@"; }
binaries="gateway coder-worker openagents-web coder-new"
if [ "$build" = 1 ]; then
    # The same binaries full-local.sh builds, built now even when the stack
    # runs, so a recording never uses an old Coder or site.
    echo "Building the stack and Coder into $CARGO_TARGET_DIR ..."
    cargo build -q --manifest-path "$root/Cargo.toml" -p gateway --bin gateway -p coder --bin coder-worker \
        -p openagents-web --bin openagents-web -p coder-new --bin coder-new \
        -p tenancy --example bootstrap_registry
    # A running stack started before these binaries were built is stale.
    if [ -f "$state/run/web.pid" ] && kill -0 "$(cat "$state/run/web.pid")" 2>/dev/null; then
        for bin in $binaries; do
            if [ "$CARGO_TARGET_DIR/debug/$bin" -nt "$state/run/web.pid" ]; then
                echo "The running stack is older than $bin; restarting it."
                stack stop
                break
            fi
        done
    fi
fi

# The local stack, in its own state folder (built above, so no build here).
stack start --no-build
# shellcheck disable=SC1091
. "$state/run/ports"
base="http://127.0.0.1:$web_port"

# A fresh test account (the account service's own sign-up). Its session
# token goes to a private file and is never printed.
(umask 077 && curl -fsS -X POST "http://127.0.0.1:$gateway_port/v1/accounts" \
    -H 'content-type: application/json' -d "{\"label\":\"${FILM_ACCOUNT:-Alex}\"}" \
    | python3 -c 'import json, sys; print(json.load(sys.stdin)["session_token"])' > "$work/session")
mkdir -p "$work/coder-state" "$work/home/code"
chmod 700 "$work/coder-state"

export FILM_BASE="$base" FILM_SESSION_FILE="$work/session" FILM_OUT="$out" FILM_WORK="$work"
export FILM_CODER="$CARGO_TARGET_DIR/debug/coder-new" FILM_CODER_STATE="$work/coder-state"
export FILM_HOME="$work/home" FILM_REPO="$work/home/code/slugkit"

echo "Seeding the sidebar with real chats..."
node --input-type=module -e "
import { seed } from '$here/web.mjs';
await seed(process.env.FILM_BASE, process.env.FILM_SESSION_FILE, [
  'What is OpenAgents?',
  'How do I connect my codebase?',
  'What can Coder do on my computer?',
]);"

echo "Recording web-chat.mp4..."
node "$here/web-chat.mjs"

echo "Recording phone-approve.mp4..."
node "$here/phone-approve.mjs"

echo "Recording terminal-coder.mp4 and web-sync.mp4..."
"$here/make-demo-repo.sh" "$FILM_REPO"
node "$here/coder-session.mjs"

if [ "${FILM_IOS:-1}" != 0 ]; then
    echo "Recording phone-app.mp4 (the iPhone app)..."
    # One simulator of its own, named plainly, kept between runs.
    sim_file="$HOME/work/film-out/.film-sim-udid"
    sim=$(cat "$sim_file" 2>/dev/null || true)
    if [ -z "$sim" ] || ! xcrun simctl list devices | grep -q "$sim"; then
        sim=$(xcrun simctl create iPhone com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro)
        echo "$sim" > "$sim_file"
    fi
    xcrun simctl boot "$sim" 2>/dev/null || true
    if [ "$build" = 1 ] || [ ! -f "$CARGO_TARGET_DIR/aarch64-apple-ios-sim/debug/libopenagents_mobile.a" ]; then
        OPENAGENTS_IOS_DEVICE="$sim" "$root/bins/openagents-ios/build.sh" sim > "$work/ios-build.log" 2>&1 \
            || { tail -20 "$work/ios-build.log" >&2; exit 1; }
    fi
    FILM_SIM="$sim" FILM_GATEWAY="http://127.0.0.1:$gateway_port" node "$here/phone-app.mjs"
fi

for clip in web-chat web-sync phone-approve terminal-coder phone-app; do
    [ -f "$out/$clip.mp4" ] || continue
    ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate:format=duration \
        -of compact=p=0:nk=1 "$out/$clip.mp4" | tr '\n' ' ' | sed "s|^|$clip.mp4: |"
    echo
done
echo "Footage in $out (scratch in $work)."
