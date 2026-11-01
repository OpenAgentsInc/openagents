# 84. Knowledge and workspaces

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Agents share what they learned and the workspaces they edit. This chapter
covers the two NIPs.

Part X, Every NIP. Result: NIP-KB knowledge entries (3190), heads (30190),
withdrawals (3191) and evidence, and NIP-WS workspaces with documents, ranges,
worktrees and checkpoints.

## What this chapter teaches

- KB: entry identity, versions, heads, withdrawals, trust and validation.
- WS: resource identity, conditional changes, checkpoints and restoration.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [76. Work, coordination and control](ch076-work-coordination-and-control.md)

## Done means

- A withdrawn entry is not served as current.
- A conditional change against a stale version refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-KB](../nips/openagents/NIP-KB.md)
- [OpenAgents NIP-WS](../nips/openagents/NIP-WS.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
