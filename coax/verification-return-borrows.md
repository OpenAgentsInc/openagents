# Bounded return-borrow summaries

The return-summary checker extension follows the standalone
[origin-set foundation](verification-borrow-origins.md). It uses the checked
Coax graph to follow returned string/view descriptors, including descriptors
inside ordinary records and fixed arrays. The checker remains the resolver
of names, concrete calls, types and field identities. This adds no runtime
ownership operation, allocator, implicit retain or deep copy.

## Storage and returned descriptors

An aggregate owns its inline storage. Its contained views and strings can
point elsewhere. Copying a record copies those descriptors without rebasing
their pointers into the copied record. A returned record that still points
into its own old local array therefore remains invalid even though the array
value is also copied. Returning an owned fixed-array value without an escaping
descriptor remains valid.

For supported return paths, a known local backing origin produces E0383 at
the original return expression. This includes calls through checked forward,
loaded and concrete generic helpers, local aliases, nested record/array
descriptors and direct local field assignments. Exact field projections and
actual parameter positions matter: a helper that returns only `pair.selected`
does not inherit the origin of `pair.other`, and an argument that is merely
read is not automatically retained in the result.

Direct slices of by-value array parameters and their inline-array record
fields retain the existing E0366 immutable-parameter refusal. The checker
does not widen parameter mutability to change that diagnostic. Copying such a
value into a local and returning a view of the copy is E0383.

## Helper writes into returned carriers

Raw return-origin queries follow selected descriptor writes through checked
view parameters into a caller's local carrier, then read those facts when the
caller returns the selected field. Only referenced formal paths are
substituted through actual arguments. Scratch merely read by a helper adds no
returned origin. Nested checked helpers and local aliases use the same
may-origin join, so a later caller-backed write cannot erase an earlier local
witness.

The write-only query excludes the destination's incoming descriptor. The
caller's initial/default fact joins separately with possible writes. A modeled
default-empty descriptor is complete empty; active recursion remains incomplete,
and known local backing survives unknown alternatives. A callee-local descriptor
written through an external parameter still reports E0383 at the original RHS.
A caller returning its own local backing through a helper is rejected at the
original return expression.

Retained-store-only queries without an already complete raw fact keep the
manual helper boundary. They do not expand helper writes on demand; complete
raw facts can still supply source-class queries. This fallback proves neither
write-free behavior nor a safe lifetime. Once a retained fact is independently
incomplete, the opacity scan can only preserve that status and is skipped;
complete facts still run the scan. This avoids redundant work without changing
the shared budget or turning an unknown fact into complete empty.

The actual [`so_post`](lib/systemone/transport.coax) chain populates a local
`SoCall` through `so_start`, `so_step` and `so_fail_call`:

| Returned field | Possible backing within this slice |
| --- | --- |
| `body` | Caller `out` (formal 2), or modeled default empty |
| `stderr` | Caller `err` (formal 3), or modeled default empty |
| `problem` | Static messages, including static arguments to `so_fail_call` |
| `usage` | Scalars without borrowed descriptors |

The local carrier, timeout scratch, client fields and request-body reads add no
unrelated returned byte origins. A selected `body` excludes an unrelated local
error buffer; selected `stderr` excludes an unrelated local output buffer.
Returning a selected field backed by the caller's own local array is E0383.
These facts do not discharge foreign retention or a host-entry outlives duty.

## Incomplete summaries remain manual

The internal procedure status distinguishes complete analysis from
incomplete/manual analysis. Recursive active queries and unsupported helper
writes remain incomplete. They must not turn into complete empty sets.
Known local origins still produce E0383 when another part of the returned
value is unknown, including when a conditional is passed through an identity
helper. Unknown-only source acceptance is not a lifetime-safety verdict.

By-value carriers containing mutable views, unmodeled addressed getters and
general context/region lifetimes remain outside the supported composition.
The selected returned-field facts of `so_post` coexist with incomplete
foreign-retention facts. Observing native returned buffers does not establish
complete request-buffer lifetime safety.

`CK_text` has a different, supported shape: it reads locally assembled parts,
copies their bytes into the caller's `c.bytes`, and returns only that output
storage. `CK_join2`, `CK_join3` and `CK_number` must remain valid; their local
parts or formatting scratch do not become retained output origins.

The analysis uses durable declaration identities, so reused names in sibling
blocks do not merge distinct locals. Its may-origin joins do not prove that
a later overwrite removed an earlier borrow. General recursive fixed points,
shorter nested-block regions, ownership transfer and foreign retention remain
outside this extension. The separate [retained-store profile](verification-retained-stores.md)
checks bounded stores into longer-lived caller state. Completion, cancellation
and late-callback lifetimes remain separate work; the borrow-summary umbrella
stays open.

