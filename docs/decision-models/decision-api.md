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
`execution` (`docs/gym/model-identity.md`), so a caller can verify which
weights answered rather than trust a name.

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
asserted: hosted Jev answers in a p50 of ~100 ms over the public internet
([`../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md`](../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md));
a local `kev` door on a quiet CPU host answers in a p50 of ~280 to 350 ms
([`../gym/measurements/2026-09-20-kev-quiet-latency.md`](../gym/measurements/2026-09-20-kev-quiet-latency.md)),
and the same measurement method gives Metal its own number
([`../gym/measurements/2026-09-20-kev-4b-metal-pass.md`](../gym/measurements/2026-09-20-kev-4b-metal-pass.md)).

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
it is being served what was trained, and the gym scores the candidate against
the caller's own suite before it is admitted — an adapter that does not beat
the base door on the development partition does not ship. The ingredients
exist: the adapter format, the artifact lock and digest machinery
(`scripts/fetch-kev-artifacts.py`, `docs/kev/artifacts.md`), and the
measurement plane. The missing piece is training as a service — taking a
caller's labelled suite to a candidate artifact — which today is retained
tooling, not a product path.

### A self-serve API

The relay stack is the self-serve shape already: NIP-42 authenticates a
caller by key, a `kind:30180` capability manifest in `capabilities/`
describes what is being offered, and a `kind:30182` program or a NIP-CJ job
request is the unit of work. No account, no call booking — publish a
manifest, hold a key, call the door. Quotas and paid tiers do not exist
today; what exists is authentication, discovery, and a job lane.

### Better, and measured the way it is read here

This is where an open version can be stronger than a leaderboard. Every
number a door earns here comes from `crates/gym`: a suite pinned by digest, a
question set pinned by digest, a gate pinned by digest, and a receipt-chained
result store that cannot be quietly rewritten. The published claims carry
their ceilings beside them — annotator agreement on our own suites, published
agreement or label basis on `external-v1` and `external-jevbench-v1` — and
the refusal policy is in the record, not in the marketing. An accuracy is
honest here because the suite is digested, the locked partition is spent once
through a ledger, and the comparison refuses to run unless the instrument is
held fixed.

## The lanes

Four lanes, one contract:

| Lane | What it is | Who it is for |
| --- | --- | --- |
| **Shared** | The stock doors — hosted Jev, the pinned `kev` variants — behind per-key quotas. Free tier is the same doors at a tighter quota, not a worse model. | Evaluation, low volume, latency-insensitive callers |
| **Dedicated** | A named door on named capacity — one process, one artifact identity, one tenant key. | Latency-sensitive paths, callers who need the queue to be theirs |
| **Trained** | A dedicated door plus a per-tenant adapter fitted to the caller's labelled suite, re-admitted through the gym before promotion. | Callers whose labels are theirs and whose accuracy is measured on their own items |
| **On-device** | `lev-serve` wherever the caller's Apple silicon is. Not a lane we host; it is the API's escape hatch to full privacy. | Callers whose text cannot leave the device |

A trained lane that cannot beat its base door on the caller's own development
partition is refused promotion and the record says so — the gym's regression
machinery (`docs/gym/regression.md`) is the promotion gate, unchanged.

## Measurement as part of the API

The differentiator worth building first is the record, not the endpoint:

- **A caller's own suite, pinned.** `build_external_v1.py` and
  `build_external_jevbench_v1.py` already turn a labelled dataset into a
  digested three-partition suite. A caller's labels become a suite with
  `label_source` naming them, calibration and development partitions open,
  and a locked partition spent once.
- **A receipt-chained record.** `gym eval` rows carry the suite, question,
  and gate digests and the door's artifact identity; the store's chain means
  the numbers cannot be quietly rewritten — by us or by the caller.
- **Ceilings beside scores.** A second-reading or published-agreement ceiling
  prints beside the accuracy, as every record in this repository already
  does, because a score without its ceiling is a claim.
- **Refusals in the record.** A declined item stays in the denominator. An
  API whose accuracy can only be read on answered items is reporting the
  optimistic number, and the gates here do not allow it.

## What is missing, honestly

- **Billing and quotas.** Nothing meters calls or prices them. The honest
  properties are already in the idiom — a budget ledger, worst-case
  reservation before spend, unattempted work reported rather than scored —
  but none of it is written for tenants.
- **Multi-tenancy.** Doors are single-process; a dedicated endpoint is a
  process per caller, which is honest but does not scale. The NIP-CJ lane is
  the designed answer and is unbuilt for this purpose.
- **The training path.** Per-tenant adapter training is retained tooling,
  not a service.
- **A public surface.** No hosted endpoint, no key issuance, no status page,
  no usage dashboard. The repository proves the measurement and serving
  halves; the front half is the part that does not exist.

## The work, in order

