# `@openagentsinc/sarah`

Shared Sarah owner-orchestrator identity, mobile/API projections, capability
directory, cited business-context model, and the compiled runtime authority
profile. Sarah is a capability inside supported OpenAgents apps—not a restored
standalone app or public `/sarah` route.

## Nostr identity (`SARAH-NR-04`)

`@openagentsinc/sarah/nostr-identity` holds the sealed signer, Secret Manager
custody mount (`SARAH_NOSTR_IDENTITY_SECRET` → `sarah-nostr-identity-secret`),
NIP-OA/AA helpers, and lifecycle transitions for `principal.sarah`.

Contract: `docs/omega/2026-07-24-sarah-nostr-identity-contract.md`.
