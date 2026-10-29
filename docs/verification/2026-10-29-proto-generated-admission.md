# Generated protobuf validator admission

Generated message validators admit a borrowed view by checking its byte count
and nesting depth before accessing it. The shared Coax wire core now owns that
scalar admission policy as well as the initial depth and field budget. The
limits and behavior are unchanged: lengths from zero through 1,048,576 bytes,
depth through 64, initial depth zero and an initial 524,288-field budget.
The minimum depth remains unrestricted, matching the previous generated
predicate. Per-message descriptor limits and each nested field's budget are
still enforced by the generated validator.

Centralizing these values allows direct Coax checks of negative foreign view
lengths and the exact endpoints without constructing an invalid Coax view.
The native ABI still requires that any admitted, positive-length view point to
accessible storage of the stated length for its use; scalar count checks do
not establish pointer provenance or ownership. The change does not turn a
protobuf projection into authentication, identity or authority.

The generated source fixtures continue to check exact output, including each
message's policy. The event and Block FI adapter experiments independently
exercise actual descriptor-generated code, protoc interoperability, malformed
input, signed-source consistency and native, Wasm and iOS Simulator paths;
their source projections remain separate unshipped work until their own full
mutation and review gates pass.

The frozen historical declaration hashes remain in
`coax/tests/proto/import-baselines.json`. The characterization test reverses
only the admission, root-depth and budget substitutions before comparing each
generated declaration stream with that historical hash. It therefore detects
other declaration changes without rewriting the reference hashes.

Candidate evidence: importer Coax tests 122/122; imported event descriptor fixture 10/10 and identity descriptor
fixture 12/12; native alias refusal and ASan/UBSan malformed-view and importer
checks; and independent protoc 36.2 event and identity round trips. The source
projection experiments also passed 18 event and 19 identity cases in checked
and release native and Wasm, plus the corresponding iOS 26.5 Simulator
fixtures. These are correctness checks of the generated output, not an
assertion that the phone's production trust or key lifecycle is complete.
The Python-free bootstrap and its mandatory source mutation gates passed, and
`apps/openagents-ios/build.sh` completed an actual Debug simulator build in
this isolated worktree. No TestFlight build was created by this work.
