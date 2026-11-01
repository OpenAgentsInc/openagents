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

## Incomplete summaries remain manual

The internal procedure status distinguishes complete analysis from
incomplete/manual analysis. Recursive active queries and unsupported helper
writes remain incomplete. They must not turn into complete empty sets.
Known local origins still produce E0383 when another part of the returned
value is unknown, including when a conditional is passed through an identity
helper. Unknown-only source acceptance is not a lifetime-safety verdict.

System One's `so_post` is an important boundary. Its local `SoCall` array
contains result descriptors that borrow the caller's output/error buffers.
The containing array's local storage is not automatically the result's
pointee. Writes through `so_start`/`so_step` remain a manual summary boundary
until the separate helper-write/retained-request work is implemented. Preserving
the API and observing its native returned buffers does not establish complete
request-buffer lifetime safety.

`CK_text` has a different, supported shape: it reads locally assembled parts,
copies their bytes into the caller's `c.bytes`, and returns only that output
storage. `CK_join2`, `CK_join3` and `CK_number` must remain valid; their local
parts or formatting scratch do not become retained output origins.

The analysis uses durable declaration identities, so reused names in sibling
blocks do not merge distinct locals. Its may-origin joins do not prove that
a later overwrite removed an earlier borrow. General recursive fixed points,
shorter nested-block regions, ownership transfer and foreign retention remain
outside this extension. Stores into longer-lived caller state and completion,
cancellation and late-callback lifetimes remain the next linked child; the
borrow-summary umbrella stays open.

## Bounds and verification

The Coax checker admits bounded query/origin workspaces and bounded projection,
depth and work. Malformed storage or capacity/depth/work exhaustion is E0381,
with no native resolver fallback or silent safe-empty result. The native bridge
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

The integrating qualification separately binds the actual compiler, checker
source and generated seeds; runs the affected Coax authority/mutation gates;
reproduces both bootstrap fixed points; and observes native caller-buffer
returns under the named optimization/sanitizer profiles. Historical producer
results remain historical. Finite checks, reproducible seeds and source
acceptance do not establish general memory safety.
