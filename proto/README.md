# OpenAgents Protocol

This root `proto/` directory is the defining schema source for **all
OpenAgents surfaces**: web, mobile, desktop, agents, cloud, decisions,
payments and Verse. Coax is the implementation target. **Verse** is the
engine and the world/product layer it runs, made of realms.

- [Protocol specification and implementation plan](../docs/protocol/openagents-protocol-spec.md)
- [Verse protocol chapter](../docs/verse/openagents-verse-protocol.md)
- [Coax language and compiler](../coax/README.md)
- [Engine names and ID migration](migrations/README.md)
- [Registry of wire shapes](registry.json), checked by `coax/web/tool.sh wire-check`
  ([how to register a shape](#register-a-wire-shape))
- [Compliance plan](../docs/protocol/2026-11-11-compliance-plan.md): which shapes move here, and in what order
- [The OpenAgents Book](../openagents/book/README.md), which says why each record exists and which chapter sends it. Start with [chapter 49, one schema, three carriages](../openagents/book/ch049-one-schema-three-carriages.md), and [chapter 52, the compliance rule](../openagents/book/ch052-the-compliance-rule.md)

The pre-reset spec is recovered and reconciled with the contracts that had
already shipped. Its historical inventories do not mean the old apps/crates
are installed. Recovery source hashes are in [recovery.json](recovery.json),
pinned to `853417f290`, immediately before the nuke. No deleted application
workspace is restored.

## Current tree

| Root | Namespace and status |
| --- | --- |
| `nostr/` | Source-pinned alpha projections: shared events/NIP-42 and selected external agent, identity and channel contracts, with generated Coax codecs. See the [coverage and runtime boundaries](nostr/README.md). |
| `openagents/common/v1/` | Recovered refs, signed-original wrappers and domain-qualified errors. |
| `openagents/engine/v1/` | Six recovered schemas: scene, agent, HUD, control, registered state and assets. **No product consumer** but for `hud.proto`: `verse/home.coax` writes its `HudInvocation`. Tests and tools use the rest. Kept frozen. |
| `openagents/host/v1/`, `openagents/reach/v1/` | Recovered phone task/admission, binding/cursor and authenticated reach records. Existing wire types are unchanged. **No product consumer**: a library adapter imports them and only tests and an example load it. `coder host` uses NIP-HOST's JSON. |
| `openagents/decision/v1alpha1/` | New protobuf projection of the frozen `openagents.systemone.v1` JSON contract (request and response), served by Psionic's `/v1/systemone`. See its [README](openagents/decision/v1alpha1/README.md). |
| `openagents/bitcoin/explorer/v1alpha1/` | New read-only explorer projection. The [HTTP v1 contract](../docs/protocol/bitcoin-explorer-v1.md) defines its JSON envelope and snapshot semantics. |
| `openagents/cli/local/v1/` | Read-only local agent/model inventory for desktop setup. See [the JSON contract](../docs/protocol/local-inventory-v1.md). |
| `openagents/options/v1alpha1/` | New experimental data-class, limit, presence, unit and contract annotations. Numbers are provisional pending registry review. |
| `openagents/verse/common/v1alpha1/` | New bounded world reference. **No product consumer**: only `presence.proto` imports it. |
| `openagents/verse/presence/v1alpha1/` | New transient presence projection using engine geometry. No change to NIP-MV, REACH or realm wire. **No product consumer**: tests only. |
| `coder/`, `verse/`, `aethyrforge/`, `aethyr/` | [JSON profiles and schemas](#json-profiles-and-schemas) for hand-packed frames and local records. |
| `registry.json`, `wire-allowlist.txt` | Every wire shape with its one codec, and what is not compliant yet. See [Register a wire shape](#register-a-wire-shape). |
| `third_party/lightninglabs/` | Byte-exact LND, Taproot Assets, Loop and Lightning Terminal release schemas, under their own packages/licenses. |
| `third_party/space/` | Recovered provider-neutral intermediate-key broker contract under its original namespace. |
| `compatibility/` | Immutable published descriptor/catalog baselines; new baselines do not replace earlier bytes. |
| `descriptors/` | Reproducible current owned/profile descriptor sets, including imports. |
| `migrations/` | Explicit engine-rename reader aliases, canonical IDs and transition rules. |

[modules.json](modules.json) is the registry of current module roots and
selected profiles; `scripts/proto.sh list` reports that inventory. Recovered
published schemas and their Nanopb `.options` retain their original contract.
New alpha contracts are prototypes, not stable APIs. Vendor roots are separate
modules; they never inherit owned namespaces or allocation annotations.

## JSON profiles and schemas

A second schema family lives beside the `.proto` files. `modules.json` does
not list it; [registry.json](registry.json) does, one row a file.

| Folder | What it holds |
| --- | --- |
| `verse/` | Wire profiles for hand-packed frames and records. A profile names each message, its fields in order, a value for each and the exact bytes those values make. `everglade-session-v5.json`, `everglade-latency-v1.json` and `orbit-session-v2.json` have a check that writes and reads those bytes through the codec. The Everglade II familiar, Healer's Kit and link-input profiles, `asset-pack-v2.json` and the character checkpoint JSON Schemas have no check yet |
| `coder/` | `session-log-v1.json`, the session log's lines byte for byte, checked through `coax/lib/agent/session/log.coax` by `log-profile.coax`; and JSON Schemas for Coder's stream records and voice input, which have no check yet |
| `aethyrforge/` | JSON Schemas for Aethyrforge's local commands. None has a check yet |
| `aethyr/` | `physics-lab-v1.json`. No check yet |

Use a profile, not a `.proto`, for a hand-packed frame: hot netcode, and
bytes that are signed, hashed or stored. A profile with no check pins nothing,
so each file here needs a registry row with a `check`, or a line in
[wire-allowlist.txt](wire-allowlist.txt) until its step adds one.

## Register a wire shape

A wire shape is any set of bytes that leaves one program for another, or
that a program stores and reads back later: a message, a frame, a file, a
line in a log. [registry.json](registry.json) has one row for each.
`coax/web/tool.sh wire-check` fails when the tree has a shape the registry
doesn't know.

A shape is compliant when all four hold:

1. **Registered.** It has a row.
2. **One codec.** One Coax file writes and reads it. Every producer and
   consumer loads that file. C calls it through a generated unit
   (`coax/web/api/agent-wire-unit.c` is the model).
3. **Pinned.** A fixture in `proto/` holds exact bytes, and a check writes
   and reads them through the codec.
4. **Schema.** Its fields are in a `.proto` in a `modules.json` profile, or,
   for a hand-packed frame, in a JSON profile such as
   `verse/everglade-session-v5.json`.

To add a shape:

1. Write the schema: a message in a `.proto` under `openagents/` (add the
   file to a profile in `modules.json` and run `scripts/proto.sh generate`),
   or a JSON profile under `verse/`, `coder/`, `aethyrforge/` or `aethyr/`.
2. Write the one codec file. Import the descriptor with `#import_proto`
   where you can ([coax/proto.md](../coax/proto.md)).
3. Add the fixture and the check that compares the codec's bytes with it.
4. Add the row to `registry.json`:

   ```json
   {"id": "openagents.example.v1", "kind": "proto",
    "schema": "proto/openagents/example/v1/example.proto#Record",
    "codec": "coax/lib/example/wire.coax",
    "check": "coax/bin/coax test coax/lib/example/wire.coax",
    "step": "C1",
    "files": ["coax/web/api/example.c"]}
   ```

5. Run `coax/web/tool.sh wire-check`.

Row fields:

| Field | Meaning |
| --- | --- |
| `id` | The schema id on the wire (`NAME.vN`), or a short name for a shape with no id. An id that ends in `*` is a family: every id that starts that way |
| `kind` | `proto` (a `.proto` message), `profile` (a JSON profile), `external` (a third party's format; we write no schema for it) or `local` (one program writes and reads it) |
| `schema` | The `.proto` file, with `#Message` when you name one, or the profile file |
| `codec` | The one file that may write the id. A folder (ending in `/`) for a family or a third party's adapter |
| `check` | The command that ties the schema to the bytes. `proto` and `profile` rows need one |
| `step` | The compliance plan's step that owns the shape. `unaudited`: outside the areas the plan audited; the row only records where the id lives today |
| `files` | Other files the row accounts for: API route files and room files that carry the shape through the codec |
| `tests` | Tests, fixtures and oracles outside test folders that name the id. On an unaudited row, every other file that carries the id today |
| `sql` | SQL files that build the shape. List one only when a test holds it to the codec's fixtures |
| `status` | `pending`: the row names the shape and its intended codec, but its schema or check is missing; it needs a `shape ID` line in the allowlist. `no-consumer`: a published schema that no product file imports |

`no_frames` in the registry lists the room files that build and read no wire
bytes of their own.

[wire-allowlist.txt](wire-allowlist.txt) lists what is not compliant yet.
It only shrinks: each step of the plan takes its lines out. A new line fails
unless it ends with a reason and an issue number (`# why (#1234)`), and a
line that no longer excuses anything fails, so the list stays true.

| Line | It excuses |
| --- | --- |
| `id ID FILE` | `FILE` carries `ID` though it is not the id's codec: a second encoder or a reader with its own parser |
| `route FILE` | An API file with a `*_route` function whose bodies no row covers |
| `room FILE` | A room file with frames of its own that no row covers |
| `profile FILE` | A profile or schema file with no check |
| `proto FILE` | A `.proto` in no profile that still has to match what ships |
| `shape NAME` | A shape that is not compliant yet: a pending row's id, or a shape with no id or file of its own |

What `wire-check` reads as a schema id: a string that ends in `.vN`, either
after a `schema` key or, for a registered id, as a whole quoted string.
Comments, `#test` blocks, test folders (`tests/`, `fixtures/`) and files
named `test_*`, `*-check.*` or `*-checks.*` don't count.

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
coax/web/tool.sh wire-check      # every wire shape is registered; needs no protoc
```

`check` verifies recovery/vendor/snapshot hashes, compiles every module with
its original include roots, checks all published descriptor baselines,
compiles each consumer profile, tests alpha presence/bounds and the real legacy
catalog migration, and checks generated outputs for drift. It creates no
network connection or app/service. `generate` updates current descriptors and
`data/asset-profiles.pb` and the app's checked-in `Home.pb` resource; it never modifies `compatibility/`.

Profiles in [modules.json](modules.json) select roots plus transitive imports;
they do not fork schemas or change field meanings. Engine paths now live under
`verse/`; published namespaces and descriptor/profile identifiers retain their
registered spelling. Root-owned APIs compile separately from each upstream
vendor module, avoiding import-name collisions.

## Coax adoption

Coax has supported [`#import_proto`](../coax/proto.md) for selected proto3
descriptor closures since 0.4.0. It generates actual Coax typed wire views, presence,
oneofs, repeated iterators and bounded caller-buffer codecs. The
[Nostr alpha profiles](nostr/README.md) are exercised against real fixture
bytes, with native C ABI round trips and pinned protoc interoperability. This
is not a signed JSON/JWS parser, signature verifier or general NIP implementation.
The reduced iOS home now imports the `ios-home` descriptor profile. Generated
Coax codecs read `openagents.metareal.home.v1alpha1.HomeConfiguration` and write
existing `openagents.engine.v1.HudInvocation` messages; the former hand-written
wire reader/writer is removed. Screen admission stays in Coax and retains its
closed schema-1 policy. `verse/test_home.py` checks actual app resources,
strict rejection rules and exact invocation bytes against protoc. Native
sanitizer and simulator touch/rotation/resume tests cover the actual host path.
The descriptor parser, validator and source generator are checked Coax in
`coax/lib/proto/importer.coax`; fingerprinting is Coax in
`coax/lib/proto/fingerprint.coax`, and the wire runtime is Coax source. They
compile to C11 inside the Python-free bootstrap. The C++ importer bridge
supplies file I/O and requested buffer allocation, with no C++ parser,
generator or hash fallback. Neither normal compilation nor generated target
code needs Python/protobuf libraries. Explicit test/reference and schema
maintenance tooling remains separate.

Further domain adapters, platform consumers and live service parity remain
separate work. See the [importer contract](../coax/proto.md) for strict
duplicate/oneof policy, unsupported shapes and memory budgets, and the
[Coax Nostr plan](../coax/nostr.md) for crypto/client/authority integration.
No generated Nostr view substitutes for signed-original verification or
domain admission.

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
