# Nostr projections for Coax

This mirror has **18 alpha schemas**: shared NIP-01 event/filter types in two
versions, NIP-42 authentication, and 15 external agent, identity, channel and
project projections.
All live under root `proto/nostr/`, with one versioned namespace per source.
They mirror the current pins in [`nips/`](../../nips/README.md); the original
Nostr/HTTP/JWS protocols remain unchanged. The v1alpha2 event profile adds an inclusive
0..65535 kind constraint; the published v1alpha1 profile continues to accept
its previous uint32 domain. The legacy `owned` and `nostr` aggregate descriptor
roots explicitly exclude the new files, so old consumers keep their exact bytes;
the new profile is compiled separately. Migrate consumers explicitly; parsed
values remain unverified and convey no authority.

| Profile | Namespaces |
| --- | --- |
| `nostr-event` | `nostr.event.v1alpha1`, `nostr.nip42.v1alpha1` |
| `nostr-event-v1alpha2` | `nostr.event.v1alpha2`, explicitly bounded Nostr kinds |
| `block-agent` | `nostr.block.{aa,ae,ao,am,ap}.v1alpha1`, plus shared/OA dependencies |
| `block-identity` | `nostr.block.{oa,fi,fa,ia}.v1alpha1`, plus shared dependencies |
| `block-channel` | `nostr.block.{cl,ar,cw,dv,wp}.v1alpha1`, plus shared dependencies |
| `block-project` | `nostr.block.mp.v1alpha1`, plus shared event/options dependencies |

[MANIFEST.json](MANIFEST.json) binds every schema to exact source commits and
SHA-256s. [MAPPING.md](MAPPING.md) records field/authority boundaries. The full
`descriptors/nostr.pb` and selected-profile descriptor sets are real protoc
outputs, regenerated and checked by `scripts/proto.sh`. A consumer imports only
its selected profile, without pulling unrelated payment, engine or vendor APIs.
These are new alpha shapes, not the broader design's proposed v1 packages.

## Representation

`SignedEvent` holds the original UTF-8 JSON bytes plus a checked-shape event
projection. Tags keep order, duplicates and extra string values. Content stays
the exact string, including NIP-44 ciphertext; parsing an encrypted event never
puts decrypted data into its envelope. Event IDs and signatures refer to NIP-01
serialization, never protobuf serialization. An ID match alone is not signature
verification. There are no `verified` or `authorized` transport flags.

Filter list messages distinguish an absent filter from an explicitly empty
array (which matches nothing). Scalar presence preserves kind/time/count zero.
Nullable wrappers distinguish missing, null, empty text and zero. JSON object
bytes preserve unfamiliar fields; exact decimal values avoid double/Struct
rounding for usage, costs and NumericDate. Original bytes also preserve numeric
spelling. Unknown protobuf fields can survive transient round trips but grant
no authority and are not silently promoted into a durable/public schema.

The published `openagents.common.v1.SignedEvent` remains unchanged. Reference
adapters round-trip its exact JSON string through the new alpha wrapper; this
is an explicit compatibility boundary, not a field-type migration.

## What is executable

`proto/tools/nostr_projection.py` is a bounded **reference adapter** for tests
and tooling. It checks structural fields, tag/cardinality examples, null and
counter rules, original/projection consistency and a subset of per-NIP rules.
It does not verify BIP-340/JWS signatures, decrypt/encrypt NIP-44, admit relay
members, compare live owner roles, authenticate adapters, enforce channel
visibility, accept artifact revisions or execute commands. In particular, its
FI parser returns an unverified candidate and its OA parser preserves condition
order; AA kind clauses must not become connection-level restrictions by accident.

`coax/examples/nostr-wire.coax` is an actual bounded Coax reader of the Event
projection's kind field, compiled to C and exercised against the checked-in
source-derived protobuf fixture. It scans wire data, skips supported unknown
fields and declines groups, duplicate kind fields, malformed lengths and
oversized inputs. It deliberately accepts a kind-only projection and does not
validate the full Event, signed JSON or authentication. Coax now has initial structs and byte views, which this proof uses directly.
[`#import_proto`](../../coax/proto.md) now imports all five descriptor profiles
into typed Coax wire views, presence/oneof accessors, repeated iterators and
bounded caller-buffer codecs. `coax/tests/proto_import.py` exercises the real
Event fixture, external types, unknown preservation, strict duplicate policy and
limits through generated native C ABI code, including sanitizer and pinned
protoc interoperability runs. The kind-only reader remains a regression
reference. Domain adapters and live product/cryptographic admission are not
implemented by these codecs.

The native host in that test lends a valid byte view through a generated C ABI
struct; the Coax reader checks its count and never mutates its contents. No mobile
screen, cloud admission path, signing domain or network service is changed by
this schema slice.

## Checks and source updates

```sh
export PROTOC=/path/to/protoc-29.3/bin/protoc
export PROTO_PYTHON=/path/to/python-with-protobuf-5.29.3
scripts/proto.sh generate
scripts/proto.sh check
scripts/check-nips-pin.sh --offline
coax/build.sh test
```

Checks compile fresh types, validate package/path and profile closure, preserve
all published baselines, and exercise the real NIP-OA event fixture and negative
cases. The source sync leaves mirrors untouched; if a pin/hash changes, the
mirror checker blocks protocol publication until a review updates its manifest.
Sync reports that required review instead of filing issues or rewriting schemas
without approval.

The original 191-file pre-reset NIP recovery remains recorded in
[recovery.json](recovery.json). Current official/external pins advance separately;
the OpenAgents-authored NIPs remain outside this mirror. Next slices include
GS, PL, RS and additional official NIPs. ER and PMA remain reserved;
no runtime support is implied by their specifications being present.
