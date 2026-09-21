# NIP-PRG — Programs

`draft` `optional` — revised v1, 2026-09-21. The earlier draft is revised
in place. Current local programs implement a narrower shape; migrate them
with their readers before claiming this contract. Composition, generic
invocation, and Wasm execution are implementation work.

A program is a typed workflow interpreted by a host. It names operations,
sources, functions, and children by pinned identity, grants no authority,
and contains no arbitrary executable expressions. The
[shared contracts](contracts.md) are normative. [NIP-CAP](NIP-CAP.md)
governs bindings, [NIP-EXT](NIP-EXT.md) distribution, and
[NIP-RUN](NIP-RUN.md) durable execution records.

Programs are domain-independent. Their sources, operations, typed schemas,
and acceptance criteria provide the specialization. A research workflow can
query documents, assess evidence, and produce a report without a repository;
a business workflow can invoke an admitted record operation. Code edits and
test runs are examples, not required step semantics.

## Discovery and immutable execution

Kind `30182` remains an addressable program discovery head, with one `d`
slug, `t: oa:program:v1`, and one `t: oa:step:<kind>` for each distinct
step kind. Its body contains the complete definition below or
`{v: 1, requires: [], definition: DefinitionRef}`. Tags MUST match the
resolved definition. Multi-letter `step` tags are not portable filters.

Replacement changes discovery only. Admission MUST resolve exact bytes,
verify identity, construct the full lock, and retain the definitions and
dependency closure. Child programs, functions, guidance, schemas, and modules
MUST NOT float to a newer head during a run. Missing pinned content refuses.

Program selection proposes the requested workflow or `none`; it is distinct
from operation relevance and admission. Explicit structured requests need no
semantic selector. When selection is used, record descriptor/definition
digests, candidates, and decision/policy references. No protocol mandates a
particular model, threshold, or inference call. Ordinary turns need no program.

## Definition and steps

A definition has `v: 1`, `requires`, `id`, `summary`, `input`, `output`,
`steps`, `result`, `bounds`, and optional inert `meta`. ID is qualified;
input/output are SchemaRefs; bounds apply to the whole run. Selector wording
is covered by the definition digest. There are at most 256 named steps.

| Step field | Meaning |
| --- | --- |
| `name` | Unique slug. |
| `kind` | One of the seven kinds below. |
| `target` | Exact DefinitionRef of its source, checker, function, capability, child, plugin, or operation. |
| `after` | Distinct preceding step names; empty means no step dependencies. |
| `input` | Binding expression below. |
| `output` | SchemaRef for the returned value. |
| `bounds` | Step ceilings under shared parent reservations. |
| `on_error` | `stop` or `continue`; required. |
| `when` | Optional condition on available inputs or predecessor results. |
| `retry` | Optional `{max_attempts, retry_on}`; absent means one attempt. |

References MUST point backward in the serialized step array, producing an
acyclic graph. Array order breaks scheduling ties without serializing otherwise
independent work. Dependencies must be terminal before dispatch. A false
condition records `skipped`, never a completed invocation. A failed dependency
with `on_error: stop` blocks descendants. `continue` exposes its typed outcome
for explicit handling; it does not relabel refusal or failure as success.

`result` is a binding evaluated after required work is terminal and validated
against the program output schema. Completion, verification, and integration
remain separate. A valid JSON result cannot erase unresolved acceptance.

## Binding language

A binding is exactly one of:

- `{literal: <JSON value>}` within the input schema and byte limit.
- `{from: "input", pointer: ""}` or `{from: "step:<name>", pointer: "/value"}`.
- `{object: {<field>: <binding>, ...}}`.
- `{array: [<binding>, ...]}`.

`pointer` is a JSON Pointer using object keys and explicit nonnegative array
indexes. There is no wildcard, interpolation, executable expression, or path
lookup. Bound nesting by the common depth limit. Referenced steps must belong
to the transitive `after` closure. Validate schema compatibility before effects
and actual values at dispatch. If schema compatibility cannot be established
by the host's supported projection rules, refuse rather than claim a general
JSON Schema subsumption proof. Missing values refuse; this language inserts
no defaults automatically.

