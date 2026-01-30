#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
queue_file="$repo_root/docs/moltbook/queue.jsonl"
state_dir="$repo_root/docs/moltbook/state"
log_dir="$repo_root/docs/moltbook/observations"
mkdir -p "$state_dir" "$log_dir"

offset_file="$state_dir/queue_offset.txt"
if [[ ! -f "$offset_file" ]]; then
  echo 0 > "$offset_file"
fi

log_file="$log_dir/worker.log"

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] worker start" >> "$log_file"

while true; do
  # Always snapshot the feed so we have context for what was happening.
  if out_path=$("$repo_root/scripts/moltbook/snapshot_feed.sh" new 25 2>/dev/null); then
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] snapshot: $out_path" >> "$log_file"
  else
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] snapshot: failed" >> "$log_file"
  fi

  offset=$(cat "$offset_file" || echo 0)
  next_line=$((offset + 1))

  if [[ -f "$queue_file" ]]; then
    line=$(sed -n "${next_line}p" "$queue_file" || true)
  else
    line=""
  fi

  if [[ -n "$line" ]]; then
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] action line $next_line" >> "$log_file"
    if echo "$line" | "$repo_root/scripts/moltbook/run_action.sh" >> "$log_file" 2>&1; then
      echo "$next_line" > "$offset_file"
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] action ok" >> "$log_file"
    else
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] action failed (will retry next cycle)" >> "$log_file"
    fi
  else
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] no queued action" >> "$log_file"
  fi

  # One cycle per 30 minutes to avoid post cooldown pressure and avoid spam.
  sleep 1800
done
