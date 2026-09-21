# NIP-CTX — Task state and context views

`draft` `optional` — v1, 2026-09-21. This specifies exchangeable artifacts,
not an implemented evidence service. The [shared contracts](contracts.md)
are normative. [NIP-RUN](NIP-RUN.md) records execution; this NIP describes
the state supplied to it. The [design addendum](../../docs/coder/design/typesafe-agent-protocol-addendum.md)
maps the source proposal to host and protocol responsibilities.

## Transport and authority

Artifacts can remain local, travel inside admitted CJ execution inputs/results,
or use the shared private `3188` artifact envelope. No new public context
catalog is defined. Recipient access, source retention, and publication are
separate admissions. An artifact author cannot label retrieved text as an
owner instruction merely by signing it.

Every artifact below includes `v`, `requires`, and optional inert `meta`.
ArtifactRefs identify exact bytes. Unless specified otherwise, lists are
ordered, IDs within a list are unique, and references must resolve within
the admitted, bounded closure. Unsupported required semantics refuse.
Every `coverage` value is `complete`, `partial`, or `unknown`, relative to
the declared request/scope. Descriptor ArtifactRefs hash stored bytes; the
descriptor's internal `id` hashes its canonical content excluding `id`.
Consumers verify both and must not interchange those two digest meanings.

## Task frames and explicit variables

A task frame has `v: "openagents.task-frame.v1"` and these fields:

| Field | Contract |
| --- | --- |
| `task` | Random common ID. |
| `owner`, `controller` | Exact pubkeys, validated against the admitted relationship. |
| `revision`, `previous` | Revision starts at zero; previous is null at zero, otherwise an ArtifactRef to revision minus one. |
| `objective` | ArtifactRef to the bounded requested objective. |
| `origin` | `user`, `program`, or `inferred`; origin records provenance, not authority. |
| `constraints`, `acceptance` | Ordered ArtifactRefs to applicable constraints and acceptance requirements. |
| `snapshot` | ArtifactRef to the snapshot described below. |
| `instructions` | NIP-POL instruction-set ArtifactRef. |
| `variables` | Entries `{name, schema, value, evidence}`: slug, SchemaRef, typed ArtifactRef, and supporting evidence descriptor ArtifactRefs. |
| `attempts`, `unresolved` | Retained run references and ArtifactRefs to unresolved questions. |
| `corrections` | Entries `{source, replaces}` binding an authenticated correction to the objective/constraint ArtifactRefs it supersedes. |

The controller accepts corrections under owner policy and records the accepted
frame in RUN. Frame revision is not a relay timestamp. Two different frames
with the same task/revision conflict; stop automatic use until the controller
resolves the conflict explicitly. Historical frames remain immutable. A
correction invalidates affected pending contexts and proposals; it does not
erase completed effects or unknown reservations.

Variables reference data, not evaluable code. Do not serialize closures,
interpreter environments, hidden model reasoning, or provider KV tensors.
Recursive exploration uses bounded PRG operations over these references;
the host still limits depth, calls, bytes, and effects. An inferred subgoal
never changes the user's objective without host acceptance.

## Snapshots, evidence, and representations

A snapshot has `v: "openagents.snapshot.v1"`, `scope` (opaque host scope ID),
`captured_at`, `sources`, and `coverage` (`complete`, `partial`, or `unknown`).
Each source contains `id` (opaque within scope), `version` (ArtifactRef),
`kind` (`repository`, `tool`, `conversation`, or `document`), and
`availability` (`retained`, `deleted`, or `unavailable`). A repository version
binds the admitted base and captured working-tree contents, including relevant
uncommitted inputs; a branch name alone is insufficient. The registered source
adapter fixes its version schema. No claim of complete coverage extends beyond
the declared scope. Evidence uses the shared `openagents.evidence.v1` descriptor.

A representation has `v: "openagents.representation.v1"`, `evidence`
(descriptor ArtifactRef), `snapshot` (ArtifactRef), `mode`, `content`
(ArtifactRef), `anchors`, `derivation` (receipt ArtifactRef or null), and
`coverage`. Mode is `original`, `excerpt`, `structured`, `short_summary`, or
`long_summary`; the last two name roles, not universal lengths.

An anchor is `{source, start, end}`: exact source ArtifactRef and half-open
byte range, `0 <= start <= end <= source.size`. Excerpts MUST match those
bytes in anchor order; text excerpts additionally respect UTF-8 boundaries.
Summaries and structured transformations require attributable derivation and
source anchors where available. They must not masquerade as verbatim excerpts.
An `original` representation's content must equal the descriptor's content
reference. An `excerpt` must name at least one anchor; its content is exactly
the concatenated ranges without added separators. Other modes can add formatting
only as an explicitly derived representation.
Omitted bytes and lost coverage remain visible through the evidence capture
record. Semantic sufficiency is a separate judgment, not guaranteed by a digest.

An optional graph artifact has `v: "openagents.evidence-graph.v1"`, `snapshot`,
`nodes` (evidence descriptor ArtifactRefs), and `edges` containing `from`, `to`
(descriptor digests), and `relation` (`derived_from`, `supports`, `contradicts`,
or `supersedes`). The author claims these relationships; they are not facts
proven by publication. `derived_from` must agree with the shared descriptor.
Graph traversal has finite node/edge/byte limits and detects cycles. A
`supersedes` edge cannot rewrite history or invalidate an owner's constraint.

