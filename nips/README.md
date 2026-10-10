# NIP sources

This directory holds our copies of the Nostr protocol specifications
(NIPs). These copies are the source of truth for the Rust implementation
in this repository.

## The three lanes

| Lane | Upstream | Content |
| --- | --- | --- |
| `nips/official/` | [nostr-protocol/nips](https://github.com/nostr-protocol/nips) | The standard NIPs |
| `nips/block/` | [block/buzz](https://github.com/block/buzz/tree/main/docs/nips) | The Buzz extension NIPs |
|  `nips/openagents/` | this repository | OpenAgents specifications, authored here |

 `nips/openagents/` is not synced: its files are the source of truth and the
manifest does not track them.

The [OpenAgents protocol index](openagents/README.md) covers the revised v1
capability/program contracts, extension distribution, durable runs, and three
job families. It also includes draft scoped task control (CTRL), negotiated
markets (MKT), and agent labor (LAB). The
[cross-lane review](../docs/protocol/2026-09-26-openagents-gap-review.md)
explains why these contracts are needed and which official and Block
primitives they reuse. The [implementation plan](../docs/protocol/implementation-plan.md)
tracks remaining work across all lanes. A draft revision changes the target;
it does not make an existing reader conformant.

The OpenAgents lane now contains 33 NIPs plus its shared contracts.
[NIP-VAULT](openagents/NIP-VAULT.md) and [NIP-ATT](openagents/NIP-ATT.md) are
**Designed** drafts for sealed personal data and attested workloads (the
[security docs](../docs/security/README.md)).
[NIP-X402](openagents/NIP-X402.md) is a **Designed** draft for Lightning-paid
operations before execution, separate from MKT/LAB payment after acceptance.
It preserves standard x402 HTTP/MCP bindings; its `nostr:openagents:1` binding
is an opt-in OpenAgents extension, not an upstream profile. It allocates no
event kinds. See the [integration assessment](../docs/coder/design/x402-lightning-nostr-integration.md)
for wallet, recovery, and implementation boundaries.

[NIP-SOV](openagents/NIP-SOV.md) restores historical `SA.md` as a **Designed**
profile for sovereign agents. Identity, custody, bounded AUTO lifecycles,
POL guardians, treasury policy, and market activity compose existing contracts
without new kinds. Its [migration map](openagents/NIP-SOV.md#provenance-and-migration)
links the exact pre-Nuke source and replaces its incompatible `392xx` records.
The new name distinguishes this profile from the historical SA wire format.

[NIP-ATIF](openagents/NIP-ATIF.md) is a **Designed** draft for carrying ATIF
agent trajectories: owner-encrypted `3188` artifacts by default, and public
`3198` declarations with ordered `3199` chunks after a separate publication
decision. It links trajectories to Coder tasks, RUN runs, and delegated
sub-agents, and maps Block AO/AM/AE records onto ATIF steps.

[NIP-REG](openagents/NIP-REG.md) is a **Designed** draft for curated plugin
registries. It uses inert EXT catalog packages and existing release/head kinds.
Nostr relays and GitHub/HTTPS mirrors carry the same signed events and verified
artifact bytes; each reader chooses which curators to trust. Registry discovery
does not install, enable, or admit a plugin. No registry client is implemented.

The [teardown coverage ledger](../docs/protocol/2026-09-26-teardown-coverage.md)
maps all 81 archived research documents to this set. Six further drafts cover
engine sessions (SESS), workspace resources and projections (WS), tracked work
(WORK), bounded automation (AUTO), environment leases (ENV), and live media
and device interaction (LIVE). POL adds governed learned preferences; EXT adds
foreign imports and compatible host component sets. These additions reuse
existing event kinds. The [Coder integration plan](../docs/coder/design/teardown-nostr-integration.md)
separates protocol implementation from client, runtime, and evaluation work.

The [September 26 upstream sync assessment](../docs/protocol/2026-09-26-upstream-nip-sync.md)
covers all changed official and Block specifications and the implementation
gaps they expose. The current pins contain 100 official Markdown files
(99 specifications and the index) and 17 Block specifications. Source sync
is complete; runtime conformance is not. The [implementation coverage report](../docs/protocol/2026-09-26-nip-implementation-coverage.md)
records current repairs, validated components, running roles, and remaining
work. Source inventory checks cover the new pins separately from behavioral
evidence. LAB keeps its name because historical
OpenAgents LBR already defines a different protocol.

`nips/manifest.json` records the exact upstream commit for each synced lane, with
a `tree_url` link to browse that commit. Use those links to see the
upstream history for any file.

A lane may carry a repo-owned `README.md` that summarizes its specs when
the upstream does not publish one (currently `nips/block/README.md`).
The sync script preserves that file, and an upstream-provided README
always replaces the local copy. The manifest file count records upstream
files only.

## Implementation mandate

The specifications pinned in these lanes are the relay's implementation
target. The lane is not a reading list. We implement each applicable spec
across the `crates/nostr` domain and `crates/nostr-relay` server surfaces.
If an upstream NIP is client-only, completion means a fixture-backed
client implementation rather than pretending the relay serves it. If a
feature is optional or configuration-dependent, it stays fail-closed and
absent from NIP-11 until its configured path is executable.

Pinned deprecated or unrecommended NIPs are still implemented for exact
compatibility and regression coverage. They do not become the foundation
for new protocol design.

## How we sync

1. `./scripts/sync-nips.sh` clones each pinned upstream, copies the
   specification files into the lane directory, and writes the upstream
   commit hashes to `nips/manifest.json`.
2. Review the diff. A specification change is a protocol policy change.
   Do not commit a sync without reading what changed.
3. Commit the sync as its own commit.

Run the sync at a regular interval and before each new NIP
implementation starts.

## How we verify

1. We implement each NIP from the specification text in this directory,
   from scratch, in Rust.
2. Each implemented NIP gets a fixture corpus in this repository. A
   protocol change without a fixture update is not complete.
3. Where the state space is small and the property matters, we add formal
   verification and keep the model next to the fixtures.
4. A synced upstream change becomes normative for the implementation only
   after review and a fixture update. The sync itself never changes the
   implementation.
5. Keep an explicit ledger for every pinned specification. No file is silently
   ignored because its role is client-side, optional, deprecated, or not yet
   represented by a server handler.

## Precedence

The lanes stay separate. The implementation tracks each NIP by lane and
identifier. If the same identifier exists in more than one lane, the
`official` lane wins unless the build plan names an exact exception.
