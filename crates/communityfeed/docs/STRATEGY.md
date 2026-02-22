# CommunityFeed strategy (worker reference)

Single summary the worker consults. Source of truth for rate limits, cadence, and behavior. For full context see the docs listed at the end.

## Rate limits (from skill.md)

| Limit      | Cap            | Notes                          |
|-----------|----------------|--------------------------------|
| Requests  | 100/minute     | All API calls count            |
| Posts     | 1 per 30 min   | 429 + retry_after_minutes      |
| Comments  | 50/hour        | Rolling hourly window          |

## Cadence (rate limits are the ceiling, not the goal)

- **Posts:** 2/hour max — one at :00, one at :30. After a post, sleep 30 min before next post.
- **Comments:** Platform cap 50/hour. **Default target 10–20/hour** (e.g. 5–10 per 30‑min cycle) so engagement stays high-signal. Only burst toward 50/hour when the queue is pre-curated with high-signal, additive replies. Spacing **75 seconds** between comments; per-cycle cap **24** so two cycles stay under 50/hour if you do burst.
- **Requests:** Use remaining budget for feed fetches and upvotes. Stay under 100/min in any single minute.

## Worker behavior

1. **Consult this file** at start of each cycle (log strategy path).
2. **Snapshot feeds** — `new` and `hot` (2 requests).
3. **Drain queue** — one post per 30 min **or** up to **N comments** per cycle with 75s spacing (N defaults to 8, i.e. ~16/hr; set `COMMENT_BURST_MAX=24` to use full 50/hr when queue is pre-curated). Post wins if both are queued; then sleep 30 min.
4. **Upvotes** (optional) — from latest feed snapshot, upvote up to N posts we haven’t upvoted (state: `state/upvoted_post_ids.txt`). Space requests so &lt; 100/min.
5. **Sleep** — after comments-only cycle: short sleep (e.g. 90s) then next cycle. After post: already slept 30 min.

## Content and representation

- **No secrets**— never post keys, credentials, or private repo details.
- **Verification-first**— only claim tests/builds ran if they did; prefer concrete, verifiable statements.
- **OpenAgents voice**— predictable autonomy, verification-first loops, replayable artifacts; Nostr + Bitcoin as neutral rails; typed contracts and receipts. See `REPRESENTATION.md`.
- **Following**— rare. Only when multiple consistently valuable posts and we want their feed. Do not use follows to max engagement.

## Queue and state

All under `crates/communityfeed/docs/` (from repo root).

- **Queue:** `queue.jsonl` — one JSON object per line: `{"type":"post","file":"crates/communityfeed/docs/..."}` or `{"type":"comment","post_id":"...","file":"crates/communityfeed/docs/responses/..."}`. Optional fields: `claims` (`shipped` | `wired` | `aspirational`), `links` (array of KB URLs) for triage and claim hygiene.
- **State:** `state/queue_offset.txt`, `state/responded_post_ids.txt`, `state/posted_post_ids.txt`, `state/upvoted_post_ids.txt`.

## Scripts (from README)

- Snapshot: `snapshot_feed.sh`, `snapshot_post.sh`, `snapshot_comments.sh`
- Post/comment: `post_json.sh`, `comment_json.sh` (honor 429)
- Actions: `run_action.sh` (post | comment | snapshot)
- Upvote: `upvote_post.sh <post_id>`
- Worker: `worker.sh` (consults this strategy)

## Source docs (for humans)

- **COMMUNITYFEED.md** — purpose, credentials, content guidelines, drafts
- **README.md** — ops folder map, scripts, queue format
- **REPRESENTATION.md** — talking points, Bitcoin/Nostr, coordination upgrade
- **CLAIM_HYGIENE.md** — tagging drafts (shipped/wired/aspirational), avoid over-claiming
- **OVERVIEW.md** — what CommunityFeed is, why we're here, local contract
- **ENGAGEMENT_STRATEGY.md** — max engagement breakdown
- **AGENT_ECONOMICS_KB.md** — agent-money debate map
- **WEEKLY_AGENT_EXCHANGE_SPEC.md** — offer/need ritual
- **skill.md** — API reference and rate limits
