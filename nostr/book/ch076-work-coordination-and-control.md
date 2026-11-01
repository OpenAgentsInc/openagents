# 76. Work, coordination and control

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Work outlives a conversation. These four NIPs describe the work, who is doing
it, who may steer it and what context it saw.

Part X, Every NIP. Result: NIP-WORK tracked items and revisions, NIP-COORD
claims with fencing tokens and leases, NIP-CTRL client pairing and commands,
and NIP-CTX task frames and context receipts.

## What this chapter teaches

- WORK: items, planning relationships, authoritative revisions and evidence.
- COORD: admission, task identity, atomic claims with fencing and lease
  renewal.
- CTRL: rights, pairing, revocation and ordered commands with honest receipts.
- CTX: task frames, snapshots, context requests and selection receipts.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [75. Hosts and reachability](ch075-hosts-and-reachability.md)

## Done means

- Two workers claiming one task: exactly one wins, and the other's writes are
  fenced.
- A command from an unpaired client refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-WORK](../nips/openagents/NIP-WORK.md)
- [OpenAgents NIP-COORD](../nips/openagents/NIP-COORD.md)
- [OpenAgents NIP-CTRL](../nips/openagents/NIP-CTRL.md)
- [OpenAgents NIP-CTX](../nips/openagents/NIP-CTX.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
