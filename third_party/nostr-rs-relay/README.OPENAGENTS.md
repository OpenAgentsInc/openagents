# OpenAgents Vendoring Note

This directory contains a vendored copy of `nostr-rs-relay` imported for the Nexus relay migration.

Source of vendored import:

- local source used for import: `/Users/christopherdavid/code/nostr-rs-relay`
- upstream project: `nostr-rs-relay`
- upstream license: MIT

Why this is here:

- OpenAgents is replacing the current in-memory Nexus relay harness with a durable relay engine.
- This vendored copy is the baseline import step for that migration.
- Nexus-specific behavior should be layered on top of this engine gradually rather than mixed into the initial import.

Current status:

- imported as upstream baseline
- preserved with upstream `LICENSE` and `README.md`
- linked into the workspace through `apps/nexus-relay`
