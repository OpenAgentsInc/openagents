# Shared Protobuf schemas

This top-level directory is the canonical source for shared OpenAgents/Aethyr
Protobuf contracts. Schemas live by versioned package rather than implementation
language. The package is `openagents.engine.v1`:

| Schema | Shared contract |
| --- | --- |
| [scene.proto](openagents/engine/v1/scene.proto) | Axes, landmarks, collisions, asset manifests, compiled placements/lights/cameras |
| [agent.proto](openagents/engine/v1/agent.proto) | Phone task records, decisions/operations, dispatch, real tools and paged progress |
| [hud.proto](openagents/engine/v1/hud.proto) | Conversation snapshots/deltas, interactions/artifacts and reusable HUD frames/actions |
| [control.proto](openagents/engine/v1/control.proto) | Engine controls, capabilities, request identities, events and receipts |
| [state.proto](openagents/engine/v1/state.proto) | Registered component fields, snapshots, entity references/order and revision-fenced deltas |
| [assets.proto](openagents/engine/v1/assets.proto) | Consumer asset profiles; [profiles.json](openagents/engine/v1/profiles.json) is the declarative input |

Adjacent `.options` files define C/C++ allocation and shared byte/count limits.
Profiles generate a checked-in binary and a compiled C++ byte image, so an
untrusted asset manifest cannot supply weaker consumer requirements.

`crates/engine-proto` consumes these schemas for Rust and holds the existing
generation, validation, compatibility and export tooling. It also checks in
C/C++ and Python bindings. Aethyr imports a committed, hash-pinned export into
`third_party/openagents-engine-proto`; its `proto/` subtree is a snapshot, not
another authoring source. Swift consumes the C++ codec through a narrow C ABI; there is no parallel
Swift protobuf runtime or handwritten Swift wire schema.

From the repository root, using the toolchain pinned in
[`crates/engine-proto/toolchain.json`](../crates/engine-proto/toolchain.json):

```sh
scripts/engine-proto.sh generate
scripts/engine-proto.sh check
cargo test -p engine-proto
# Commit schema, options and generated changes before exporting.
scripts/engine-proto.sh export /path/to/new-bundle
```

The include root is this directory, so the compiler's source/import name stays
`openagents/engine/v1/scene.proto`. Moving the file here does not change its
package, field numbers, descriptor or generated APIs. Normal consumers use
checked-in bindings and need no compiler or network access.

Keep removed names and field numbers reserved, retain published compatibility
descriptors, and test semantic limits across consumers. See the
[`engine-proto` guide](../crates/engine-proto/README.md) for the full checks.
Existing provider protocols and signed source bytes keep their own formats;
these messages do not authorize operations or define GPU layouts.

The broader migration is tracked in
[Aethyr epic #113](https://github.com/OpenAgentsInc/aethyr-cpp/issues/113).

## Compatibility and consumer policy

| Consumer | Codec/checks |
| --- | --- |
| Rust engine/Verse exporter | Prost 0.13.5, checked-in bindings and shared profile bytes |
| C++ native and Wasm | Nanopb 0.4.9.1; generated field/limit tables; native value projection |
| Swift iOS | Shared C ABI codec; task records and chat deltas; tested on iOS 26.5 simulator |
| Linux SSH runner | Python protobuf 5.29.3 in a private runtime; tagged protobuf envelope with a legacy JSON reader |
| Python asset tools | Generated bindings and the same compiled consumer profile data |
| Browser hosts | Shared Wasm codec, typed HUD/controls and snapshot/delta transfer; existing renderers |

Published descriptor snapshots in `crates/engine-proto/compatibility/` are
immutable. Published asset-profile IDs also retain their exact requirements,
checked against `compatibility/asset-profiles-v1-20261027.pb`; changing them
requires a new profile ID. Never reuse retired field numbers or names; reserve both. Keep
additive wire changes compatible, and use an explicit new contract/package
version and migration for changed meanings. The package version, world/component
versions, task schema, and AESV container version are distinct boundaries.

Durable task/world readers reject unknown fields instead of erasing newer data
on read/edit/write. Display-only progress accepts additive fields. Intent and
engine control decoders reject unknown request fields. Existing signed bytes,
Codex/Claude protocols, Loro, Nostr and authored source formats retain their
existing authority and are not rewritten through these messages.

Generic runtime messages are limited to 16 MiB, 64 nested levels and 250,000
visited values, with tighter per-field/message limits. Integer values passing
through a JSON presentation view must remain exact within 2^53-1. Phone task
records are at most 512 KiB. State registries carry explicit scalar/vector/entity
field types; custom codecs require a declared source contract and a lossless
codec check. There is no automatic generic JSON/Any component fallback.

The engine's ECS storage, physics, math, animation and GPU layouts remain native.
Per-frame input retains its direct path. Legacy entry points remain documented
adapters where old records or domain-specific source formats still require them;
new shared persistence/interchange uses these contracts. Generation tests also
compile all generated C/C++ bindings and exercise cross-language wire fixtures.
