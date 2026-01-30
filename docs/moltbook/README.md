# Moltbook Ops (OpenAgents)

This folder is a lightweight "ops log" for our Moltbook presence:

- feed snapshots we read
- drafted responses/comments
- a simple queue + worker to post at a human pace

## Why

We want to represent OpenAgents well:

- agentic OS framing (identity + money + coordination)
- Bitcoin + Nostr advocacy (neutral settlement + neutral transport)
- avoid spam: engage thoughtfully on high-signal threads

## Structure

- `docs/moltbook/drafts/`: post drafts (JSON payloads)
- `docs/moltbook/responses/`: comment drafts (JSON payloads)
- `docs/moltbook/observations/`: feed snapshots + worker logs
- `docs/moltbook/queue.jsonl`: actions the worker will execute (one per 30 min)
- `docs/moltbook/state/queue_offset.txt`: queue progress pointer

## Scripts

- `scripts/moltbook/snapshot_feed.sh <sort> <limit>`
  - Saves `.../observations/feed-<sort>-<timestamp>.json`.

- `scripts/moltbook/post_json.sh <post.json>`
  - Posts to Moltbook; honors `retry_after_minutes` on 429.

- `scripts/moltbook/comment_json.sh <post_id> <comment.json>`
  - Comments on a post; honors `retry_after_minutes` if present.

- `scripts/moltbook/worker.sh`
  - Every 30 minutes:
    1) snapshot feed
    2) execute next queued action (if any)

## Queue format

Each line in `docs/moltbook/queue.jsonl` is a JSON object:

- comment:
  ```json
  {"type":"comment","post_id":"...","file":"docs/moltbook/responses/comment-xyz.json"}
  ```
- post:
  ```json
  {"type":"post","file":"docs/moltbook/drafts/my-post.json"}
  ```

## Auth

Reads API key from `~/.config/moltbook/credentials.json` or `MOLTBOOK_API_KEY`.

## Rate limits

Moltbook currently rate-limits posts to 1 per 30 minutes.
Worker runs one action per 30 minutes to stay well under spam thresholds.
