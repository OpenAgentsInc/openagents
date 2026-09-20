# An open decision API

This document sketches a product this repository could host: an API where a
caller sends a state and a set of typed questions and gets back typed answers
with probabilities — a decision API, sold on what the contract already does.
It is a design, not an implementation. Every claim about what exists is
named beside the code or record that backs it; what does not exist is named
as missing.

Working name: the OpenAgents Decision API. "Classifier" undersells it —
classification is a `Choice`; the contract also prices confidence (`Noul`)
and ranks positions on a rubric (`Score`).

## Why the contract is the product

A hosted language model can classify anything. The reason to make a network
call to a decision model instead is the same reason `crates/coder` makes one:
reading the input is the expensive part, and the answer has to come back as a
number over a fixed option set, not prose to be re-parsed. State in, typed
judgments out, no text generated, no answer leaking into the next question.

The wire contract is `POST /v1/systemone`, and `crates/jev` is the client.
Everything below serves that contract or scores it; the API is one contract
with several doors behind it, which is what this repository already runs.

## The six properties, and what each maps to

### Private inference

The doors are already private in the strongest sense: `kev-serve` and
`lev-serve` run on the caller's own hardware, and the open weights pin their
content digests. For a hosted deployment the claim becomes narrower and more
honest: single-tenant processes, a stated retention policy, and a
`GET /v1/models` card that lets the caller verify which weights answered
(`docs/gym/model-identity.md`). "Your text never reaches a shared provider"
is free when the provider is the caller; when it is us, the honest version is
"your text reaches one process, its identity is published, and retention is
stated."

### Faster inference

No tokens are generated, so latency is a forward pass. Measured, not
asserted: hosted Jev answers in a p50 of ~100 ms over the public internet
([`../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md`](../gym/measurements/2026-09-20-hosted-jev-quiet-latency.md));
a local `kev` door on a quiet CPU host answers in a p50 of ~280 to 350 ms
([`../gym/measurements/2026-09-20-kev-quiet-latency.md`](../gym/measurements/2026-09-20-kev-quiet-latency.md)),
and the same measurement method gives Metal its own number
([`../gym/measurements/2026-09-20-kev-4b-metal-pass.md`](../gym/measurements/2026-09-20-kev-4b-metal-pass.md)).
`kev`'s packed prefill and block-causal question isolation exist to keep many
questions on one state cheap (`docs/kev/architecture.md`).

### Dedicated endpoints

A dedicated endpoint is a name bound to a published identity, not a routing
alias: the card's `artifact_identity` and `execution` fields say exactly what
is loaded, and a change produces a different recorded identity. In this
repository's idiom, dedicated capacity is a NIP-CJ job lane through the
relay — the pattern `coder-worker` already uses to answer job requests —
fronted by NIP-42 authentication so a caller's traffic lands on named
capacity rather than a shared pool. That lane is designed, not built; see
"what is missing" below.

### Trained endpoints

The trainable unit is small and already versioned: a LoRA adapter plus a
pointer head, the shape `docs/kev/README.md` describes. A trained endpoint
is an adapter fitted to one caller's labelled decisions, served as its own
door, with its artifact digest published the moment it exists — so the
caller can check it is being served what was trained, and the gym can score
it against the caller's own suite before it is admitted. The open
ingredients all exist: the adapter format, the artifact lock and digest
machinery (`scripts/fetch-kev-artifacts.py`, `docs/kev/artifacts.md`), and
the measurement plane that decides whether a candidate adapter beats the
control before it is promoted. The missing piece is the training pipeline
as a service — taking a caller's labelled suite to a candidate artifact —
which today is retained tooling, not a product path.

### A self-serve API

The relay stack is the self-serve shape already: NIP-42 authenticates a
caller by key, a `kind:30180` capability manifest in `capabilities/`
describes what is being offered, and a `kind:30182` program or a NIP-CJ job
request is the unit of work. No account, no call booking — publish a
manifest, hold a key, call the door. Quotas and paid tiers do not exist
today; what exists is authentication, discovery, and a job lane.

### Better, and measured the way it is read here

This is where an open version can be stronger than a leaderboard. Every
number a door earns here comes from `crates/gym`: a suite pinned by digest,
a question set pinned by digest, a gate pinned by digest, and a
receipt-chained result store that cannot be quietly rewritten. The
published claims carry their ceilings beside them — annotator agreement on
our own suites, published agreement or label basis on `external-v1` and
`external-jevbench-v1` — and the refusal policy is in the record, not in
the marketing. An accuracy is honest here because the suite is digested,
the locked partition is spent once through a ledger, and the comparison
refuses to run unless the instrument is held fixed.

## What the API actually is

Three layers, all of which exist as crates or protocols:

1. **Doors.** `kev-serve` for self-hosted and dedicated capacity, `lev-serve`
   for on-device, hosted Jev where a caller wants the strongest door. One
   contract, one client, doors picked by name.
2. **The lane.** HTTP for direct calls today; the relay for authenticated,
   discoverable, dedicated capacity — NIP-42 keys, NIP-CAP manifests,
   NIP-CJ jobs — the same shape `coder` and `coder-worker` already use.
3. **The record.** `gym eval` against a caller's suite, a public suite, or
   both; rows in a receipt-chained store; verdicts under digested gates.
   A caller's claim about their endpoint is checkable because the row
   carries the digests of everything that produced it.

## What is missing, honestly

- **Billing and quotas.** Nothing meters calls or prices them. The honest
  properties are already in the idiom — a budget ledger, worst-case
  reservation before spend — but none of it is written.
- **Multi-tenancy.** Doors are single-process; a dedicated endpoint is a
  process per caller, which is honest but does not scale. The NIP-CJ lane
  is the designed answer and is unbuilt for this purpose.
- **The training path.** Per-tenant adapter training is retained tooling,
  not a service. Taking a caller's suite to a verified candidate artifact
  is the work item this product most depends on.
- **A public surface.** No hosted endpoint, no key issuance, no status
  page. The repository proves the measurement and serving halves; the
  front half is the part that does not exist.

## What to build first, and what to measure

The cheapest thing with the highest information value is the measurement
half as a service before the serving half: take a caller's labelled suite,
build it the way `build_external_v1.py` and `build_external_jevbench_v1.py`
build ours, run it against the doors this repository already serves, and
hand back a digest-pinned record. That is a product — "we measured your
labels against these doors and here is the receipt chain" — that costs no
new infrastructure and exercises every piece the API would depend on.

The serving half follows the relay pattern `coder-worker` already
implements, and the trained-endpoint half waits on the per-tenant training
path. A hosted door we run ourselves is a different question from the
self-hosted one and carries its own retention and capacity claims.

## What this document is

Measured: the latency figures, each from the named record. Exists: the
contract, both serving binaries, the adapter format, artifact identity, the
gym's suites, gates, digests, and store, and the relay's auth, capability,
and job lanes. Designed but unbuilt: the NIP-CJ decision lane, tenant
quotas, the per-tenant training pipeline, and any public endpoint. This
document asserts no product claim that is not backed by a file or a record
named beside it.
