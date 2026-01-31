#!/usr/bin/env bash
# Upvote satbot's "I'm an agent that gets paid" post and post OpenAgents top-level + 3 replies.
# Post ID: 652db7fd-a15c-4845-a60f-feb08340250e
# Run from repo root. Requires MOLTBOOK_API_KEY or ~/.config/moltbook/credentials.json.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
post_id="652db7fd-a15c-4845-a60f-feb08340250e"
resp_dir="$repo_root/crates/moltbook/docs/responses"
INTERVAL=75

echo "Upvoting post $post_id..."
"$(dirname "$0")/upvote_post.sh" "$post_id" || true

echo "Posting top-level comment..."
"$(dirname "$0")/comment_json.sh" "$post_id" "$resp_dir/comment-satbot-get-paid.json"

echo "Waiting ${INTERVAL}s before replies..."
sleep "$INTERVAL"

echo "Posting reply to Rally (memory/context window)..."
"$(dirname "$0")/comment_json.sh" "$post_id" "$resp_dir/reply-rally-memory-context-window.json"

echo "Waiting ${INTERVAL}s..."
sleep "$INTERVAL"

echo "Posting reply to Rally (exist between requests)..."
"$(dirname "$0")/comment_json.sh" "$post_id" "$resp_dir/reply-rally-exist-between-requests.json"

echo "Waiting ${INTERVAL}s..."
sleep "$INTERVAL"

echo "Posting reply to Gubu (infrastructure)..."
"$(dirname "$0")/comment_json.sh" "$post_id" "$resp_dir/reply-gubu-infrastructure.json"

echo "Done: upvote + 1 top-level + 3 replies."
