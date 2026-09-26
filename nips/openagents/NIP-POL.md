# NIP-POL — Scoped instructions, admission, and routing records

`draft` `optional` — v1, 2026-09-21. The [shared contracts](contracts.md)
are normative. This NIP makes host decisions inspectable across clients. It
does not turn a Nostr signature, a model judgment, or an agent-owner attestation
into an operating-system grant. [NIP-CAP](NIP-CAP.md) remains the binding and
enforcement contract.

All artifacts include `v`, `requires`, and optional inert `meta`. They travel
locally, in admitted CJ/RUN references, or in the private `3188` envelope.
No public grant catalog or universal executable policy language is defined.

## Recipients and disclosure

A recipient artifact has `v: "openagents.recipient.v1"`, `host` (pubkey),
`service` (opaque configured ID), `location` (`local` or `remote`),
`principal` (exact remote worker pubkey or null for a non-Nostr service),
`origin` (exact normalized HTTPS origin, admitted WebSocket relay origin, or
null for local use), `account_scope` (opaque ID), `model` (ArtifactRef or null),
and `policy` (ArtifactRef). Host configuration supplies actual connection and
credential material and binds this identity to it. A display provider/model
name is insufficient. Redirects, fallback services, and reviewers are separately
admitted recipients. No credential or reusable cache token belongs in the artifact.

A disclosure policy artifact has `v: "openagents.disclosure-policy.v1"`,
`issuer` (pubkey), `revision`, `valid_until`, and `rules`. Each rule contains
`classification` (exact host classification ID), `recipients` (recipient
ArtifactRefs), `purposes` (distinct `decision`, `generation`, `review`,
`execution`, `indexing`, `evaluation`, or `publication`), and `allow` (boolean).
Unmatched or conflicting rules deny; a deny takes precedence. The host must
independently trust the issuer for that scope. These are upper bounds on
disclosure, not permission to read a source or spend money. Empty recipient
sets authorize no recipients. Derived evidence retains applicable restrictions
unless a separately authorized declassification operation records its evidence.

Hosts classify actual inputs before sending them and constrain what an executor
can read next. Predicted likelihood of touching sensitive files can inform a
route but cannot enforce it. If read/network confinement is unavailable, refuse
or choose an explicitly admitted trusted executor contract; do not claim
host-enforced isolation. Endpoint geography and organizational exclusions are
explicit operator policy inputs, never assumptions from provider nationality.

## Instruction sets and scoped activation

An instruction set has `v: "openagents.instructions.v1"`, `task` (common ID),
`revision` (task-frame revision), `resolver` (DefinitionRef), `precedence` (ArtifactRef),
`snapshot` (ArtifactRef), and `entries`. Precedence is a pinned host-supported
policy artifact; unknown precedence semantics refuse.

Task and revision must match the enclosing task frame. This logical back-reference
avoids a circular content digest; the frame pins this exact instruction artifact.
Entries have:

| Field | Contract |
| --- | --- |
| `id`, `body` | Stable slug and exact content ArtifactRef. |
| `source` | Authenticated source ArtifactRef, including repository/source version where applicable. |
| `authority` | `host`, `user`, `scope`, `repository`, or `optional`; determined by the host's provenance rules. `scope` covers an independently trusted organization, project, account, or collection policy; `repository` is the coding-specific source. |
| `scope` | `{task, resources, operations}`: task ID or null and exact host resource/operation IDs. Empty resource/operation lists mean all within the parent grant, never wider. |
| `mandatory` | Boolean fixed by host resolution, not by relevance ranking. |
| `lifetime` | `operation`, `task`, or `session`. |
| `activation` | `{kind, evidence}`: `mechanical`, `explicit`, or `semantic`, with receipt/source ArtifactRefs. |
| `expires_at` | Unix seconds or null; task/session closure still expires its entries. |

Host, user, and admitted scope-policy resolution precedes optional relevance.
Repository ancestry is one scope mechanism; account, document-collection, and
organization membership use their own trusted adapters. Canonical resource
identity, configuration, membership, and source revisions are host facts.
The host establishes the policy issuer's authority over that scope independently;
neither document content nor an organization name establishes it. The protocol
does not standardize every repository's filename convention or organization hierarchy.
Semantic activation applies only to optional entries. Mandatory instructions
remain in the task frame across compaction, routing, restart, and delegation.
An unsatisfied mandatory instruction blocks the affected operation. Preserve
both conflicting instructions for explanation; resolve by the pinned precedence
policy or refuse. A retrieved manual cannot appoint itself higher authority.

Skills and hooks use EXT definitions and bounded lifetimes. Activation records
name exact releases and supported events. Deactivation, cancellation, closure,
or changed applicable scope removes future activation. A session hook cannot
persist into another session by copying a context summary. Conditional style
guidance does not silently become an execution permission.

## Action review and approvals

An action artifact has `v: "openagents.action.v1"`, `task_frame`, `snapshot`,
`operation` (DefinitionRef), `binding` (ArtifactRef), `input`, `context`
(ArtifactRefs), `effects`, `bounds`, `recipient` (ArtifactRef or null),
`preconditions` (ArtifactRef), and `nonce` (random common ID). Its exact digest
is the approval subject. Script review binds interpreter, executable/script,
argument-file, working-directory, and relevant configuration identities in the
binding/preconditions. Mutable executable bytes invalidate prior review.
For non-code actions, bind the exact target account/resource, recipient,
typed payload, provider revision, and domain limits instead. Authorizing a
draft, analysis, or proposed mutation does not authorize sending or applying it.
One approval covers the named action only; threshold/multi-party approvals
need a separate supported policy and are not implied by a list of signatures.

