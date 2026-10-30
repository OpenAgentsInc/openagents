# NIP-MP projection verification

This is an untrusted data representation for a planned project-page migration,
not an implementation of event admission, project listing or authorization.
The source fixtures are semantic examples with synthetic signatures. In
particular, a successful decode does not prove that a projection matches its
original signed event. No live consumer uses this profile yet.

The profile was generated with protoc 29.3 and its checked-in descriptor has
SHA-256 `d1f95b8fa2ac9bbceb65bdfae251fc35adcedbb3123e4beed8f904d60b28ec7e`.
Its complete dependency set is `google/protobuf/descriptor.proto`,
`openagents/options/v1alpha1/options.proto`, `nostr/event/v1alpha1/event.proto`,
and `nostr/block/mp/v1alpha1/project.proto`. The manifest pins NIP-MP and its
fixtures to Block commit `24a370606044c0c7be787cc2fd98ec0bfe1d606b`.

`coax/tests/nostr/block_project.py` constructs representative pinned-fixture
projections and independently decodes them with protoc. Actual generated
Coax readers and writers run in checked and release modes under native
ASan/UBSan and WASI; Python's protoc-generated classes verify output and
preservation of original JSON, presence and unknown fields. It exercises
members at 64 and 65, metadata at and over its byte bound, truncation and an
intentional original/projection mismatch. `coax test` also checks required
presence and absent versus empty relay hints. These are codec tests; they do
not establish NIP conformance or cryptographic correctness.

Reproduce the affected tests with the repository's pinned `PROTOC`,
`PROTO_PYTHON` and a provenance-verified `coax/bin/coax`:

```sh
coax/bin/coax test coax/tests/nostr/mp_projection_checks.coax
"$PROTO_PYTHON" coax/tests/nostr/block_project.py
"$PROTO_PYTHON" proto/tools/check_nostr_sources.py
scripts/check-nips-pin.sh --offline
```

The Coax codec tests used source checkout `5c6d3d767c` and the
provenance-verified compiler with input fingerprint
`2bbea32fe212e27ae60bc2b6bbf69f632b7d2f6c86fd5cbfed55614489508b24`
and binary SHA-256
`b2e0ee26b88f8728504742fb3fc1007b18a53c15575734e9c20f12e6324e5fb5`.
The protocol check (`scripts/proto.sh check`) also passed its 29 conformance
checks and confirmed that all prior published descriptors remain unchanged.
