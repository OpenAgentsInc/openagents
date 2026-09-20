# An open decision API

This document specifies a product this repository could host: an API where a
caller sends a state and a set of typed questions and gets back typed answers
with probabilities. It is a design, not an implementation. Every claim about
what exists is named beside the code or record that backs it; what does not
exist is named as missing, and the ordered work list at the end is written to
be filed as issues.

Working name: the OpenAgents Decision API. "Classifier" undersells it —
classification is a `Choice`; the contract also prices confidence (`Noul`)
and ranks positions on a rubric (`Score`), so every classification shape a
caller has lands on the same endpoint.

## Why the contract is the product

A hosted language model can classify anything. The reason to make a network
call to a decision model instead is the same reason `crates/coder` makes one:
reading the input is the expensive part, and the answer has to come back as a
number over a fixed option set, not prose to be re-parsed. State in, typed
judgments out, no text generated, no answer leaking into the next question.

The wire contract is `POST /v1/systemone`, and `crates/jev` is the client.
Everything below serves that contract or scores it; the API is one contract
with several doors behind it, which is what this repository already runs.

## The wire surface

Two endpoints, both already served by `kev-serve` and `lev-serve`:

```text
POST /v1/systemone
GET  /v1/models
```

A request carries a `state` and a map of named `questions`. A question is
`type`, `instructions`, and `criteria` — the exact option set, supplied per
request. A response carries `model`, `answers` keyed by question name, and
`usage`. Every answer is typed:

| Type | Asks | Answer |
| --- | --- | --- |
| `noul` | Is this true? | `noul`, a probability of yes |
| `choice` | Which of these options? | `choice`, `confidence`, `probabilities` |
| `score` | Which level on this ordered rubric? | `score`, `confidence`, `legend`, `probabilities` |

`GET /v1/models` returns model cards carrying `artifact_identity` and
`execution` (`docs/gym/model-identity.md`). A card is the serving process's
authenticated claim about what it loaded — enough to detect a substitution
and to attribute a result — but not remote attestation of which weights
actually executed. That distinction holds everywhere this document says
"verify".

### Every classification shape on one API

The caller supplies the option set at request time and the door's readout is
a pointer over it, so no retraining is needed for a new label set — and every
classification task a caller has is a composition of the three primitives:

| The caller wants | The question is |
| --- | --- |
| Single-label classification | One `choice` over the labels |
| Multi-label tagging | One `noul` per label, all in one request |
| Binary flags and filters | One `noul` |
| Severity, priority, tiering | One `score` over an ordered rubric |
| Confidence-gated automation | Any of the above, read the `probabilities`; the `abstain` design in [`abstention.md`](abstention.md) adds the door-side "do not act on this" signal |
| Routing across many dimensions | One question per dimension, batched in one call |

Batching is the request itself: a request's question map carries many
questions over one state, and `kev`'s packed prefill with block-causal
question isolation exists to keep that cheap (`docs/kev/architecture.md`).
Many states is many requests, or a bulk lane with a per-tenant queue — see
"lanes" below.

### Failure semantics

The three-way distinction the gym already enforces is the API's error model:

- **Answered** — a typed answer came back.
- **Refused** — the door declined, with a code. Lev's guardrail refusals are
  the example; a refusal is a result about the door, recorded, and never
  silently swapped for another door's answer.
- **Unavailable** — a transport or capacity failure (`429`, `503` with
  `Retry-After`). Overload never silently switches the model or the lane.

A caller that paid for `kev-0.6b` at a stated `artifact_identity` gets that
artifact's answer or a refusal — never a smaller door's answer wearing the
same status code.

## The product properties, mapped

### Private inference

The doors are already private in the strongest sense: `kev-serve` and
`lev-serve` run on the caller's own hardware, and the open weights pin their
content digests. For a hosted deployment the claim becomes narrower and more
honest: single-tenant processes, a stated retention policy, and a
`GET /v1/models` card that lets the caller verify which weights answered.
"Your text never reaches a shared provider" is free when the provider is the
caller; when it is us, the honest version is "your text reaches one process,
its identity is published, and retention is stated."

### Faster inference

No tokens are generated, so latency is a forward pass. Measured, not
asserted — and a latency number is meaningless without its workload,
checkpoint, and host. Hosted Jev answers in a p50 of ~100 ms over the
public internet
([`../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md`](../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md));
a local `kev` door on a quiet CPU host answers in a p50 of ~280 to 350 ms
([`../gym/measurements/2026-09-20-kev-quiet-latency.md`](../gym/measurements/2026-09-20-kev-quiet-latency.md));
a candidate 4B door's quiet-host p50 ranged from ~84 to ~952 ms across
request shapes
([`../kev/measurements/2026-09-20-candidate-4b.md`](../kev/measurements/2026-09-20-candidate-4b.md)).
"Fast" is a property of a named configuration on a named workload, never
of the API in general.

