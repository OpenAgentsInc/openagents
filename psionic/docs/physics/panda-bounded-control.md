# Making `pc_step` genuinely compiler-bounded

Read-only design audit, 2026-10-31. Latest observed `origin/main`: adbcbbae477b66f7635b3740dc68869ac1a8db3b.
The latest main R3/R11/R14 roadmap and spec were read with `git show`; the
working controller/matrix sources still match all five SHA-256 bindings in the
retained effects report. No tests, SDK steps, models or old inventories were
rerun. The audit changed no product code and ran no old gates. This document preserves its findings; it does not implement bounded admission.

## Assessment

`pc_step` is a good, small consumer for a staged implementation, but cannot
honestly be admitted as `#bounded` today. The source has a finite, nonrecursive
numeric call graph and no explicit allocation or I/O. The compiler does not yet
prove its borrowed storage, intrinsic resources, loop limits, fatal paths,
caller scratch or target stack. Fixed arrays and previous numerical/sanitizer
passes do not discharge those obligations.

The next smallest honest compiler delivery is a refusal-first bounded-contract
checker backed by resource/region/loop/frame facts, with a very small proven
scalar consumer and explicit refusals for this controller. Acceptance of the
controller follows only when its actual transitive obligations are closed.
Full R14 also requires a Verse per-frame consumer; neither a scalar proof nor
this Panda consumer alone completes that roadmap milestone.

## Existing evidence and the actual boundary

- `coax/ROADMAP.md`, R3: simple local-view escapes are checked, but
  interprocedural/aggregate-contained/general-region lifetimes are unfinished.
- R11: summaries must cover allocation, blocking, I/O, clocks, random, foreign,
  atomics, actual aliases/regions, instantiated helpers/callbacks, cleanup and
  error paths. Unknown foreign effects require reviewed summaries; unknown
  must not be treated as absence.
- R14: a contract declares maximum stack and a **named caller-owned fixed-size
  scratch region**, forbids allocation/blocking/recursion/unbounded loops and
  unknown callees transitively, proves visible loop bounds/nonescape, reports a
  per-target stack bound from lowered form and compares it with native evidence.
  An uncomputable bound is a refusal, not an estimate.
- `coax/EFFECTS.md` and `docs/verification/coax-effects-2026-10-31/panda-pc-step.json`:
  `pc_step` reaches **35 of 53** checked procedures; no reachable recursion;
  mask **125 = host_io | unknown | may_trap | borrowed_read | borrowed_write |
  intrinsic_resources_unqualified**. There is no reachable explicit foreign
  procedure bit, uninitialized-storage bit or address-observation bit here.
  These are inspection facts, not a no-allocation/blocking certificate.

The exact bounded candidate is the numeric controller call. The C SDK forward,
telemetry/mass/Jacobian collection, rendering, neural work, clock reads, trace
I/O and outer-loop cleanup are **outside** `pc_step`. They cannot be smuggled
into the claim by saying the controller is bounded. The genuine loop remains
an SDK evaluation; 500Hz is nominal simulation cadence, not a proved 2ms wall
execution deadline.

## Closed source call graph

The retained graph has these 35 reachable source procedures (not the checks):

- Controller: `pc_step`, `pc_sample`, `pc_values`, `pc_count`,
  `pc_rotation_ok`, `pc_close`, `pc_determinant`, `pc_limit`, `pc_state`,
  `pc_layout`, `pc_goal`, `pc_clip`, `pc_side`, `pc_zero`,
  `pc_delta_rotation`, `pc_quaternion_matrix`, `pc_outer`,
  `pc_orientation_error`, `pc_sign`.
- Matrix: `pm_inverse`, `pm_size`, `pm_values`, `pm_count`, `pm_finite_bits`,
  `pm_symmetric`, `pm_mul`, `pm_pair`, `pm_pinv`, `pm_pinv_limit`,
  `pm_sweeps`, `pm_greater`, `pm_orthogonal`, `pm_rotation`,
  `pm_large_rotation`, `pm_cosine`.

There are no source callbacks, `#foreign` functions, dynamic containers,
recursive calls, `defer` blocks or owning resources in this reachable numeric
closure. Test fixtures are loaded but not reachable from this product root.
The compiler must still handle/refuse hidden effects through those constructs
in negative contract tests; their absence from this consumer is no waiver.

## R3: exact storage obligations

