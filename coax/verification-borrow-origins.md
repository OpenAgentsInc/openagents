# Borrow-origin policy foundation

`lib/compiler/borrow-summary.coax` supplies bounded operations on explicit
borrow-origin facts. A0 qualified it as a standalone, unloaded `#authority`
kernel. The bounded [return-summary integration](verification-return-borrows.md)
now loads these operations in the checker. Retained request-buffer work remains
in `r3a-retained-request-borrows`. The original E0383 profile remains described
in [Local-array return diagnostics](verification-local-borrows.md).

For `BS_union` and `BS_substitute`, `ok` means that supplied facts passed
representation checks and the facts required for that operation were marked complete. The kernel does not establish
their source-level completeness. Source/type resolution, fact correctness,
return admission and request lifetime obligations belong to the integrating
checker and consumer. This foundation alone grants no lifetime-safety verdict.

## Two fact channels

Inline storage and descriptor pointees have separate identities. A local
record can contain a view into caller storage. Copying that record copies the
view descriptor; it does not make the pointee part of the copy. Conversely,
a view into an inline array parameter refers to the callee's by-value copy,
even if the caller's original array was static.

An origin is static backing storage, local backing storage or a relative
parameter projection. Relative queries distinguish inline storage from a
contained descriptor. Their exact logical paths contain checked field
ordinals, element indices and permitted dereference steps. They are type
facts, not target byte offsets. Dynamic indices need a conservative supplied
union or remain unknown. The kernel does no name lookup or path-prefix guess.

## Bounded operations

`BS_path_valid` and `BS_origin_valid` validate the representation.
`BS_union` constructs a canonical unique set. `BS_substitute` replaces only
used, exactly matched formal projections with their supplied actual facts.
Unused actual facts do not enter the substituted set. Substitution takes one level: terms
from actual facts are caller-relative and never reinterpreted as callee formals.

A complete empty set is distinct from missing or incomplete facts. A runtime
slice of length zero still retains its supplied backing origin; the kernel
has no slice-length input. By-value inline bindings always produce callee-local
storage. Contained-descriptor bindings preserve the actual pointee identities.

The internal profile bounds each input/output set and binding list to 64,
the shared origin pool to 256, projection depth to eight and candidate work
to 128 insertions. Duplicates count as work but consume no additional output
capacity. Status values distinguish `ok`, `invalid`, `unknown`, `capacity`
and `work`.

Outputs are unique and ordered by logical origin fields. Operations validate
and stage results before publication. Every refusal returns count zero and
leaves the entire output view unchanged; successful publication writes its
count prefix and preserves the tail. Native C padding is unspecified inside
that successful prefix. Source spans must be valid, immutable and disjoint
from output for the call, as required by the internal borrowed ABI.

## Qualification

The [2026-10-31 record](../docs/verification/coax-borrow-origin-foundation-2026-10-31.json)
binds 25 authored checks / 287 cases, 297 killed root mutants and 94 native
cases per profile at checked O0, O2 and ASan/UBSan O1. The initial mutation
failure and the ordering simplification remain recorded alongside the final
qualification. The final main merge added upstream checker changes after
that run; the record keeps the earlier producer attribution. Qualification
of the merged compiler belongs to the following integration child.

The focused gates are:

```sh
coax/bin/coax test coax/lib/compiler/borrow-summary.coax --library
coax/bin/coax mutate coax/lib/compiler/borrow-summary.coax --root-only --library
python3 coax/tests/borrow_summary_native.py
```

The source checks use independent small cases for origin roles, projection
identity, argument permutations, missing/empty inputs and exact capacity/work
boundaries. The native harness uses the generated header, independent LP64
layout and literal-result expectations, poisoned output guards and sanitizers.
It exercises policy operations; helper-return diagnostics, checker seeds and
System One request completion/cancellation are qualified by later children.

The historical A0 qualification did not load the kernel into the compiler;
its record binds that actual producer, source and unchanged compiled closure.
The later return-summary integration has its own checker/producer/seed record.
Neither record establishes general ownership or retained request lifetimes.
