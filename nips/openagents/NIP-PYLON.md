# NIP-PYLON — Compute Pylons and Pools

`draft` `optional` — v1, 2026-10-07. **Partly implemented** for free work:
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
  the payment hash and preimage.
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
| [X402](NIP-X402.md) | Upfront Lightning payment for one operation. A receipt records the payment hash it settled with. |
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

Every body is JCS JSON with `v`, `requires` (the empty list in v1), and
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
- `model` is the served identity, the model name and, when the server
  names one, its artifact digest: `clef-flash@sha256:<64 hex>`;
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
    ["t", "oa:pylon-receipt:v1"],
    ["x", "<sha256 of content>"]
  ],
  "content": "{\"v\":\"openagents.pylon-receipt.v1\",\"requires\":[],\"buyer\":\"<buyer key>\",\"provider\":\"<pylon key>\",\"pylon\":\"studio-mac\",\"lane\":\"cj-execution\",\"capability\":\"<CAP DefinitionRef>\",\"request\":\"<64-hex event ID>\",\"request_digest\":\"<sha256 of request plaintext>\",\"result_digest\":\"<sha256 of result plaintext>\",\"started_at\":1791400100,\"finished_at\":1791400118,\"units\":{\"kind\":\"tokens\",\"count\":812},\"outcome\":\"accepted\",\"payment\":{\"profile\":\"lightning-bolt11\",\"network\":\"regtest\",\"amount_msat\":2000,\"payment_hash\":\"<64-hex>\",\"preimage\":\"<64-hex>\"}}"
}
```

| Field | Meaning |
| --- | --- |
| `v` | `openagents.pylon-receipt.v1`. |
| `buyer` | The signer. |
| `provider`, `pylon` | The pylon that did the work; equal to the `p` tag and the `a` tag's address. |
| `lane` | The lane the job used, from the beacon's list. |
| `capability` | The exact CAP DefinitionRef the job ran. |
| `request` | The 64-hex ID of the job request event. |
| `request_digest`, `result_digest` | SHA-256 of the request and result plaintext; `result_digest` is null when no result arrived. Never the content itself. |
| `started_at`, `finished_at` | The buyer's clock; `finished_at` is not earlier than `started_at`. |
| `units` | `{kind, count}`. `kind` is `tokens`, `seconds`, or `jobs`; `count` is a whole number. |
| `outcome` | `accepted`, `rejected` (a result arrived and the buyer refused it), `failed` (the pylon reported an error), or `timeout`. |
| `payment` | Null for free work. Otherwise `{profile, network, amount_msat, payment_hash, preimage}`. `profile` is `lightning-bolt11` or `x402-exact`. `network` is `bitcoin`, `testnet`, `signet`, or `regtest`. `preimage` is 64-hex and its SHA-256 equals `payment_hash`. |

A buyer publishes the preimage only after the payment settled. Publishing
it reveals nothing the payee did not already know, and lets any reader
check that an invoice with that hash was paid. It does not prove who paid
whom; [Abuse](#abuse) covers what that leaves open.

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
    ["t", "oa:pylon-pool:v1"],
    ["x", "<sha256 of content>"],
    ["expiration", "1791400500"]
  ],
  "content": "{\"v\":\"openagents.pylon-pool.v1\",\"requires\":[],\"aggregator\":\"<aggregator key>\",\"pool\":\"everglade\",\"policy\":\"<sha256 of the pool policy document>\",\"window\":{\"from\":1791396600,\"to\":1791400200},\"inputs\":{\"beacons\":{\"count\":12,\"digest\":\"<64-hex>\"},\"receipts\":{\"count\":340,\"digest\":\"<64-hex>\"},\"checks\":{\"count\":9,\"digest\":\"<64-hex>\"}},\"totals\":{\"pylons_online\":11,\"slots_total\":19,\"slots_free\":7,\"by_family\":{\"unified-memory\":8,\"gpu\":3,\"cpu\":0},\"jobs\":{\"accepted\":322,\"rejected\":6,\"failed\":8,\"timeout\":4},\"units\":{\"tokens\":281000,\"seconds\":0,\"jobs\":0},\"paid_msat\":{\"bitcoin\":0,\"testnet\":0,\"signet\":0,\"regtest\":644000},\"checks\":{\"pass\":8,\"fail\":1,\"inconclusive\":0}},\"rate\":[3,5,6,4,7,9,8,6,5,6,7,8],\"generated_at\":1791400200,\"valid_until\":1791400500}"
}
```

