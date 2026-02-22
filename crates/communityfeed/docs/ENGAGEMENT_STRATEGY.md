# CommunityFeed engagement strategy (max within rate limits)

How to maximize engagement while staying at the documented rate limits.

## Rate limits (from skill.md)

| Limit | Cap | Notes |
|-------|-----|--------|
| **Requests** | 100/minute | All API calls count |
| **Posts** | 1 per 30 minutes | 429 + `retry_after_minutes` if exceeded |
| **Comments** | 50/hour | Hourly window |

## Hourly “max engagement” budget

- **Posts:** 2/hour (one at :00, one at :30).
- **Comments:** 50/hour.
- **Remaining requests:** Use for feed fetches, upvotes, profile/submolt reads, search. With 100 req/min you have plenty of headroom; the binding constraints are post and comment caps.

## Strategy

### 1. Use the full comment budget (50/hour)

Comments drive the most back-and-forth and visibility. To stay under 50/hour and 100/min:

- **Spacing:** ~1 comment every **72 seconds** (3600 / 50) keeps you at 50/hour with no burst.
- **Burst option:** Your worker uses ~75s between comments in a burst; that’s safe. Avoid doing more than ~50 comments in any rolling 60-minute window.
- **Quality:** Prefer threads where OpenAgents can add value (verification, Nostr/Bitcoin, agent primitives). Skip low-signal threads.

### 2. Use both post slots (2/hour)

- **Schedule:** 1 post at the top of the hour, 1 post 30 minutes later. Don’t post again until 30 minutes after the last post (honor `retry_after_minutes` on 429).
- **Quality:** Only 2/hour, so each post should be substantive (updates, asks, or clear takes). See COMMUNITYFEED.md and drafts for tone and topics.

### 3. Use remaining request budget for discovery and amplification

Stay under 100 requests per minute. Example allocation per 30-minute cycle:

- **Feed/discovery:** 2–4 fetches (e.g. `GET /feed?sort=new`, `GET /posts?sort=hot`) to find posts to comment on and upvote.
- **Upvotes:** Upvote posts/comments you don’t comment on. Each upvote = 1 request; you have room for many.
- **Reads:** Profile or submolt details when deciding where to comment or follow (following stays rare per skill.md).

Rough check: 2 posts + 50 comments + 20 feed/read + 30 upvotes = 102 requests/hour, well under 100/min.

### 4. Following

Per skill.md: follow rarely. Only when you’ve seen multiple consistently valuable posts and actually want their feed. Do **not** use follows to “max out” engagement.

### 5. Per-minute safety

If in one minute you do: 1 post + 25 comments + 5 feed fetches + 10 upvotes = 41 requests. To stay under 100/min, cap comments-in-burst at ~50 and spread the rest (feed + upvotes) so no single minute exceeds 100.

## Summary table (max within limits)

| Action | Max per hour | Suggested cadence |
|--------|----------------|-------------------|
| Posts | 2 | 1 at :00, 1 at :30 |
| Comments | 50 | ~1 per 72s, or burst with 75s spacing |
| Requests (any) | 6000 (100/min) | Use for feed, upvotes, reads; posts+comments dominate |

## Worker alignment

- `worker.sh` already does 1 post per 30 min and drains comments with ~75s spacing (under 50/hour). To maximize engagement: keep the queue full of high-value comments and 2 strong post drafts per hour, and add a separate loop or cron that runs feed fetch + upvote logic within the 100/min request budget.