`pc_step(state: []PcState, sample: PcSample, action: []f64, new_goal: bool)`
checks `state.count == 1` and `action.count == 7`, reads the old controller,
computes a local next state and writes `state[0]` once at the end.
`PcSample` is a by-value 144-F64 record. `PcState` and `PcOutput` contain values,
not borrowed members; no borrowed result escapes by source inspection.

What is missing from compiler enforcement:

1. Valid allocation, alignment, extent and lifetime of the raw borrowed state
   and action spans, plus exclusive state access/no concurrent invalidation.
   Counts alone do not establish those facts at the exported C boundary.
2. Region/range permissions for state reads and final write, action read-only
   access and local array slices passed into `pc_values`. The current inventory
   knows read/write through a borrowed base, not exact bytes or disjointness.
3. Context-sensitive bounded counts/nonescape for `pc_values(n, a, limit)`:
   its foreach iterates `a.count`. Its actual controller call sites supply
   fixed local arrays (largest 49) or the admitted seven-element action span,
   but the helper's type alone allows an arbitrary borrowed view.
4. Last-use/alias rules across nested calls. A conservative first slice can
   require disjoint action/state/scratch regions; a more permissive rule must
   prove read-before-final-write and all lifetimes rather than infer safety
   from field names. A runtime overlap observation also needs its own qualified
   resource summary; it does not create ownership.
5. Named caller-owned scratch and noescape. The current solver allocates local
   fixed arrays, not an R3 scratch region. Moving them into a workspace without
   borrow/alias/lifetime enforcement would still not satisfy R14.

A possible migration keeps the old C ABI as an explicitly unbounded acquisition
wrapper and introduces a new, versioned value/fixed-borrow core with a verified
caller scratch region. Making action `[7]f64` and next state a returned value
can narrow outer borrow obligations, but local slices, intrinsic calls,
initialization/copying, stack and scratch still require proof. It must not be
advertised as bounded merely because its arguments are fixed arrays.

## R11: remaining intrinsic and failure effects

The current root witness for trap/I/O/unknown and borrowed-read is
`panda-control.coax:94`, `state[0].ready`; the final borrowed write is line112.
The intrinsic witness is line105, `sqrt(150.0)`. A shortest witness per effect
is not an exhaustive list of sites to discharge.

Relevant intrinsic families are `abs`, `sqrt`, `sin`, `cos`, F64 bit conversions
and F64→F32 narrowing. Source bit conversions lower to fixed-size `memcpy`;
math lowers through the native math backend. They currently have unqualified
resource facts. C-generated aggregate copies/zero-initialization may also become
native memcpy/memset helpers; ABI copies and backend-introduced calls must be
included in the target resource model.

Qualify concrete primitive lowerings/implementations per target: allocation,
blocking, stack, argument/result domains and any errno/FP-environment/lazy-link
obligations. Hardware/fixed bit operations can be the first small slice. Do
not clear all intrinsic unknown bits by name or replace foreign/resource
review with a trusted label. There is no explicit SDK FFI call inside this
controller; SDK foreign facts block an annotation on the **outer loop**, not
this source root by themselves.

Always-on indexing, slicing and narrowing sites remain even under wrapping
arithmetic. Current native `coax_trap` does `fflush(stdout)`, `fprintf(stderr)`
and `abort()`: stdio can block/allocate and fatal termination promises no
normal cleanup. Wrapping merely drops conditional integer-overflow sites; it
is not a no-trap/no-I/O solution. Either prove each trap unreachable under the
admitted region/domain/loop facts, or refuse; do not disable guards to gain a
passing resource mask.

## Visible finite loop obligations

| Family | Actual bounded work to prove |
| --- | --- |
| Controller layout | `pc_layout()` returns xyz3/task6/joints7; propagate those facts or prove callee size guards |
| `pc_values` | View count refined by call site (max49 here), chained finite counters at most144 |
| Rotation/goal/orientation | Fixed3/4 loops; outer-product4×4; index arithmetic stays in the declared arrays |
| `pm_mul` | Rows/inner/cols1..7 after `pm_size`; max343 multiply-accumulate visits per call |
| `pm_inverse` | Cholesky and two triangular solves, all bounded by admitted n≤7, including descending indices |
| `pm_pinv_limit` | n1..7, sweeps0..64 checked before loops; triangular pair and row loops; early break/continue do not enlarge the budget |
| Controller selected pseudoinverses | One n6 and two n3 calls; at most960+192+192=1344 pair visits over64 sweeps each |

