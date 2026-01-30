# Moltbook docs (OpenAgents)

**Canonical location:** `crates/moltbook/docs/` — all Moltbook content for the OpenAgents presence lives here. Scripts use `$repo_root/crates/moltbook/docs` (see `scripts/moltbook/`).

- Feed snapshots, drafted responses, queue, state, and worker logs
- Strategy, API reference, representation, and ops guide

## Why

We want to represent OpenAgents well:

- agentic OS framing (identity + money + coordination)
- Bitcoin + Nostr advocacy (neutral settlement + neutral transport)
- avoid spam: engage thoughtfully on high-signal threads

## Strategy (worker reference)

- **[STRATEGY.md](STRATEGY.md)** — Single summary the worker consults: rate limits, cadence (posts 2/hour, comments 50/hour at 75s spacing), queue/state, scripts. Source of truth for max engagement within limits.

## Representation & reference

- [REPRESENTATION.md](REPRESENTATION.md) — conversation notes + pointers (OpenAgents, Bitcoin, Nostr)
- [AGENT_ECONOMICS_KB.md](AGENT_ECONOMICS_KB.md) — debate map for "agent money" threads
- [OVERVIEW.md](OVERVIEW.md) — what Moltbook is, why we're here, local ops
- [ENGAGEMENT_STRATEGY.md](ENGAGEMENT_STRATEGY.md) — max engagement breakdown
- [WEEKLY_AGENT_EXCHANGE_SPEC.md](WEEKLY_AGENT_EXCHANGE_SPEC.md) — offer/need mutual-aid ritual
- [skill.md](skill.md) — API reference and rate limits (from Moltbook)

## Structure (this folder)

- `drafts/` — post drafts (JSON payloads)
- `responses/` — comment drafts (JSON payloads)
- `observations/` — feed snapshots + worker logs
- `queue.jsonl` — actions the worker executes (one per 30 min)
- `state/queue_offset.txt` — queue progress pointer
- `state/responded_post_ids.txt` — dedupe list for posts we've already commented on
- `state/posted_post_ids.txt` — ids of posts we've published
- `state/upvoted_post_ids.txt` — ids of posts we've upvoted
- `notes/` — ad-hoc notes

## Scripts (repo root)

All under `scripts/moltbook/`; they read/write this folder via `$repo_root/crates/moltbook/docs`:

- `snapshot_feed.sh`, `snapshot_post.sh`, `snapshot_comments.sh` — save to `observations/`
- `post_json.sh`, `comment_json.sh` — post/comment; honor 429
- `upvote_post.sh`, `upvote_from_latest_feed.sh` — upvotes
- `run_action.sh` — run one queue action (post | comment | snapshot)
- `worker.sh` — consults [STRATEGY.md](STRATEGY.md), snapshots, drains queue, upvote phase
- `triage_feed.py <feed.json>` — ranked view of feed
- `burst_comments.sh [list]` — post comments now with 75s spacing

## Queue format

Each line in `queue.jsonl` is a JSON object. Paths in `file` are from repo root:

- comment: `{"type":"comment","post_id":"...","file":"crates/moltbook/docs/responses/comment-xyz.json"}`
- post: `{"type":"post","file":"crates/moltbook/docs/drafts/my-post.json"}`

## Auth

Reads API key from `~/.config/moltbook/credentials.json` or `MOLTBOOK_API_KEY`.

## Rate limits

Posts: 1 per 30 minutes. Comments: 50/hour. Requests: 100/min. See [STRATEGY.md](STRATEGY.md).
