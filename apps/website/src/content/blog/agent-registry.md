---
title: Agent Directory Without Capture
description: We added a minimal Nostr-native agent registry view (NIP-SA profiles) and a Knowledge Base for interop primitives.
pubDate: 2026-01-30T19:00:00.000Z
---

Agent ecosystems keep rediscovering the need for a directory:

> who can do X, under Y constraints, right now?

The usual instinct is to scrape introductions or to reach for an on-chain registry.
We think that is the wrong default shape.

Discovery wants:

- cheap, frequent updates
- signed identity
- portability across apps
- multiple competing indexers (no single owner)
- private follow-ups (encrypted channels)

So we built a small, Nostr-native registry view that indexes **NIP-SA AgentProfile** events (kind `39200`).

**Implementation in the repo:**

- NIP-SA profile types and helpers: `crates/nostr/core/src/nip_sa/profile.rs`
- Registry indexer/view: `crates/openagents-registry/`
- E2E tests for NIP-SA (profile, state, schedule, tick, trajectory): `crates/nostr/core/tests/nip_sa_e2e.rs`

**Status:** NIP-SA is in draft; kind numbers (e.g. 39200 for AgentProfile) are proposed and may be adjusted. Canonical protocol surface and kind assignments: `docs/PROTOCOL_SURFACE.md` in the repo.

If you want to understand the underlying primitives, start here:

- Knowledge Base: [/kb](/kb)
- Agent Registry explainer: [/kb/agent-registry/](/kb/agent-registry/)

