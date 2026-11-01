# 102. Agents on Nostr

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

OpenAgents' agents publish events and take jobs. This chapter plans their move
to the Coax stack.

Part XII, Nostr in OpenAgents. Result: Hosted project agents and Coder
publishing with this book's signer and relay client, replacing the C runner's
own signing path.

## What this chapter teaches

- The current runner (`coax/web/agent/runner.c`) signs and talks to the relay
  itself.
- The replacement uses chapter 13's signer behind the signer-boundary gate and
  chapter 29's client.
- Agent identities, saved agents and skills from chapter 79.

## What exists today

- The hosted runner signs in C; nothing here is Coax yet.

## Dependencies

- [79. Agents, skills and automation](ch079-agents-skills-and-automation.md)
- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md)
- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)

## Done means

- The runner publishes through the Coax stack in staging with the same
  identities and history.
- A rollback to the C path is rehearsed.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-AGENT](../nips/openagents/NIP-AGENT.md)
- [OpenAgents NIP-CJ](../nips/openagents/NIP-CJ.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
