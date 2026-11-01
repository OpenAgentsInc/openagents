# 77. Capabilities, policy and extensions

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

A host advertises what it can run and decides what it will run. These NIPs
make both inspectable.

Part X, Every NIP. Result: NIP-CAP execution descriptions and bindings,
NIP-POL inspectable host policy and approvals, NIP-EXT signed component
packages, and NIP-REG curated plugin registries.

## What this chapter teaches

- CAP: description, binding and grant; discovery and probes.
- POL: instruction sets, learned preferences, action review and approvals.
- EXT: releases, manifests, revocation and freshness.
- REG: registry identity, catalog document, Nostr publication and HTTPS
  mirror.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [75. Hosts and reachability](ch075-hosts-and-reachability.md)

## Done means

- A capability without a matching binding is refused at dispatch.
- A revoked extension release does not install.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-CAP](../nips/openagents/NIP-CAP.md)
- [OpenAgents NIP-POL](../nips/openagents/NIP-POL.md)
- [OpenAgents NIP-EXT](../nips/openagents/NIP-EXT.md)
- [OpenAgents NIP-REG](../nips/openagents/NIP-REG.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
