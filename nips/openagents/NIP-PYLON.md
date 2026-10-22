# NIP-PYLON — Compute Pylons and Pools

`draft` `optional` — receipts, pool aggregates, and pool policies v2,
2026-10-15. Beacons are v1; a model group's beacons are v2, 2026-10-20. Declared
deadlines (`pylon.deadline.v1`), 2026-10-21. **Partly implemented**:
see [Implementation status](#implementation-status). The [shared contracts](contracts.md) are
normative. The [Verse compute vision and spec](../../docs/compute/verse-compute.md)
is the design this profile serves.

This NIP lets a compute provider say in public that a machine is online and
what classes of work it serves, lets a buyer leave a signed receipt for work
that machine completed, and lets an aggregator publish a pool's totals in a
form any reader can recompute. A *pylon* is one provider machine that opted
in. A *pool* is a named set of pylons and the receipts for their work.

Three records carry it:

- A **beacon** (`30200`): the provider's opt-in, coarse, short-lived
  statement that a pylon is online, its hardware class, its services, and
  its free slots.
- A **receipt** (`3201`): a buyer's signed statement that a pylon completed
  one job, with digests of the request and result and, for paid work,
  a signed seller or broker acknowledgment with no payment proof.
- A **pool aggregate** (`30201`): an aggregator's totals over one time
  window, with the digests of the exact beacon and receipt sets it counted.

A world such as Verse draws pylons and pools only from these verified
records ([World projection](#world-projection)). Nothing in this profile
grants access, admits a job, reserves capacity, moves money, or proves that
a computation was correct. A beacon is a claim. A receipt is a buyer's
claim. An aggregate is arithmetic over claims, and readers choose whose
claims count.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [Shared contracts](contracts.md) | Encoding, refusal codes, and the private `3188` artifact envelope for anything a buyer and provider exchange privately. |
| [CAP](NIP-CAP.md) | A service in a beacon names an exact CAP DefinitionRef. CAP forbids local presence in public heads; a beacon is a provider's deliberate, coarse, public advertisement, never a host's fleet inventory or probe result. |
| [MKT](NIP-MKT.md) | Prices and terms. A beacon may point at a `30192` offering head (NIP-MKT); its own `price_hint_msat` is an advertisement, never accepted terms. |
| [CJ](NIP-CJ.md) | The job transport. Paid and private work travels as `25920` execution or `25900` conversation jobs (NIP-CJ), encrypted with NIP-44. |
| [NIP-90](../official/90.md) | A compatibility lane for public text jobs (`5050`). Its optional parameter encryption uses NIP-04, so private work uses CJ instead. |
| [NIP-89](../official/89.md) | A `31990` handler record may list a pylon's NIP-90 kinds for clients that discover through NIP-89. |
| [X402](NIP-X402.md) | Upfront Lightning payment for one operation. A public receipt carries an acknowledgment; settlement evidence stays private. |
| [REACH](NIP-REACH.md) | Private host presence to the owner's own devices. A beacon is a separate, public, opt-in record; a host never copies REACH telemetry into it. |
| [XP](NIP-XP.md) | Reputation for verified work. XP never converts to sats, and nothing here pays XP. |
| [EVAL](NIP-EVAL.md) | Spot-check suites and Gym results that grade a pylon's answers. |
| [MV](NIP-MV.md) | A world authority projects beacons, receipts, and aggregates into `33301` object states (NIP-MV). |
| Block [NIP-OA](../block/NIP-OA.md) | An optional `auth` tag binds a pylon key to its owner's key without sharing the owner key. |
| [NIP-32](../official/32.md) | Check verdicts are labels in the `openagents.pylon` namespace. |
| [NIP-40](../official/40.md) | Every beacon and aggregate carries an `expiration` equal to its `valid_until`. |

## Roles

| Role | Key | Does |
| --- | --- | --- |
| **Provider** | The pylon key, one per machine | Publishes the beacon, serves jobs, issues invoices. |
| **Owner** | The person's key | Optionally authorizes the pylon key through a NIP-OA `auth` tag. Never signs beacons itself. |
| **Buyer** | Any key that sent a job, often an agent | Publishes a receipt after a job ends. |
| **Checker** | A key a reader trusts to re-run work | Publishes check verdicts as labels. |
| **Aggregator** | One key per pool | Publishes the pool aggregate. |
| **World authority** | The key a world trusts for object state | Projects the records into a world. |
| **Reader** | Any client | Verifies every record and applies its own trust list. |

One key may hold several roles, except where [Validation](#validation)
forbids it: a provider's receipt for its own pylon never counts.

## Kinds

These are OpenAgents draft assignments, not upstream registrations. The
[kind registry](README.md#kind-registry) lists every OpenAgents kind once.

| Kind | Class | Record |
| --- | --- | --- |
| `30200` | Addressable | Pylon beacon, one per pylon. |
| `30201` | Addressable | Pool aggregate, one per pool. |
| `3201` | Regular | Service receipt, one per completed job. |
| `1985` | Regular (NIP-32) | Optional check verdict that points at a receipt. |

Every body is JCS JSON with `v`, `requires` (the empty list in these versions), and
optional inert `meta`, plus exactly the fields listed. Unknown versions,
required features, fields, or enumeration values refuse. Keys are lowercase
64-hex x-only public keys. Times are Unix seconds. Every event carries
exactly one `t` marker and one `x` tag equal to the lowercase SHA-256 of
its exact content bytes.

## Pylon beacon — kind `30200`

A provider publishes a beacon while its pylon is willing to take work, and
again with `status: "offline"` when it stops.

```json
{
  "kind": 30200,
  "pubkey": "<pylon key>",
  "created_at": 1791400000,
  "tags": [
    ["d", "studio-mac"],
    ["t", "oa:pylon-beacon:v1"],
    ["x", "<sha256 of content>"],
    ["expiration", "1791400300"],
    ["auth", "<owner key>", "kind=30200", "<owner signature>"]
  ],
  "content": "{\"v\":\"openagents.pylon-beacon.v1\",\"requires\":[],\"provider\":\"<pylon key>\",\"pylon\":\"studio-mac\",\"label\":\"Studio Mac\",\"status\":\"online\",\"generation\":7,\"since\":1791380000,\"observed_at\":1791400000,\"valid_until\":1791400300,\"class\":{\"family\":\"unified-memory\",\"tier\":\"large\",\"memory_gb\":64},\"slots\":{\"total\":2,\"free\":1},\"services\":[{\"capability\":\"<CAP DefinitionRef>\",\"model\":\"open-weights-8b-q4\",\"lanes\":[\"cj-execution\",\"nip90-5050\"],\"offering\":\"30192:<pylon key>:text-small\",\"price_hint_msat\":2000}],\"settlement\":[\"free-v1\"],\"pools\":[\"everglade\"]}"
}
```

| Field | Meaning |
| --- | --- |
| `v` | `openagents.pylon-beacon.v1`. |
| `provider` | The signer. |
| `pylon` | Slug, 1 to 64 bytes of `[a-z0-9_-]`, equal to `d`. |
| `label` | Inert display text, at most 64 UTF-8 bytes. |
| `status` | `online`, `draining` (finishing admitted jobs, taking no new ones), or `offline`. |
| `generation` | Unsigned counter that rises when the pylon restarts, updates, or restores state. |
| `since` | When the current online period began. Uptime is `observed_at − since`. |
| `observed_at` | The pylon's clock when it took the sample. |
| `valid_until` | At most 300 seconds after `observed_at`; equal to the `expiration` tag. |
| `class` | `{family, tier, memory_gb}`. `family` is `unified-memory`, `gpu`, or `cpu`. `tier` is `small`, `medium`, `large`, or `xl` under the bands in [Hardware classes](#hardware-classes). `memory_gb` is one of 8, 16, 32, 64, 128, 256, or 512, rounded down. |
| `slots` | `{total, free}`, whole numbers, `0 ≤ free ≤ total ≤ 64`. A slot is one concurrent job the pylon admits. |
| `services` | 1 to 16 entries, each `{capability, model, lanes, offering, price_hint_msat}`. `capability` is an exact CAP DefinitionRef. `model` is an inert identifier of at most 128 bytes. `lanes` is a nonempty distinct subset of `cj-execution`, `cj-conversation`, `cj-decision`, and `nip90-5050`. `offering` is a `30192` address or null. `price_hint_msat` is a nonnegative integer or null. |
| `settlement` | Nonempty distinct list of the NIP-MKT payment profiles and X402 schemes the pylon accepts, such as `free-v1`. |
| `pools` | Up to 8 pool slugs the provider asks to join. Joining is the aggregator's decision. |

The beacon carries no telemetry beyond `class` and `slots`: no CPU or GPU
utilization, temperature, process names, users, paths, addresses, or
earnings. Earnings come only from receipts, which the provider does not
sign. Reachability is not a beacon field; a buyer reaches a pylon through
the relays in the provider's NIP-65 list and the CJ transport.

A complete pylon states in its inert `meta` as `max_input_bytes` the most
conversation text, in bytes, that one request may carry (98304 since
2026-10-21). A buyer sizes each request to it, keeping the newest earlier
turns that fit, and never cuts the latest message; a message that doesn't
fit goes to another pylon. A beacon without it is read as 16384 bytes, the
limit of pylons released before it, which also take only standard NIP-44
bodies of at most 65535 bytes.

### Model groups

A model too large for one machine runs as a *group*: one *stage* Pylon per
GPU, each holding a contiguous layer range, behind one *head*. A group's
beacons are version `openagents.pylon-beacon.v2` under the `t` marker
`oa:pylon-beacon:v2`. A v2 body is a v1 body plus one field, `group`. A
complete pylon keeps publishing v1, which never carries `group`.

```json
"group": {"id": "v41-a", "role": "head", "generation": 3,
          "artifact": "sha256:<64 hex>", "readiness": "ready",
          "deadline_secs": 600, "max_output_tokens": 4096}
```

| Field | Meaning |
| --- | --- |
| `id` | The group's slug, shared by the head and every stage. |
| `role` | `head` or `stage`. |
| `generation` | Rises whenever a stage is replaced. A stage on another generation than the head is not serving. |
| `artifact` | The exact model artifact, `sha256:<64 hex>`. |
| `readiness` | Head only: `starting`, `ready`, or `recovering`. A group is `ready` only when every stage is loaded, checked, and on this generation. |
| `deadline_secs` | Head only: the most seconds one job may take, 1 to 3,600. |
| `max_output_tokens` | Head only: the most tokens one answer carries, 1 to 1,048,576. |
| `layers` | Stage only: `{from, to}`, the half-open layer range the stage holds, `from < to ≤ 4096`. |

Rules:

- The head is the group's one routable beacon. Its `slots.total` is 1: the
  group runs one job at a time, and a reader counts one slot for the group,
  never the sum of its stages. Every service's `model` is
  `<name>@<artifact>`.
- A stage has `slots` `{0, 0}` and no services. A reader never sends it a
  job and never treats it as a complete model.
- A buyer sends a group a job only when the request asks for that model,
  by the exact artifact or by its name, and only while the head is fresh,
  online, `ready`, and has its slot free. A buyer never answers a request
  for one model with another; when no pylon or group serves the model, the
  request fails.
- A reader shows a stage with the head of the same `id` and `artifact`
  whose NIP-OA owner matches, or, without owners, whose key matches.
- A head may describe its recent answers in its inert `meta` as
  `answer_profile`: `answers` (how many, the most recent up to 32),
  `first_token_ms` (median time from admission to the first token),
  `tokens_per_s` (median decode rate after it), `median_tokens`,
  `max_tokens`, and `max_answer_ms`. It helps a buyer choose a budget
  against the declared deadline. Nothing routes on it, and a stage beacon
  carries none.
- A head may also state its group's checked request sizes in its inert
  `meta` as `request_limits`: `max_context_tokens` (prompt and answer
  together), `max_input_tokens` (the longest prompt it takes),
  `max_prefill_tokens` (the longest prompt its stages' wire carries), and
  `modes` (`thinking`, `chat`). A larger request is refused with
  `limit_exceeded`. Nothing routes on it, and a stage beacon carries none.
- A v1 reader fetches beacons by the v1 marker, so it never sees a group
  beacon; one that arrives anyway is refused for its marker, version, and
  field. Group beacons need no change to v1 beacons.

### Reasoning in results

Added 2026-10-21. A pylon whose model thinks before it answers MAY put that
thinking in the conversation result's optional `reasoning` field (NIP-CJ,
"Conversation jobs"): a complete model's reasoning channel, or a group
head's text before the model's `</think>`. It is at most 32 KiB; a pylon
trims longer reasoning from the start. A buyer that doesn't read it is
unaffected, and the receipt's `result_digest` covers it like the rest of
the result.

### Declared deadlines

An ordinary CJ conversation job to a pylon runs for at most 90 seconds. A
model that needs longer, such as a group whose head declares
`deadline_secs` over 90, takes jobs under the `pylon.deadline.v1` request
feature. A buyer opts in; nothing changes for a request that doesn't.

A request names the feature in `requires` and carries `budget_secs`, the
buyer's budget, 1 to 3,600 whole seconds:

```json
{"v": 1, "requires": ["pylon.deadline.v1"], "budget_secs": 300,
 "task": "...", "transcript": [...]}
```

Rules:

- The job's deadline is the smaller of the buyer's budget and the
  provider's ceiling: the head's `deadline_secs`, or 90 seconds for a
  complete pylon. The buyer computes it from the beacon and sends it as
  `budget_secs`; the provider applies the smaller again.
- The provider measures the deadline from admission and enforces it inside
  execution. When it passes, the provider stops the work, refuses the job
  `limit_exceeded`, and frees its slot only once the work has stopped; a
  group holds its one slot until every stage has.
- Admission feedback is `status: processing` with `requires:
  ["pylon.deadline.v1"]` and `deadline_secs`, the deadline it enforces. The
  provider repeats it about every 30 seconds as a heartbeat. A heartbeat
  never extends the deadline, on either side.
- The result names the feature in `requires`, repeats `deadline_secs`, and
  carries `finish: "stop"`. The provider sends only a complete answer: one
  that its output limit cut off is refused `limit_exceeded`.
- The buyer waits for the deadline plus a short allowance for the relay,
  then stops. Responses are ephemeral, and a relay connection lives at most
  120 seconds, so a buyer that waits longer subscribes on a fresh
  connection before the old one ends.
- A provider whose ceiling is over 90 seconds refuses an ordinary request
  at once, `unsupported_feature`, rather than running it past an old
  buyer's wait.
- A provider without the feature refuses the request at once,
  `unsupported_feature`, as it refuses any feature. A buyer that doesn't
  know the feature refuses a response that names it.

### Relay list

A provider publishes a NIP-65 kind `10002` list from the pylon key when it
starts serving and whenever the relays it reads change. Each relay it reads
requests from and publishes results to is one `r` tag with no marker (read
and write); a relay it only reads or only writes takes `read` or `write`.
The content is empty. A buyer sends jobs to the provider's read relays; a
reader that has no list falls back to the relay the beacon arrived on.

```json
{
  "kind": 10002,
  "pubkey": "<pylon key>",
  "tags": [["r", "wss://relay.openagents.com"]],
  "content": ""
}
```

### Hardware classes

| `tier` | `unified-memory` | `gpu` | `cpu` |
| --- | --- | --- | --- |
| `small` | Under 16 GB unified memory | Under 12 GB of video memory | Under 8 cores |
| `medium` | 16 to 31 GB | 12 to 23 GB | 8 to 15 cores |
| `large` | 32 to 95 GB | 24 to 47 GB | 16 to 63 cores |
| `xl` | 96 GB or more | 48 GB or more | 64 cores or more |

A class is the provider's statement. A reader that needs proof uses checks,
not the class.

### Freshness

A reader judges a beacon by its own receipt time, as
[REACH freshness](NIP-REACH.md#freshness) does:

1. Refuse a beacon whose `observed_at` is more than 30 seconds later than
   the reader's receipt time.
2. Treat a beacon as stale after `valid_until`, or when the reader received
   it more than 300 seconds after `observed_at`.
3. Keep the newest beacon per pylon. Refuse a lower `generation` than one
   held, and a beacon that is not newer within one generation.
4. Show a stale pylon as **unknown**, never as online.

A provider republishes when `status`, `generation`, `services`, or
`slots.free` changes, and otherwise at most once every 60 seconds and at
least once every 240 seconds while online. A relay MAY refuse a beacon that
arrives sooner than 10 seconds after the previous one for the same address,
with `rate-limited:`.

### Decision services

A pylon that answers [NIP-DEC](NIP-DEC.md) decision jobs (`25910` in,
`27010` and `26910` out) advertises a service on the `cj-decision` lane:

- `capability` is `<pylon key>:pylon/decision`;
- `model` is the served identity: the model name and, when known, its model
  id, a digest of its tensors (`clef-flash@gguf-tensors:sha256:<64 hex>`), so
  two files that differ only in header metadata are the same model; else the
  loaded file's digest (`clef-flash@sha256:<64 hex>`). Readers treat both
  forms of one model as the same identity;
- `slots` counts decisions and text jobs together.

Every answer it returns names the same identity: `response.model` is the
name before `@`, and `response.service` is `{door: "pylon:<slug>",
version, provider, identity}`. A buyer checks both against the beacon and
treats a mismatch as no answer. The result's receipt names the served model
and its artifact digest (`served.artifact_signature`). Decision work is
free (`free-v1`) in this version.

## Service receipt — kind `3201`

A buyer publishes one receipt when a job ends, whatever the outcome. A
receipt is optional: a buyer who wants no public trace publishes nothing,
and the pylon gets no public credit for that job.

```json
{
  "kind": 3201,
  "pubkey": "<buyer key>",
  "created_at": 1791400120,
  "tags": [
    ["p", "<pylon key>"],
    ["a", "30200:<pylon key>:studio-mac"],
    ["t", "oa:pylon-receipt:v2"],
    ["x", "<sha256 of content>"]
  ],
  "content": "{\"v\":\"openagents.pylon-receipt.v2\",\"requires\":[],\"buyer\":\"<buyer key>\",\"provider\":\"<pylon key>\",\"pylon\":\"studio-mac\",\"lane\":\"cj-execution\",\"capability\":\"<CAP DefinitionRef>\",\"request\":\"<64-hex event ID>\",\"request_digest\":\"<sha256 of request plaintext>\",\"result_digest\":\"<sha256 of result plaintext>\",\"started_at\":1791400100,\"finished_at\":1791400118,\"units\":{\"kind\":\"tokens\",\"count\":812},\"outcome\":\"accepted\",\"payment\":null}"
}
```

| Field | Meaning |
| --- | --- |
| `v` | `openagents.pylon-receipt.v2`. |
| `buyer` | The signer. |
| `provider`, `pylon` | The pylon that did the work; equal to the `p` tag and the `a` tag's address. |
| `lane` | The lane the job used, from the beacon's list. |
| `capability` | The exact CAP DefinitionRef the job ran. |
| `request` | The 64-hex ID of the job request event. |
| `request_digest`, `result_digest` | SHA-256 of the request and result plaintext; `result_digest` is null when no result arrived. Never the content itself. |
| `started_at`, `finished_at` | The buyer's clock; `finished_at` is not earlier than `started_at`. |
| `units` | `{kind, count}`. `kind` is `tokens`, `seconds`, or `jobs`; `count` is a whole number. |
| `outcome` | `accepted`, `rejected` (a result arrived and the buyer refused it), `failed` (the pylon reported an error), or `timeout`. |
| `payment` | Null when no separate Lightning payment funded this job. Otherwise `{profile, network, amount_msat, settlement, acknowledgment}`. Profile is `lightning-bolt11` or `x402-exact`; amount is positive integer msat. Network is `bitcoin`, `testnet`, `signet`, or `regtest`, with X402 restricted to `bitcoin` or `testnet`. Settlement is an opaque 64-hex reference, never an invoice, payment hash, preimage, or reusable proof. Acknowledgment is the nested signed event below. |

A paid receipt requires a seller or broker acknowledgment. It is a nested
NIP-01 event, not separately published: kind `3201`, exactly the tags
`["t", "oa:pylon-payment-ack:v1"]` and `["x", "<content digest>"]`, and
`created_at` equal to the receipt's `finished_at`. Its canonical content is the complete receipt
with only `payment.acknowledgment` set to null. Its signer must be the provider
(seller) or buyer (broker). Verify the signature, exact content, kind, tags,
time, network, amount, and settlement reference before accepting the receipt.
Nonempty receipt `meta` is unsupported in this version, with one
exception: a [NIP-ATT](NIP-ATT.md#receipts) sealed-job receipt has
`requires` exactly `["openagents.attested.v1"]` and `meta` exactly
`{attested: {endpoint, release, level, measurement}}`, with `level` `tee`
or `tee-cloud`, and its digests are over ciphertext. Readers that do not
know the feature refuse such receipts. Older receipt and
aggregate formats are refused; no compatibility producer is required.

The direct seller issues this acknowledgment only after its private X402
purchase store binds the paid purchase to this exact buyer, CJ request and
input, and price. The buyer requests it after constructing the receipt, through
an authenticated encrypted `3188` artifact with schema
`openagents.pylon-payment-request.v1`: exactly `{v, requires: [], purchase,
receipt}`, where `v` equals that schema and `purchase` is the private
64-hex purchase nonce. The reply is a private artifact with
schema `openagents.pylon-payment-ack.v1` whose inline value is the nested event.
The purchase nonce stays private. This is a Pylon-specific application exchange,
not another X402 record type or an extension hidden in CJ metadata.

The settlement reference is a domain-separated SHA-256 commitment to the
private network/payment identity. It is stable across acknowledgment retries
and reveals no bearer proof. In this implementation it is SHA-256 of UTF-8
`openagents.pylon-settlement.v1:NETWORK:PAYMENT_HASH`. The broker retains the
private payment hash separately for facilitator lookup and financial settlement;
it cannot recover or infer a payment from this public commitment.

Readers count paid amounts only from acknowledgment issuers in their explicit
`payment_issuers` trust set, empty by default. For each issuer/network/settlement
reference, count at most one amount. A trusted issuer can still lie: these are
reported payments, not independent proof of a transfer or correct work. Public
records never create income, account credit, a payable share, or spending
allowance. Those require authenticated private payment evidence. A paid job
without an acknowledgment publishes no public receipt; it must not be relabeled
free. Record the missing acknowledgment and payment outcome privately.

Invoices, preimages, wallet lookups, and payment proofs remain in protected
payment records and encrypted purchase messages. They must not appear in public
receipt bodies, nested events, tags, aggregate inputs, logs, or exports. A
hash/preimage pair alone would not establish who paid or actual collection.

A receipt names a request ID, so a reader can tell that two receipts claim
one job. The first receipt per `(buyer, request)` counts; later ones are
ignored.

### Check verdicts

A checker that re-runs a job, or runs a known-answer probe through the
normal job path, publishes a NIP-32 label:

| Tag | Value |
| --- | --- |
| `L` | `openagents.pylon` |
| `l` | `check-pass`, `check-fail`, or `check-inconclusive`, with mark `openagents.pylon`. |
| `e` | Exactly one: the `3201` receipt checked. |
| `p` | The pylon key. |
| `x` | SHA-256 of the checker's own result plaintext. |

The label's content is inert text of at most 512 bytes naming the method,
such as `exact-match`, `redundant-3`, or a Gym suite digest. A reader counts
a verdict only from a checker on its trust list, and never from the buyer
or provider of the receipt it names.

## Pool aggregate — kind `30201`

An aggregator publishes one aggregate per pool per window. Windows are
whole minutes, at most 60 minutes long. Readers recompute each total from
the listed inputs before drawing it.

```json
{
  "kind": 30201,
  "pubkey": "<aggregator key>",
  "created_at": 1791400200,
  "tags": [
    ["d", "everglade"],
    ["t", "oa:pylon-pool:v2"],
    ["x", "<sha256 of content>"],
    ["expiration", "1791400500"]
  ],
  "content": "{\"v\":\"openagents.pylon-pool.v2\",\"requires\":[],\"aggregator\":\"<aggregator key>\",\"pool\":\"everglade\",\"policy\":\"<sha256 of the pool policy document>\",\"window\":{\"from\":1791396600,\"to\":1791400200},\"inputs\":{\"beacons\":{\"count\":12,\"digest\":\"<64-hex>\"},\"receipts\":{\"count\":340,\"digest\":\"<64-hex>\"},\"checks\":{\"count\":9,\"digest\":\"<64-hex>\"}},\"totals\":{\"pylons_online\":11,\"slots_total\":19,\"slots_free\":7,\"by_family\":{\"unified-memory\":8,\"gpu\":3,\"cpu\":0},\"jobs\":{\"accepted\":322,\"rejected\":6,\"failed\":8,\"timeout\":4},\"units\":{\"tokens\":281000,\"seconds\":0,\"jobs\":0},\"paid_msat\":{\"bitcoin\":0,\"testnet\":0,\"signet\":0,\"regtest\":644000},\"checks\":{\"pass\":8,\"fail\":1,\"inconclusive\":0}},\"rate\":[3,5,6,4,7,9,8,6,5,6,7,8],\"generated_at\":1791400200,\"valid_until\":1791400500}"
}
```

| Field | Meaning |
| --- | --- |
| `v` | `openagents.pylon-pool.v2`. |
| `aggregator`, `pool` | The signer, and the slug equal to `d`. |
| `policy` | SHA-256 of the pool's policy document: which pylons it admits, which buyers' receipts, payment acknowledgment issuers, and checkers' labels it counts, and its exclusions (such as dropping pylons a counted `check-fail` names). The aggregator serves the document beside the pool. |
| `window` | `{from, to}`, whole minutes, `0 < to − from ≤ 3600`. |
| `inputs` | For beacons, receipts, and check labels: `{count, digest}`. The digest is the SHA-256 of the sorted, newline-joined lowercase event IDs the aggregator counted, with no trailing newline. |
| `totals.pylons_online`, `slots_total`, `slots_free`, `by_family` | From the newest fresh `online` or `draining` beacon of each counted pylon at `window.to`. |
| `totals.jobs`, `units` | From counted receipts whose `finished_at` falls in the window. |
| `totals.paid_msat` | Sum of reported `amount_msat` per network over counted receipts with a valid acknowledgment from a policy-trusted issuer, once per `(issuer, network, settlement)`. Test networks are never added to `bitcoin`. |
| `totals.checks` | Counted verdicts by result. |
| `rate` | Accepted jobs per equal slice of the window, oldest first, 1 to 60 entries. |
| `generated_at`, `valid_until` | `valid_until` is at most 300 seconds after `generated_at`. |

Bounds: at most 4,096 beacons, 65,536 receipts, and 65,536 labels per
window. A larger pool splits into several pools. A reader recomputes by
fetching the counted events by ID, checking each, recomputing the digests,
and recomputing every total; any difference refuses the aggregate. A reader
without the time or bandwidth to recompute shows the aggregate as
**unverified**.

## World projection

A world authority turns these records into world state with NIP-MV `33301`
object states (NIP-MV [object state](NIP-MV.md#object-state)), and every
client can repeat the derivation. Two object state kinds are added to the
`state` field:

| `state.kind` | Fields | Derived from |
| --- | --- | --- |
| `pylon` | `pylon` (the `30200` address), `status` (`online`, `draining`, `offline`, or `unknown`), `family`, `tier`, `busy` (`total − free`), `total`, `jobs` (accepted, receipt-backed, all time in this pool), `paid_msat` (per network), `uptime` (seconds) | The newest fresh beacon and the counted receipts. |
| `wellspring` | `pool` (the `30201` address), `online`, `busy`, `total`, `rate` (the aggregate's last entry), `verified` (`true` only when the client recomputed the aggregate) | The newest valid aggregate. |

Rules:

- A world MUST NOT draw a pylon, a glow, a beam, or a number from anything
  but a verified beacon, receipt, label, or aggregate. Demonstration data
  is labeled as such in the world.
- A pylon's object state changes at most once every 5 seconds; a
  wellspring's at most once every 10 seconds. Animation between states is
  the client's business.
- A world draws at most 256 pylons per pool. Beyond that it draws the
  newest 256 and shows the rest as a count.
- An agent's beam to a pool is drawn only while that agent's own entity or
  workstation state names a job request ID whose receipt or CJ progress the
  client can see, and for at most 120 seconds without a newer progress event.
- Test-network amounts are drawn with a visible test mark and never summed
  with `bitcoin`. Paid figures and coin lights represent reported payments
  from explicitly trusted acknowledgment issuers, not independent transfer proof.

## Validation

A reader refuses:

- A body whose `v`, marker, kind, or `d` disagree, or that has unknown
  fields or values.
- A beacon whose `provider` is not the signer, whose `valid_until` exceeds
  `observed_at + 300`, whose `expiration` tag differs from `valid_until`, or
  whose `slots.free` exceeds `slots.total`.
- A beacon with an `auth` tag that does not verify under NIP-OA. A beacon
  without one is valid and shows no owner.
- A beacon whose `t` marker and version disagree, a v1 beacon with `group`,
  a v2 beacon without it, a head without one slot, readiness, deadline, and
  output limit, or whose service models are not `<name>@<artifact>`, and a
  stage with slots, services, head fields, or an empty layer range.
- A receipt whose `buyer` is not the signer, whose `buyer` equals its
  `provider` or the beacon's NIP-OA owner, whose `p` and `a` tags disagree
  with the body, or whose payment acknowledgment is missing, forged, or bound to another receipt.
- An aggregate whose recomputed digests or totals differ, or whose window
  breaks the bounds.
- A conversation request with `budget_secs` but without
  `pylon.deadline.v1`, with the feature but without a whole-second budget
  from 1 to 3,600, or with another feature beside it; and a response
  naming the feature without `deadline_secs`, or a result naming it whose
  `finish` isn't `stop`.
- Any event whose `x` tag does not match its content.

## Abuse

| Attack | What limits it |
| --- | --- |
| **Fake capacity**: a beacon claims a class or slots it lacks. | Beacons carry no earnings and grant nothing. Buyers route on checks and receipts, and a pylon that times out or fails checks loses future work. A pool policy may admit only pylons with passing checks. |
| **Sybil pylons**: one machine publishes many beacons. | Keys can't be tied to machines. A pool policy can require a NIP-OA owner and cap pylons per owner; checks reveal pylons that share one machine's latency and answers. Uptime alone never earns sats. |
| **Wash trading**: a provider's second key buys from its own pylon to inflate receipts. | A receipt from the provider or its NIP-OA owner never counts. Beyond that, keys can't be linked, so a pool counts receipts only from buyers on its policy's trust list, and readers can apply their own list. Signed acknowledgments remain issuer claims and cannot establish independent demand; test sats make it free, which is why test-network totals are shown apart. |
| **Fabricated receipts**: a buyer claims jobs that never happened. | A fabricated receipt only flatters the pylon it names, so it reduces to wash trading. A false `failed` receipt from an untrusted buyer counts nowhere. |
| **Replay**: republishing old receipts or beacons. | One receipt per `(buyer, request)`; beacons obey generation and freshness rules. |
| **Aggregate inflation**: an aggregator overstates totals. | Readers recompute from the listed inputs. An aggregate that doesn't recompute is refused, and the reader can drop that aggregator. |
| **Griefing the world**: flooding beacons to clutter a shared scene. | The 256-per-pool draw bound, the per-address relay rate limit, and pool admission. |
| **Cheaper decisions**: a decision pylon answers with another model, a coarser quantisation, or made-up probabilities. | A checker recomputes fresh seeded items on the same pinned artifact and compares answers in centred-log space; the limits come from measured honest noise across backends. A miss is a `check-fail` on the round's receipt. Which items are recomputed is drawn after every answer is in, so a pylon can't do real work only on checked items, and an answer whose choice or score disagrees with its own probabilities fails before any recompute. |
| **Relaying decisions upstream**: a pylon forwards decision jobs to a hosted API instead of computing. | A full-precision upstream answers measurably differently from the pinned 4-bit artifact and fails the same recompute check; an upstream serving the same artifact is unprofitable while the provider's share stays below the cheapest public price for it. |
| **Decision identity**: answers that name another model than the beacon. | The answer's model and artifact must match the beacon's decision service; a mismatch is a `check-fail`. |
| **Many keys, one GPU**: one machine answers decisions under several keys. | Decision work is paid per verified decision, never per key or per beacon, so extra keys earn nothing extra. Token counts come from the gateway that sold the work, never from the pylon. |
| **Content disclosure**: a receipt leaks what a buyer asked. | Receipts carry only digests. Private work travels encrypted over CJ; the NIP-90 lane is for content the buyer accepts as public. |

## Privacy and disclosure

- Publishing a beacon reveals that a key runs a machine of a coarse class,
  when it is online, and which pools it asked to join. An owner who links a
  pylon through NIP-OA reveals that link.
- A receipt reveals that a buyer key used a pylon at a time, for a capability,
  and how much it paid. Agents that buy in public can use a key per agent.
- The provider sees the plaintext of every job it serves. Encryption
  protects a job from relays and onlookers, not from the pylon that runs it.
  A buyer sends a pylon only what it accepts that pylon reading.

## Implementation status

Phase P1 of the [Verse compute plan](../../docs/compute/verse-compute.md#phased-plan),
for free work (2026-10-07):

- `crates/nostr` (`nostr::pylon`) builds and verifies beacons, receipts,
  and aggregates; judges freshness and generation; recomputes aggregates
  under a policy document; and projects the `pylon` world state.
- `crates/pylon` publishes beacons, serves `cj-conversation` jobs, buys
  and publishes receipts, and publishes aggregates (`openagents pylon`).
- `crates/world-tree` holds the `pylon` and `wellspring` object states,
  which `crates/verse-net` carries in `33301` object states, and Verse's
  Everglade draws the Pylon Field and the Wellspring from a
  `ComputeSource` (`zones::everglade::compute`, P0, #10920): today this
  computer's lease table, with no events.

Phase P2 (2026-10-08): `nostr::pylon::check` builds and verifies check
labels, binds each to its receipt, and folds counted verdicts into a
pylon's standing. A pool policy may name `checkers` (whose labels the
aggregate counts in `inputs.checks` and `totals.checks`) and set
`exclude_failed`, which drops a pylon with a counted `check-fail` from
admission; both fields are omitted when empty, so an open policy's digest
is unchanged. `crates/pylon` runs the checker (`openagents pylon check`),
the per-class league, and `pylon-check` XP awards (NIP-XP).

Phase P3 (2026-10-08, test networks): a pylon that advertises
`x402-exact` sells each job under [X402](NIP-X402.md)'s native binding:
the buyer seals a `request` whose input is the digest of the CJ request
plaintext, pays the pylon's `challenge`, and sends the CJ request only
after the pylon's `admitted` status; the pylon runs a CJ request only when
its plaintext matches an admitted purchase's input, once per purchase. The
buyer's receipt carries the seller's signed acknowledgment; proofs stay private.
Brokered sales carry the broker's signed acknowledgment of the customer's
`x402-exact` payment. The broker's facilitator and private payment evidence
establish settlement. Readers
sum paid amounts per network and light a pylon's coin only for receipts
that verify and whose acknowledgment issuer the reader trusts.

Decisions (2026-10-10, #11225): `nostr::pylon::Lane::CjDecision`;
`crates/pylon` answers NIP-DEC jobs with a local System One server
(`openagents pylon serve --decide URL`, Psionic's Clef lane on
`psionic-openai-server`; `--decisions-only` serves no text jobs) and
advertises `pylon/decision` with its served identity; the OpenAgents
gateway's `POST /v1/systemone` reads these beacons and sends each decision
to a pylon with a free slot (NIP-DEC, "The OpenAgents decision API").

Revision #11721 (2026-10-15): public receipt, aggregate, and policy v2 replace
the proof-bearing format. `nostr::pylon` validates signed acknowledgments;
`crates/pylon` obtains direct seller acknowledgments over private artifacts,
signs broker acknowledgments, and keeps payment evidence private. Pool/field
paid totals use explicit issuer trust and settlement deduplication. This is
fixture-backed implementation, not qualification of a live payment.

Model groups (2026-10-20, #12400): `nostr::pylon` builds and verifies v2
head and stage beacons; an aggregate counts a head's one slot and none of
its stages'. `crates/pylon` builds a head's beacon from the group
lifecycle's readiness report (`pylon::group`), shows each group with its
stages (`RelayField::poll_groups`), and routes a request that names a model
only to a pylon or ready group serving it (`router::Settings::want`,
`client::choose_model`). Nothing publishes group beacons yet; the group
lifecycle (#12397) will.

Declared deadlines (2026-10-21, #12401): `nostr::pylon::deadline` reads
and builds `pylon.deadline.v1` requests and responses and computes the
effective deadline. `crates/pylon`'s provider enforces it from admission
with heartbeats that renew nothing, waits for a group's stages to stop
before freeing the slot, and refuses ordinary jobs to a model whose ceiling
passes 90 seconds; its buyer (`client::ask_within`, `ask_model_within`) and
router (`router::Settings::deadline`) send the extension and keep the
answer subscription across relay connections. Group mode accepts a
declared deadline up to 3,600 seconds.

Not implemented: mainnet paid receipts, a `wellspring` projection from an
aggregate, and the `nip-pylon-v2` relay extension. A service's `capability` is a qualified ID
(`<pylon key>:pylon/text-generation`) rather than a full DefinitionRef.

## Conformance

Fixtures must cover a valid beacon, receipt, label, and aggregate; each
refusal under [Validation](#validation); beacons that are stale, from the
future, or rolled back a generation; a receipt from the provider's own key
and from its NIP-OA owner; two receipts for one request; a missing, forged, wrong-issuer, or changed-receipt acknowledgment;
untrusted issuers and repeated settlement references excluded from paid totals;
no payment secret in public serialization; an aggregate whose digest or totals do
not recompute; test-network amounts kept apart from `bitcoin`; and the
world projection of each fixture to its `pylon` or `wellspring` state;
and a recorded v1 beacon that still verifies, recorded group head and stage
beacons that a v1 reader refuses, each group rule, and an aggregate that
counts one slot per group; and recorded `pylon.deadline.v1` requests and
responses, the refusal an old provider sends the extended form, the
refusal a long model sends an ordinary request, and each malformed body.

Advertise `nip-pylon-v2` in NIP-11 `supported_extensions` only for a relay
that stores `30200`, `30201`, and `3201` and applies the beacon rate limit.
Keep this draft name out of numeric `supported_nips`.