## Context requests and selection receipts

A context request has `v: "openagents.context-request.v1"`, `request`,
`task_frame`, `snapshot`, `query` (ArtifactRefs), `recipient` (NIP-POL
recipient ArtifactRef), `policy` (ArtifactRef), `mandatory` (evidence/constraint
ArtifactRefs), `limits`, and `allowed_modes`. Limits contain `input_bytes`,
`tokens` (integer or null), and `tokenizer` (ArtifactRef or null); a token bound
requires a pinned tokenizer. Host resource ceilings apply in addition.

Mechanical scope and disclosure filtering precede semantic selection and
apply to the selector's own input. A hosted reranker is a recipient too.
Mandatory material survives selection. If mandatory content cannot fit, refuse
or obtain a newly admitted narrower task; never silently summarize it away.

The result has `v: "openagents.context-build.v1"`, `request` (ArtifactRef),
`manifest` (the shared `openagents.context.v1` ArtifactRef), `representations`
(ArtifactRefs), `selection` (ArtifactRef below), `serialized_input`
(ArtifactRef), and `usage` (`bytes`, `tokens`, `tokenizer`; unknowns are null).
Every manifest representation must be present in the result's closure and
refer to its stated evidence. The manifest's recipient is the receiving host
pubkey, or `local`; the request's NIP-POL recipient additionally fixes any
downstream provider/model. `local` never authorizes hosted inference. The
supplied bytes, including framing and schemas, must match `serialized_input`.

A selection receipt has `v: "openagents.context-selection.v1"`, `request`,
`candidate_set`, `retrieval`, `decisions`, `choices`, and `coverage`.
Request and candidate set are ArtifactRefs. The candidate set is an ordered
list of `{evidence, representations, mandatory}` using ArtifactRefs.
Retrieval contains pinned `operation`, `input`, `snapshot`, `limits`, and
`omitted_count` (integer or null). Decisions are receipt ArtifactRefs;
choices contain candidate evidence digest, selected representation digest or
null, and reason (`mandatory`, `selected`, `irrelevant`, `budget`, `denied`,
`unavailable`, or `unknown`). Every candidate has exactly one choice. Sensitive
denied candidates must be excluded from a less privileged recipient's receipt;
produce an explicit redacted derivative with partial coverage instead.

A decision failure is `unknown`, not irrelevance. Expansion may collect more
evidence within the existing authority/budget or request new admission.
Changing the query, source snapshot, required instructions, recipient, model
format, or selection policy requires revalidation. Old state may be useful
history but cannot silently become a current observation.

## Hierarchical history and bounded expansion

A history index has `v: "openagents.history-index.v1"`, `scope`, `snapshot`,
`roots`, and `nodes`. Nodes contain `id` (slug), `summary` (representation
ArtifactRef or null), `children` (node IDs), and `evidence` (descriptor
ArtifactRefs). Reject duplicate IDs, missing children, and cycles. Cross-links
in an evidence graph remain separate. A node summary does not prove that all
its descendants are irrelevant to another query.

Registered operations may expose these exact input/result pairs through CAP
and CJ; no extra job family or relay query extension is introduced:

| Operation role | Input | Result |
| --- | --- | --- |
| Context construction | `openagents.context-request.v1` | `openagents.context-build.v1` |
| Evidence expansion | `openagents.expansion-request.v1` | `openagents.expansion-result.v1` |

An expansion request contains `request`, `task_frame`, `snapshot`, `recipient`,
`targets` (representation or index ArtifactRefs), `query` (ArtifactRef),
`limits` (common bounds), and `max_nodes`. Its result binds the request
ArtifactRef and contains `representations`, `visited` (index digest/node ID
pairs), `unexpanded` (pairs with reasons), and `coverage`. A missing retained
source yields `content_unavailable`. The host chooses beam width, lexical
fallback, or exhaustive bounded traversal; the protocol promises no logarithmic
recall or hidden-state access. Record work and missed coverage honestly.

## Cache reuse and rendering

Stable context ordering can help a provider reuse a prefix. Reproducible input
does not prove a cache hit. NIP-POL records route/cache estimates and observed
usage. Cache handles remain private to the exact provider/account/model and
are never portable authority or a substitute for retained source content.

Clients can display source expansion, representation choices, omissions, and
stale snapshots. A relevance heatmap is a view of recorded scores and anchors,
not a claim about a generator's internal attention. Formatting, highlighting,
summary generation, tokenization, indexing, and storage remain host/client work.

## Conformance

Required cases include missing mandatory input, changed source digests, false
excerpt anchors, incomplete captures, forged instruction origin, frame forks,
recipient changes, reranker disclosure, deleted sources, cyclic indexes,
bounded expansion, and replay of supplied bytes without inference. Exercise
private-envelope access on every relay query surface. Advertise `nip-ctx-v1`
only for the configured artifact validation/exchange or host role actually
proven; a relay cannot advertise semantic relevance or cache correctness.
