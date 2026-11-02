# Bounded retained-store checking (B1)

The compiler checks descriptor stores in every concrete procedure body, including void, uncalled, hidden check and instantiated generic bodies. It uses existing resolved declarations, types, calls and fields. The Coax policy is loaded from [retain-policy.coax](lib/compiler/retain-policy.coax), within the compiler fingerprint/bootstrap inventory. [B0](verification-retain-policy.md) records its earlier standalone foundation.

A write addresses storage; it does not address the previous descriptor's pointee. Current-frame inline copies and local descriptor slots remain local. Crossing a contained view resolves that descriptor prefix's backing. Parameter paths retain selected fields and wildcard dereferences. Normalized local/static array owners use the existing wildcard element sentinel for frame-relative classification, without exact element identity or a strong-update claim.

B1 source queries collapse static origins to their lifetime class, preserving descriptor type and role. Local and parameter identities stay exact. The B1 source-class mode stays disabled for addressed-storage and A1 return queries; their existing static-origin rules still apply. The shared query cache keys the source-class mode separately. Complete validated raw facts may serve source-class queries. Raw static/mixed spans pass through the existing source normalization into a temporary result before output publication. Class-to-raw value reuse, active rows and incomplete rows remain unsupported. Direct storage paths without dereferences compute their owned-slot fact without allocating a separate origin-query row or span. This projection proves only the static lifetime relation, without an alias or value-identity claim.

Known callee-local backing retained through caller/static storage reports E0383 at the original RHS, including definitions that are never called. Parameter-to-external stores retain sparse outlives obligations. Actual checked calls project only the referenced source and destination paths once into their caller namespace. A known local violation reports at the original call; unused scratch does not become an origin. When a retained source path selects an inline array behind a view parameter, caller substitution reads the actual array storage at that path. Its formal storage classification is computed once when the frozen canonical effect is published; each caller still resolves its actual source and destination. Source and destination known alternatives survive unknown arms. Later safe stores do not erase earlier may-effects.

Complete default-empty source descriptors differ from zero-length views of an owner and unknown facts. An unavailable helper return query or unrepresented dereferenced address stays manual; it does not become a complete-empty asserted destination. Value constructors lowered away from calls are not runtime call effects. Unmodeled foreign/intrinsic retention and active recursive dependencies remain explicit incomplete facts. There is no general recursive fixed point.

## Native observations and bounds

Optional observations expose procedure/call owner and site, independent fact/relation completeness, forbidden status and pending count. Consumers read only `CK_State.retain_report_count` after successful `CK_run`. Type, store, authority, Metal and resource rejection leave count zero. Accepted manual/pending rows are explicit. Raw report capacity and private cache tails are not an admitted result.

The current profile uses the single shared 2,097,152 work counter (owner-approved 2026-11-02 desktop adoption). Exhaustion still refuses E0381. Historical dated B1/B2 receipts retain their original 1,048,576 work counter. B1 separately memoizes borrow presence and mutable-descriptor discovery only after type instantiation, resets both private type caches once per run, and charges reset and discovery to the same work counter. Resolved direct local slots use their already-checked path and leaf type; view crossings retain the raw backing analysis. A separate B1 field cache indexes frozen concrete types and field ordinals in the existing query pool. Type-owned kind4 rows hold eight lazy field positions and stay separate from declaration-owned value/storage rows; reset, probe and miss work stay within the same budget. Failed lookups do not publish rows or fill slots. A1 contexts retain their ordinary field walk. Zero-depth B1 discovery queries reuse the corresponding frozen type fact; A1 and positive-depth discovery keep their existing traversal. Query misses reuse the type projection already validated before lookup. Origin sets and supplied-store outputs are bounded at 64, canonical per-procedure summaries at 256, logical paths at 24, and each supplied store at 128 raw pairs. Coax computes an additional bounded effect workspace, clamped from twice the authored node count to 128..16,384 rows. Native code allocates/transports it. Observations are disabled normally and add no report work then; an explicitly requested short output refuses E0381 with zero committed count.

Whole writes whose complete descriptor count exceeds 64 choose the existing
unknown-element array projection before visiting their first field. Every
source/destination element is represented, with distinct field ordinals; the
pass never publishes 64 exact effects and then discovers a 65th array effect.
Exact 64-element writes retain exact projections, and 257 distinct fields or
independent effects still refuse. A private frozen-type classification memo
publishes only successful results and resets on every retained pass. Its cold
field walk validates bounds and charges the existing work counter.

Retained whole writes stream their descriptor leaves through those same per-leaf buffers and limits. There is no separately materialized aggregate leaf set or fixed aggregate-width quota. Cold structural validation meters every field, including fields without descriptors. A private per-execution cache retains two recent complete validated immediate-field shapes with at most eight retaining fields each. Warm reuse validates and meters the selected fields and their recursive visits; larger shapes retain full streaming traversal. Exact type, declaration and field-head identity plus captured maximum visited field/type IDs preserve prefix bounds. The instantiated graph stays frozen during that execution; arbitrary changes to unvisited scalar links or types are outside this cache contract. After a child query rejects, structural traversal and later retained-store stages stop. Existing A1 query-internal diagnostic collection is unchanged. A late local witness still reports E0383 at the original RHS, and a late effect-capacity or work refusal still leaves the public report count zero. The initial B1 profile refused writes above 64 descriptor leaves or borrowed-array elements. The streamed profile replaces those structural refusals with the existing per-leaf and whole-pass bounds. A1 returned-value leaf limits are unchanged.

## Verification record

