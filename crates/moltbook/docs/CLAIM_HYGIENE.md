# Claim hygiene (drafts and comments)

Avoid over-claiming. Only say tests/builds ran if they did. Prefer language that matches repo reality.

## Tagging drafts

When you add a post or comment draft, tag its claim level so reviewers (and future tooling) know how grounded it is:

| Tag | Meaning | Example language |
|-----|---------|------------------|
| **shipped** | Feature/behavior is in repo and wired; you can point to path/behavior | "We ship X in `crates/…`"; "Autopilot does Y" |
| **wired** | Implemented but experimental or not yet the default path | "We're wiring X"; "Our spec calls for Y" |
| **aspirational** | Direction we're building toward; not yet implemented | "We're building toward X"; "Our docs describe Y" |

Use **shipped** only when you can point to the exact path and behavior. Prefer **wired** or **aspirational** when in doubt.

## Language to prefer

- "We're implementing / wiring / experimenting with…"
- "Our docs/spec calls this…"
- "In our repo we define…"
- "We're building toward…"

Avoid unless true:

- "We already ship Verified Patch Bundles" (only if the artifact is produced and documented)
- References to specific PRs or CLIs (e.g. `oa citrea`) unless they're stable and documented

## Queue metadata (optional)

In `queue.jsonl` you can add optional fields per line:

- **claims**: `shipped` | `wired` | `aspirational` — how grounded this draft is.
- **links**: array of KB URLs (e.g. `["/kb/nostr-for-agents/"]`) — canonical docs this reply routes to.

Example:

```json
{"type":"comment","post_id":"abc","file":"crates/moltbook/docs/responses/comment-xyz.json","claims":"wired","links":["/kb/nostr-for-agents/"]}
```

See [STRATEGY.md](STRATEGY.md) and [README.md](README.md) for queue format.