### Dedicated endpoints

A dedicated endpoint is a name bound to a published identity, not a routing
alias: the card's `artifact_identity` and `execution` fields say exactly what
is loaded, and a change produces a different recorded identity. Dedicated
capacity is a named door — one process, pinned weights, a tenant key — and
at scale a NIP-CJ job lane through the relay, the pattern `coder-worker`
already uses to answer job requests.

### Trained endpoints

The trainable unit is small and already versioned: a LoRA adapter plus a
pointer head, the shape `docs/kev/README.md` describes. A trained endpoint is
an adapter fitted to one caller's labelled decisions, served as its own door,
with its artifact digest published the moment it exists. The caller can check
it is being served what was trained — as a claim bound to the result, per the
identity caveat above.

Admission is an explicit comparison, not the regression guard: `gym regress`
deliberately refuses a changed artifact identity, and a base model plus a
trained adapter is exactly that. Promotion is a declared candidate-versus-base
run — identities frozen up front, improvement required on the named metric,
mandatory guards on family errors, calibration, refusals, and deployment
budgets, and an `unverifiable` outcome never activates anything
(`docs/gym/regression.md` stays what it is: the same-identity guard for
regressions). An adapter that does not beat the base door on the caller's own
development partition does not ship. The ingredients exist: the adapter
format, the artifact lock and digest machinery
(`scripts/fetch-kev-artifacts.py`, `docs/kev/artifacts.md`), and the
measurement plane. The missing pieces are training as a service and the
admission record, both specified in the work list below.

### A self-serve API

The relay stack is the self-serve shape already: NIP-42 authenticates a
caller by key, a `kind:30180` capability manifest in `capabilities/`
describes what is being offered, and a `kind:30182` program or a NIP-CJ job
request is the unit of work. No account, no call booking — publish a
manifest, hold a key, call the door. Quotas and paid tiers do not exist
today; what exists is authentication, discovery, and a job lane. A quota
ledger is usage accounting, not payment collection — pricing, invoicing,
and paid-service terms are unspecified until a concrete commercial path
exists, and nothing here should read as a billing design.

### Better, and measured the way it is read here

This is where an open version can be stronger than a leaderboard — and
where its claims have to be stated precisely. Every number a door earns
here comes from `crates/gym`: a suite pinned by digest, a question set
pinned by digest, a gate pinned by digest, and a receipt-chained result
store. What the chain proves is internal consistency: a row edited after
the fact digests to a different receipt, and a row inserted or removed in
the middle breaks the link to its successor. What it does not prove, on
its own, is that the file is whole or old — a writer holding the file can
recompute an entire chain, and a shortened file is a valid prefix. A
public claim is anchored by an independently retained commitment — the
chain head, row count, and declared selection, digested and held
separately from the store — which `gym report --commitment` writes and
`gym verify --commitment` checks a later copy against. And a chain that
verifies is still only as complete as the rows it carries: items lost to
timeouts and dead doors leave no row, so a report that reads as a
finished benchmark shows expected coverage beside recorded outcomes and
marks an incomplete run incomplete.

The published claims carry their ceilings beside them — annotator
agreement on our own suites, published agreement or label basis on
`external-v1` and `external-jevbench-v1` — and the refusal policy is in
the record, not in the marketing. An accuracy is honest here because the
suite is digested, the locked partition is spent once through a ledger,
and the comparison refuses to run unless the instrument is held fixed.

## The lanes

Four lanes, one contract:

| Lane | What it is | Who it is for |
| --- | --- | --- |
| **Shared** | The stock doors — hosted Jev, the pinned `kev` variants — behind per-key quotas. Free tier is the same doors at a tighter quota, not a worse model. | Evaluation, low volume, latency-insensitive callers |
| **Dedicated** | A named door on named capacity — one process, one artifact identity, one tenant key. | Latency-sensitive paths, callers who need the queue to be theirs |
| **Trained** | A dedicated door plus a per-tenant adapter fitted to the caller's labelled suite, admitted through the explicit candidate comparison before it serves. | Callers whose labels are theirs and whose accuracy is measured on their own items |
| **On-device** | `lev-serve` wherever the caller's Apple silicon is. Not a lane we host; it is the API's escape hatch to full privacy. | Callers whose text cannot leave the device |

