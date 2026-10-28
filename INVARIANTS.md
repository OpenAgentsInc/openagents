# Invariants

## One protocol for all OpenAgents surfaces

Root `proto/` and the OpenAgents Protocol specification define interchange for
all surfaces, including Verse. Coax is the product implementation target;
Metareal is the engine name. Profiles select imports from one source tree,
not separate definitions per app, target or implementation language.

Check: `scripts/proto.sh check` compiles actual roots and profile closures.
Coax descriptor ingestion is checked by `coax/tests/proto_import.py`. Product
migration and domain/admission validation remain explicit consumer gates.

## Published contracts and signed originals retain identity

All `proto/compatibility/` snapshots and vendor source/license bytes are
immutable. Published names, numbers, types and presence must pass every frozen
descriptor baseline. Nostr/HOST/REACH and other signed/framed protocols retain
their original bytes and authority owners. A decoded DTO grants no authority.

Check: recovery hashes, compatibility mutation tests, isolated vendor builds,
and alpha missing-versus-zero/limits tests in `scripts/proto.sh check`.
These checks do not claim that deleted runtime admission code is installed.

## Metareal rename has explicit readers and versions

Legacy IDs live in the migration registry, frozen evidence and byte-exact
recovered or upstream NIP source specifications. Current
asset catalogs use canonical IDs; their other required data is unchanged.
Migrations preserve original input, unknown fields and signatures, reject
collisions and never reinterpret an old signature under a new domain. Other
record/bridge/signature migrations remain gated on actual consumers.

Check: real frozen asset catalog round trip, idempotence, collision tests and
signature-domain refusal in `proto/tools/test_contracts.py`.
The NIP recovery source/tree identity is recorded in `proto/nostr/recovery.json`;
restoring historical text does not restore its retired implementation mandates.
Current official/Block mirrors may advance beyond that historical tree only
with exact upstream pins and per-file hashes; `nips/openagents` is not synced.
Check both online pin parity (`scripts/check-nips-pin.sh --latest`) and offline
inventory/hash parity (`--offline`), including non-Markdown fixtures.

## No GitHub-hosted CI

Protocol and compiler checks run locally or on owned infrastructure. Do not
add GitHub Actions workflows. A schema check needs no running service,
production credentials or network download.

## The reduced iOS home has one product implementation

Coax owns its scene, animation, layout, reusable HUD touch policy and bounded
protobuf admission. Native adapters own platform lifecycle, serialized opaque
storage handles, packet transfer, font rasterization and Metal bindings. A
screen does not acquire chat, network or device permissions through this port.
The home accepts only its documented schema-1 resource subset, rejects unknown
or duplicate fields, and emits existing HudInvocation bytes. Start currently
replays the home; it does not claim to open an unported destination.

Check: `coax/build.sh test`, `PROTOC=... python3 metareal/test_home.py`, the
sanitizer harness and the iOS native touch/rotation test. Metal layout/stage
bindings remain an explicit native boundary; the home retains its initial
flat-storage implementation while the newer Coax data types are evaluated.
There is no claim of a full engine migration or general protobuf compiler.


## Nostr projections cannot create authority or replace signed bytes

New `nostr.*.v1alpha1` values are source-pinned projections. A SignedEvent keeps
original JSON alongside ordered string tags and exact content; the published
common.v1 wrapper stays intact behind an explicit adapter. JSON null, missing,
zero and unknown fields remain distinguishable where the source requires it.
No DTO, parsed owner claim, label, command, revision link or decoded JWS counts
as signature verification, owner membership, relay identity, grant, CAS
acceptance or encryption. Decrypted memory/telemetry/metrics and identity
credentials must not be published, logged or placed in a public snapshot.

Check: `proto/tools/check_nostr_sources.py` and `scripts/proto.sh check` reject
stale mirror pins/hashes and schema inventory drift. Reference tests check real
event ID/original round trips, unknown fields, missing/null/zero cases, token
classes, byte bounds and selected-profile imports. Coax tests consume the actual
Event protobuf fixture through both the narrow reader and actual descriptor-
generated typed codecs. `coax/tests/proto_import.py` checks presence/oneofs,
repeats, unknown retention, writer capacity/alias errors, policy bounds and
native ASan/UBSan; its explicit protoc oracle checks interoperability. Codecs
borrow input/output storage, allocate no target memory, and reject unsupported
descriptors and duplicate known singular/oneof fields. Domain migration and all
cryptographic/relay admission remain explicit unimplemented gates.
