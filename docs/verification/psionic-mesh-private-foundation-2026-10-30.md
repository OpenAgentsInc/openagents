# Private mesh foundation — 2026-10-30

This scoped change defines the existing public Pylon receipt/check/pool
projections, a separate private replay witness, a public-only reproducibility
bundle, and the exact checker-owned inconclusive observation. Published kinds
and versions retain their meanings. No payment or production-key path is added.

The four pure policy roots passed **145/145 selected mutants**, with zero
survivors or equivalent waivers: selection 12, message openings 61, generated
witness admission 30, and exact original binding/context/times 42. Mutation
uses the optional entry-source filter; all imported baseline checks still run.
The [retained commands, source/compiler hashes and logs](data/oct30-mesh-private-foundation/results.json)
identify this evidence. Unchanged cryptographic dependencies were not re-gated.

The owned descriptor plus `pylon-mesh`, `mesh-witness`, `mesh-pool-inputs` and
`mesh-observation` profiles compile and pass namespace checks with pinned
protoc 29.3. Witness synthetic decode vectors were independently serialized
through that protobuf implementation. The encoder uses actual generated typed
setters; native/protoc carriage and live checker integration are separate gates,
**not claimed complete by this foundation record**.

A witness is memory-only through a non-TTY FIFO or Unix socket. It includes
only nonce-specific opening keys, never a signing key or reusable conversation
key. Openings authenticate the original signed ciphertext and exact plaintext;
public receipt hashes alone do not prove completion or model execution.