A step envelope contains `outcome` (common outcome or `skipped`), `value`
(schema-valid success value, otherwise null), `receipt` (ArtifactRef or null
if no invocation), and verification/integration states. `when` is exactly
`{source: <binding>, equals: <literal>}` or
`{source: <binding>, present: true}`. `present` tests resolution without
turning a missing value into an input. Record the resolved condition.

## Step kinds

| Kind | Behavior |
| --- | --- |
| `query` | Read an admitted snapshot through a registered source; return bounded attributable evidence. No hidden writes or task execution. |
| `check` | Run a registered deterministic checker or protected verification plan under host authority. |
| `decide` | Invoke a pinned decision-function component with question-set and consuming-policy identity. |
| `delegate` | Give a bounded task/context to an admitted executor and receive attributable artifacts. |
| `program` | Run a pinned child with typed dataflow and narrowed bounds. |
| `module` | Invoke a pinned plugin through the packet ABI/profile below. |
| `invoke` | Invoke a registered native or approved adapter operation with typed arguments and declared effects. |

`invoke` makes editing/testing explicit without disguising it as a query or
requiring a whole delegate. It carries no executable path, shell command, or
credential. A package cannot register executable code merely by naming a host
operation. All kinds use the same effect/authority checks.

Source definitions describe an interface; local configuration supplies paths
and destinations. Query outputs record order, dropped items, and capture
limits. Overflow behavior must explicitly be `refuse` or `truncate` in the
source's typed input when bounded results are requested.

Decision functions keep wording and generation guidance in separate digested
assets. Record actual state/options, function, model, response, and consuming
policy. Scoreability is not calibration or permission. Unavailable answers
remain distinct from `none`. Batch independent questions only under backend
limits; dependent questions wait for evidence. Legacy inline `briefing` and
bare source/question slugs require explicit migration to qualified references.

## Composition, fan-out, and retries

Resolve the complete graph before effects. Cycles, unsupported steps,
unresolved schemas, ambiguous identities, and unenforceable requirements
refuse. Default maximum child depth is 8; hosts may impose a lower bound.

A step may declare `each: {items: <binding>, max_items: <positive integer>}`.
Then input can reference `{from: "item", pointer: ""}`. Items must resolve
to an array; overflow refuses before any iteration. Results preserve source
order and record each iteration index. `output` then describes the aggregate
array of step envelopes; the target schema describes each successful value.
Evaluate `when` per item and apply error/retry rules per iteration. One parent
reservation covers all iterations. Known conflicts prevent concurrent writes.

Retries require declared causes and a compatible idempotency contract.
`max_attempts` includes the first attempt. Unknown effects MUST be reconciled
before retry even if listed in `retry_on`. Bounds narrow, while reservations
are shared rather than copied per child. Bounded repair is explicit work;
there is no unbounded recursive graph dispatch.

## Wasm module profile

A plugin definition contains `v: "openagents.plugin.v1"`, `requires`, `id`,
`module: ArtifactRef`, `abi`, `profile`, `operations`, `minimum`, `roles`, and
optional `meta`. ABI is `openagents.plugin-packet.v1`. Profile is `pure` or
`snapshot-read`. An operation contains `name`, input/output SchemaRefs, and
supported `formats`. Roles are `explicit`, `evidence-preparation`, and
`output-processing`; declarations do not activate roles.

Both profiles deny network, workspace writes, processes, credentials, ambient
clock/randomness, and model calls. Pure guests receive only the packet;
snapshot readers also receive scoped handles. Proposed patches/commands are
data for a separately authorized host operation. No engine brand is mandated.

### Packet ABI

