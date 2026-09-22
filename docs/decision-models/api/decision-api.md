# OpenAgents Decision API

The Decision API turns application state into typed judgments that code can
use: probabilities, choices, rubric scores, and compositions of those
primitives. The product includes the inference service, batch execution,
agent tools, customer accounts, commercial controls, and the evidence needed
to evaluate a decision on a caller's own workload.

This document specifies the target product. It does not announce a hosted
service or make proposed features available. The serving binaries and
measurement tools exist; most service and customer features remain open
work. The [delivery plan](#delivery-plan-and-issue-ownership) identifies an
owner for every workstream. Product tracker: [#9481](https://github.com/OpenAgentsInc/openagents/issues/9481).

[Coder and Coder Terminal](../../coder/design/coder-as-decision-router-consumer.md)
are the flagship consumers of this public contract. Their roadmap links
typed decisions to bounded programs, independent completion evidence, and
an inspectable terminal experience. Consumer integration is tracked in
[#9501](https://github.com/OpenAgentsInc/openagents/issues/9501).

Product implementations follow [the repository contract](../../../AGENTS.md):
Rust, with the existing Swift bridge exception for Apple's model. Web
interfaces, gateways, workers, CLI tools, and MCP servers follow that rule.
Client packaging must resolve the language boundary before adding another
product language. Verification runs manually or on non-GitHub infrastructure
under [the verification guide](../../verification.md).

## Existing foundation and evidence

| Capability | Current evidence and limit |
| --- | --- |
| Native typed decisions | `crates/jev` implements the client contract; `kev-serve` and `lev-serve` serve `POST /v1/systemone` and `GET /v1/models`. |
| Tenant foundations | `crates/tenancy` implements registry bindings, key storage/lifecycle, and durable quota reservations. These library components do not establish the hosted gateway or full customer account system. |
| Execution receipt schema | `crates/receipts` defines per-attempt identities, typed outcomes, timing, and request/result digests. HTTP/relay publication and Coder consumption remain integration work under [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) and [#9505](https://github.com/OpenAgentsInc/openagents/issues/9505). |
| Local serving | [Kev](../../kev/README.md) runs the open model locally; [Lev](../../lev/README.md) reaches Apple's on-device runtime through the Swift helper. Neither binary establishes the hosted tenant service. |
| Artifact and execution identity | [Model identity](../../gym/model-identity.md) records the serving process's claim about loaded artifacts and configuration. Authentication can make that claim attributable; the card alone is not remote execution attestation. |
| Caller-owned evaluation | [Measured records](../../gym/measured-records.md) covers `gym build`, evaluation, reports, and receipt-chain verification. Intake, reports, and Rust intake are landed: [#9464](https://github.com/OpenAgentsInc/openagents/issues/9464), [#9465](https://github.com/OpenAgentsInc/openagents/issues/9465), and [#9477](https://github.com/OpenAgentsInc/openagents/issues/9477). |
| Report coverage and commitments | [Measured records](../../gym/measured-records.md) now includes expected coverage, `gym report --commitment`, and `gym verify --commitment`; [#9478](https://github.com/OpenAgentsInc/openagents/issues/9478) and [#9479](https://github.com/OpenAgentsInc/openagents/issues/9479) are landed. Authenticity still depends on the independently trusted channel that carries the commitment. |
| Relay infrastructure | Authentication, capability manifests, and Coder jobs exist. A versioned decision job, shared authorization/accounting, and decision-worker admission remain open: [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469) and [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470). |
| Training and admission | Adapter formats, artifact locks, retained training tooling, and Gym exist. [Candidate admission](../service/candidate-admission.md) implements frozen comparisons, retained evidence, one-shot confirmation, and registry activation/rollback. Synthetic verification does not qualify a production candidate. Tenant training remains tracked in [#9472](https://github.com/OpenAgentsInc/openagents/issues/9472). |

Latency evidence applies to its recorded workload and host. Hosted Jev's
quiet internet p50 is about 100 ms
([record](../../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md));
local CPU Kev's is about 280–350 ms
([record](../../gym/measurements/2026-09-20-kev-quiet-latency.md)).
A candidate 4B door ranges from about 84–952 ms across request shapes
([record](../../kev/measurements/2026-09-20-candidate-4b.md)). These are not
service-level commitments, bulk-throughput measurements, or evidence for a
different model.

## Native decision contract

The native API remains `POST /v1/systemone`. A request carries `state`
and named `questions`. Each question defines its `type`, `instructions`,
and `criteria`. A response carries model identity, named typed answers,
and usage. Preserve existing client compatibility when adding service
metadata, receipts, or version negotiation.

| Primitive | Meaning | Output |
| --- | --- | --- |
| `Noul` | Probability that a proposition holds | `noul`, from 0 to 1; no separate confidence |
| `Choice` | One alternative from a supplied set | Selected choice, confidence, and categorical probabilities |
| `Score` | Position on an ordered, described rubric | Probability-weighted score, confidence, legend, and probabilities |

Independent questions over one state belong in one request. They cannot
read each other's answers. A second request is appropriate when an earlier
answer determines new evidence or options. Structured state and mixed
question types remain first-class; the text facade does not replace them.

Typed output guarantees a shape, not truth. Choice and Score confidence
describe distribution concentration, not permission to act or an
end-to-end correctness probability. A Noul near 0.5 expresses uncertainty
about the proposition, not medium severity. Known rules, calculations,
permissions, and execution stay in code.

### Outcomes and identity

Every surface preserves answered, model-refused, and unavailable outcomes.
Batch and accounting records also distinguish unattempted work and unknown
completion. A missing answer is neither a wrong answer nor a successful
zero-cost call. Partial outcomes must not disappear from aggregate results.

Strict model selection returns the requested artifact or an explicit
failure. Capacity pressure never silently changes the model, checkpoint,
execution profile, or lane. Review and fallback are separate, opt-in
policies whose additional identities remain visible.

The versioned execution receipt binds logical request and attempt IDs,
tenant reference, request/result digests, requested and actual
artifact/execution identity, policy identity, outcomes, timing, and
usage/accounting references. Batch receipts also identify items and
dimensions; reviewer and fallback attempts retain their own records.

Bind identity to the operation that produced the result. A later
`GET /v1/models` lookup cannot establish which version answered an earlier
request. Keep transport, queue, forwarding, inference, and review timing
distinguishable across HTTP and relay execution.

Ordinary inference requires no Gym suite or gate. Evaluation references
appear only when a declared evaluation supplies them. Authenticated runtime
receipts are attributable serving claims; benchmark commitments and remote
execution attestation are separate guarantees. Owner: [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471);
the versioned receipt type is landed in `crates/receipts`, and the HTTP and
relay paths adopt it with [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468) and [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469).

## Classification and batch contract

Add `POST /v1/classify` as a convenience facade over the native primitives.
A caller supplies input text or input records, stable IDs, labels or named
dimensions, optional instructions, model selection, capacity selection, and
a decision policy. Label sets are supplied at request time.

| Workflow | Contract |
| --- | --- |
| Single-label classification | One Choice per input, with the complete label distribution |
| Multi-label tagging | One independent Noul per label; return all scores and the selected labels |
| Multidimensional classification | Named dimensions with their own criteria, outputs, confidence, and outcomes |
| Binary filtering | One Noul per input; return scores and a caller-selected subset |
| Ranking and rubric assessment | Comparable per-item Scores with explicit rubric definitions |
| Counts and uncertainty review | Aggregate returned outcomes or select uncertain items without requiring an agent to read the entire corpus |

Multi-label probabilities need not sum to one. Specify threshold, top-N,
ties, exclusions, and no-match behavior in the versioned policy. Do not
assign a universal confidence threshold. Preserve raw scores so callers can
change a filter without rerunning inference when the evidence and question
meanings are unchanged.

The initial facade design targets 1–1,000 inputs, 2–100 labels, up to 20
dimensions, and a ceiling of 1,000 item-dimension decisions per synchronous
call where the backend supports them. These are schema design targets,
not present capacity or speed claims. Multi-label work counts input-label
judgments, not merely input rows. Native binary decisions remain valid.

The selected backend can impose tighter limits on text bytes/characters,
tokens, labels, label length, instructions, questions, and total context.
Discovery and preflight validation publish those limits. State, instructions,
and options must all fit; reject oversize atomic work without silent
truncation. Do not advertise the facade maximum for a backend that cannot
serve it.

Validate the request envelope before dispatch. Preserve input order and IDs,
including when workers finish out of order. Each item/dimension reports its
outcome, actual model, raw scores, selected output, review status, usage,
and timing. Return explicit mixed outcomes after partial execution. Define
empty inputs, duplicate IDs, Unicode, invalid labels, and malformed
dimensions in shared contract fixtures. Owner: [#9482](https://github.com/OpenAgentsInc/openagents/issues/9482).

## Batch execution and durable jobs

A bulk payload and efficient inference batching are different capabilities.
The scheduler packs compatible work by context and resource budget, uses
bounded backend concurrency, and reconstructs results without crossing
item, question, or tenant boundaries. Publish which adapters batch work
natively and which use bounded independent calls.

Interactive and bulk capacity have separate queue and admission policies.
Bound pending work, active requests, tokens/options/questions, estimated
memory, deadlines, and per-tenant resource use. Dedicated capacity belongs
to its assigned tenant. Cancellation, cold starts, backend death, and
saturation produce explicit outcomes; retryable pressure carries
`Retry-After`. Split only at declared item/question boundaries.

Measure completed decisions per second, total request latency, per-item
latency, queue time, cold/warm behavior, cost, and expected coverage. Compare
serial and packed execution on pinned workloads. Amortized milliseconds
per decision are not the time a caller waits for a batch.
Scheduler owner: [#9483](https://github.com/OpenAgentsInc/openagents/issues/9483).

For work that outlives a request, add a durable job API:

- Persist the accepted manifest before acknowledging submission. Bind
  idempotency to tenant and request content.
- Expose job status, progress, cancellation, and paginated or streamed result
  export with stable job, item, request, and attempt IDs.
- Preserve expected, attempted, answered, refused, unavailable, unattempted,
  and unknown counts through queued, running, cancelling, and terminal
  states. A terminal job can still have incomplete successful coverage.
- Recover after gateway or worker restart without losing accepted inputs or
  double-settling usage. Do not claim exactly-once inference when completion
  is ambiguous.
- Define result retention, deletion, cursor expiry, and download
  authorization. Exports preserve order/IDs, outcomes, identities,
  policies, and receipts.
- Deliver results through polling and opt-in signed webhooks. Bound retries,
  make duplicate delivery harmless, rotate signing secrets, and validate
  destinations.

A relay event or an in-memory queue is not durable job storage.
Job owner: [#9484](https://github.com/OpenAgentsInc/openagents/issues/9484);
the implemented contract is [durable-jobs](../service/durable-jobs.md).

## Review, abstention, and fallback

A caller can choose direct decisions, uncertainty-targeted review, or a
declared multi-pass workflow. The policy names the primary model, reviewer,
trigger, allowed fallbacks, maximum reviewed items, and attempt, latency,
and spend budgets. Version and digest that policy.

Preserve the original output and the review result separately. Return the
selected final output, review reason and outcome, identities, usage, and
timing. If a reviewer changes a label, it does not inherit the original
model's probability. A reviewer without a valid scored distribution returns
a null or unavailable score. Validate its output against the allowed labels
or typed schema.

Choose triggers using the caller's development data and consequences.
Confirm quality on held-out data. Report confident errors, review coverage,
reviewer errors, calibration, risk-coverage curves, and total cost/latency;
review is not assumed to improve every task. The
[abstention design](../research/abstention.md) remains a distinct proposed door-side
signal and must not be presented as already implemented.

Fallback is opt-in and names permitted causes and destinations. A transport
failure, capacity failure, and semantic refusal are different causes.
Fallback must not bypass an applicable refusal rule or expand data
disclosure without authorization. Return the complete attempt chain.
Reviewer failure or exhausted budget remains visible even if the caller
elects to use the original answer. Owner: [#9485](https://github.com/OpenAgentsInc/openagents/issues/9485).

## Models, capacity, and deployment

Model selection, capacity, and decision policy are independent axes.
Changing a processing lane must not silently select a different checkpoint.
Reject unsupported combinations instead of accepting options with no effect.

| Deployment | Intended behavior |
| --- | --- |
| Shared hosted | Stock authorized models under per-tenant quotas and shared admission; a free allowance changes limits, not model identity |
| Dedicated hosted | Named tenant capacity, pinned artifacts, explicit queue and isolation guarantees |
| Trained | An admitted tenant artifact bound to shared or dedicated capacity under an explicit isolation policy |
| Self-hosted | The same contract operated on caller-controlled hardware, including Kev |
| On-device | Lev through the supported Apple runtime and Swift bridge; input remains local when the complete workflow is local |

A model card describes primitives, modality, languages, context limits,
label/question limits, batch support, probability semantics, artifact and
execution identity, supported capacity, and availability. The tenant registry
authorizes access to those capabilities. Atomic updates and rollback retain
the binding under which an in-flight request was admitted. Owner:
[#9474](https://github.com/OpenAgentsInc/openagents/issues/9474), landed in
`crates/tenancy` as a versioned, self-digested manifest with admission
snapshots and digest-archived revisions.

Add a Rust Laya integration alongside the existing doors, with licensed,
digested artifacts and explicit tokenizer/runtime configuration. The
[existing Laya review](../others/2026-09-19-laya.md) is reference material, not
evidence that an integration exists. English and multilingual checkpoints
must remain identifiable. Any automatic language routing is versioned and
evaluated on mixed-language and routing-error cases.

Each new backend needs conformance, quality, calibration, coverage,
cold/warm latency, throughput, memory, and cost evidence on declared
workloads. Upstream confidence claims and another model's benchmark cannot
stand in for local validation. Owner: [#9486](https://github.com/OpenAgentsInc/openagents/issues/9486).

## HTTP behavior and compatibility

The gateway admits every route through the same authentication,
authorization, quota, identity, and receipt path. It bounds bodies, tokens,
options, questions, queue depth, active forwards, and per-tenant resources.
Cancellation and restart must not leave unbounded work running.
Owner: [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468).

The admission path is landed in `crates/gateway`: authenticate the
bearer key, authorize the named door, bound the door's declared
capacity and the process's forward count, reserve quota durably, verify
the backend's published model card against the binding, forward, settle
once, and leave a sealed receipt in `receipts.jsonl`. The native
`POST /v1/systemone` contract, `GET /v1/models` discovery, idempotency
semantics, refusal codes, deployment shape, and retention rules are in
[gateway.md](../service/gateway.md). The classification, batch, and job routes the
expanded product names still follow — each joins this same admission
path rather than growing a second one.

Keep the existing native routes and add the classification and job routes
through versioned contracts. Specify any root POST, batch, or GET
compatibility aliases before exposing them. GET quick examples use public
sample input; private input uses POST because URLs can enter browser,
proxy, and access-log history. An alias cannot bypass admission.

Define stable typed errors for invalid input, authentication, authorization,
quota/spend exhaustion, missing accounts, unsupported models, capacity,
backend failure, and idempotency conflicts. Return request/version
identifiers, applicable rate-limit metadata, and retry guidance. Daily
quota exhaustion is different from temporary capacity pressure.

Publish public versus credentialed CORS rules, content types, health and
readiness behavior, and an additive-versioning policy. The release-policy
target is at least six months of notice for supported public API breaking
changes, with documented security exceptions; ratify that policy before
launch. Keep changelogs and migration instructions beside the contract.

A deterministic free simulator provides examples and failure cases without
inference or billing. Live test mode is separately labeled, authorized, and
metered. Do not call a live production alias a free sandbox.

## MCP and agent integration

Provide Rust Streamable HTTP MCP servers backed by the same schemas and
admission path as REST.

| Surface | Tools |
| --- | --- |
| Inference | `classify_texts`, `classify_dimensions`, `classify_multi_label`, `count_labels`, and `review_uncertain` |
| Native decisions | A typed-decision tool exposing Noul, Choice, Score, and structured state |
| Documentation | `list_docs`, `read_doc`, `search_docs`, and `get_examples` |

Return structured content plus concise text. Aggregates and filtered
uncertain items should keep unnecessary input out of the agent context.
Preserve identity, policy, partial outcomes, idempotency, cost, and errors.
Tool annotations must describe actual effects: an inference tool can
consume money or quota even when it changes no external content.

Public docs and authorized inference have different access rules.
Authenticate with supported scoped credentials; advertise OAuth metadata
only when that flow is implemented and verified. Test protocol negotiation,
reconnection, tool errors, quota exhaustion, and supported client
configurations. Owner: [#9487](https://github.com/OpenAgentsInc/openagents/issues/9487).

## CLI and language clients

Ship the Rust CLI and usable caller documentation with the first HTTP
service. The CLI supports positional input, standard input, line-oriented
text, JSON, and NDJSON; selected fields; stable IDs; labels and dimensions;
multi-label output; counts; uncertainty filtering; and model/capacity/review
selection.

Define ordered streaming output, bounded batch/concurrency settings,
progress on standard error, quiet machine-readable output, cancellation,
broken-pipe behavior, and stable exit codes for mixed results and
client/configuration failures. Retries honor `Retry-After`, are bounded,
and do not loop indefinitely on an exhausted daily allowance. Invalid rows
remain visible. Credentials come from protected configuration or the
environment and stay out of output and process arguments.

Explain the build/evaluate/report/verify flow and the meaning of every
primitive. Include one refusal, one retryable failure, one partial batch,
and one review with no valid score. Rust CLI and docs owner:
[#9476](https://github.com/OpenAgentsInc/openagents/issues/9476).

The first caller surface is landed: `crates/oak` is the CLI (`ask` for one
state or a bounded batch, `models` for the caller's doors), and
[caller.md](../guides/caller.md), [openapi.yaml](openapi.yaml), the runnable
[examples/](../examples/), and the `decision-api` skill cover the contract.
Labels, dimensions, multi-label output, and capacity/review selection wait
for the classification and review routes that carry them.

Extend the Rust client and provide runnable curl, Python, Go, and JavaScript
HTTP examples. Supported Python and Go SDK distributions are part of the
target product, with a tracked architecture decision: use a Rust-owned
core/binding or obtain an explicit product-language policy exception before
adding another implementation language. Examples alone do not satisfy SDK
delivery. Do not introduce a TypeScript product implementation.

Publish package support/version matrices, installation and update guidance,
checksummed CLI binaries, release provenance, typed errors, timeout and
cancellation behavior, and shared conformance fixtures. Actual package
publication follows the manual release process. Owner: [#9489](https://github.com/OpenAgentsInc/openagents/issues/9489).

## Machine-readable discovery

A stable public origin exposes browsable docs and agent-readable discovery:

- OpenAPI 3.1, `/api`, `llms.txt`, `agents.md`, and `auth.md`.
- A public skill document and a well-known agent-skills index.
- MCP server cards, an agent card, and an API catalog for supported
  protocols.
- A docs API with list/read/search/examples, stable IDs, pagination, and
  bounded responses.
- HTML, plain-text, Markdown, and JSON representations where appropriate,
  with correct content negotiation, canonical URLs, sitemap, and robots
  metadata.
- Declarative Codex and Claude-compatible skill/plugin manifests, manual
  installation instructions, and supported client versions.

Generate or validate overlapping metadata against the deployed schema.
A proposed feature does not appear as available merely because this
specification describes it. Discovery includes authentication, limits,
tool costs, retention, and uncertainty guidance.

Public discovery complements authenticated NIP-CAP discovery; neither
authorizes execution. Owners: [#9488](https://github.com/OpenAgentsInc/openagents/issues/9488) for the public surface and
[#9470](https://github.com/OpenAgentsInc/openagents/issues/9470) for relay capabilities.

## Accounts, workspaces, and credentials

Provide sign-up/sign-in, account recovery, session expiry/logout, and
onboarding from account creation to a first successful call. Anonymous
access is an explicit, bounded, operator-funded free tier with abuse
controls, not an authentication bypass to private models.

Separate users, workspaces, credentials, and billing accounts. Support
personal and organization workspaces, membership, workspace switching,
expiring invitations, seat limits, ownership transfer, last-owner
protection, and an owner/admin/member permission matrix. Removing a member
must revoke their access, including existing sessions and applicable keys.

Support default and named keys with scoped models/actions, creation,
copying, pause, rotation, and revocation. Prefer one-time secret display;
any reveal feature needs an explicitly protected design. Persist credential
references for attribution without exposing secrets. Keys do not own a
workspace's usage, credit grant, or history, so rotating a key resets none
of them.

Account APIs and browser sessions enforce tenant isolation and session
protections. Retain access history and redact secrets. Authentication before
inference remains [#9466](https://github.com/OpenAgentsInc/openagents/issues/9466); customer account/workspace lifecycle is
[#9490](https://github.com/OpenAgentsInc/openagents/issues/9490).

The concrete first-service protocol is landed in `crates/tenancy`:
`Authorization: Bearer oak_<id>.<secret>`, where the store
(`keys.json` beside `registry.json`) keeps only the secret's SHA-256
digest. The `tenant-keys` binary is the operator provisioning path —
`issue`, `rotate`, `revoke`, `list` — and a secret is printed once, at
issuance, never to the store or a log. Authentication resolves the key
to its tenant; authorization is then the registry's `authorize` lookup,
so a caller-supplied model name cannot escape the bound doors, and a
rotated key inherits the tenant's doors and quota rather than resetting
them. A scoped key narrows further — `models` names the doors and
`actions` the verbs (`inference`, `models`, `balance`, `accounts`) —
and the gateway refuses `out_of_scope` before the binding is even
named. Anonymous access is an explicit operator choice and reaches only
the manifest's `shared` bindings — never a dedicated or trained door.
On the relay lane, the authenticated NIP-42 principal maps to the same
tenant record before authorization, so one binding decides both
transports. The trusted boundary is the gateway: a directly reachable
door is a misconfiguration, and the deployment policy is that backends
bind a private interface and accept forwarded calls only from the
gateway's identity.

The account lifecycle is landed too — `accounts.json` and
`sessions.json` beside the registry, served by the gateway's `accounts`
module when the `accounts` config block is present. `POST /v1/accounts`
is self-serve sign-up onto the configured `signup_tenant`: account,
personal workspace, first `oak_` key, and first `sess_<hex>` session in
one answer, each secret existing only in that answer. `POST /v1/sessions`
signs an `oak_` key in; `GET`/`DELETE /v1/session` describe and end the
session. On `POST /v1/systemone` a `sess_` token works like a key with
one extra rule — a user session names its workspace with
`X-Workspace-Id`, and the account's fresh membership decides. The
management matrix is HTTP: `POST /v1/workspaces` mints an organization
workspace, and the `{workspace}` routes cover invitations, roles,
removal, ownership transfer, recovery tokens, and keys — named and
scoped at issue, copied, paused, resumed, rotated, and revoked, with the
new secret leaving once and `rotated_from`/`copied_from` lineage kept
for attribution. The `anonymous` config block is the funded public
lane: a stated `bound`, a per-session `session_cap`, and a `ttl_secs` —
`POST /v1/sessions` with no credential mints a session that draws the
budget once per call and reaches only `shared` doors. Removing a
member revokes the membership and ends their sessions in the same
committed write; their keys refuse the workspace on the very next
authentication. `GET /v1/account/access` and the workspace's `/access`
route answer the bounded history — actor, action, references — with
secrets redacted by construction. See
[workspace membership](../service/workspace-membership.md).

## Pricing and monetary accounting

Quota accounting controls resource admission. It does not collect payment
or establish a price. Preserve [#9467](https://github.com/OpenAgentsInc/openagents/issues/9467)'s durable reservation,
idempotency, crash recovery, and unknown-completion semantics.

The durable half of that contract is landed in `tenancy::quota`. A
tenant's `quota` field names its budget in resources — requests,
questions, input bytes per day, and a concurrency bound — plus a
versioned settlement policy (`quota-v1`). The ledger is an append-only
`quota-ledger.jsonl` beside the registry, opened under an exclusive
lock file so one writer owns it at a time. A reservation is durable
before dispatch: `reserve` writes the `reserved` event before the work
it pays for begins, and `(request, attempt)` is the idempotency pair —
the same pair reserved again with the same request digest returns the
reservation that exists, while the same pair with different content is
refused as a conflict. `settle` records the attempt's outcome and
measured units once; a second settle is the same record, never a second
charge. Every reservation carries a deadline: a writer that dies
mid-flight leaves a reservation recovery marks `orphaned` with outcome
`unknown`, counted rather than silently freed, and work that was never
dispatched is `released` back to the budget. Under `quota-v1` an
answered, refused, or unavailable outcome counts against the budget —
the compute ran whether or not the caller received an answer — and
`unknown` counts conservatively because the ledger cannot prove it did
not. Budgets bind to the stable tenant identity, so a rotated key keeps
spending against the same position. The `tenant-usage` binary is the
operator view: each tenant's settled, outstanding, and orphaned counts,
and the reservations still held.

The monetary half is landed in `tenancy::money`, separate from quota.
Amounts are fixed-point millionths of an explicitly named currency; a
price is a versioned schedule binding a model, capacity, review policy,
currency, and rational rates over the billable resources — input, cached
input, output, and reasoning tokens plus compute milliseconds — with
input excluding cached input and output excluding reasoning. Usage must
name every priced resource explicitly; missing usage is unknown, not
zero, and a provider adapter resolves overlapping counters rather than
inventing token counts for an opaque provider.

The ledger is a private append-only `money.jsonl` held under an exclusive
lock, each accepted mutation a digest-linked record carrying an
idempotent source and an audit reference. It records account creation,
grants, top-ups, and adjustments; reserves authorized worst-case spend
under a pinned price before dispatch; settles known usage with a receipt
reference; and keeps unknown completion at full reservation until a later
settlement or an explicitly evidenced release reconciles it — never a
quiet zero. Refunds and their reversals bind to a settled retail charge.
A workspace account carries a hard lifetime spending ceiling and an
explicit top-up opt-in, and binds to the stable workspace rather than a
key, so rotation cannot reset a grant or budget. Retail charge,
provider-reported cost, and allocated hosting cost are separate amounts —
free retail inference still consumes operator funds. `Balance` exposes
exact credited, reserved, settled, refunded, available, and remaining
authorized spend plus the price versions the account transacted under;
`tenant-money` is the operator's view of the same state, and `gateway`'s
monetary admission (`docs/decision-models/service/monetary-accounting.md`)
charges dispatched work under an explicit opt-in.
`docs/decision-models/service/monetary-ledger.md` covers the ledger contract.
Owner: [#9491](https://github.com/OpenAgentsInc/openagents/issues/9491).

## Plans, payments, and entitlements

Define versioned free and paid plans, period allowances, model/features,
seat limits, and subscription transitions. Choose actual launch prices and
currencies through a separate commercial decision supported by measured
costs. This specification supplies the billing architecture, not a price
list or authorization to spend.

Provide checkout, invoices/receipts, a customer billing portal, renewal,
upgrade/downgrade, cancellation, payment failure, refund, and dispute
handling. Browser return URLs never grant money or entitlement. Verify
server-side payment events, tolerate duplicates and out-of-order delivery,
and reconcile lost events against provider state.

Grant sign-up credit once under a stated eligibility policy and a paid
period's allowance once per eligible period. Define credit expiry,
proration, refunds, and seat changes explicitly. Attach grants and purchases
to stable workspace/billing identity. Purchase terms, support/refund
procedures, and actual price configuration must exist before checkout is
enabled. Owner: [#9492](https://github.com/OpenAgentsInc/openagents/issues/9492).

The billing surface is landed in `tenancy::billing` mounted through
`gateway`'s `billing` config block — which requires `accounts` and
`money`, because a subscription binds a workspace and its grants ride
the money ledger. `GET /v1/plans` publishes the configured catalog:
versioned free and paid plans carrying a price in millionths of a
named currency, a period, a per-period allowance, a once-per-workspace
sign-up credit, an optional credit-expiry bound, covered seats,
top-up policy, and a door list (`"all"` or an explicit set). An owner
— `ManageBilling` is owner-only — drives a workspace through
`/v1/workspaces/{id}/billing/*`: direct subscribe for a free plan,
checkout for a paid one, a scheduled plan change at the next renewal
(seats may not shrink below active membership), cancel-at-period-end,
top-ups, the portal link, and reconciliation. `GET
/v1/billing/sessions/{id}` is the browser's display-only return target
— it moves nothing; only a signed `POST /v1/billing/webhook` event
completes a checkout, pays or fails an invoice, or claws credit back.
Signatures are HMAC-SHA256 over `"<t>.<body>"` with the timestamp
inside the MAC and a configured skew bound, so a replayed signature is
stale by construction. Events deduplicate on id and resolve
out-of-order delivery against committed state rather than arrival
order — an event that lands before the state it amends is journaled
unapplied and replayed by reconciliation once the earlier state
commits. Every
grant and clawback posts to the money ledger under a stable
`billing:*` source with its audit string journaled first, so a crash
between the ledger append and the billing seal replays the identical
mutation — a clawback debits the lesser of its amount and the
available balance, never credit already committed to work. Under a
billing config, `POST /v1/systemone` requires a subscribed workspace
whose plan covers the named door, checked before registry
authorization and any quota or money reservation. The provider today
is `sandbox` — an operator-driven event journal emitted by the
`billing-sandbox` binary, exercising checkout, renewal, seat changes,
duplicate and out-of-order events, payment failure and recovery,
cancellation, refund, and dispute end to end. See
[plans, checkout, and entitlements](../service/billing.md) and the
published [purchase terms](../service/billing-terms.md).

## Usage APIs and dashboard

Provide authenticated APIs for exact balances and outstanding reservations,
usage summaries, time series, breakdowns, recent activity, keys, workspace
membership, and entitlements. Filter and paginate by time, key,
agent/source, model, capacity, policy/tier, outcome, and workspace.

Keep exact ledger totals separate from sampled or aggregated operational
statistics. State lag, timezone, rounding, retention, and unknown data.
Attribute primary, reviewer, and fallback costs separately. Quota headroom
does not imply available GPU capacity.

Build a responsive, accessible dashboard for onboarding, key management,
workspace/members, plan/billing access, hours/days usage charts, activity,
and receipt inspection. Include export and deletion controls consistent
with retention and required financial records. Raw input and answers are
private by default. Verify mobile layouts, role-based views, totals,
delayed settlement, and key rotation. Owner: [#9493](https://github.com/OpenAgentsInc/openagents/issues/9493).

Landed in `gateway::usage` and `gateway::dashboard` behind the `accounts`
document: `GET /v1/workspaces/{id}/usage` reports the workspace's
position — totals by outcome, quota-derived units, exact cost fields in
millionths, outstanding holds, breakdowns by model/key/lane/transport,
and the billing entitlement — and `/usage/activity`, `/usage/timeseries`,
`/usage/receipts/{digest}`, and `/usage/export` (NDJSON) read the same
receipts with filters for time window, key, model, outcome, lane,
transport, job, policy, and capacity, under keyset-cursor pagination.
`ExecutionReceipt.workspace` attributes each call to the workspace that
admitted it; unattributed receipts are counted in the disclosure, never
reassigned. The dashboard at `/dashboard` signs a member in with a
session token over an `HttpOnly` cookie and renders the overview, usage
chart, activity, receipt, members, keys, and billing pages — references
and digests only, no raw input or answers. Every answer carries a
disclosure block stating scale, UTC timezone and day boundary, lag,
retention, and skipped/truncated counts. Export is the workspace's own
sealed receipts verbatim; deletion is not offered — the receipt and
ledger logs are the financial record. See
[../service/usage-dashboard.md](../service/usage-dashboard.md).

## Playground and interactive demo

Provide a playground for pasted inputs, bounded file/dataset uploads,
labels, dimensions, native judgments, and model/capacity/review selection.
Show the actual request, raw scores, selected outputs, partial failures,
original and reviewed results, model identity, usage, time, and receipt.
Support export and copying equivalent CLI/API requests.

Keep deterministic simulated examples separate from live inference.
Label synthetic inputs and simulated results. A bounded chat demo can use
the documented tools, showing actual calls/results with caps on turns,
tool steps, and spend. It must not invent outcomes.

If the demo searches or fetches external content, disclose that behavior
and the data destination before use. Bound network access and treat fetched
content as data. Specify session/input retention, deletion, upload limits,
cancellation, quota feedback, and accessible mobile behavior. Owner:
[#9494](https://github.com/OpenAgentsInc/openagents/issues/9494).

## Skill directory and recipes

Publish a versioned directory of reusable decision skills with browse,
search, categories/tags, authors, ranking, raw Markdown, stable version
links, and explicit installation instructions. Accept bounded `SKILL.md`
submissions with content digests, authorship, license/rights, and publication
consent. Detect duplicate and superseded versions.

Review submissions through static validation, decision-model checks, and
bounded reasoning review. Submitted instructions remain inert. Record each
review stage's model/policy version, score, rationale, cost, and failure
state. Define admission, rejection explanations, corrections/appeals,
moderation, withdrawal, and takedown. A model's review is not a security
guarantee. Do not publish rejected private content.

Separate assessed quality from measured task performance. Attach pinned
recipe suites and reports when available, and label missing empirical
evidence. Owner: [#9495](https://github.com/OpenAgentsInc/openagents/issues/9495).

The initial recipe library covers these workflows:

| Area | Recipes |
| --- | --- |
| Filtering and retrieval | Bulk filtering, news/headline feeds, retrieval reranking, and citation checks |
| Support and trust | Ticket triage, content moderation, listing abuse, and prompt-injection screening |
| Operations | Log classification, security alert triage, and semantic linting |
| Agent composition | Model/skill routing and bounded computer-action selection |
| Business and knowledge | Lead qualification, resume/job competency assistance, knowledge relations, and specification conformance |
| Personal workflows | Document intake, file sorting, and voice-command transcript classification |

Every recipe supplies input schema, question definitions, composition code,
expected outputs, source/license, version, bounded cost, uncertainty
behavior, and runnable examples for supported surfaces. Document limits:
transcript classification is not speech recognition; selecting supplied
candidates is not unconstrained generation.

Keep execution permissions in host code. Include dry-run and undo for file
actions and human review for consequential decisions. Measure representative
cases, refusals, and coverage; choose thresholds on development data.
Do not add GitHub workflows for recipes or semantic linting. Owner:
[#9496](https://github.com/OpenAgentsInc/openagents/issues/9496).

## Agent feedback

Publish a structured feedback policy and API for observations,
expected/actual behavior, environment/version, reproduction evidence, and
bounded attachments. Return a receipt and status lookup with an explicit
triage lifecycle, duplicate handling, ownership, and human-readable updates.

Apply authentication, abuse limits, idempotency, redaction, retention, and
tenant visibility. Forward private content only with explicit consent.
Automated categorization cannot silently discard reports. A feedback
receipt is distinct from an inference receipt and a report commitment.
Owner: [#9497](https://github.com/OpenAgentsInc/openagents/issues/9497).

## Measurement and trained endpoints

Preserve measurement as a product, available before hosting. A real caller
can bring permitted labeled data, build a digested suite, evaluate named
doors, and verify the resulting record. The pilot in [#9480](https://github.com/OpenAgentsInc/openagents/issues/9480) tests
that offering against the caller's current workflow, including quality,
coverage, latency, cost, and whether the evidence is useful.

Reports pin suite, questions, gate, partitions, model artifacts, and
execution settings. Publish label provenance and agreement ceilings.
Count refusals and missing work with their denominators. The report now
compares declared expected coverage with recorded outcomes and marks
incomplete runs. Preserve that publication guard as new doors, policies,
and modalities arrive. Landed: [#9478](https://github.com/OpenAgentsInc/openagents/issues/9478).

`gym report --commitment` now writes a digested commitment to the chain
head, row count, declared selection, suite/question/gate/provenance digests,
and each door's run identities and coverage. `gym verify --commitment`
checks the recorded prefix against the caller's retained copy, detecting a
rewritten chain or dropped tail while allowing later appended rows.

A chain that verifies internally does not establish that it is the original
whole store. The retained commitment strengthens that claim only when its
own authenticity comes from a separately trusted channel. Signed or relayed
publication remains work under [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) and [#9475](https://github.com/OpenAgentsInc/openagents/issues/9475); neither a commitment nor its signature attests
remote execution. Local commitment support is landed in [#9479](https://github.com/OpenAgentsInc/openagents/issues/9479).

Training uses caller-permitted data with training examples held apart from
calibration, development, and locked confirmation. Retain provenance,
licenses, leakage controls, recipe/trial budgets, all attempted candidates,
and artifact locks. Deletion and retention include uploaded data and
produced adapters. Owner: [#9472](https://github.com/OpenAgentsInc/openagents/issues/9472).

Candidate admission is an explicit cross-artifact comparison.
`gym regress` remains the same-identity regression guard. Freeze candidate
and base identities, metrics, required improvement, per-family errors,
calibration, coverage/refusal policy, transfer checks, and deployment
budgets before comparison. Choose on development data, then perform the
declared one-shot locked confirmation before activation.

Losing, tied where improvement is required, incomplete, or unverifiable
candidates do not activate. Do not reuse a historical workload's noise floor
as a universal acceptance threshold. Store the admission record and bind
activation/rollback through the registry. Owner: [#9473](https://github.com/OpenAgentsInc/openagents/issues/9473).

The implementation and operator procedure are documented in
[candidate admission](../service/candidate-admission.md). The native evaluator requires
separate retained reports for development, locked confirmation, and transfer,
with exact row and selection verification. The transfer workload declares its
own variance. Explicit deployment evidence must name the frozen workload;
unknown cost cannot become a zero-cost claim. `Registry::activate` consumes a
replayed admission record and refuses a stale base. Authorized discovery cards
publish admitted scope, the record reference, and the registry revision.


Public benchmark/status snapshots use committed evidence with coverage,
identities, conditions, freshness, label basis, privacy controls, and cost
provenance. Separate p50/p95 request latency, queueing, and amortized
throughput. Publish repeatability/determinism only for a tested execution
profile. Owner: [#9475](https://github.com/OpenAgentsInc/openagents/issues/9475).

## Operations, privacy, and portability

Provide versioned packages and configuration for tested shared, dedicated,
self-hosted, and on-device deployments. Document model/host prerequisites,
artifact fetching, TLS/trusted-network boundaries, secrets, health/readiness,
backup/restore, upgrades, and rollback. Verify a fresh install through a
bounded real call and recovery, not just a successful build.

State per-lane retention for raw state, answers, diagnostics, usage,
receipts, uploads, and trained artifacts. Raw payload logging is off by
default. Publish provider/subprocessor disclosure, operator access,
encryption boundaries, deletion/export behavior, and support/security
contacts. A shared host cannot inherit a dedicated tenant's isolation claim;
a dedicated host is still able to read inputs unless a separate mechanism
prevents it.

Local privacy means the whole workflow stays local; a remote reviewer,
telemetry path, or external fetch changes that boundary. Public incident
status and commercial policies reflect actual operation. Offer product
updates only through verified opt-in subscriptions with preferences and
unsubscribe. Keep support consent separate from marketing consent.

Release readiness includes schema/client conformance, tenant isolation,
quota and monetary reconciliation, recovery, accessible customer flows,
privacy behavior, and honest discovery. Use bounded synthetic/fake-provider
tests where they establish behavior; run real model/payment sandbox checks
only under explicit test budgets. Owner: [#9498](https://github.com/OpenAgentsInc/openagents/issues/9498).

## Image-capable decisions

Images are a separate planned capability, not an implied feature of the
existing text endpoints. Define a versioned state representation with
content hashes, media types, count/size/resolution limits, and text-image
composition. Specify upload and any remote-fetch boundaries,
orientation/decoding/downsampling, and which representation the model saw.

Select an actually image-capable backend with provenance, license,
hardware/cost, artifact identity, supported primitives, and calibration
limits. Build permitted evaluation data, separate development and locked
partitions, and measure quality, refusals, coverage, latency, throughput,
and memory before admission.

Carry image handling through receipts, retention, batches, review, and
client/tool contracts. Keep availability experimental until capability
and evidence pass. Owner: [#9499](https://github.com/OpenAgentsInc/openagents/issues/9499).

## Confidential hosted inference

Investigate protecting input from the host operator under an explicit
threat model. Define adversaries, protected data, metadata leakage, trust
roots, key ownership/revocation, side-channel assumptions, and availability.

Compare local execution, dedicated hosting, confidential-computing
attestation, and cryptographic inference. TLS and single-tenant processes
alone do not provide host-blind inference. A proof of concept must bind
model/configuration, handle keys and upgrades, and measure latency,
throughput, cost, supported models, and any claimed answer equivalence.

Require independent protocol/security review before making a public
confidentiality or remote-execution-attestation claim. Publish a feasibility
decision, including rejection if requirements cannot be met. Existing
receipts and hash chains do not imply this guarantee. Owner:
[#9500](https://github.com/OpenAgentsInc/openagents/issues/9500).

## Delivery plan and issue ownership

The expanded product is a set of dependent releases, not one gateway
launch. Keep useful narrow releases available while later work proceeds.

| Stage | Issues and completion boundary |
| --- | --- |
| Landed measurement tools | [#9464](https://github.com/OpenAgentsInc/openagents/issues/9464), [#9465](https://github.com/OpenAgentsInc/openagents/issues/9465), [#9477](https://github.com/OpenAgentsInc/openagents/issues/9477): caller intake, reports, and Rust suite builder; keep closed |
| Caller validation | [#9480](https://github.com/OpenAgentsInc/openagents/issues/9480): permitted workload and independently usable measured record; does not block independent Rust foundation work |
| Landed evidence controls | [#9478](https://github.com/OpenAgentsInc/openagents/issues/9478), [#9479](https://github.com/OpenAgentsInc/openagents/issues/9479): report coverage and independently retained commitments; keep closed and preserve their guarantees in service publication |
| Shared serving foundation | [#9466](https://github.com/OpenAgentsInc/openagents/issues/9466), [#9467](https://github.com/OpenAgentsInc/openagents/issues/9467), [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471), [#9474](https://github.com/OpenAgentsInc/openagents/issues/9474): agree on tenant, request/attempt, identity, outcome, and reservation contracts; the registry itself is landed in `crates/tenancy` |
| First usable HTTP service | [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468), [#9476](https://github.com/OpenAgentsInc/openagents/issues/9476): bounded gateway and usable native docs/CLI |
| Classification and scale | [#9482](https://github.com/OpenAgentsInc/openagents/issues/9482), [#9483](https://github.com/OpenAgentsInc/openagents/issues/9483), [#9484](https://github.com/OpenAgentsInc/openagents/issues/9484): facade, efficient scheduling, then durable jobs |
| Decision policies and backends | [#9485](https://github.com/OpenAgentsInc/openagents/issues/9485), [#9486](https://github.com/OpenAgentsInc/openagents/issues/9486): measured review/fallback and capability-aware additional models; neither is required for strict native inference |
| Agent and client distribution | [#9487](https://github.com/OpenAgentsInc/openagents/issues/9487), [#9488](https://github.com/OpenAgentsInc/openagents/issues/9488), [#9489](https://github.com/OpenAgentsInc/openagents/issues/9489): MCP, discovery/plugins, client packaging and SDK policy decision |
| Customer access | [#9490](https://github.com/OpenAgentsInc/openagents/issues/9490): accounts/workspaces/keys on the foundation authentication and quota contracts |
| Commercial service | [#9491](https://github.com/OpenAgentsInc/openagents/issues/9491) then [#9492](https://github.com/OpenAgentsInc/openagents/issues/9492): exact money accounting, then verified payments/plans; no paid launch before both |
| Customer visibility | [#9493](https://github.com/OpenAgentsInc/openagents/issues/9493): usage APIs and dashboard using the account, receipt, quota, and money sources of truth |
| Interactive adoption | [#9494](https://github.com/OpenAgentsInc/openagents/issues/9494): playground and bounded chat using implemented tools and explicit live budgets |
| Reusable workflows | [#9495](https://github.com/OpenAgentsInc/openagents/issues/9495), [#9496](https://github.com/OpenAgentsInc/openagents/issues/9496): skill directory and verified recipes, released incrementally |
| Feedback and operation | [#9497](https://github.com/OpenAgentsInc/openagents/issues/9497), [#9498](https://github.com/OpenAgentsInc/openagents/issues/9498): trackable feedback, portable deployment, privacy/support/version policies, and operating verification |
| Relay distribution | [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469), [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470): versioned decision jobs and authenticated capability discovery sharing the foundation contracts |
| Trained endpoints | [#9473](https://github.com/OpenAgentsInc/openagents/issues/9473) defines admission; [#9472](https://github.com/OpenAgentsInc/openagents/issues/9472) produces candidates; [#9473](https://github.com/OpenAgentsInc/openagents/issues/9473)/[#9474](https://github.com/OpenAgentsInc/openagents/issues/9474) confirm and activate; relay is not a prerequisite |
| Public evidence | [#9475](https://github.com/OpenAgentsInc/openagents/issues/9475): publish supported snapshots after coverage/commitments; do not defer basic caller documentation until this stage |
| Experimental extensions | [#9499](https://github.com/OpenAgentsInc/openagents/issues/9499), [#9500](https://github.com/OpenAgentsInc/openagents/issues/9500): image decisions and confidential inference require separate feasibility and admission evidence |

Track overall delivery in [#9481](https://github.com/OpenAgentsInc/openagents/issues/9481). Close feature issues only when
their acceptance evidence exists; writing this specification completes none
of the unbuilt service features.

The remaining deployment comparisons in [#9382](https://github.com/OpenAgentsInc/openagents/issues/9382) and
[#9393](https://github.com/OpenAgentsInc/openagents/issues/9393), and serving-verification closure in [#9426](https://github.com/OpenAgentsInc/openagents/issues/9426),
remain open and deferred by the operator. This update does not restart
those runs. Attention optimization, caching, and Coder-specific fine-tuning
in [#9459](https://github.com/OpenAgentsInc/openagents/issues/9459), [#9460](https://github.com/OpenAgentsInc/openagents/issues/9460), and [#9461](https://github.com/OpenAgentsInc/openagents/issues/9461) remain
measurement-led decisions, not blanket prerequisites for the first service.
