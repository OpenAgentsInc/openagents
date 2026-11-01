# 78. Runs, sessions, terminals and trajectories

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

A run records what happened; a session is how a client attaches; a terminal is
a live shell; a trajectory is the full record of an agent's steps. This
chapter builds the four record types.

Part X, Every NIP. Result: NIP-RUN durable run journals with controller
handoff, NIP-SESS engine sessions, NIP-TERM terminal sessions, and NIP-ATIF
agent trajectories as manifests and chunks.

## What this chapter teaches

- RUN: record identity, state transitions, fencing, heads, replay and
  retention.
- SESS: adapters, effective configuration, commands and queue law.
- TERM: rights, frames, lifetime and transport.
- ATIF: manifests, chunks, public and private records, links.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [76. Work, coordination and control](ch076-work-coordination-and-control.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Done means

- A run journal replays to the same head after a controller handoff.
- A trajectory chunk missing from its manifest is reported, not ignored.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-RUN](../nips/openagents/NIP-RUN.md)
- [OpenAgents NIP-SESS](../nips/openagents/NIP-SESS.md)
- [OpenAgents NIP-TERM](../nips/openagents/NIP-TERM.md)
- [OpenAgents NIP-ATIF](../nips/openagents/NIP-ATIF.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