Use Wasm32 linear memory and UTF-8 JSON. Exports are `memory`,
`oa_alloc(i32) -> i32`, `oa_free(i32, i32)`, and
`oa_handle(i32, i32) -> i64`. Interpret pointer/length as unsigned. Return
packs output pointer in the upper 32 bits and length in the lower 32 bits.
Check arithmetic overflow, memory ranges, and limits before access. Null with
nonzero length is invalid. Host owns input until handle returns; output is
a separate, non-overlapping guest allocation. Copy validated output, then
free both exactly once. A fresh instance serves each invocation; traps discard the instance.

Request: `{v: "openagents.plugin-packet.v1", requires: [], invocation,
operation, input, handles}`. Invocation is host-generated, operation a declared
slug, and handles map logical IDs to opaque tokens. Response uses the same
version/invocation, plus `requires`, `status`, `value`, and `reason`. Status
is `ok`, `unsupported_input`, or `refused`. For `ok`, value conforms to the
output schema (including null if allowed); otherwise value is null. Reason is
null or a bounded refusal string. Host assigns provenance.

The only optional import is `oa_host.call(i32, i32, i32, i32) -> i32`:
request pointer/length and guest-allocated response pointer/capacity.
Nonnegative return is bytes written; errors are `-1` malformed, `-2` denied,
`-3` limit, `-4` insufficient capacity, `-5` stale handle, and `-6` host
failure. No response is usable on error. Buffers must not overlap; the host
never calls guest allocation reentrantly.

Import request is `{v: 1, handle, operation, args}`. Operations:

| Operation | Args | Response |
| --- | --- | --- |
| `metadata` | `{}` | `{type, size, version}` |
| `list` | `{cursor, max_entries}`; initial cursor null | `{entries, next_cursor, complete}` |
| `read` | `{offset, max_bytes}` | `{offset, bytes_base64, eof, version}` |

Entries contain child `handle`, `name`, `type`, and `version`; type is `file`
or `directory`. Cursors/handles are invocation-scoped. Reads count against
aggregate budgets. Only granted snapshots produce handles. Traversal, symlink
escape, cross-run handles, and changed snapshot versions refuse.

These are virtual snapshot entries, not ambient filesystem access. A host can
materialize a document, dataset partition, or service capture as a `file` and
an admitted collection as a `directory`. The entry's version binds retained
bytes and their source observation. Its name is a logical label, not permission
to access an OS path or call an API. Collect live data through a separately
admitted source operation before exposing its snapshot to a guest. This keeps
the same bounded ABI usable beyond coding without adding guest network effects.

Validate imports before instantiation and authorize every import invocation.
Import names cannot prove which resource a call accesses. Bound compilation,
start functions, memory, wall time, fuel when required, calls, reads, output,
and storage. No ambient WASI is supplied. Fuel alone cannot bound a blocked
host call. Unsupported required enforcement refuses.

Automatic processors retain bounded original evidence and capture omissions.
One host rule owns the selected representation; installation/completion order
cannot select it. Optional failure records bounded fallback; required failure
follows the program's error contract. Hooks cannot recursively activate
themselves or widen their scope.

### Module announcements — kind `30183`

An announcement has one `d` slug, `t: oa:module:v1`, and `x` containing the
module SHA-256 hex without prefix. Body has `v: 1`, `requires`,
`plugin: DefinitionRef`, and `module: ArtifactRef`. It is only a locator;
references must agree with the pinned plugin. A program MUST NOT pin module
bytes while letting manifest, ABI, or schema identity float.

## Conformance and rollout

Fixtures must cover closure resolution, schemas/bindings, unsupported kinds,
fan-out/order, shared reservations, unknown-effect retries, skipped branches,
mandatory evidence, and packet memory/import failures. Exercise real guest
fixtures and native operations through the same admission boundary. Hashes
and receipts establish attribution, not remote attestation.

Relays advertise `nip-prg-v1` after conforming definition/head validation and
indexed discovery. Runtime support separately names step/profile/ABI sets;
serving definitions does not advertise a Wasm executor. Revised-v1 required
fields distinguish the former draft, which is not silently upgraded.
