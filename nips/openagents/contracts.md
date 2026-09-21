# Shared OpenAgents protocol contracts

`draft` `optional` — normative for CAP, PRG, EXT, RUN, CTX, POL, COORD,
and EVAL v1, and the CJ execution family. These contracts are not implemented merely
because this document defines them. Existing CJ conversation and decision payloads retain their own rules.

The uppercase requirement words express conformance requirements. A reader
MUST validate the complete required contract before any effect. Signatures
establish authorship; hashes establish identity; neither grants permission or
proves execution, safety, calibration, or continued availability.

## Encoding and compatibility

New bodies are UTF-8 JSON objects with a required `v`. Reject duplicate object
keys, invalid Unicode, non-finite numbers, and integers outside
`[-9007199254740991, 9007199254740991]`. Counters, sizes, and Unix-second
timestamps are nonnegative integers in that range. Durations name their unit.
Omission and `null` are distinct; `null` is permitted only where specified.

Use [RFC 8785 JCS](https://www.rfc-editor.org/rfc/rfc8785) when a contract
explicitly asks for a canonical object digest. Do not change the NIP-01
event-ID algorithm. A byte-artifact digest always hashes the exact bytes,
including JSON whitespace; it is not a canonical object digest.

Every new extensible body has `requires`, a list of distinct feature IDs,
and optional `meta`, an object of inert annotations. An unknown schema version,
feature, semantic field, enum, step, bound, or import MUST refuse as
`unsupported_version` or `unsupported_feature`. Unknown keys inside `meta`
MAY be ignored but MUST NOT change execution. Optional display metadata is
not a place to hide an instruction or a required restriction. New execution
semantics need a new version or a specified feature ID; adding a field alone
does not establish compatibility. The initial feature list is empty.

Unqualified slugs match `^[a-z0-9][a-z0-9_-]{0,63}$`. Qualified component IDs
are `<publisher-pubkey>:<package-slug>/<component-slug>`, using a 64-character
lowercase hexadecimal x-only public key. A display name is never an identity.
Request/run IDs are 32 random bytes encoded as 64 lowercase hexadecimal
characters. Step IDs use the slug grammar. A retry preserves logical IDs.

The common decoded-body ceiling is 1,048,576 bytes and JSON nesting depth is
at most 64. A concrete profile or host MAY impose lower documented limits;
it MUST refuse oversized input rather than truncate a definition. Enforce
limits during parsing and before allocation or decompression. These are wire
ceilings, not recommended per-invocation resource budgets.

## References and digests

`Digest` is `sha256:` followed by 64 lowercase hexadecimal characters.
`EventRef` is `{id, pubkey, kind}` with exact 64-hex ID and signer and an
integer kind. An optional `coordinate` is a NIP-01 address string; a fetched
event MUST match every supplied field. Verify its NIP-01 ID and signature.
Coordinates and relay hints aid discovery and MUST NOT replace exact identity.

`ArtifactRef` contains:

| Field | Requirement |
| --- | --- |
| `digest` | Required Digest of exact bytes. |
| `size` | Required exact byte length, checked before accepting content. |
| `media_type` | Required lowercase MIME type, without execution authority. |
| `schema` | Required version/dialect identifier for a structured semantic artifact; absent for uninterpreted bytes. |
| `event` | Optional EventRef authenticating a containing declaration; its declaration must bind this artifact. |
| `sources` | Optional ordered list of `{url}` or `{event}` locator hints. |

The host MAY use a local store and ignore locator hints. Fetching requires
its own authority, destination validation, deadline, and byte limit, including
redirects. A bad locator cannot change the expected digest. Retain referenced
bytes and signed definitions according to the admitted retention policy;
an addressable relay may discard older events. Missing content means
`content_unavailable`, never permission to follow the latest version.

`SchemaRef` is an ArtifactRef for `application/schema+json` in the
[JSON Schema 2020-12 dialect](https://json-schema.org/draft/2020-12/json-schema-core).
Resolve `$ref` only within the pinned schema closure. Remote schema loading
while validating is forbidden. The host MUST support every required vocabulary
or refuse before execution. Format assertions are enforced only when the
schema explicitly requires that vocabulary. Bound schema evaluation as well
as input parsing; schema validation does not prove semantic correctness.

`DefinitionRef` is `{id, artifact, event?}`: qualified ID, ArtifactRef of the
definition, and optional exact EventRef of its publisher's declaration. The
definition and declaring release MUST agree on identity and digest. A local
definition can omit `event` only under a locally recorded provenance policy;
it cannot claim a remote signature it does not possess.

## Locks and resolution

A lock body has `v: "openagents.lock.v1"`, `requires`, `root: DefinitionRef`,
and `entries`, an array of `{id, definition, dependencies}`. Each dependency
is another exact qualified ID in `entries`. IDs are unique. The root MUST
appear exactly once. Entries are sorted by ID before canonical hashing.
Cycles, unresolved references, conflicting bytes for one ID, and unsupported
components refuse before the first effect. The full definition/schema/artifact
closure is pinned; permitted runtime inputs are not dependency updates.

`lock_digest = SHA256(JCS(lock body))`, expressed as a Digest. A run records
this digest plus exact host bindings, policy/grant references, and input
identity. Installation locks are not grants. A catalog update cannot mutate
an admitted run's lock. Resolver limits include total definitions, depth,
bytes, and attempts; all must be finite and recorded.

## Effects, requirements, and enforcement

An effects object has `reads`, `writes`, `network`, `process`, `delegates`,
and `spend`. The first three are arrays of logical scope IDs resolved by the
host; the last three are booleans. Empty arrays and false deny those effects.
Raw paths, credentials, or wildcard grants MUST NOT be inferred from a scope
name. Unknown effects are not equivalent to read-only behavior.

Bounds use this initial vocabulary:

| Bound | Meaning |
| --- | --- |
| `wall_ms` | Elapsed deadline including host calls and children. |
| `memory_bytes` | Maximum guest or enforceable process memory. |
| `input_bytes`, `output_bytes`, `read_bytes`, `storage_bytes` | Aggregate bytes in their named scope. |
| `calls`, `attempts`, `concurrency`, `depth` | Aggregate invocation/retry count, simultaneously active work, and composition depth. |
| `fuel` | Wasm instruction allowance when the host supports metering. |
| `spend_microunits` | Integer monetary ceiling paired with a required `currency` code. |

Bounds are ceilings, not grants. A component can separately declare `minimum`
requirements. Admission MUST reject a minimum above the effective ceiling.
Unknown monetary cost is not zero. Required hard bounds that no participant
can enforce cause `cannot_enforce`; estimation cannot satisfy a hard ceiling.

The host records an enforcement plan assigning each required bound to a
specific host mechanism or an explicitly trusted executor contract, including
its evidence and limitations. A declaration alone is not measured enforcement.
The plan MUST state whether assurance is host-enforced or a remote claim;
policy decides which assurance it accepts. Missing coverage refuses.

Children share the parent's reservations. Per-child limits cannot each spend
the parent's full allowance. Effective limits and scopes narrow under
composition, including operator-imposed stricter limits, and MUST be recorded.
Cancellation or an unknown outcome does not automatically release an
unsettled reservation. Reconcile before reuse or retain it as unknown.

## Evidence and context

An evidence descriptor has `v: "openagents.evidence.v1"`, `requires`, `id`
(a Digest of its canonical descriptor excluding `id`), `content: ArtifactRef`,
`source`, `capture`, `derived_from`, and `scope`:

- `source`: `{kind, identity, version}`. `kind` is `repository`, `command`,
  `decision`, `plugin`, `delegate`, or `document`; identity/version are stable
  scoped strings whose interpretation is fixed by the producing adapter.
- `capture`: `{complete, omitted_bytes, reason}`. `complete` is boolean;
  omitted bytes is a nonnegative integer or `null` when unknown; reason is
  `null` or a typed string supplied by the capture profile. Incomplete input
  cannot yield a derivative claiming complete source coverage.
- `derived_from`: ordered evidence descriptor digests, empty for an original.
- `scope`: `{task, recipients, classification}` with task ID, explicit pubkeys,
  and a host-policy classification ID. An empty recipient list permits local
  use only. A recipient entry is a restriction, not a grant to fetch content.

Derivatives additionally contain `transform: DefinitionRef` and `parameters`
(a Digest), binding the actual transformation. Host-owned provenance cannot
be set by an untrusted guest. The host retains the original bounded capture
and can report when deletion or retention policy makes expansion unavailable.

A context manifest has `v: "openagents.context.v1"`, `requires`, `task`,
`recipient` (pubkey or `"local"`), `policy: ArtifactRef`, `entries`,
`omissions`, and `coverage` (`complete`, `partial`, or `unknown`). An entry
contains `evidence` (descriptor Digest), `representation` (ArtifactRef), and
`mandatory` (boolean). An omission contains evidence Digest and reason.
Optional `selection` is a receipt ArtifactRef, not an authorization.
Ordered entries determine the supplied context; hash the whole manifest with
JCS. The executing host MUST validate recipient/scope and required coverage
before disclosure. Mandatory constraints cannot be discarded by relevance.

Keep descriptors, source paths, plaintext hashes that reveal sensitive content,
and context inside recipient-scoped encrypted storage or transport. Public
package metadata never contains private task evidence. A local evidence ID
does not imply a globally readable event or force a network read.

## Outcomes and records

Shared execution outcomes are `completed`, `refused`, `failed`, `cancelled`,
and `unknown`; `unattempted` is a refused/cancelled attempt's dispatch status,
not evidence that a dispatched effect did not occur. Record `dispatched`
separately. Verification is `passed`, `failed`, `unverifiable`, or `not_run`;
integration is `accepted`, `rejected`, `pending`, or `not_requested`.
These fields MUST NOT collapse into one success boolean.

Common refusal codes are `malformed`, `unsupported_version`,
`unsupported_feature`, `not_admitted`, `unavailable`, `content_unavailable`,
`identity_mismatch`, `incompatible`, `revoked`, `stale`, `cannot_enforce`,
`limit_exceeded`, `idempotency_conflict`, and `conflict`. A profile can add
versioned causes; human messages are data and MUST NOT be executed or rendered
as terminal control sequences. Existing Decision API outcomes and refusal
codes retain their own schema rather than being silently renamed.

An execution receipt binds run/step/attempt, component/lock/input/context
digests, effective authority and enforcement plan, actual recipient, limits,
usage (unknown values explicit), output/artifact references, outcome,
verification, and integration. ATIF and decision receipts are linked records,
not replaced. Offline replay reconstructs recorded inputs and transitions;
re-execution is a separately admitted attempt.

## Nostr envelopes and discovery

Only NIP-01 single-letter tags are assumed queryable. New public definitions
use `t` values such as `oa:profile:executor` or `oa:step:module`; tag values
duplicating body fields MUST agree. Multi-letter tags are annotations, not
portable filters. Discovery describes potential operations and executes none.

Private profiles use NIP-44 v2 and exact authorized recipients. NIP-42
authenticates a relay connection; the verified event signer is the forwarded
request's principal. Never infer the sender's grants from another socket.
Encryption hides content, not visible tags or traffic patterns. Each private
profile defines read ACLs, retention, fanout, and COUNT/search behavior.

### Private artifact envelope

Kind `3188` is a regular immutable declaration of one scoped artifact for
one recipient. CTX, POL, COORD, and EVAL use it when a separately signed
artifact is needed outside a RUN controller's journal. It does not dispatch
work. The event has exactly one `p` recipient, one `h` random 64-hex mailbox,
and `t: oa:artifact:v1`. Mailboxes are generated per admitted sharing scope
and recipient, never derived from a source path, plaintext digest, or task name.

Content is NIP-44 v2 encrypted to that recipient. The decrypted body is
`{v: "openagents.artifact-envelope.v1", requires: [], artifact, inline,
issued_at, retain_until}`. Artifact is an ArtifactRef; inline is null or a
JSON object whose JCS bytes MUST exactly match the ArtifactRef's digest and
size. Binary/noncanonical payloads require separately admitted artifact
storage and `inline: null`. The artifact's schema is mandatory. `issued_at`
is observational Unix seconds; `retain_until` is a later retention request,
not a promise by a relay. It does not establish application freshness or
override an approval/lease expiry inside the artifact.

Event ID and signer authenticate this exact declaration. Embedding a foreign
ArtifactRef without its original signed declaration does not adopt its claimed
authority. Each consuming NIP defines which issuer is trusted and for what
purpose. Copies for different recipients have different event IDs and retain
the same artifact bytes only when their disclosure policy permits. A redacted
copy has a new digest and explicit derivation; it is not the original record.

A conforming relay requires author authentication at publication, restricts
all reads, ID lookups, COUNT, and live fanout to the authenticated author or
exact recipient, and excludes these events from search. Apply visibility before
limits/counting. Do not log plaintext or ciphertext bodies. NIP-42 authentication
does not substitute for event-signature verification. Envelope validation does
not validate encrypted artifact semantics. Plaintext must fit both the common
bound and NIP-44's smaller payload bound; use an ArtifactRef when it does not.

Consumers resolve references only under fetch/disclosure authority, verify
content and retained provenance, and report unavailable/deleted bytes explicitly.
Do not use EOSE, the newest timestamp, or absence of a record as proof of
current authorization, completeness, or nonexecution. Required durable retention
is agreed with storage separately. Deleting an envelope cannot revoke a grant
already consumed or erase remote copies. Source hashes and private relationships
stay encrypted. Visible tags still reveal traffic and recipient relationships.

Advertise `oa-private-artifacts-v1` in NIP-11 `supported_extensions` only for
configured envelope validation, privacy, and retention/retrieval behavior
supported by fixtures. Keep these draft names out of numeric `supported_nips`.

The kind allocations in this lane are OpenAgents draft assignments, checked
against the repository's pinned lanes; they are not upstream registration.
Before implementation or public interoperation, recheck conflicts against a
reviewed upstream snapshot. If an allocation changes, version the envelope
mapping; do not silently reinterpret old events.
