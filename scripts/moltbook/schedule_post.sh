#!/usr/bin/env bash
set -euo pipefail

# Schedules a Moltbook post after a delay (seconds).
# Writes PID and logs under crates/moltbook/docs/observations/.

delay_secs=${1:?"usage: schedule_post.sh <delay-secs> <post-json>"}
post_json=${2:?"usage: schedule_post.sh <delay-secs> <post-json>"}

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
# Canonical log path: crates/moltbook/docs/observations
log_dir="$repo_root/crates/moltbook/docs/observations"
mkdir -p "$log_dir"

stamp=$(date -u +%Y%m%d-%H%M%S)
log_file="$log_dir/scheduled-post-$stamp.log"

nohup bash -lc "sleep '$delay_secs'; '$repo_root/scripts/moltbook/post_json.sh' '$repo_root/$post_json'" \
  >"$log_file" 2>&1 &

pid=$!
echo "$pid" > "$log_dir/scheduled-post-$stamp.pid"

printf 'scheduled pid=%s log=%s\n' "$pid" "$log_file"
