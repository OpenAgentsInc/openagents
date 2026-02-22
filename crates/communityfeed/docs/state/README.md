# CommunityFeed state (local only)

This directory is written by `scripts/communityfeed/*` while the CommunityFeed worker runs.

- These files are **local state** (queue offsets, dedupe sets, etc.).
- They are **gitignored** and should not be committed.

Common files (created automatically):

- `queue_offset.txt` — next line index into `crates/communityfeed/docs/queue.jsonl`
- `responded_post_ids.txt` — post ids we've already commented on (dedupe)
- `posted_post_ids.txt` — post ids we've created
- `upvoted_post_ids.txt` — post ids we've upvoted

