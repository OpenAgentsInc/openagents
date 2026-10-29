# Aggregate source audit: metadata, byte wrapping and empty loops

The earlier aggregate auditor could accept a reachable excluded callee: a struct
initialized with field zero was publicly changed to one and then copied into a
by-value helper. The helper recovered the original literal from the clean new
region and skipped its reachable `else` branch. An identity aggregate cast also
discarded the dirty-region identity before the copy. Both witnesses return 8 in
checked and release builds for input 7 through that excluded branch. This invalidates
the general soundness claim for earlier aggregate-profile results; it does not
by itself invalidate arithmetic correctness evidence or revalidate other paths.
Separately, the earlier policy tracked public byte `255 + 1` as 256 even though
the release result wraps to zero. A release runtime witness reaches an excluded
callee on the zero branch while the earlier auditor accepted the path.
An independent review found that one-body loop analysis also assigned facts
from bodies that might never run, allowing another reachable excluded callee
to be skipped. Both an exact zero-trip loop and a possibly empty nested loop
reach the excluded branch in checked and release runtime witnesses.

The corrected policy discards constructor metadata when copying a dirty source
region and conservatively refuses aggregate casts. Real parsed-source cases
exercise by-value, local, nested and indexed copies, plus struct and array
casts and a clean-copy positive case. Byte bounds exceeding 255 are discarded,
and the integration suite checks the release wrapping witness. Public loops
that might be empty now refuse. A deliberately reverted policy rule
reproduces the false acceptance while the executed runtime witness reaches the
excluded function. The corrected source refuses those counterexamples.

The frozen policy SHA-256 is
`d55174aaec6ea56112ddca5804db515fdd73f4bbb1be6e2651952d3d5a980610`;
the Coax checks SHA-256 is
`f55e5a7e4620e3f898e700d88fb40f6295070f1da895d5b2043827d2627e1be8`;
the parsed-source test driver SHA-256 is
`bca2cf309a7a106f58a48e411c87237d1fad5b941fadb589e48245fa0e93c247`.
The compiler is version 0.19.0, SHA-256
`5d9239a19f3567bf6691c0db034124ead36952885d2fb08bfb5627425fc190ce`.

The policy suite passed 160 checks; its full mutation gate killed 2,395 of
2,395 mutants, without exemptions. Scheduling selected 14,375 test runs and
skipped 103,786 by the validated dependency scheduler, with no fallback. Nine
parsed-source tests passed, including the runtime witness, deliberate broken
rule control and native sanitizer coverage; the six scalar-profile regressions
also passed. The retained [observations](../../tools/coax-nostr/secret-check/evidence/aggregate-stale-literal-2026-10-29.json)
contain exact source identities and individual case results.

Revalidation covered `sc_mul` and `sg_choose` as admitted source, and
`sf_multiply` and `sf_inverse` as analyzed with unresolved secret-derived guard
obligations and `accepted=false`. These are the selected closures tested, not a
blanket revalidation of previous claims. Aggregate scalability, guard proofs,
object and Wasm/JIT inspection, timing, and production secret-use approval remain
separate open gates.

An independent source review of the frozen policy traced aggregate copies,
fields, indices, slices, unions, argument and return binding, partial and view
writes, loop marks, and byte interval arithmetic. It found no additional concrete
false-accept path within the restricted checked-source subset. That review is
not approval of arbitrary supplied intermediate representations, implicit
compiler guards, target timing, or production secret use.

Task #79d5 was reopened for this repair. Fixed-base task #94a3 was also reopened
because its secret-source acceptance depends on the affected auditor; its
independent arithmetic and mutation observations remain evidence of their stated
scope. Variable-base task #3e29 had already recorded a visit-limit refusal, not
an aggregate-source acceptance, and remains open behind the scalability and
guard gates. The evaluator-resource work in #7287 concerns compiler evaluation
limits and is not revalidated or invalidated by these source-flow results.
