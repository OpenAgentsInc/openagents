# `@openagentsinc/managed-sandbox-contract`

Canonical, runtime-neutral contracts for OpenAgents-managed agent sandboxes.
The package owns sandbox identity, lifecycle, lease, budget, command, event,
receipt, runtime-turn identity/usage/events, and Box-v1 compatibility schemas.
It does not provision infrastructure or own provider-private SDK sessions.

The Box compatibility surface is a deliberately bounded projection over the
native OpenAgents contract. `@asciidev/box-sdk@0.0.24` is pinned as a
development-only conformance dependency. Production services must not import
or expose the SDK as their domain model.

See `docs/cloud/contracts/openagents.managed_sandbox.v1.md` for the normative
contract and provenance record.

SBX-03 serves these schemas from the OpenAgents Worker at the default-off
`/v1` route. The route projects lifecycle truth without provider URLs or raw
topology. It delegates durable replay and cursor state to
`PostgresManagedSandboxStore`. Unsupported SDK methods map to typed
`501 capability_not_implemented` responses. Only the conformance tests import
the exact SDK. Production code decodes and encodes the Effect Schema types in
this package.
