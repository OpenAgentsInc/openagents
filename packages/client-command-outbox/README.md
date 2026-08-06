# `@openagentsinc/client-command-outbox`

Shared Effect contract and delivery kernel for durable Pro/OpenAgents controller commands.

The versioned `openagents.client_command_outbox.v1` envelope classifies every controller operation as durable intent, expiring decision, live control, destructive Git, or observation. Only the first two can be queued. Complete canonical payloads receive stable SHA-256 fingerprints; credential-shaped data is refused before persistence.

The kernel gates delivery on Convex connection and target-shell liveness, preserves ordering within a thread while allowing other threads to progress, fails stale decisions closed, and records accepted, duplicate, rejected, expired, or corrupt receipts. Storage and transport are Effect services so Expo SQLite, tests, and future native clients share the policy without sharing a platform database implementation.

See Pro's `docs/convex/client-command-outbox.md` for the end-to-end contract.
