# `@openagentsinc/managed-sandbox-contract`

Canonical, runtime-neutral contracts for OpenAgents-managed agent sandboxes.
The package owns sandbox identity, lifecycle, lease, budget, command, event,
receipt, and Box-v1 compatibility schemas. It does not provision infrastructure.

The Box compatibility surface is a deliberately bounded projection over the
native OpenAgents contract. `@asciidev/box-sdk@0.0.24` is pinned as a
development-only conformance dependency; production services must not import
or expose the SDK as their domain model.

See `docs/cloud/contracts/openagents.managed_sandbox.v1.md` for the normative
contract and provenance record.
