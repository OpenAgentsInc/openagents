# Public Nostr event ingress: partial evidence

This is a single-event building block for open task #8eb5, not a completed
store. `ns_admit_one` parses the original text, derives its event ID, verifies
the signature over that ID and refuses ephemeral events before writing one
original into an NSLG frame. It does not accept a caller-supplied verified flag.
The framing and policy helpers do not authenticate a frame, and a constructed
`NsAdmission` value is not authority for another operation.

The host must keep original bytes immutable through parsing, verification and
copying, and provide live, physically valid, exclusive and disjoint input,
scratch and output storage with a truthful overlap observation. Numeric range
checks do not prove those host properties. This partial provides no replacement,
deletion, duplicate-quota, replay or durable-recovery implementation; #f539
and #966b remain prerequisites of #8eb5. It handles public event data and does
not claim secret-flow, target timing or production approval. Historical
secret-source audit acceptances are suspended after checked-cast and arithmetic
overflow counterexamples.

The exact ingress source SHA-256 is
`b0c636a735536e615cd448b88c3a30e58b921ceed854ac6f31cf0a0b16e54090`.
Its independently reviewed runtime implementation is unchanged from the
earlier review; subsequent edits added wrong-signer, ephemeral and short-output
checks. The [source inventory](data/nostr-store-ingress-2026-10-29/source-inventory.json)
records its 31-unit tested closure and compiler. All closure files match this
snapshot. The compiler used for these tests was coax0 0.22.0 SHA-256
`f335e7ec7b3f6d5966244a09aa8002b11b5ba607a1f673353c51effd9dab84a1`;
this does not assert that later compiler source is identical.

The [full ingress mutation run](data/nostr-store-ingress-2026-10-29/admit-mutation.log)
killed all 4,534 mutants, with 249,658 full checks and none skipped. The
separately tested value-policy, layout and snapshot helpers killed all
[70](data/nostr-store-ingress-2026-10-29/policy-mutation.log),
[28](data/nostr-store-ingress-2026-10-29/layout-mutation.log) and
[196](data/nostr-store-ingress-2026-10-29/snapshot-mutation.log) mutants,
respectively. The positive ingress check used 18,688,237 of the unchanged
20,000,000 evaluator steps.

The retained [oracle report](data/nostr-store-ingress-2026-10-29/oracle-report.json)
records 65 signed, forged, ephemeral and boundary cases in each checked and
release mode on native and Wasm, plus a native address/undefined-behavior
sanitizer fixture. The test used a pinned external native signature oracle;
its binary identity appears in the report. The historical driver did not
inventory every Python helper at execution time: the inventory records their
current matching checkout hashes, which are not proof of their historical
bytes. Wasm uses a 2 MiB stack and a 16 MiB memory limit in this fixture;
these finite tests do not prove a general target stack high-water bound.