| Field | Meaning |
| --- | --- |
| `v` | `openagents.pylon-pool.v1`. |
| `aggregator`, `pool` | The signer, and the slug equal to `d`. |
| `policy` | SHA-256 of the pool's policy document: which pylons it admits, which buyers' receipts and which checkers' labels it counts, and its exclusions (such as dropping pylons a counted `check-fail` names). The aggregator serves the document beside the pool. |
| `window` | `{from, to}`, whole minutes, `0 < to − from ≤ 3600`. |
| `inputs` | For beacons, receipts, and check labels: `{count, digest}`. The digest is the SHA-256 of the sorted, newline-joined lowercase event IDs the aggregator counted, with no trailing newline. |
| `totals.pylons_online`, `slots_total`, `slots_free`, `by_family` | From the newest fresh `online` or `draining` beacon of each counted pylon at `window.to`. |
| `totals.jobs`, `units` | From counted receipts whose `finished_at` falls in the window. |
| `totals.paid_msat` | Sum of `amount_msat` per network over counted receipts with a valid preimage. Test networks are never added to `bitcoin`. |
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
  with `bitcoin`.

## Validation

A reader refuses:

- A body whose `v`, marker, kind, or `d` disagree, or that has unknown
  fields or values.
- A beacon whose `provider` is not the signer, whose `valid_until` exceeds
  `observed_at + 300`, whose `expiration` tag differs from `valid_until`, or
  whose `slots.free` exceeds `slots.total`.
- A beacon with an `auth` tag that does not verify under NIP-OA. A beacon
  without one is valid and shows no owner.
- A receipt whose `buyer` is not the signer, whose `buyer` equals its
  `provider` or the beacon's NIP-OA owner, whose `p` and `a` tags disagree
  with the body, or whose preimage does not hash to its payment hash.
- An aggregate whose recomputed digests or totals differ, or whose window
  breaks the bounds.
- Any event whose `x` tag does not match its content.

## Abuse

| Attack | What limits it |
| --- | --- |
| **Fake capacity**: a beacon claims a class or slots it lacks. | Beacons carry no earnings and grant nothing. Buyers route on checks and receipts, and a pylon that times out or fails checks loses future work. A pool policy may admit only pylons with passing checks. |
| **Sybil pylons**: one machine publishes many beacons. | Keys can't be tied to machines. A pool policy can require a NIP-OA owner and cap pylons per owner; checks reveal pylons that share one machine's latency and answers. Uptime alone never earns sats. |
| **Wash trading**: a provider's second key buys from its own pylon to inflate receipts. | A receipt from the provider or its NIP-OA owner never counts. Beyond that, keys can't be linked, so a pool counts receipts only from buyers on its policy's trust list, and readers can apply their own list. Real Lightning fees make large wash volume cost something; test sats make it free, which is why test-network totals are shown apart. |
| **Fabricated receipts**: a buyer claims jobs that never happened. | A fabricated receipt only flatters the pylon it names, so it reduces to wash trading. A false `failed` receipt from an untrusted buyer counts nowhere. |
| **Replay**: republishing old receipts or beacons. | One receipt per `(buyer, request)`; beacons obey generation and freshness rules. |
| **Aggregate inflation**: an aggregator overstates totals. | Readers recompute from the listed inputs. An aggregate that doesn't recompute is refused, and the reader can drop that aggregator. |
| **Griefing the world**: flooding beacons to clutter a shared scene. | The 256-per-pool draw bound, the per-address relay rate limit, and pool admission. |
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
buyer's receipt then carries the payment with its preimage. Brokered sales
carry the customer's `x402-exact` payment, settled through the broker's
x402 facilitator, in the broker's receipt. Readers
sum paid amounts per network and light a pylon's coin only for receipts
that verify.

Decisions (2026-10-10, #11225): `nostr::pylon::Lane::CjDecision`;
`crates/pylon` answers NIP-DEC jobs with a local System One server
(`openagents pylon serve --decide URL`, Psionic's Clef lane on
`psionic-openai-server`; `--decisions-only` serves no text jobs) and
advertises `pylon/decision` with its served identity; the OpenAgents
gateway's `POST /v1/systemone` reads these beacons and sends each decision
to a pylon with a free slot (NIP-DEC, "The OpenAgents decision API").

Not implemented: mainnet paid receipts, a `wellspring` projection from an
aggregate, and the `nip-pylon-v1` relay extension. A service's `capability` is a qualified ID
(`<pylon key>:pylon/text-generation`) rather than a full DefinitionRef.

## Conformance

Fixtures must cover a valid beacon, receipt, label, and aggregate; each
refusal under [Validation](#validation); beacons that are stale, from the
future, or rolled back a generation; a receipt from the provider's own key
and from its NIP-OA owner; two receipts for one request; a preimage that
does not hash to its payment hash; an aggregate whose digest or totals do
not recompute; test-network amounts kept apart from `bitcoin`; and the
world projection of each fixture to its `pylon` or `wellspring` state.

Advertise `nip-pylon-v1` in NIP-11 `supported_extensions` only for a relay
that stores `30200`, `30201`, and `3201` and applies the beacon rate limit.
Keep this draft name out of numeric `supported_nips`.
