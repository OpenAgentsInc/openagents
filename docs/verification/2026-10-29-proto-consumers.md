# Nostr generated-protobuf consumer checks

These are experimental, unverified consistency projections for Nostr events
and Block FI assertions. They compare descriptor-generated protobuf values to
original borrowed bytes. A match is not signature verification, identity,
membership, freshness or authorization. Borrowed input must be physically
valid, disjoint where required, live and unchanged during use. Applications
must perform their independent verification and policy checks.

The event projection applies the original event parser's bounded JSON and
kind rules; the generic protobuf `uint32` validator does not enforce the
Nostr kind comment's 0..65535 range. The event parser limits tags and values;
the FI parser limits header, payload and audiences more narrowly than the
generic descriptor. Unknown fields are not authority. #ef04 tracks explicit
schema numeric ranges; #5869 tracks generated getter error precedence.

The consumer source and descriptor identities are recorded in
`2026-10-29-proto-consumer-inputs.json`. The tool used was Coax binary SHA-256
`2169233ded48c68186dcad149ec43bf81b351d7482663ebb64a52b9fb28629a3`;
importer SHA-256 was
`66cca8e68a0417a7f92b3bb5cec20eaceb73f8fc5f801692b49edd0fbc8b61f8`,
and wire runtime was
`bf7f55c25616bcabe0417b0ed2cfa56272e75545b2dc8e13c961d88bb458b87a`.
`coax deps` was run with the documented 250,000,000 aggregate compilation
steps; the checks retained the default 20,000,000 steps per check.

Reproduction on the recorded tool and sources:

```sh
coax/bin/coax test coax/lib/nostr/adapters/event-projection.coax --run-steps 250000000
coax/bin/coax test coax/lib/nostr/adapters/fi-projection.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/adapters/event-projection.coax --run-steps 250000000 --mutation-reference --mutation-trace
coax/bin/coax mutate coax/lib/nostr/adapters/fi-projection.coax --run-steps 250000000 --mutation-reference --mutation-trace
COAX_IOS_SIMULATOR=<booted-simulator-udid> python3 coax/tests/nostr/event_projection.py
COAX_IOS_SIMULATOR=<booted-simulator-udid> python3 coax/tests/nostr/fi_projection.py
```

The fixture scripts require protoc, Node, OpenSSL, the local WASI toolchain,
and Xcode for the optional simulator part. Python is test tooling only: the
compiler bootstrap does not depend on it. The scripts compare actual generated
Coax projections against independent protoc and signed or forged fixtures,
exercise checked/release native and Wasm, and run sanitizer and simulator
fixtures. Wasm uses a 2 MiB stack; this is a target fixture configuration, not
an increased Coax check budget. They are not proofs for arbitrary descriptors,
and the simulator fixture is not a physical-device or app authorization test.

On the recorded graph the event checks passed 195/195 and FI checks 210/210.
The FI full-reference mutation run killed 3,162/3,162 with no equivalence
waivers; its trace SHA-256 is
`6c5913bf94d0a8047f982cef067fe9885b0cc3e0ac46e091f26aaadc030dc93e`.
The checked/release event (18 cases) and FI (19 cases) fixtures passed on
native, Wasm, sanitizers and iOS Simulator; protoc 36.2 round-tripped the
original fixtures, and the FI controls also used OpenSSL. Their log hashes are
`181d391d9836955eb46ebc90e7670d0d0490ea24c28689b99d4c3468440345b9`
and `fd342b24105183b2cb22a1c57b92d311b191121f271809690ac8a62e47e1e1bb`.

The complete corrected event reference run killed 4,306/4,306 mutants, with
no survivors or equivalence waivers. Its trace SHA-256 is
`c695531970a3cbe94606682b7aaea05ba4c2d88a2c4ecdb6164c75252105a21b`.
The earlier 4,302-mutant run failed with one survivor: the ordinary envelope
decoder's non-minimal wire path lacked a check. The corrected suite tests both
ordinary acceptance and minimal-decoder rejection of a non-minimal envelope;
the independent focused diagnostic caught that change and the four newly
reachable minimal-decoder mutations. Only the complete rerun is the authority
gate.

Independent review covered the source/generated correspondence and the event
and FI consistency boundaries under the documented borrowed-storage premise;
its findings about kind range and getter error precedence are tracked
separately. The event production logic was unchanged after that review; the
final test-only edit adds the non-minimal envelope boundary described above.
Neither the review nor these checks establish cryptographic or policy
admission. Earlier importer, runtime and bootstrap evidence is in
`2026-10-29-proto-generated-validator-review.md`.
