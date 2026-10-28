# OpenAgents Protocol

This root `proto/` directory is the defining schema source for **all
OpenAgents surfaces**: web, mobile, desktop, agents, cloud, decisions,
payments and Verse. Coax is the implementation target. **Metareal** is the
engine; Verse is the world/product layer.

- [Protocol specification and implementation plan](../docs/protocol/openagents-protocol-spec.md)
- [Verse protocol chapter](../docs/verse/openagents-verse-protocol.md)
- [Coax language and compiler](../coax/README.md)
- [Metareal names and ID migration](migrations/README.md)

The pre-reset spec is recovered and reconciled with the contracts that had
already shipped. Its historical inventories do not mean the old apps/crates
are installed. Recovery source hashes are in [recovery.json](recovery.json),
pinned to `853417f290`, immediately before the nuke. No deleted application
workspace is restored.

## Current tree

| Root | Namespace and status |
| --- | --- |
| `nostr/` | 16 new alpha schemas: shared events/NIP-42 and 14 Block agent, identity and channel projections. |
| `openagents/common/v1/` | Recovered refs, signed-original wrappers and domain-qualified errors. |
| `openagents/engine/v1/` | Six recovered schemas: scene, agent, HUD, control, registered state and assets. Metareal and other consumers share them. |
| `openagents/host/v1/`, `openagents/reach/v1/` | Recovered phone task/admission, binding/cursor and authenticated reach records. Existing wire types are unchanged. |
| `openagents/options/v1alpha1/` | New experimental data-class, limit, presence, unit and contract annotations. Numbers are provisional pending registry review. |
| `openagents/verse/common/v1alpha1/` | New bounded world reference. |
| `openagents/verse/presence/v1alpha1/` | New transient presence projection using engine geometry. No change to NIP-MV, REACH or realm wire. |
| `third_party/lightninglabs/` | Byte-exact LND, Taproot Assets, Loop and Lightning Terminal release schemas, under their own packages/licenses. |
| `third_party/space/` | Recovered provider-neutral intermediate-key broker contract under its original namespace. |
| `nostr/` | Mirror plan and source authority; no invented compiled Nostr mirrors yet. |
| `compatibility/` | Nine immutable pre-reset descriptor/catalog snapshots. |
| `descriptors/` | Reproducible current owned/profile descriptor sets, including imports. |
| `migrations/` | Explicit Metareal reader aliases, canonical IDs and transition rules. |

There are 12 owned schemas and 38 vendor schemas. The owned nine published
schemas and their Nanopb `.options` are recovered without changes. New alpha
contracts are prototypes, not stable APIs. Vendor roots are separate modules;
they never inherit owned namespaces or allocation annotations.

## Check and generate

The standalone tools use protoc 29.3 and Python protobuf 5.29.3, pinned in
[toolchain.json](toolchain.json). Install the official protoc distribution for
your platform, then set its path. No dependencies are downloaded by the check.

```sh
python3 -m venv /tmp/openagents-proto-tools
/tmp/openagents-proto-tools/bin/pip install -r proto/tools/requirements.txt
export PROTO_PYTHON=/tmp/openagents-proto-tools/bin/python
export PROTOC=/path/to/protoc-29.3/bin/protoc
scripts/proto.sh list
scripts/proto.sh check
scripts/proto.sh generate
```

`check` verifies recovery/vendor/snapshot hashes, compiles every module with
its original include roots, checks all published descriptor baselines,
compiles each consumer profile, tests alpha presence/bounds and the real legacy
catalog migration, and checks generated outputs for drift. It creates no
network connection or app/service. `generate` updates current descriptors and
`data/asset-profiles.pb` and the app's checked-in `Home.pb` resource; it never modifies `compatibility/`.

[modules.json](modules.json) declares `coax-foundation`, `phone-agent`,
`metareal`, `verse`, `ios-home`, `nostr-event`, `block-agent`, `block-identity`
and `block-channel` profiles. Profiles select roots plus transitive imports;
they do not fork schemas or change field meanings. Root-owned APIs compile
separately from each upstream vendor module, avoiding import-name collisions.

## Coax adoption

Coax 0.4.0 implements [`#import_proto`](../coax/proto.md) for selected proto3
descriptor closures. It generates actual Coax typed wire views, presence,
oneofs, repeated iterators and bounded caller-buffer codecs. The
[Nostr alpha profiles](nostr/README.md) are exercised against real fixture
bytes, with native C ABI round trips and pinned protoc interoperability. This
is not a signed JSON/JWS parser, signature verifier or general NIP implementation.
The reduced iOS home now imports the `ios-home` descriptor profile. Generated
Coax codecs read `openagents.metareal.home.v1alpha1.HomeConfiguration` and write
existing `openagents.engine.v1.HudInvocation` messages; the former hand-written
wire reader/writer is removed. Screen admission stays in Coax and retains its
closed schema-1 policy. `metareal/test_home.py` checks actual app resources,
strict rejection rules and exact invocation bytes against protoc. Native
sanitizer and simulator touch/rotation/resume tests cover the actual host path.
Current C++ compiler code and Python schema checks are bootstrap infrastructure.
The initial compiler/codec prerequisites now exist. Checked domain adapters,
platform consumers and live service parity remain separate work. The importer
uses Python standard-library host tooling; generated target code needs no
Python/protobuf runtime. See its contract for strict duplicate/oneof policy,
unsupported shapes and memory budgets.

The Python alpha validator is a fixture/reference checker, not app admission.
Production consumers still need limits before untrusted allocation, recursive
privacy/unknown-field rules, authenticated owners, revisions and actual
transport parity. A decoded message never grants authority.

## Compatibility

Reserve retired names and numbers. Keep every frozen snapshot's bytes. Do not
change published types such as REACH's integer protocol field or common
`ArtifactRef.schema` string. New incompatible representations need new versions
and explicit adapters. Metadata ID migration is separately versioned from
protobuf field layout; no signature, old stored record or existing host
protocol is rewritten by a brand rename.

Additional domains follow the specification's dependency order and start as
alpha only when their source mapping and actual consumer are ready. Stable
JSON does not automatically make a new protobuf representation stable.