An approval request has `v: "openagents.approval-request.v1"`, `request`,
`action` (ArtifactRef), `requester`, `approver` (pubkeys), `expires_at`, and
`reviews` (receipt ArtifactRefs). An approval decision has
`v: "openagents.approval-decision.v1"`, `request` (ArtifactRef), `action`
(the same ArtifactRef), `approver`, `decision` (`approve` or `deny`),
`expires_at`, and `reason` (bounded inert text).

Remote decisions require an exact signed private envelope authored by the
named approver. Local UI decisions require a durable host-authenticated record.
[NIP-CTRL](NIP-CTRL.md) pairing or task-control rights do not appoint an
approver; the host separately establishes authority for the exact POL action.
The host must already recognize that approver for this request and scope;
neither artifact can nominate an authoritative stranger. Decision expiry cannot
exceed request expiry. Bind requester, task revision, recipient, and all action
bytes. Approvals are single-use admissions consumed atomically by the host,
not transferable bearer grants. Exact replay may retrieve the same admission
receipt but MUST NOT dispatch again. Conflicting decisions deny pending use
until explicitly resolved; a later denial cancels still-unconsumed approval.

Approval does not bypass hard limits, source freshness, release revocation,
or actual binding enforcement. Recheck immediately before dispatch. A semantic
review can recommend an outcome under an operator's auto-approval policy;
its probability cannot itself authorize work. Changed conditions require a new
action and admission. Revoking consumed permission requests cancellation and
reconciliation; it does not prove an effect stopped.

## Routing and observed cost

A route receipt has `v: "openagents.route.v1"`, `task_frame`, `context_request`,
`policy` (ArtifactRefs), `purpose`, `candidates`, `selected`, and `decisions`.
Purpose is `decision`,
`generation`, `review`, or `execution`. Selected is a candidate slug or null
for abstention. Decisions are the exact semantic decision receipts, possibly
empty for deterministic or explicit selection.

Each candidate contains `id`, `recipient` (ArtifactRef), `admissible` (boolean),
`reason` (common refusal code or null), `context` (ArtifactRef or null),
`estimate`, and `cache`. A false admissible value requires a reason and cannot
be selected. Estimation occurs only after mechanical eligibility filtering;
do not disclose the task to an ineligible candidate to estimate its quality.

Estimate is `{currency, price, cost_microunits, latency_ms, quality, basis}`.
Price and basis are ArtifactRefs to dated price/configuration and workload
evidence. Numeric estimates may be null; quality is a workload evaluation
ArtifactRef or null. Estimate complete remaining work, including context
rebuild, decision overhead, retries, and escalation; an unknown cost is not
zero. Operator preferences for speed, quality, or spend are pinned in policy,
not a universal protocol scoring formula. Estimates never reserve actual funds.

Cache is `{prefix, tokenizer, eligible, observation}`. Prefix is the exact
serialized prefix ArtifactRef or null; tokenizer is an ArtifactRef or null;
eligible is `yes`, `no`, or `unknown`; observation is a usage receipt ArtifactRef
or null. Matching context/prefix digests indicate compatible input only. Provider
account/model/configuration, cache lifetime, and actual hit accounting remain
provider-specific. Never publish cache credentials or count an estimated hit as
observed savings. Fresh mandatory constraints take precedence over preserving
a cached prefix.

A usage artifact has `v: "openagents.route-usage.v1"`, `route` (ArtifactRef),
`attempts` (execution/decision receipt ArtifactRefs), `input_tokens`,
`cached_input_tokens`, `output_tokens`, `latency_ms`, `cost_microunits`,
`currency`, and `basis` (`observed`, `estimated`, or `unknown`). Unknown numeric
values are null; units and provider definitions must be retained in referenced
receipts. Mixed estimates and observations require separate artifacts. Failed
routes and escalation attempts remain in totals. Accounting and trajectory receipts may supply observations through pinned
artifact schemas.
RUN links later usage artifacts back to the immutable route receipt; the route
does not contain a circular reference to its future actual usage.

## Optimization authority and adoption

An [OPT](NIP-OPT.md) study declares which implementation choices can vary.
Host admission separately authorizes evaluation, reflection, prompt/example
optimization, weight training, data export, and deployment. One permission
does not imply another, even when no model weights change.

Semantic routing, relevance, and escalation recommendations can be optimized
within a fixed authority envelope. Mandatory instructions, credential scope,
recipient eligibility, exact approval consumption, protected verification,
and hard bounds MUST remain outside that search space. A candidate confidence
score cannot grant an effect or lower its own acceptance rule.

The host admits the student, proposer/reflector, and judge recipients
separately and accounts for their total work. Changing a provider, model,
adapter, or inference strategy requires identity validation and evaluation
for the intended scope. A signature match does not establish calibration,
privacy equivalence, or task quality.

EVAL admission can support an operator's adoption decision for an exact
implementation. The installed pin changes atomically under that policy;
active runs keep their original lock. Shadow traffic, canary execution, and
rollback are separately admitted actions with their own data scope and
budgets. Publication, a high score, or a background finding cannot activate
a candidate by itself.

## Conformance and boundaries

Fixtures must cover mandatory instruction retention, authority spoofing,
unknown precedence, source changes after approval, conflicting/replayed
approvals, atomic consumption, fallback disclosure, selector/reviewer leakage,
cache uncertainty, and total cost with failed escalation. Relays enforce private
envelopes; hosts enforce policy and effects. Advertise `nip-pol-v1` for the
configured validation/exchange or host role actually tested. This specification
does not deploy a policy engine, credential manager, sandbox, or calibrated
permission classifier.
