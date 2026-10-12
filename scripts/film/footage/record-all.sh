#!/bin/sh
# Records the V1 launch film's screen footage (#11432) from the real
# product, running locally: docs/verse/film/cinematic.md.
#
#   scripts/film/footage/record-all.sh [--no-build]
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
#      web-sync.mp4       and the same chat appearing on the website.
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

# The local stack, in its own state folder.
FULL_LOCAL="$state" FULL_LOCAL_ENVIRONMENTS=0 "$root/scripts/dev/full-local.sh" start "$@"
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

for clip in web-chat web-sync phone-approve terminal-coder; do
    ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate:format=duration \
        -of compact=p=0:nk=1 "$out/$clip.mp4" | tr '\n' ' ' | sed "s|^|$clip.mp4: |"
    echo
done
echo "Footage in $out (scratch in $work)."
