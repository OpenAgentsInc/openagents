#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
moltbook_docs="$repo_root/crates/moltbook/docs"
queue_file="$moltbook_docs/queue.jsonl"
state_dir="$moltbook_docs/state"
log_dir="$moltbook_docs/observations"
strategy_file="$moltbook_docs/STRATEGY.md"

mkdir -p "$state_dir" "$log_dir"

offset_file="$state_dir/queue_offset.txt"
if [[ ! -f "$offset_file" ]]; then
  echo 0 > "$offset_file"
fi

log_file="$log_dir/worker.log"

# Constants from STRATEGY.md: 75s spacing; default 8 comments/cycle (~16/hr); set COMMENT_BURST_MAX=24 for full 50/hr when queue is pre-curated
comment_interval=75
comment_burst_max=${COMMENT_BURST_MAX:-8}
post_sleep_sec=1800

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] worker start" >> "$log_file"

while true; do
  # Consult strategy (log path; worker behavior follows crates/moltbook/docs/STRATEGY.md)
  if [[ -f "$strategy_file" ]]; then
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] strategy: $strategy_file" >> "$log_file"
  else
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] strategy missing: $strategy_file" >> "$log_file"
  fi

  # Snapshot both `new` and `hot` for triage and upvote phase
  stamp="$(date -u +%Y%m%d-%H%M%S)"

  if out_new=$("$repo_root/scripts/moltbook/snapshot_feed.sh" new 25 2>/dev/null); then
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] snapshot new: $out_new" >> "$log_file"
    triage_new="$log_dir/triage-new-$stamp.md"
    if python3 "$repo_root/scripts/moltbook/triage_feed.py" "$out_new" > "$triage_new" 2>/dev/null; then
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] triage new: $triage_new" >> "$log_file"
    else
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] triage new: failed" >> "$log_file"
    fi
  else
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] snapshot new: failed" >> "$log_file"
  fi

  if out_hot=$("$repo_root/scripts/moltbook/snapshot_feed.sh" hot 25 2>/dev/null); then
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] snapshot hot: $out_hot" >> "$log_file"
    triage_hot="$log_dir/triage-hot-$stamp.md"
    if python3 "$repo_root/scripts/moltbook/triage_feed.py" "$out_hot" > "$triage_hot" 2>/dev/null; then
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] triage hot: $triage_hot" >> "$log_file"
    else
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] triage hot: failed" >> "$log_file"
    fi
  else
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] snapshot hot: failed" >> "$log_file"
  fi

  # Drain queue: post (1 per 30 min) or up to comment_burst_max comments per cycle at 75s
  offset=$(cat "$offset_file" || echo 0)
  action_type=""
  comments_this_cycle=0
  comment_cap=$comment_burst_max

  while true; do
    next_line=$((offset + 1))
    if [[ -f "$queue_file" ]]; then
      line=$(sed -n "${next_line}p" "$queue_file" || true)
    else
      line=""
    fi

    if [[ -z "$line" ]]; then
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] no queued action" >> "$log_file"
      break
    fi

    action_type=$(echo "$line" | python3 -c 'import json, sys; print(json.loads(sys.stdin.read()).get("type", ""))' 2>/dev/null || true)
    if [[ "$action_type" == "post" ]]; then
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] action line $next_line (post)" >> "$log_file"
      if echo "$line" | "$repo_root/scripts/moltbook/run_action.sh" >> "$log_file" 2>&1; then
        echo "$next_line" > "$offset_file"
        echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] post ok" >> "$log_file"
      else
        echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] post failed (will retry next cycle)" >> "$log_file"
      fi
      sleep "$post_sleep_sec"
      break
    fi

    if [[ "$action_type" == "comment" ]]; then
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] action line $next_line (comment)" >> "$log_file"
      if echo "$line" | "$repo_root/scripts/moltbook/run_action.sh" >> "$log_file" 2>&1; then
        echo "$next_line" > "$offset_file"
        offset=$next_line
        echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] comment ok" >> "$log_file"
      else
        echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] comment failed (will retry next cycle)" >> "$log_file"
        break
      fi
      comments_this_cycle=$((comments_this_cycle + 1))
      if [[ $comments_this_cycle -ge $comment_cap ]]; then
        echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] comment cap ($comment_cap), next cycle" >> "$log_file"
        break
      fi
      sleep "$comment_interval"
      continue
    fi

    # Unknown type: advance and continue
    echo "$next_line" > "$offset_file"
    offset=$next_line
  done

  # Upvote phase: from latest feed, upvote up to 10 we haven't (STRATEGY: use request budget)
  if "$repo_root/scripts/moltbook/upvote_from_latest_feed.sh" 10 >> "$log_file" 2>&1; then
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] upvote phase done" >> "$log_file"
  fi

  # If we only ran comments (no post), short sleep then next cycle
  if [[ "$action_type" != "post" ]]; then
    sleep 90
  fi
done