There are five `pm_mul` calls, one seven-joint inverse and three pseudoinverse
calls per successful controller step. A first compiler range proof can use
n≤7 uniformly (looser but valid) before adding specialization. It needs guard
dominance, simple predicate summaries (`pm_size`, `pm_sweeps`), interval/index
arithmetic and loop/call composition. The existing effect inventory explicitly
does not infer iteration bounds. A source `for` is not sufficient when its
endpoint comes from a view, parameter or returned layout record.

`pm_pinv_limit` returns convergence failure when64 sweeps do not settle; no
unbounded retry exists. Resource boundedness is independent of numerical
convergence. For the later bounded-solver library contract, note that
`PmResult.residual` is declared but not populated by these routines, and used
sweep count is not returned. The default zero field is not a measured residual.
This is a solver-library evidence gap, not justification to change its already
qualified arithmetic during the initial compiler work.

## Error, panic and cleanup coverage

All ordinary controller failures (shape, number, invalid state/pose/limits,
mass singularity/range or pseudoinverse convergence) return before `state[0]`
is published. The output is a value; the controller owns no resource needing
cleanup. `pc_step` itself has no `defer`, allocation or SDK cleanup path.
Those source facts make a small proof feasible, but compiler enforcement must
cover every early return and possible fatal edge, not just the successful
numerical fixture path. R2 `try`/mandatory error syntax is still planned;
current status enums and caller checks are not that general compiler guarantee.

The outer host's `REQUIRE`/I/O/resource teardown and partial SDK effects are a
separate nonbounded boundary. Current targeted failure ledgers prove abort
behavior, not worst-case cleanup resources. Generic/defer/callback/foreign
negative fixtures must remain refused until their complete paths are known.

## Stack and scratch: current evidence is insufficient

No compiler-derived per-target controller frame/call-stack bound or native
stack high-water comparison is provided by the existing control qualification.
Data-layout queries, ASan, process RSS and observed latency are not substitutes.

For scale only, the source materializes four49-F64 work matrices (1568 bytes)
and nine `PmResult` values (3744 bytes with the current LP64 value layout), plus
`PcState`176, `PcOutput`96, other arrays and a1152-byte sample. These are **not
a stack bound**. Callee frames, by-value array arguments/returns, hidden sret
and evaluation temporaries, alignment/spills, live ranges, initialization,
libm and fatal paths can change actual peak requirements.

The proof must consume lowered target storage/call facts, assign named scratch
banks and nonescape/lifetime facts, bound every frame/temporary and compose the
maximum transitive active stack. Unknown native math/helper frame costs refuse.
Native measurement then checks the computed conservative bound for each claimed
target/compiler/profile; it must not replace derivation with an empirical max.
C emission makes ABI/host compiler lowering part of that evidence boundary.

## Smallest honest delivery sequence

1. Define a narrow R3 fixed-region/borrow contract: exact spans, access modes,
   caller lifetime, noescape and fixed scratch. Retain an explicit unchecked
   FFI acquisition boundary. Full owning containers need not be invented first,
   but these selected storage facts cannot be skipped.
2. Add reviewed primitive/target resource summaries and complete per-site
   witnesses, including native traps and generated copy/zero helpers. Unknown
   foreign/intrinsic/callback facts continue to refuse.
3. Implement a bounded contract admission/refusal report in Coax over resolved
   lowering facts. Start with a closed scalar helper such as `pc_side` and
   constant finite loops, **only** when its stack and named scratch obligation
   are actually represented. No decorative accepted annotation, zero-mask
   waiver or guessed stack constant. Leave `pc_step` refused with precise paths.
4. Add guard-derived span counts and integer/index/loop/call budgets for the
   actual35-procedure controller closure, then target frame/scratch derivation.
   Preserve error returns and source numeric semantics; prove or refuse traps.
5. Qualify a new bounded controller core against the unchanged numerical proof,
   resource observations and measured stack, with negative injections of
   allocation, blocking, recursion, unbounded loops, unknown foreign calls,
   generic/callback/defer-only bad paths and scratch escape/aliasing. Admission
   changes whenever a transitive source or target fact changes.
6. Add the required Verse per-frame kernel and target coverage before calling
   R14 delivered. Real-time scheduling/watchdogs remain distinct from bounded
   computation and from nominal simulation rates.

Do not rerun original models or full SDK episodes for these compiler slices.
Changed compiler facts/negative admissions, a tiny native control slice and
specific storage/stack/resource measurements are the relevant new evidence.