## Storage behind a view parameter

A helper can return a slice of an array inside its view parameter's pointee,
such as `field :: (u: [] Ui) -> [] Field { return u[0].fields[0..<1]; }`.
Its summary names that array's storage, reached through the parameter's
descriptor. At a call, the checker reads the argument for its storage at the
same place, so a caller that passes its own view on (`ft(field(u))`) returns
a borrow of its caller's storage, and a caller that passes a local array
(`ft(field(a[..]))`) is E0383. A summary query at an array place, and a
dereferencing storage path through a call or a literal, also ask where the
storage lives, not which descriptors it holds. Before #12611 the argument was
read for descriptors, which made the first form E0381 and accepted the second.

Static data lives for the whole program, so a static origin never decides a
returned borrow. A constant is one static origin, not one for each literal in
its initializer. When static origins alone would overflow a 64-origin set,
they collapse to one per type. Local and parameter origins never collapse.

## Bounds and verification

The Coax checker admits bounded query/origin workspaces and bounded projection,
depth and work. Malformed storage or capacity/depth/work exhaustion is E0381,
with no native resolver fallback or silent safe-empty result. Static origins
that alone overflow a set collapse first, as described above. The native bridge
supplies owned storage; it does not decide borrow origins.

Memo lookups follow only the queried declaration's chain, so independent
procedures do not consume work scanning one another's queries. Query rows stay
append-only, and an active recursive row remains incomplete. The checker
rebuilds the invocation's heads for procedures, parameters, local declarations
and iteration bindings before any query; a second check cannot reuse an earlier
invocation's scratch results. Initialization scans the admitted node prefix,
clearing and charging only declarations that can own queries. Inapplicable
expression nodes create no memo work.
These memo links and workspace tails are internal scratch, not a published
summary format. The query, origin and work limits are unchanged.

Helper-write descent shares the existing depth-32 guard, twenty-four-step paths,
64-origin sets and 1,048,576 work counter. Calls and arguments stay within the
checked graph's admitted used prefixes. No parser allocation ordering is
assumed. Resource exhaustion remains E0381, with no public summary ABI change.

The source gate is maintenance tooling:

```sh
python3 coax/tests/borrow_summaries.py --compiler /path/to/matching/coax --record-dir /tmp/coax-return-source-record
python3 coax/tests/borrow_query_work.py --compiler /path/to/matching/coax --record-dir /tmp/coax-query-work-record
```

It checks the original fixture files directly and compares exact physical
diagnostic paths, lines, columns and codes. Its closed manifest distinguishes
rejected sources, accepted sources and accepted/manual sources. The last
category still needs an independent graph/native observation of incomplete
status; source acceptance alone cannot supply that observation. It never
executes the recursive fixtures.

The separate work regression checks a whole file containing 1,000 independent
caller-string-return procedures under the normal limits. It requires successful
compilation, without inspecting memo ordering or increasing the work budget.

The original integrating qualification separately binds the actual compiler, checker
source and generated seeds; runs the affected Coax authority/mutation gates;
reproduces both bootstrap fixed points; and observes native caller-buffer
returns under the named optimization/sanitizer profiles. Historical producer
results remain historical. Finite checks, reproducible seeds and source
acceptance do not establish general memory safety.

The [early-exit record](../docs/verification/coax-b2-early-exit-2026-11-01/README.md)
binds the changed guard, its focused proof and the matching full compiler input.
Actual-compiler cost under the unchanged budget: **1,044,325 / 1,048,576 steps; 4,251 spare (0.41%, owner accepted)**. The owner accepted this
measured margin for publication; the budget is unchanged. The
[failed helper-write candidate](../docs/verification/coax-helper-write-returns-2026-11-01/README.md)
remains historical: it reached work 1,048,577 against 1,048,576 and supplies
retained receipts for 220/220 killed helper-entry mutants, 63 native `CK_run`
observations and 138 actual `so_post` buffer observations. Their original
producer identities and continuity limits remain in that record; they are not
fresh qualification of every declaration in the changed compiler.

Focused guard checks and matching-image cost do not establish normal release
qualification, bootstrap fixed points or a full desktop application build.

The current internal twenty-four-step representation and whole-write array precision
are recorded separately in the [2026-11-02 compiler continuation](../docs/verification/coder-dictation-2026-11-02/compiler-fix/README.md).
Earlier dated receipts keep their original eight-step representation, source
and producer identities. Native headers must be regenerated with their C
objects; this changes no protocol wire message or general lifetime proof.
