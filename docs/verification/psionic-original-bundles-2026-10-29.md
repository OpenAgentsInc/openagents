# Psionic original rank-five bundles

Verified 2026-10-29 with coax0 0.16.0, SHA-256
`5388de6c9c976ff6328fd2f4f4aa3433e28d529deaac5f78a74abaab39c52498`.
The bundle root passed 103 Coax checks in debug and release and killed all
2,717 mutations, without exemptions. Its native harness passed debug and
release on the Mac, plain and ASan+UBSan; generated release C and the same
harness passed plain CPU on `coderos-4080` (Linux x86-64). Linux sanitizers
and GPU execution were not tested.

On the Mac, the pinned Elephant original was authenticated and admitted by
Coax as 760 tensors covering 18,819,627,488 payload bytes. The original
Mammoth two-shard bundle was authenticated and admitted by Coax as 1,184
tensors covering 54,713,457,120 payload bytes. That full Mammoth run used the
first implementation of the optional parameter-count check; after refactoring
its bounded sum, the current index validator was separately run against the
same two real headers and index and passed all 1,184 entries and both totals.
Independent SHA-256 checks matched both pinned Mammoth shard digests. The
[model metadata and verification procedure](../../psionic/docs/original-bundles.md)
include reproducible manifests and original-file harnesses.

These results establish original bytes and inventory admission, not model
numerical parity, complete request handling or serving. The focused root ran in
this increment; the full earlier Psionic suite was not repeated. The
[source hashes](psionic-original-bundles-2026-10-29-sources.json) identify the
checked source, fixtures and compiler.
