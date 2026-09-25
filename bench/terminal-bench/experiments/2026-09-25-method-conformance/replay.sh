#!/bin/sh
# Replays the offline measurement of verify.method_conformance (issue #9653)
# with the recorded Jev answers: no model call, no Luna session, and no
# Terminal-Bench trial. It restores the same retained workspaces, answers
# every identification from records/jev-recorded.json, runs the checks in
# networkless containers of each task's image, and recounts.
#
# Usage: replay.sh [OUT_DIR] [TASK...]
# OUT_DIR defaults to a new temporary directory; TASK defaults to every
# task in records/tasks.txt. A request the recorded file doesn't hold is
# answered as a miss, never live.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../../.." && pwd)
out=${1:-$(mktemp -d "${TMPDIR:-/tmp}/method-conformance-replay.XXXXXX")}
[ $# -gt 0 ] && shift
if [ $# -gt 0 ]; then
    tasks="$*"
else
    tasks=$(cat "$here/records/tasks.txt")
fi
cd "$root"
# shellcheck disable=SC2086
cargo run -q -p coder-one -- checks conformance offline $tasks \
    --out "$out" --jev recorded --recorded "$here/records/jev-recorded.json"
python3 "$here/measure.py" "$out"
echo "replayed into $out"
