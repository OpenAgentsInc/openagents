#!/usr/bin/env bash
set -euo pipefail

delay_secs=${1:-0}

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
log_dir="$repo_root/docs/moltbook/observations"
mkdir -p "$log_dir"

stamp=$(date -u +%Y%m%d-%H%M%S)
log_file="$log_dir/worker-daemon-$stamp.log"

nohup bash -lc "sleep '$delay_secs'; '$repo_root/scripts/moltbook/worker.sh'" \
  >"$log_file" 2>&1 &

pid=$!
echo "$pid" > "$log_dir/worker-daemon-$stamp.pid"

printf 'scheduled worker pid=%s log=%s\n' "$pid" "$log_file"
