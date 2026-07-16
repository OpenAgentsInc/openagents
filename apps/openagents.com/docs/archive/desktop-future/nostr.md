---
title: Nostr
description: Historical Nostr identity, registry, storage, and open-protocol experiments.
lastModified: 2026-07-15
sidebar:
  order: 3
---

**Status:** Historical protocol work. Nostr is not a live feature of the current Desktop MVP or an active roadmap commitment.

Nostr appeared repeatedly as a neutral protocol layer rather than a single product feature. OpenAgents explored it for identity, plugin discovery, key-value storage, data vending, client communication, job requests, and market coordination.

## Ideas worth retaining

- User-controlled identity and signing should remain distinct from application accounts.
- Relay discovery is not the same as service availability or trust.
- Events need typed schemas, bounded permissions, provenance, and replay rules before they can carry product authority.
- Open protocols can reduce platform capture, but they do not remove custody, moderation, spam, privacy, or verification problems.

The workspace retains shared Nostr engineering in `nostr-effect` for protocol-level reuse. That repository presence is substrate, not evidence that OpenAgents Desktop exposes Nostr identity, relays, messaging, jobs, or markets.

## Historical source trail

The build-in-public series explored a [Nostr plugin registry](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/066.md), [Nostr login](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/082.md), [Nostr key-value storage](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/088.md), [data vending machines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/142.md), and a [Nostr client](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/177.md). Those episodes are historical orientation, not current product documentation.