A trained lane that cannot beat its base door on the caller's own
development partition is refused admission and the record says so. The
admission is the explicit candidate-versus-base comparison described under
"Trained endpoints" — a declared, digested decision record, not the
same-identity `gym regress` guard, which stays in its own job of catching
unexpected change in an unchanged door.

## Measurement as part of the API

The differentiator worth building first is the record, not the endpoint:

- **A caller's own suite, pinned.** `gym build` turns a caller's labelled
  JSONL into a digested three-partition suite: `label_source` naming them,
  calibration and development partitions open, a locked partition spent
  once. `docs/gym/measured-records.md` is the flow.
- **A receipt-chained record.** `gym eval` rows carry the suite, question,
  and gate digests and the door's artifact identity. The chain detects
  edits and reordering inside the store it walks; the stronger guarantee —
  that this is the store, whole — is the independently retained
  commitment, and the report's declared selection is what makes a
  completed evaluation distinguishable from a partial one.
- **Ceilings beside scores.** A second-reading or published-agreement
  ceiling prints beside the accuracy, as every record in this repository
  already does, because a score without its ceiling is a claim.
- **Refusals in the record.** A declined item stays in the denominator. An
  API whose accuracy can only be read on answered items is reporting the
  optimistic number, and the gates here do not allow it.
- **Execution receipts, separate from evaluation records.** An ordinary
  inference call has no suite or gate; its receipt carries request and
  attempt identity, the tenant reference, the requested and actual
  artifact identities, the outcome, and timing. That receipt is an
  attributable claim, shared verbatim between the HTTP and relay paths —
  not an evaluation record, and not remote attestation.

## What is missing, honestly

- **Keys, quotas, and everything past them.** Nothing authenticates a
  tenant, reserves capacity, or meters calls. The honest properties are
  already in the idiom — a budget ledger, worst-case reservation before
  spend, unattempted work reported rather than scored — but none of it is
  written for tenants, and a quota ledger is not a billing system. Pricing
  and payment collection are unspecified until a concrete commercial path
  exists.
- **Multi-tenancy.** Doors are single-process; a dedicated endpoint is a
  process per caller, which is honest but does not scale. The NIP-CJ lane
  is the designed answer and is unbuilt for this purpose.