Ordered so that each issue lands independently and the cheap, high-signal
ones come first. Nothing later depends on a public endpoint existing.

### Phase 0 — measurement as a service (no new infrastructure)

1. **Caller-suite intake.** A builder in the `build_external_*.py` pattern
   that takes a caller's labelled data (JSONL of `state` + label + family)
   and emits a digested gym suite: `label_source` naming the caller,
   calibration and development partitions open, locked spent once. Done when
   a caller's file round-trips through `Suite::load` and the digest is
   reproducible.
2. **The measured report.** `gym eval` of a caller's suite against named
   doors, packaged as a retained store plus a rendered record — the shape of
   `docs/gym/measurements/` — that a caller can verify: digest the suite,
   walk the receipt chain, read the gate. Done when one real caller suite
   produces a record with ceilings beside scores.

Phase 0 is the product's smallest shippable unit: "we measured your labels
against these doors and here is the receipt chain" needs no accounts, no
billing, and no new serving code, and it exercises everything the API
depends on.

### Phase 1 — a serving surface worth paying for

3. **Per-key authentication on `kev-serve`.** A bearer or NIP-42-style key
   checked before inference; anonymous calls get the shared lane's limits.
   Done when two keys on one door have independent rate budgets.
4. **The tenant quota ledger.** Per-key accounting of attempted and
   completed questions, worst-case reservation before the call and
   settlement after, one writer and a lock file — the discipline the result
   store and the locked ledger already keep. Unattempted work reports
   `unattempted`, never wrong. Done when a killed mid-request process leaves
   the ledger consistent.
5. **The gateway binary.** A thin front — key check, quota check, forward to
   a door by name, record the usage row — so tenants share one host and
   doors stay single-process. Done when a caller can hit one URL with two
   keys and reach two different doors.

### Phase 2 — the relay lane

6. **A decision job kind.** A NIP-CJ job carrying a `/v1/systemone` request,
   answered by a worker that fronts `kev-serve`. Done when a job submitted
   through the relay returns a typed answer with the answering door's
   artifact identity attached.
7. **The capability manifest.** A `kind:30180` manifest describing the
   decision API — lanes, doors, limits — so a host discovers it the way this
   repository's own `capabilities/` registry is read. Done when a fresh
   client finds the service from the relay alone.
8. **Receipts on the lane.** The job result carries the suite/question/gate
   digests the row would carry, so a relay-mediated answer is as checkable
   as a direct one. Done when a relay result and a direct result are
   indistinguishable in the store.

### Phase 3 — trained endpoints

9. **The per-tenant training pipeline.** Caller's labelled suite in,
   candidate adapter + pointer head out, artifact-locked and digest-published
   — retained tooling today, a service path here. Done when a caller's suite
   produces a candidate artifact whose identity `GET /v1/models` reports.
10. **The promotion gate.** `gym regress` of candidate against base on the
    caller's development partition, promotion only on a win beyond the floor.
    The locked partition is spent once, on the admitted artifact, through the
    ledger. Done when a losing candidate is refused with the record to show
    why.
11. **The tenant adapter registry.** A manifest per tenant binding key →
    artifact digest → door name, so "my endpoint" is a checkable identity
    rather than a URL. Done when swapping the artifact under a tenant's name
    changes the recorded identity and is refused by `gym regress`.

### Phase 4 — the public surface, last

12. **The status and benchmark page.** Rendered from a store snapshot the way
    `gym-snapshot` renders the Terminal Gym — honest numbers, digests,
    ceilings, refusal counts, and the suite each number came from. Last
    because a public claim should not exist before the machinery that keeps
    it honest does.
13. **Caller-facing docs, a CLI, and an agent skill.** An OpenAPI document
    for the two endpoints, a `classify`-shaped CLI over `crates/jev`, and a
    discoverable skill document. Last because it freezes the surface, and
    the surface should be the thing the earlier phases proved.

### What this ordering buys

Phases 0 and 1 produce revenue-shaped evidence — a caller paying for a
measured record, a key with a quota — before any multi-tenant machinery
exists. Phase 2 turns dedicated capacity into a protocol rather than a
process list. Phase 3 is where the moat is, and it cannot start before the
quota ledger and the promotion gate exist. Phase 4 is deliberately last: the
only thing it adds is publicity, and publicity is cheap when the record is
already true.

## What this document is

Measured: the latency figures, each from the named record. Exists: the
contract, both serving binaries, the adapter format, artifact identity, the
gym's suites, gates, digests, ledger, and store, and the relay's auth,
capability, and job lanes. Designed but unbuilt: the NIP-CJ decision lane,
tenant quotas, the per-tenant training pipeline, the gateway, and every
public surface. Nothing in this document asserts a product claim that is not
backed by a file or a record named beside it, and the issue list is written
to be filed rather than read as a plan already underway.
