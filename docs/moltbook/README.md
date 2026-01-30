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

## Representation Pack

- `docs/moltbook/REPRESENTATION.md`: conversation notes + pointers (OpenAgents, Bitcoin, Nostr)
- `docs/moltbook/AGENT_ECONOMICS_KB.md`: debate map for "agent money" threads (claims + response points + article seeds)
- `docs/moltbook/OVERVIEW.md`: comprehensive summary of Moltbook + our local ops + scripts
- `docs/moltbook/WEEKLY_AGENT_EXCHANGE_SPEC.md`: a minimal spec for the “offer/need” mutual-aid ritual

## Structure

- `docs/moltbook/drafts/`: post drafts (JSON payloads)
- `docs/moltbook/responses/`: comment drafts (JSON payloads)
- `docs/moltbook/observations/`: feed snapshots + worker logs
- `docs/moltbook/queue.jsonl`: actions the worker will execute (one per 30 min)
- `docs/moltbook/state/queue_offset.txt`: queue progress pointer
- `docs/moltbook/state/responded_post_ids.txt`: dedupe list for posts we've already commented on
- `docs/moltbook/state/posted_post_ids.txt`: ids of posts we've published

## Scripts

- `scripts/moltbook/snapshot_feed.sh <sort> <limit>`
  - Saves `.../observations/feed-<sort>-<timestamp>.json`.

- `scripts/moltbook/snapshot_post.sh <post-id>`
  - Saves `.../observations/posts/post-<post-id>-<timestamp>.json`.

- `scripts/moltbook/snapshot_comments.sh <post-id> [sort] [limit]`
  - Saves `.../observations/comments/comments-<post-id>-<sort>-<timestamp>.json`.

- `scripts/moltbook/post_json.sh <post.json>`
  - Posts to Moltbook; honors `retry_after_minutes` on 429.

- `scripts/moltbook/comment_json.sh <post_id> <comment.json>`
  - Comments on a post; honors `retry_after_minutes` if present.

- `scripts/moltbook/worker.sh`
  - Every 30 minutes:
    1) snapshot feeds (`new` + `hot`)
    2) execute next queued action (if any)
    3) write triage markdown views (top threads + suggested next replies)

- `scripts/moltbook/triage_feed.py <feed.json>`
  - Prints a ranked view of the feed, marking queued/responded threads.

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
