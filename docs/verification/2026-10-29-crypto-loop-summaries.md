# Bounded source summaries for public-loop crypto

The repaired aggregate auditor reached its 262,144-visit limit on the actual
`smc_multiply` closure after nine procedures and one guard. A bounded lookup
cache alone reached 39 procedures and three guards but still refused at the
same limit. The completed summary prototype analyzes 43 reachable procedures
in 97,203 visits and reports three authored secret-derived guard obligations.
Its result is `accepted=false`, `analyzed-with-guard-obligations`; this is not
full secret-path admission.

A summary exists only within one `AG_check` of an immutable checked source
graph. Parameter type admission and by-value region copying precede lookup.
The key contains the resolved procedure and normalized argument types, origin
masks, literal identity, bounds, knownness and extents. Inputs with view
aliases do not use summaries. Stored returns contain value facts but no region
identity; a hit creates a fresh result region. The first completed analysis has
already added its guards and reached procedures to monotone invocation-wide
sets. Summary hits retain those sets, propagate their transitive reached bits and
maximum logical call depth, and fall back if an active ancestor intersects or
the depth bound would be exceeded. Failed, partial, view-returning and
unsupported analyses do not create summaries. These rules rely on the source
profile's refusal of unknown/foreign effects and on no view argument crossing
the summary boundary.

The existing visit, 16-call-depth, 64-reached-procedure, 4,095-region and
256-guard limits are unchanged. Each invocation has at most 128 name lookups
and 128 summary rows, with eight argument snapshots per summary and 16 scope
rows. A full cache falls back to direct bounded analysis. Summary and lookup
scans charge the existing visit counter; ancestor matching is separately
bounded by 16 active calls and 64 reached procedures. Name comparisons are
bounded by the existing source-size limit. No result or cache is shared across
independent audits, compilers or source graphs.

Seven independently compared parsed-source fixtures agree with the repaired
uncached auditor on status, return origin, reached count and guard records,
covering repeated pure calls, public-then-secret input, dirty aggregate copies,
view writes, zero-trip refusal, transitive variable-time refusal and guard
obligations. Additional tests exercise changed callees across audits, guard
context, summary key identities, fresh-region snapshots, cache saturation,
active ancestor and depth fallback, and exact range boundaries.

The tested policy SHA-256 is
`1875f648c5738d45b58d77ac5c763e64a7dedded7eb83a60c8bb6684f1f18a1d`
and the tested compiler is Coax 0.21.0 SHA-256
`73d2aeb0e1e57c355d87242946f89e1462f8b765f57087f4307d471a0277e1d4`.
The corrected graph passed 172 policy checks, 2,563 of 2,563 policy mutants,
and 10 parsed-source tests. The shared scalar policy passed 40 checks, 741 of
741 mutants and six integration tests. Mutation scheduling selected 19,539
checks and skipped 121,698 by the validated dependency scheduler, without
fallback or exemptions. The preceding graph had one actual surviving mutant:
skipping the first seen ancestor; the corrected graph adds that boundary case.
The earlier graph's full reference run was still in progress at this snapshot.
[Retained evidence](../../tools/coax-nostr/secret-check/evidence/aggregate-loop-summary-2026-10-29.json)
includes exact graph and compiler identities, actual closure reports, parsed
cases and the uncached comparison. The comparison driver takes an absolute
worktree path; run it once for a reference and once for the candidate.

An independent review of the frozen policy traced normalization, view
exclusion, by-value return reconstruction, recursive and depth fallback,
transitive summaries, and invocation-wide guards. It found no further concrete
memoization counterexample in the restricted compiler-checked subset.

The source review covers this restricted compiler-checked subset, not arbitrary
supplied intermediate representations. Existing assertions remain unproved;
object code, Wasm/JIT behavior, platform timing, erasure and production secret
use require separate evidence. Fixed-base and variable-base tasks remain open
until their full source scopes and remaining gates are independently revalidated.