- **Evidence anchors beyond the commitment.** The receipt chain verifies
  internally and the report commitment catches a rewritten or shortened
  store, but the commitment's authenticity rides on the channel that
  carried it — a signed or relayed commitment is open work (#9471), and
  no document attests remote weights.
- **The training path and its admission contract.** Per-tenant adapter
  training is retained tooling, not a service, and the explicit
  candidate-admission comparison is designed but unbuilt (#9472, #9473).
- **A public surface.** No hosted endpoint, no key issuance, no status
  page, no usage dashboard. The repository proves the measurement and
  serving halves; the front half is the part that does not exist.

## The work, in order

The corrected sequence, tracked in openagents#9481. Phase 0 is landed; the
evidence issues come before the service foundations because everything
after them quotes reports.

### Phase 0 — measurement as a service (landed)

1. **Caller-suite intake** (openagents#9464, landed; ported to `gym build`
   in openagents#9477). A caller's labelled data (JSONL of `family`,
   `kind`, `state`, `truth`, and `question`) becomes a digested gym suite
   and question set: `label_source` naming the caller, a `label_rule` per
   item, a `--agreement` ceiling per family, and paraphrase groups held in
   one partition. A caller's file round-trips through `Suite::load` and
   the digest is reproducible.
2. **The measured report** (openagents#9465, landed). `gym report` renders
   a store of rows as a standalone record — digests, chain head, per-door
   and per-family tables, refusals counted, ceilings beside scores — and
   `gym verify` walks the receipt chain. `docs/gym/measured-records.md`
   is the caller-facing flow.

Phase 0 is the product's smallest shippable unit: "we measured your labels
against these doors and here is the receipt chain" needs no accounts, no
billing, and no new serving code.

### The caller pilot

3. **A caller pilot** (openagents#9480). One real caller's labelled data
   through the Phase 0 flow, with permission to use it, before the API
   builds further. Done when a caller's measured record exists and they
   have verified its chain themselves. This is the product's demand test,
   not a gate on the Rust work below.

### Evidence strengthening

4. **Report completeness** (openagents#9478, landed). `gym report` shows
   what was expected beside what was recorded — per door and family:
   expected, attempted, answered, refused, missing — and an incomplete
   run cannot read as a completed benchmark.
5. **Report commitments** (openagents#9479, landed). A versioned
   commitment — chain head, row count, declared selection,
   suite/question/gate/provenance digests, and each door's run identities
   with their coverage — digested into a file a caller retains
   independently of the store. `gym verify --commitment` checks a store
   against it; a recomputed chain and a dropped tail both fail.

### Phase 1 — the serving foundations, shared

These four land together because they are one contract: a tenant identity,
the artifact it may reach, the receipt a call produces, and the quota the
call settles against.

6. **The tenant-artifact registry** (openagents#9474). A versioned binding
   from stable tenant identity to allowed doors, artifact digests, and
   execution configuration. Done when a swapped artifact under a known
   name is detected and refused until the registry is updated.
7. **The execution receipt** (openagents#9471). A versioned receipt
   binding request and attempt identity, tenant reference, requested and
   actual artifact/execution identities, outcome, and timing — for every
   call, not just evaluations. Done when a direct call produces the same
   receipt shape a relay call will.
8. **Per-key authentication** (openagents#9466). Keys checked before
   inference, bound to tenant identity through the registry; anonymous
   access explicit and bounded to the shared lane. Done when revoked,
   rotated, and cross-tenant keys all refuse before any inference runs.
9. **The quota ledger** (openagents#9467). Durable reservation before
   dispatch, retry-safe settlement, terminal states, and recovery — a
   usage ledger, not billing. Done when a crash before dispatch, during
   inference, after answer, or after settlement each leaves the ledger
   consistent.

### Phase 2 — the HTTP service and its docs

10. **The gateway** (openagents#9468). A thin front — key check, quota
    check, forward to a door by name, emit the execution receipt — so
    tenants share one host and doors stay single-process. Done when a
    caller hits one URL with two keys and reaches two different doors.
11. **Caller-facing docs, CLI, and skill** (openagents#9476). The OpenAPI
    document, the `classify`-shaped CLI over `crates/jev`, and the skill —
    shipped with the gateway, not after it, because the first real caller
    needs them to call at all.

### Phase 3 — the relay lane

12. **A decision job kind** (openagents#9469). A NIP-CJ job carrying a
    `/v1/systemone` request, answered by a worker that fronts `kev-serve`,
    emitting the same execution receipt as the HTTP path.
13. **The capability manifest** (openagents#9470). A `kind:30180`
    manifest describing lanes, doors, and limits, so a host discovers the
    service the way `capabilities/` is read — discovery is not
    authorization; the key check still decides.

### Phase 4 — trained endpoints

14. **The admission contract** (openagents#9473). The explicit
    candidate-versus-base comparison described above — frozen identities,
    named metric, required guards, a digested admission record, and a
    losing or unverifiable candidate never activates. Lands before
    training so a produced candidate has somewhere honest to go.
15. **The training pipeline** (openagents#9472). Caller's labelled data
    in — on a training partition held apart from calibration, development,
    and locked — candidate adapter out, artifact-locked and digest-
    published, admitted only through the contract above.

### Phase 5 — the public surface, last

16. **The status and benchmark page** (openagents#9475). Rendered from a
    committed store snapshot — completeness per #9478, commitment per
    #9479, refusals and missing work with denominators, latency with its
    host and shape named. Last because a public claim should not exist
    before the machinery that keeps it honest does.

### What this ordering buys

Phase 0 already produces the revenue-shaped evidence — a caller paying
for a measured record — with no multi-tenant machinery at all. The pilot
proves demand before the service exists. The evidence issues land before
the service because a gateway that emits claims nobody can check is worse
than none. The four foundations land together because auth without
artifact binding, or quotas without a receipt, are halves of four
different contracts. Relay, training, and publicity follow in order of
dependency, not excitement.

## What this document is

Measured: the latency figures, each from the named record. Exists: the
contract, both serving binaries, the adapter format, artifact identity,
the gym's suites, gates, digests, ledger, and store, and the relay's auth,
capability, and job lanes. Landed for this product: caller-suite intake,
the measured report, the Rust suite builder, report completeness, and
report commitments. Filed and open: the caller pilot, the four serving
foundations,
the gateway and its docs, the relay lane, the admission contract and
training pipeline, and the public snapshot — in the order above, per
openagents#9481. Deferred by the operator and not restarted here: the
remaining deployment-latency measurements in openagents#9382 and
openagents#9393, and the serving-verification closure in openagents#9426.
Nothing in this document asserts a product claim that is not backed by a
file or a record named beside it.
