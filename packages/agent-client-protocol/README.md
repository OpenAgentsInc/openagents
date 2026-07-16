# Agent Client Protocol authority

`@openagentsinc/agent-client-protocol` is OpenAgents' pinned wire authority for
hosting foreign coding agents such as Grok (`grok agent stdio`) and Cursor
(`agent acp`). Here, **ACP means Agent Client Protocol**. The unrelated BeeAI /
Linux Foundation Agent Communication Protocol and A2A are not implemented by
this package.

## Authority and API decision

The normal package surface is generated from the official stable
[`schema-v1.19.0`](https://github.com/agentclientprotocol/agent-client-protocol/releases/tag/schema-v1.19.0)
`schema.json` and `meta.json`, negotiated as wire protocol version 1. The four
release assets, their SHA-256 digests, the upstream commit, and the full
Apache-2.0 license are vendored under `upstream/schema-v1.19.0/` so normal
generation and drift checks are offline and deterministic.

The official TypeScript SDK is exact-pinned at
`@agentclientprotocol/sdk@1.2.1` as the generated **unstable-only type source**
and an audited comparison point in this layer. Stable structural types are
generated locally from `schema.json`. The SDK's own generator consumes
`schema.unstable.json`, and its shipped schema hash is byte-identical to this
release's unstable artifact. Therefore:

- `.` and `./stable` expose stable types, stable methods, and runtime codecs;
- `./unstable` is a physically separate import containing unstable-only types
  and the reviewed unstable inventory;
- `./extensions/grok` and `./extensions/cursor` keep vendor-prefixed method
  identity outside both upstream namespaces, with versioned method inventories,
  lossless envelope codecs, and an explicit peer-profile gate; and
- SDK method constants are not re-exported as OpenAgents' stable authority.

Runtime validation uses the pinned JSON Schema through AJV. Every schema
definition is compilable as a codec, while `decodeAcpMethodPayload` binds
method direction and request/notification phase to the generated manifest.
Decoded and rejected messages retain a lossless `private-native` raw envelope;
ordinary failure details include schema paths/reasons, never payload values.
Unknown future variants are retained for private diagnosis but cannot mutate
canonical OpenAgents state until a reviewed codec/profile exists.

## Generate and verify

```bash
pnpm --dir packages/agent-client-protocol update:upstream # explicit networked upgrade only
pnpm --dir packages/agent-client-protocol generate
pnpm --dir packages/agent-client-protocol check:generated
pnpm --dir packages/agent-client-protocol test
pnpm --dir packages/agent-client-protocol typecheck
```

`check:generated` never fetches. It verifies every vendored digest, verifies
that SDK 1.2.1 still carries the pinned unstable schema, regenerates into a
temporary directory, and byte-compares the generated types and manifests.
Schema upgrades must run `update:upstream` intentionally and review the method,
capability, type, and digest diff.

Vendor extension payloads are intentionally `opaque-native` at this authority
layer because neither vendor publishes those payloads in the upstream ACP
schema. Their envelope codecs reject use unless the caller explicitly enables
the matching `grok-cli` or `cursor-agent` peer profile. Later peer adapters may
promote reviewed payload fields without confusing them with upstream stable or
unstable types.

## Stable method policy

`manifests/stable.json` records direction, request/notification kind, required
capability, params/result schema, and current OpenAgents support state for all
23 stable methods. Baseline session new/prompt/cancel/update methods are
distinct from optional lifecycle, filesystem, terminal, authentication,
content, and MCP forms. `session/fork`, tunneled MCP, providers, elicitation,
NES, and document notifications remain in `manifests/unstable.json` and require
an explicit peer/version gate.

## Migration boundary

`packages/grok-harness` remains a fixture until the transport and Grok profile
issues migrate its callers. New shared protocol code must import this package;
it must not add more hand-written ACP declarations to the fixture. Advertised
filesystem and terminal capabilities remain false until their authority
brokers and reverse handlers land.

## Trusted peer profiles and admission

`./profiles` carries the declarative trusted peer-profile contract, the
trusted registry (the only launch authority), bounded official-registry
discovery ingestion, and the fail-closed admission path. Grok CLI
(`grok agent stdio`) and Cursor Agent CLI (`agent acp`) are the two reference
profiles. See `PEER_PROFILES.md` for the contract, threat model, and the
add-a-peer procedure.