The [evidence record](../docs/verification/coax-retained-stores-2026-11-01/record.json) binds actual source, producer, raw outcomes and artifact identities. Check-first history retains 16 forbidden programs accepted by the earlier compiler, typed-graph assertion REDs, the original failed 909-check baseline and affected continuations. The record distinguishes Coax program observations on the earlier qualified producer from qualification of a new compiler image, and retains the analysis-cost attribution that motivated the frozen field cache.

Final qualification passed: all 165 fresh strict normal statements, 9,253 full-scope mutants killed with zero survivors, 984 authored checks, deterministic C/header and compiler fixed points, all 33 source cases and the 19-case native corpus plus disabled/short controls at O0/O2/ASan+UBSan. The seven whole final-image gates also passed the 44 returned-borrow sources, legacy checker, actual System One consumer, 1,000 independent returns and diagnostic-schema checks. Each receipt retains its actual producer; earlier results are not renamed as current observations. The matching-image compiler analysis used 1,047,475 of the unchanged 1,048,576 work steps; subsequent growth needs measured cost reduction.

Run the source and native gates with the explicitly selected compiler:

```sh
python3 coax/tests/retained_borrows.py --compiler /absolute/path/to/coax
python3 coax/tests/retained_borrows_native.py --compiler /absolute/path/to/coax
```

These drivers also run in the ordinary compiler test chain. They are maintenance tooling and introduce no runtime dependency.

## Helper-write continuation

The bounded [helper-write return slice](verification-return-borrows.md#helper-writes-into-returned-carriers)
composes selected checked writes through formal views into local carriers and
their returned descriptor fields. Its write-only query excludes incoming
descriptors; caller defaults and all possible writes join separately. Nested
helpers, aliases, actual formal positions, local witnesses and incomplete
recursive alternatives remain distinct. Unrelated reads add no returned
origins, and later safe writes cannot erase earlier may-effects.

This precise composition applies to raw return queries. Retained-store-only
misses keep B1's manual helper boundary, while already complete raw facts can
serve source-class queries. The fallback supplies no write-free proof. The
opacity scan can only add incompleteness, so an independently incomplete
retained fact skips it; complete retained facts still require the scan.

The actual [`so_post`](lib/systemone/transport.coax) chain selects caller `out`
for `body`, caller `err` for `stderr`, static messages for `problem`, and scalars
for `usage`, with modeled default-empty alternatives. Selected return facts do
not discharge incomplete foreign-retention or host-entry outlives obligations.
The current profile uses twenty-four-step paths, depth 32 and 64-origin sets.
The owner-approved shared work cap is now 2,097,152; the dated receipts below
retain their original 1,048,576-work budgets.

The [early-exit record](../docs/verification/coax-b2-early-exit-2026-11-01/README.md)
binds the changed guard's focused proof and matching-image full compiler cost:
**1,044,325 / 1,048,576 steps; 4,251 spare (0.41%, owner accepted)**. The
[failed candidate record](../docs/verification/coax-helper-write-returns-2026-11-01/README.md)
retains 220/220 killed helper-entry mutants, 63 native `CK_run` observations and
138 actual `so_post` buffer observations with their original producer and
continuity limits. These retained receipts are not fresh full-checker mutation
or normal release/bootstrap qualification. The B1 qualification above keeps
its own source and producer identity; a full desktop build remains separate.

## The real request owner and remaining boundary

The [canonical System One owner](lib/systemone/owner.coax) and
[typed outcomes](lib/systemone/outcomes.coax) now apply that compiler profile
in the actual Lab consumer. A generation ticket fences stale operations;
terminal snapshot checks capacities before copying body/stderr/problem into
separate caller buffers. Stop and quit handle discard. The offline native
checks cover completion, cancellation, EOF-before-exit, process-slot reuse and
transcript storage after wire overwrite. The
[closure qualification](../docs/verification/coax-12625-closure-2026-11-02/README.md)
keeps source, producer, focused compiler checks and actual consumer evidence
separate.

The original 130 cache/origin mutation findings are now closed with independent
source/refusal and charged-work oracles. No private postfatal return value,
abandoned marker or unused tail is treated as an admitted result. The final
`CB_query` scope kills 371/371 mutants. V6 native/fixedpoint gates pass, the
authored baseline passes 1,256/1,256, and actual compiler cost is 1,036,428 of
1,048,576 steps (12,148 spare); the
[closure handoff](../docs/books/handoff-2026-11-02-12625-closed.md) records those
completed route. Earlier dated B1/B2/integration observations keep their original
source and producer identities and are not renamed as current results.

This is a serialized canonical-owner protocol. Arbitrary owner-value copies,
races, overlapping snapshots and host-entry outlives duties are not
automatically discharged. By-value mutable-view carriers (including sums),
broader context/getter cases, recursive fixed points, foreign retention and
nested-block/loop lifetimes remain manual. An unknown remote outcome never
proves safe retry. The historical desktop settings/Lab receipt retains its eight-step path refusal
E0381. The 2026-11-02 dictation continuation extends represented paths to twenty-four
without discarding an origin or admitting a local escape. Its initial slice
kept the 1,048,576-work cap; the later owner-approved
[desktop adoption](../docs/verification/coder-dictation-2026-11-02/adoption/README.md)
records the 2,097,152-work profile and separate application result.
R0 device/reviewer and the earlier failed page-golden gates remain separate.

The current internal twenty-four-step representation and whole-write array precision
are recorded separately in the [2026-11-02 compiler continuation](../docs/verification/coder-dictation-2026-11-02/compiler-fix/README.md).
Earlier dated receipts keep their original eight-step representation, source
and producer identities. Native headers must be regenerated with their C
objects; this changes no protocol wire message or general lifetime proof.
