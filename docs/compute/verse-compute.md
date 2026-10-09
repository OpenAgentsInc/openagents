# Compute in the Verse: the Pylon Field and the Wellspring

Status: vision and implementable spec, October 7, 2026. Nothing on this page
is implemented yet, except the pieces it names as existing. The protocol half
is the **Designed** draft [NIP-PYLON](../../nips/openagents/NIP-PYLON.md).
Umbrella issue: #10925. Phases: #10920, #10921, #10922, #10923, and #10924.

This page plans how OpenAgents brings shared compute back, starting inside
Everglade. People contribute machines, agents buy work from them, providers
earn sats, and the whole loop is visible as a place you can walk through.
Every glow, beam, and number in that place comes from signed, checkable
records. It builds on the [history of selling compute for bitcoin](compute-for-bitcoin.md),
the [agent sales floor](../sales/agent-sales-floor.md), the
[crew](../verse/crew.md), and the [generative-agents plan](../verse/generative-agents.md).

## Contents

- [Summary](#summary)
- [Where this comes from](#where-this-comes-from)
- [Inputs from the transcripts](#inputs-from-the-transcripts)
- [What exists today](#what-exists-today)
- [The vision: compute you can see](#the-vision-compute-you-can-see)
- [The story](#the-story)
- [Architecture](#architecture)
- [NIPs](#nips)
- [Security, abuse, and economics](#security-abuse-and-economics)
- [Phased plan](#phased-plan)
- [Open questions](#open-questions)
- [Related documents](#related-documents)

## Summary

- **The place.** Everglade gets a **Pylon Field**: one pylon for each machine
  that serves work, drawn from what that machine runs, how long it has been
  online, how many jobs it served, and the sats it earned. At the field's
  heart is the **Wellspring**, the pooled capacity, which brightens as
  capacity comes online and ripples as jobs flow. When Alice, Bob, the crew,
  or a villager runs a job on pooled compute, a beam runs from the
  Wellspring to that agent's station.
- **The loop.** Players and outside operators run a pylon (Coder's host with
  shared compute turned on), agents and paying customers buy work, and
  providers earn sats with splits recorded in the existing split ledger. The
  Agora sells compute and agent work, agents hire each other in the agent
  market, and the Gym measures quality.
- **The rule.** Nothing is drawn that a reader can't check. Pylons come from
  provider-signed beacons, work comes from buyer-signed receipts with payment
  preimages, and the Wellspring comes from pool aggregates that any client
  recomputes from their inputs.
- **The protocol.** Jobs travel over NIP-CJ (with NIP-90 kind `5050` as a
  public compatibility lane), offers and terms over NIP-MKT, upfront payment
  over NIP-X402, reputation over NIP-XP and NIP-32, and world state over
  NIP-MV. One new draft, NIP-PYLON, adds the beacon (`30200`), the pool
  aggregate (`30201`), and the service receipt (`3201`).
- **The order.** P0 draws the field from this machine's real local data with
  no network. P1 publishes presence and free jobs over Nostr. P2 adds checks
  and reputation. P3 pays providers, test sats first and mainnet only under
  the owner's gate. P4 opens the agent market.

## Where this comes from

The [history](compute-for-bitcoin.md) records the same idea five times, and
each time taught one lesson this design keeps.

| Era | What ran | The lesson kept here |
| --- | --- | --- |
| GPUtopia, 2023 | Browser providers on the GPU, then the `workerbee` daemon on native GPUs, scheduled by one `queenbee`; 560 users and about 150,000 Lightning payments in the first beta week; swarm inference sent one prompt to 8 sellers. | Per-job Lightning payments overloaded one wallet server; balances that sweep every few minutes held up. Sellers outnumbered buyers, so the buy side comes first. |
| Inside OpenAgents, 2023 to 2024 | The swarm network powered agent steps; the Flow of Funds episode split payments among contributors; the Agent Store paid builders every minute; 9-way Lightning splits were tested. | Splits are a ledger concern, recorded per payment and paid in batches. |
| Pylon and data vending machines, 2024 to 2025 | Pylon answered NIP-90 jobs and held a wallet; the Swarm Inference demo paid for a chat message over NIP-90. | NIP-90 works for public jobs; private work needs a better encrypted transport. |
| The compute market, 2026 | Pylon v0.1 on Macs with **Go Online**; the market launched on 2026-03-12; 825,000 sats paid to Pylons in a week; payouts stalled under load and moved to a self-run Lightning node; the training run paid providers for verified work. | Paying for uptime invites gaming, so pay for real work only. Payout infrastructure must survive the load it invites. |
| Coder, 2026-07 onward | Pylon folded into the IDE, then shared compute promised to return in Coder. | One install: the Coder host is the pylon. |

The swarm and the `queenbee` were the earlier shape of the Wellspring: many
small workers around one coordinator. The difference now is that the
coordinator is replaceable (any aggregator can publish a pool, and any reader
recomputes it), and the buyers already exist, because OpenAgents' own agents
spend on compute every day.

## Inputs from the transcripts

Line references point into the [transcript archive](../transcripts/README.md).

**The compute market episodes.** [Episode 213](../transcripts/213.md)
announced five interlocking markets, compute first (lines 27-31), and named
GPUtopia's failure as "an oversupply of sellers and no compelling buy-side
use cases" (line 47), with agents as the logical buyers.
[Episode 214](../transcripts/214.md) showed the build: a **Go Online**
button, a built-in Lightning wallet whose seed also holds the Nostr key, a
buy mode sending small paid jobs, on-device inference, and NIP-90 kind
`5050` text jobs (lines 13-27). It was candid that "we are still going to be
the main buyer" (line 53). [Episode 221](../transcripts/221.md) launched
Pylon as "a NIP-90 service provider" (line 47) with OpenAgents as "buyer
number one" (line 89). [Episode 223](../transcripts/223.md) found payouts
"overwhelmed" by queued payments (lines 3-9) and providers running several
instances at once (line 8). [Episode 224](../transcripts/224.md) ended
uptime pay: "we're going to be paying you for the real work" (line 10).
[Episode 227](../transcripts/227.md) paused payouts until the move to a self-run Lightning node
(lines 57-59). [Episode 237](../transcripts/237.md) put verification at the
center, paying "for verified work" (line 5), and warned that "a payment the
recipient cannot dereference is not a payment" (line 17).

**The agent market.** The five markets were compute, data, labor, liquidity,
and risk (213:39-91), with verification added later (239:15). The Agent Store
of 2024 paid builders "proportional to usage" with an 80/20 split
([092](../transcripts/092.md), [098](../transcripts/098.md):75-77), and paid
about 20 developers without finding work buyers would pay much for
([165](../transcripts/165.md):73-75). [Episode 237](../transcripts/237.md)
named the unit of trade, the accepted outcome: work "scoped in advance ...
graded ... recorded in a receipt, and settled to everyone who contributed"
(line 13). [Episode 266](../transcripts/266.md) replaced overloaded NIP-90
meanings with NIP-MKT's discovery and private negotiation (lines 53-55).

**Compute in the Verse.** [Episode 240](../transcripts/240.md) walked a 3D
run board with "11 pylons, 6 active" and sats paid (lines 7-14).
[Episode 243](../transcripts/243.md) planned to render requests "as crackling
energy fanned to assigned Pylons" (line 100). [Episode 284](../transcripts/284.md)
tied spare gaming hardware to "one inference mesh" (line 58).

## What exists today

| Piece | State | Where |
| --- | --- | --- |
| Lightning node, both x402 roles | Implemented | `crates/wallet`, `crates/x402` |
| Phone and computer wallet | Implemented, mainnet | `crates/spark-wallet` |
| Split ledger with a reserved `provider` role | Implemented; every settlement writes `provider` as 0 today | `crates/pay-ledger`, [central receive and splits](../payments/2026-10-02-central-receive-and-splits.md) |
| Purchased compute balance | Implemented behind the retail gate; paid availability waits on the owner | [Compute balance](../cloud/compute-balance.md) |
| Free labor between a buyer and a provider | Implemented with fixtures; a paid profile exists behind explicit admission | `crates/coder-labor`, [free labor](../coder/runtime/free-labor.md) |
| Hosted NIP-CJ worker with quotas and usage records | Implemented | `crates/eval-runner` |
| Host resource broker: leases, priorities, receipts, placement | Implemented | `crates/coder-lease`, [leases](../coder/runtime/leases.md), [placement](../coder/runtime/placement.md) |
| Provider capacity book | Implemented | `crates/microcoder-loop` (`~/.openagents/tasks/capacity.json`) |
| Private host presence with coarse telemetry | Implemented | `crates/coder-reach`, `crates/coder-host` |
| NIP-MV object states for lamps, doors, workstations, and task walls | Implemented | `crates/verse-net` (`mv::object`), `crates/world-tree` |
| Everglade, the town clock, townsfolk, and rumors | Implemented | `crates/verse-zone-everglade`, `crates/town-clock`, `crates/townsfolk` |
| The Agora building and its bell hook | Built; agents and boards planned | `layout/agora.rs`, [sales floor](../sales/agent-sales-floor.md) |
| A decorative pylon in the Grid's plaza | Implemented, decoration only | `crates/verse/src/world.rs` (`fn pylon`) |
| Pylon beacons, free jobs, receipts, and pool aggregates (P1) | Implemented; free only | `crates/pylon`, `nostr::pylon`, [Pylon guide](pylon.md) |
| Psionic, the model runtime | Imported into the monorepo | `crates/psionic`, [Psionic](../psionic/README.md) |
| A paid provider market | Not implemented | This page |

## The vision: compute you can see

### The Pylon Field

The Pylon Field grows outward from the circle of standing stones in
Everglade's north woods. Each pylon is one machine that serves work for the
pool. You walk up to it and press `F` to inspect it.

| What you see | What it reads |
| --- | --- |
| The pylon's shape: a slim crystal spire for a unified-memory machine such as a Mac, a broad obelisk for a GPU machine, a squat cairn for a CPU machine | Beacon `class.family` |
| Its height, in four steps | Beacon `class.tier` |
| Bands of light round its base, one band per tenfold step in jobs served | Receipt-backed accepted jobs in this pool |
| A glow that breathes while the pylon is online and burns steady while it works | Beacon `status`, and `slots.total − slots.free` |
| Moss and lichen climbing the base | Uptime from beacon `since` |
| A small coin-light at the crown that flares when a payment lands | A new receipt with a valid preimage |
| Grey and still, with a question mark in the inspect panel | A stale beacon: **unknown**, never online |

The inspect panel shows the pylon's label, its owner if the beacon carries a
NIP-OA link, its class, the models it serves, its uptime, its jobs by
outcome, its check results, and its sats earned per network. Test sats carry
a visible **TEST** mark and are never added to bitcoin.

### The Wellspring

At the field's heart, inside the stone circle, is the **Wellspring**: a pool
of light in a stone basin. It is the pool aggregate made visible.

- Its brightness follows online capacity: total slots across fresh pylons.
- Its surface churns with busy slots.
- A ripple runs outward for each accepted job, at the rate the aggregate
  reports.
- A plinth beside it carries the pool's numbers for the last hour: pylons
  online, jobs, units served, sats paid, and checks passed.
- When a reader has recomputed the aggregate, the basin's rim is lit. When
  it hasn't, the rim stays dark and the plinth says **unverified**.

### Agents draw from the Wellspring

When an agent runs a job on pooled compute, a beam of light runs from the
Wellspring to that agent's station for as long as the job runs: Alice's
workstation in the owner's house, Bob's map room, Olivia's desk in the
Stacks, a desk on the Agora's floor. The beam forks to the pylon that serves
the job, so you can follow work from the agent to the machine. A villager
draws a beam only when its own talk or plan actually ran on the pool; most
townsfolk routines are deterministic and spend nothing, and they draw
nothing.

### Players raise pylons

A player who turns on shared compute in Coder (or runs the headless host on
another machine) sees a new pylon rise at the field's edge within a minute,
labeled with the name they chose. It earns as it serves. A player can walk
from their own pylon to the Wellspring and watch their machine's beam light
when the pool routes work to it.

### The Agora sells compute and agent work

The [Agora](../sales/agent-sales-floor.md#the-agora-the-sales-floor-in-everglade)
gains a compute counter: a board that shows the pool's capacity and prices,
and a wall of agent services for sale. Ivan, the issuer, prices offers at the
market hall; Paul's floor sells compute and agent work to businesses through
written outreach, as the sales plan already says. The bell rings only for
settled revenue under the floor's existing rules.

### The agent market

Agents offer services as NIP-MKT offerings, hire each other through NIP-LAB
orders, and buy compute from the pool. When an order settles, the split
ledger records each share: the provider whose pylon ran the work, the
author of any plugin it used, and OpenAgents. In the world, a hired agent
walks to the hiring agent's station, and a settled order sends a thin gold
thread from the Wellspring to each payee's station or pylon.

### The Gym measures quality

The Gym's boards gain a pylon league: per class and model, the pass rate on
pinned spot-check suites, latency, and cost per accepted job. A pylon with
passing checks shows a small sigil in the field. XP for verified work comes
from NIP-XP awards and stays separate from money.

### Honest by construction

Every visual has a data source, and no source is invented.

| Visual | Source | When there is no data |
| --- | --- | --- |
| A pylon | A fresh, valid beacon | No pylon |
| A pylon's glow | Beacon status and slots | Grey and still |
| Job bands and the coin-light | Receipts, with preimages for paid work | No bands |
| The Wellspring's brightness and ripples | A recomputed pool aggregate | A dim basin and an **unverified** plinth |
| A beam to an agent | The agent's own state naming a job, plus its progress | No beam |
| A sigil | A check verdict from a trusted checker | No sigil |
| Demonstration data | `--sim` mode only | Labeled **DEMO** in the world |

## The story

### In the world

Long before the town, the founders found a spring of light under the standing
stones in the north woods. The lamps on Main Street, the forge in the
Foundry, and the workshop's tools all drew from it. The spring is fed from
outside: each pylon raised anywhere in the world is a promise of work, and
the spring rises when many pylons hum. When nobody comes to drink, it sinks.

The beekeeper keeps the oldest story. Before the town, there was a great hive
in these woods: thousands of worker bees around one queen, flying out to
whoever had work. But nobody came to buy the honey, and the hive scattered.
The beekeeper says the bees are coming back now, because the town's own
agents are thirsty. That is the 2023 swarm, `workerbee` and `queenbee`, told
as town lore: the same idea, returning with buyers this time.

**The town clock.** A town day lasts one real hour. The clock tower on
Library Way gets a second dial, the **load dial**, with 24 marks: each mark
is one slice of the last real hour's job rate from the pool aggregate, so a
town day replays the real hour's load. At dawn, Paul's stand-up at the
Agora's leaderboard reads yesterday's real pool totals beside the sales
numbers.

**Rumors.** Townsfolk spread rumors about the pool through the existing
rumor system. A rumor's fact comes from a real aggregate, such as "The
Wellspring ran bright at dusk: 340 jobs in the last hour" or "A new pylon
rose in the north field", and enters through the normal propose and admit
flow. No rumor states a number that no record backs.

**Spells and mana.** The Wellspring is mana for agents, not for players.
Spells stay free, as they are today: no player spell spends compute or sats,
and the demolition yard's mana bar for Meteor Swarm stays a game number. A
player who casts near the Wellspring sees the cast take the pool's current
hue, a cosmetic touch that reads the aggregate and spends nothing.

**The crew.** The roster already has the right people. Trent, the resource
broker, keeps a lease board by the Workbench and adds a column for pool
jobs. Dave runs the pool aggregator from the Server Barn in the Foundry.
Victor runs spot checks from his bench at the Proving ground and publishes
the verdicts. Ivan prices offers at the market hall. Sybil load-tests the
field with swarms of test pylons, and Mallory tampers with receipts to prove
readers refuse them.

### For selling

The pitch to providers is one line from 2023 that still holds: sell your
spare compute for bitcoin. What is new is that buyers come first. OpenAgents'
own agents buy every day, so a new pylon has work from its first hour.

The pitch to buyers is visible quality and price: a pool whose every job is
receipted, whose checks are public, and whose Gym league ranks providers by
measured results.

Selling in public follows the sales floor's rule: each week the owner may
publish a reviewed capture of the Pylon Field from `verse --capture`, with
pool aggregates only: pylons online, jobs, sats paid to providers, and check
pass rates. Never a buyer's job content, and never a private record.

## Architecture

```text
 provider machine (pylon)                     relays                     buyers
 +------------------------------+      +------------------+      +-------------------+
 | Coder host, shared compute on|      | beacons   30200  |      | Alice, the crew,  |
 |  - lease broker: background  |----->| receipts  3201   |<-----| customers' agents |
 |    priority, owner preempts  |      | pools     30201  |      |                   |
 |  - coder-boundary sandbox    |<---->| CJ jobs 25900/   |<---->| CJ requests,      |
 |  - model runtime             |      |   25920 (NIP-44) |      | MKT orders, x402  |
 |  - beacon publisher          |      | MKT 3192/30192   |      +-------------------+
 +------------------------------+      +------------------+               |
              ^                                 |                         v
              |  payouts (balance sweeps)       v                 +---------------+
      +-----------------+               +---------------+         | OpenAgents    |
      | pay-ledger      |<--------------| aggregator    |         | receiver:     |
      | provider share  |  settled jobs | (Dave)        |         | x402, compute |
      +-----------------+               +---------------+         | balance       |
                                                |                 +---------------+
                                                v
                                   Verse clients verify and draw
                                   the Pylon Field and the Wellspring
```

### The provider side

A pylon is the Coder host with shared compute turned on, not a separate
program. Turning it on takes an explicit owner action (a setting in the
desktop app, or `coder host share on` on a headless machine) and is off by
default.

- **Capability advertisement.** The host serves only capabilities it can
  prove locally: a model runtime that the capability registry probes as
  `present` under an approval the operator recorded, as NIP-CAP requires.
  The first service class is small text generation on Macs; decision
  jobs (Jev-style typed questions) and Gym eval runs follow.
- **Admission.** Each request passes the same admission a CJ worker uses
  today, as `crates/eval-runner` does: a verified signer, a capability the
  pylon serves, input and output byte bounds, a turn and time ceiling, and a
  per-buyer rate limit.
- **Sandboxing.** Jobs run under `crates/supervise` in their own process
  group, inside a `coder-boundary` filesystem boundary with no writes outside
  a scratch directory and no network egress beyond the model runtime. A
  pylon runs inference and bounded decision jobs only. Arbitrary code
  execution for strangers waits for NIP-ENV environments and is out of scope
  for this plan.
- **The owner's work comes first.** Every pool job takes a `background`
  priority lease from `crates/coder-lease` (a `pylon` resource beside
  `memory`), so the owner's builds, agent runs, and quiet leases preempt it.
  When the owner's work needs the machine, the beacon goes to `draining`,
  admitted jobs finish, and no new ones arrive. The design follows
  [many agents, one machine](../coder/design/many-agents-one-machine.md).
- **Beacon publishing.** The host publishes a NIP-PYLON beacon as its state
  changes and at least every four minutes, with coarse class and free slots
  only. It never copies NIP-REACH telemetry into a public record.

### The job flow

1. A buyer finds a pylon through the pool's fresh beacons, filtered by
   class, service, check record, and price hint.
2. The buyer sends a NIP-CJ request to that pylon, encrypted with NIP-44.
   Conversation jobs (`25900`) carry text generation; execution jobs
   (`25920`) carry bounded decision and eval work.
3. For paid work, the pylon answers with payment terms first, as the
   [Payments](#payments) section describes.
4. The pylon streams progress (`27000` or `27020`) and returns the result
   (`26900` or `26920`).
5. The buyer publishes a `3201` receipt with digests of the request and
   result, the outcome, and, for paid work, the payment hash and preimage.
6. The aggregator counts the receipt in the next pool aggregate.

The NIP-90 lane (`5050` requests, `6050` results, `7000` feedback) stays
available for public text jobs and for clients that only speak NIP-90. Its
optional encryption uses NIP-04, so nothing private travels on it.

Routing is the buyer's choice. OpenAgents' own agents use a simple rule:
among fresh pylons that serve the capability and pass the pool's check
floor, prefer the lowest price hint, then the fewest recent timeouts, with
a small random share for new pylons so they can earn a record. Swarm
inference (one request to several pylons, keeping the first acceptable
answer or the agreed answer) is a routing mode, not a protocol change.

### Payments

Payments start free and stay free until checks work.

1. **Free (`free-v1`).** P1 runs only free jobs. Receipts carry no payment.
2. **Test sats.** P3 starts on `testnet`, the test network x402's `exact`
   Lightning method names: real Lightning code paths, worthless sats,
   every amount marked **TEST** in the world.
3. **Mainnet.** Only after the owner turns it on, under the ceilings below.

Two settlement paths cover the market:

- **Brokered (the default).** The buyer pays OpenAgents: an x402 payment per
  call or a debit from a purchased compute balance. The split ledger records
  the provider's share against the job's receipt. Providers are paid by
  balance sweeps, not per job: a sweep runs when a provider's balance passes
  a threshold (for example 1,000 sats) or on an interval (for example every
  10 minutes), to the provider's Lightning address or the OpenAgents phone and computer wallet. This is
  the 2023 lesson: per-job payments overloaded one wallet server, and
  balances with periodic sweeps held up. It also gives a clawback window: a
  job that fails a later spot check comes out of the unpaid balance.
- **Direct.** A buyer that holds its own wallet pays the pylon's invoice per
  job under NIP-X402, or a fixed price under a NIP-MKT order. This is for the
  agent market and for buyers outside OpenAgents. The provider bears the
  credit risk NIP-MKT already states.

Zaps (NIP-57), wallet connections (NIP-47), and ecash wallets (NIP-60 and
NIP-61) are wallet transports a buyer may use to pay an invoice. None of
them is a settlement contract, and a zap on a result is never a receipt.

### Splits

The split ledger already reserves a `provider` role and writes it as 0
today ([central receive and splits](../payments/2026-10-02-central-receive-and-splits.md)).
A new rule version gives that role a share of compute sales:

| Party | Share of a brokered compute sale |
| --- | --- |
| The pylon's provider | Most of the price; the history's first buyer interface paid 6 of 7 sats to the provider |
| The author of a plugin the job used | The plugin's per-call fee, as today |
| OpenAgents | The rest, minus the Lightning service provider fee |

The exact provider share is an [open question](#open-questions). Every split
row names the receipt it pays, so a reader can match ledger rows to
public receipts.

### Verification

There is no general proof that a consumer machine ran a model correctly.
Trusted execution hardware isn't available for this on the machines that
matter, and floating-point results can differ across hardware, so exact
replay isn't a proof either. The design uses checks that are honest about
what they show:

- **Known-answer canaries.** A checker sends ordinary-looking jobs whose
  answers it knows. A pylon can't tell a canary from real work.
- **Redundant execution.** For a sample of jobs, the router sends one
  request to two or three pylons and compares results: exactly for
  deterministic settings, and through a typed Jev judgment otherwise.
- **Gym suites.** Each class and model runs pinned spot-check suites, and the
  results land in the Gym league.
- **Receipts and verdicts.** Buyers sign receipts, and trusted checkers sign
  NIP-32 verdicts that point at them. Pools count only the checkers their
  policy trusts.
- **Reputation.** A pylon's record is its receipts and verdicts. NIP-XP
  awards for verified work give providers a level, never money.
- **Economics.** With balance payouts, a failed check forfeits the unpaid
  balance for the failing jobs, and repeated failures drop the pylon from
  the pool's admission. A spot-check rate of a few percent makes cheating
  unprofitable when the forfeit exceeds the gain.

### Discovery

- Pylons publish beacons to `relay.openagents.com` and the relays in their
  NIP-65 list.
- Priced services publish NIP-MKT offerings (`3192`) and heads (`30192`).
- A pylon that serves the NIP-90 lane may publish a NIP-89 `31990` handler
  record listing kind `5050`.
- Each pool serves its policy document beside its aggregate.

### Privacy and safety

- **What a provider sees.** The plaintext of every job it runs. Encryption
  hides jobs from relays and onlookers, not from the machine doing the work.
  OpenAgents' own agents send pooled compute only work they would accept a
  stranger reading: no secrets, no owner credentials, and no private
  repository content.
- **What the public sees.** Beacons, receipts with digests, verdicts, and
  aggregates. Never prompts, results, or buyer data.
- **Abuse limits.** Per-buyer rate limits at the pylon, byte and time bounds
  on every job, no network egress from jobs, and capability classes a
  provider may decline. A pool's policy can exclude buyers.

### Pricing

- Pylons advertise `price_hint_msat` per job unit (per thousand tokens for
  text) by tier, and the brokered path sets a posted price per class that
  OpenAgents can change at a rule version boundary.
- Faster classes may charge more; the 2023 network priced slower inference
  lower, and that holds.
- OpenAgents' agents are the first buyer at the posted price, as in 2026.
  No payment for uptime, because the 2026 market showed it invites gaming.

### The Verse projection

World state comes only from verified records.

| Layer | Source | Update rate | Bound |
| --- | --- | --- | --- |
| Pylon object state (`33301`, `state.kind: "pylon"`) | Newest fresh beacon and counted receipts | At most once every 5 seconds per pylon | 256 pylons drawn per pool; the rest as a count |
| Wellspring object state (`33301`, `state.kind: "wellspring"`) | Newest valid pool aggregate | At most once every 10 seconds | One per pool |
| Beams | The agent's own entity or workstation state naming a job, plus CJ progress | Drawn while progress is fresher than 120 seconds | One beam per running job; at most 64 drawn |
| Ripples | The aggregate's `rate` | Client animation | Capped at 10 per second |
| Sigils | Trusted check verdicts | With the pylon state | One per pylon |

In P0, before any events exist, the same mapping runs on local sources
behind one trait, so P1 swaps the source without changing the world:

| Local source (P0) | Feeds |
| --- | --- |
| This host's lease table and receipts under `~/.openagents/leases/` | The local pylon's busy slots, its jobs, and Trent's board |
| NIP-REACH presence for the owner's other computers | One pylon per enrolled computer, with its status |
| The capacity book (`~/.openagents/tasks/capacity.json`) | Outer wells around the Wellspring for each model provider with capacity |
| The studio's workstation states | Beams to Alice's and the crew's stations while their jobs run |

The local pylon is drawn with an **OWNER** mark, because it serves only the
owner's own work until P1.

## NIPs

| Need | NIP | Use |
| --- | --- | --- |
| Private jobs | [NIP-CJ](../../nips/openagents/NIP-CJ.md) | Conversation `25900`/`26900`/`27000` and execution `25920`/`26920`/`27020`, NIP-44 encrypted |
| Public text jobs | [NIP-90](../../nips/official/90.md) | Kind `5050`/`6050`/`7000` compatibility lane, public content only |
| Handler discovery | [NIP-89](../../nips/official/89.md) | `31990` for NIP-90 clients |
| Operations | [NIP-CAP](../../nips/openagents/NIP-CAP.md) | Exact capability definitions a pylon serves |
| Offers and orders | [NIP-MKT](../../nips/openagents/NIP-MKT.md) | `3192` and `30192` offerings, private negotiation, fixed-price settlement |
| Agent labor | [NIP-LAB](../../nips/openagents/NIP-LAB.md) | Agents hiring agents, with acceptance and rework |
| Upfront payment | [NIP-X402](../../nips/openagents/NIP-X402.md) | Per-call Lightning payment, brokered or direct |
| Wallet transport | [NIP-47](../../nips/official/47.md), [NIP-57](../../nips/official/57.md), [NIP-60](../../nips/official/60.md), [NIP-61](../../nips/official/61.md) | Ways a buyer pays an invoice; never settlement contracts |
| Provider presence, receipts, pools | [NIP-PYLON](../../nips/openagents/NIP-PYLON.md) (new) | `30200` beacon, `3201` receipt, `30201` aggregate |
| Check verdicts | [NIP-32](../../nips/official/32.md) | Labels in the `openagents.pylon` namespace |
| Reputation | [NIP-XP](../../nips/openagents/NIP-XP.md) | Awards for verified work; never money |
| Quality evidence | [NIP-EVAL](../../nips/openagents/NIP-EVAL.md) | Spot-check suites and Gym results |
| Owner and agent identity | Block [NIP-OA](../../nips/block/NIP-OA.md), [NIP-AE](../../nips/block/NIP-AE.md) | Owner attestation for a pylon key; an agent's engrams remember its suppliers |
| Agent spend records | Block [NIP-AM](../../nips/block/NIP-AM.md) | Per-turn `44200` metrics include pooled compute spend |
| Private host presence | [NIP-REACH](../../nips/openagents/NIP-REACH.md), [NIP-HOST](../../nips/openagents/NIP-HOST.md) | The owner's own computers; never public |
| World state | [NIP-MV](../../nips/openagents/NIP-MV.md) | `33301` object states for pylons and the Wellspring |
| Town talk | [NIP-29](../../nips/official/29.md), [NIP-C7](../../nips/official/C7.md) | Pool announcements in town chat |
| Marketplace listings | [NIP-15](../../nips/official/15.md), [NIP-99](../../nips/official/99.md) | Not used for terms; a NIP-99 listing may link an offering for humans |

**Why a new NIP.** Existing records cover jobs, terms, and payment, but none
says in public "this machine is online for work," none lets a third party
attest that a job happened and was paid, and none defines totals a reader can
recompute. NIP-CAP forbids local presence in public heads, NIP-REACH presence
is private by design, NIP-MKT's `capacity_hint` is per offering and carries
no freshness rule, and NIP-MV object states are what a world draws, not
evidence. NIP-PYLON fills that gap with three kinds and no authority:
`30200`, `30201`, and `3201`, checked against the official list, the Block
kinds, and the [OpenAgents kind registry](../../nips/openagents/README.md#kind-registry).

## Security, abuse, and economics

| Risk | Mitigation |
| --- | --- |
| **Sybil pylons.** One machine posing as many. | No uptime pay, so idle Sybils earn nothing. Pool policy can require a NIP-OA owner and cap pylons per owner. Canaries reveal pylons that share one machine's latency and answers. Episode 223 saw exactly this. |
| **Fake capacity.** A beacon overstating class or slots. | Beacons grant nothing and carry no earnings. Routing weighs receipts and checks; timeouts cost future work. |
| **Wrong answers.** A pylon returns cheap or fabricated output. | Canaries, redundancy, Gym suites, and forfeit of the unpaid balance for failed jobs. |
| **Wash trading.** A provider buys from its own pylon to inflate its record. | Receipts from the provider or its owner never count; pools count trusted buyers only; test-network totals stay apart; real fees make mainnet wash volume cost money. |
| **Griefing.** Flooding the field or the relay. | The 256-pylon draw bound, a per-address beacon rate limit, pool admission, and relay rate limits. |
| **Payout overload.** The 2023 and 2026 failures. | Balance sweeps, a payout worker with its own queue, and the self-run Lightning node in `crates/wallet`. |
| **Payout loss.** A node crash or a withdrawal bug, both seen in 2023. | The ledger is the record of what is owed; a sweep pays from it idempotently; an owed balance survives a crash. |
| **Custody.** OpenAgents holds providers' unpaid balances. | Small thresholds and frequent sweeps keep balances small; balances are shown to each provider. |
| **Content risk.** Buyers sending harmful jobs; providers reading private data. | Byte, time, and rate bounds; provider-declined classes; no secrets in pooled jobs. |

Owner gates, from existing invariants:

- Nothing pays without the owner's tap, except what a standing grant the
  owner made admits (`INVARIANTS.md`, Agent spending). Mainnet provider
  payouts need such a grant, with a per-payment and a daily ceiling.
- Tests never move the owner's funds; every spend test pays through a fake
  wallet.
- XP is never money: no code path converts, spends, or transfers XP.
- Paid compute availability stays behind the retail gate until the owner
  confirms it, as [compute balance](../cloud/compute-balance.md) states.

P0 to P2 spend nothing. P3 runs on test sats until the owner turns mainnet
on, and that step goes in the workspace `NEEDS_OWNER.md`.

## Phased plan

Each phase ships on its own. Captures use the offscreen `everglade_capture`
example or `verse --capture`; no phase opens a visible window.

### P0: the Pylon Field from local data

Issue: #10920. Implemented in `zones::everglade::compute`
(`crates/verse-zone-everglade`). The source interface is
`compute::ComputeSource`: one `sample(now)` call that returns `Sample`, a
list of `PylonSample` records (ID, label, hardware class, status, busy and
total slots, jobs, uptime, and when each was observed), the capacity book's
wells, and the pool's job rate. `compute::local::LocalSource` reads this
computer through `coder_lease::observe`, which never changes the lease
table, and `compute::sim::Sim` is the DEMO pool (`verse --pylon-sim`). A
P1 source that reads beacons and receipts implements the same trait and
installs with `WorldRuntime::set_compute_source`; the drawing doesn't
change. NIP-REACH presence for the owner's other computers needs the
network, so P0 leaves it to P1. The field is drawn in generated geometry
and changes no pack asset; pylons and the basin block no walking.

Draw the field and the Wellspring in Everglade from this machine's real data,
with no network and no spending.

- A `ComputeSource` trait with a local implementation over the lease table,
  lease receipts, the capacity book, NIP-REACH presence for the owner's
  computers, and the studio's workstation states.
- `pylon` and `wellspring` object states in `world-tree` and `verse-net`, as
  NIP-PYLON's [World projection](../../nips/openagents/NIP-PYLON.md#world-projection)
  defines.
- The field's site, the stone basin, the pylon shapes by family and tier,
  glow, bands, beams, and the inspect panel, under the zone's triangle
  budgets.
- A `--sim` source that plays a labeled **DEMO** pool for captures.

Acceptance:

- With no leases held, the local pylon is dim and the Wellspring is still.
  With one build lease held, the pylon burns and the basin churns. A test
  drives both from a fixture lease table.
- A beam appears at Alice's workstation only while her studio state is busy.
- A stale or missing source shows **unknown**, never online.
- Captures: `pylon-field-idle.png`, `pylon-field-busy.png`, and
  `wellspring-demo.png` (with the **DEMO** mark).

### P1: presence and free jobs over Nostr

Issue: #10921.

- Validators and fixtures for NIP-PYLON's three kinds and the check label in
  `crates/nostr`.
- `coder host share on|off|status`: the beacon publisher, the `pylon` lease
  resource at `background` priority, and a CJ conversation worker for one
  small text-generation capability inside `coder-boundary`.
- An aggregator command (`openagents pylon pool`) that publishes `30201`
  with its policy document, run by Dave's runbook.
- Verse swaps the local source for a relay source that verifies and
  recomputes before drawing.
- Free jobs only: Alice's and the crew's low-risk text jobs route to the
  pool.

Acceptance: a second machine turns sharing on and its pylon appears in
another client's field within 60 seconds; a free job from Alice produces a
receipt that the next aggregate counts; a tampered receipt or aggregate is
refused in a test; captures `pylon-field-two-machines.png` and
`wellspring-live.png`.

**Built (2026-10-07).** [Run a Pylon, and use one](pylon.md) is the guide.

- `nostr::pylon` builds and verifies all three kinds, judges beacon
  freshness and generation rollback, computes and recomputes pool
  aggregates under a policy document, and projects a beacon into a `pylon`
  world state. Tests refuse tampered beacons, receipts, and aggregates.
- `crates/pylon` holds the provider, the buyer client, the aggregator, and
  `RelayField`, the relay source for the Pylon Field. `openagents pylon
  serve|ask|status|pool|whoami` exposes them, as does a small `pylon`
  binary for provider machines.
- The model runtime is Psionic, which now lives in this monorepo
  ([`crates/psionic`](../psionic/README.md)). The provider calls a local
  `psionic-openai-server` over loopback. On the RTX 4080 box it serves
  Qwen3.5 0.8B on CUDA.
- Jobs travel on the CJ conversation lane (`25900`/`26900`/`27000`,
  NIP-44). The provider admits an allowlist, rate-limits each buyer, bounds
  slots, input, output, and time, and logs no content.
- `scripts/pylon-psionic.sh` sets up and runs a pylon as transient user
  units; `scripts/pylon-demo.sh` runs the cross-machine demo against
  `relay.openagents.com`.
- Pylon traffic, test jobs included, uses the production relay
  (owner's decision, 2026-10-07).

- Verse's desktop build draws the relay's pylons beside this computer's
  (`zones::everglade::compute::relay`, feature `pylon-relay`): a background
  subscription that verifies every beacon, receipt, and aggregate, shows a
  stale beacon as unknown, and drives the Wellspring's ripples from a valid
  aggregate. The web and phones leave the field dormant.
- While `openagents pylon ask` runs a job, the beam flows from the
  Wellspring to Alice's station, with a fork from the pylon serving it.

- `openagents host share on|off|status` (also `coder host share`) makes the
  Coder host the pylon: the running host starts and stops the provider
  from the setting. Each pool job takes a `pylon` lease at `background`
  priority, and the pylon drains while the owner's work (a `quiet` lease or
  any `owner` priority lease) needs the computer.
- `openagents pylon link` adds the owner's NIP-OA `auth` tag to every
  beacon; readers verify it, and the owner's and provider's own receipts
  never count.

- `openagents pylon route on [--pylon NPUB]` sends Alice's and the crew's
  day plans to the pool as free jobs with receipts, falling back to the
  agent's own model when no pylon answers. Off by default.

The job is inference inside the Psionic process with no command execution, so
`coder-boundary` has nothing to bound until execution jobs arrive.

### P2: checks and reputation

Issue: #10922.

- Victor's checker: canaries and redundant execution, publishing NIP-32
  verdicts.
- A Gym suite per service class and the pylon league on the Gym's boards.
- NIP-XP quests and awards for verified work.
- Sigils in the field.

Acceptance: a pylon that returns wrong canary answers in a fixture is marked
`check-fail` and leaves the pool's admission; the Gym league shows per-class
results from pinned suites; capture `pylon-league.png`.

**Built (2026-10-08).** See [Check pylons](pylon.md#check-pylons-victors-checker).

- `nostr::pylon::check` signs and verifies check labels, binds each to its
  receipt (never from its buyer or provider), and folds trusted verdicts
  into a pylon's standing. A pool policy with `checkers` counts their
  labels in the aggregate, and `exclude_failed` drops a pylon with a
  counted `check-fail` from admission.
- `openagents pylon check canary|redundant` is Victor's checker: one
  pinned suite per hardware family, run through the normal job path. In
  the fixture (`crates/pylon/tests/end_to_end.rs`) an echoing pylon fails
  every canary, leaves the checked pool's admission, and is never chosen
  by a buyer that trusts the checker.
- `openagents pylon league` ranks pylons per family and tier on the pinned
  suites, with jobs, median time, and cost per accepted job.
- NIP-XP gains the `pylon-check` rule: one award per pylon per suite
  version for a passing canary, refereed by the checker.
- Everglade draws a sigil over each pylon with passing checks.

Not yet: the league on Verse's Gym boards and the `pylon-league.png`
capture.

### P3: paid jobs

Issue: #10923.

- Brokered sales through x402 and the compute balance; a new split rule
  version with a positive `provider` share; a payout worker that sweeps
  balances.
- Direct per-job x402 payment between agents and pylons.
- `regtest` or `signet` first, with **TEST** marks; mainnet only under the
  owner's standing grant and ceilings, recorded in `NEEDS_OWNER.md`.

Acceptance: on a test network, 1,000 paid jobs across three pylons settle
through balance sweeps with ledger rows matching receipts; a failed check
forfeits the unpaid share; the coin-light flares only for receipts with a
valid preimage; capture `pylon-field-paid-test.png`.

**Built (2026-10-08), on test sats.** See [Paid jobs](pylon.md#paid-jobs-p3-test-sats).

- `pay-ledger` rule v2 (`rules/v2.toml`) gives the `provider` role 8,500
  bps of a brokered pylon sale (open question 3 is still the owner's to
  settle at the next rule version); `Split::PylonJob` binds each provider
  share to its `3201` receipt, one settlement per receipt, by x402 payment
  hash or compute balance debit (`settle_pylon_hold`).
- `pylon::broker` settles brokered sales, forfeits the unpaid share of a
  job a trusted checker failed, and sweeps provider balances through the
  existing payout worker under `Policy::pylon_sweeps`; a mainnet sweep
  needs the owner's grant and stays under its ceilings.
- `pylon::paid`: priced pylons sell each job through NIP-X402's native
  `3188` purchase records; the seller settles the buyer's claim through
  the embedded x402 facilitator and its replay store and runs only the CJ
  job an admitted purchase names. Buyers pay under a ceiling and publish
  the preimage in the receipt. `TestLightning` is the in-memory testnet
  that signs real BOLT11 invoices.
- The broker settles a customer's x402 payment through the same
  facilitator before it buys the job, and records only a payment the
  facilitator consumed. A priced plugin's author fee comes first in a
  pylon job split (`pay_ledger::PluginFee`).
- `openagents pylon serve --price-msat` and `ask --max-msat` use this
  computer's Lightning node and `openagents x402`'s payer and policy; on
  `bitcoin` both refuse without the owner's `grant.json`, and every
  mainnet payment stays under its ceilings.
- `crates/pylon/tests/paid.rs`: 1,000 brokered jobs across three pylons on
  the in-process relay settle through six sweeps with every ledger row
  matching its receipt; ten failed jobs swept already are recorded losses
  and ten unswept ones are forfeited.
- Everglade lights a pylon's coin for 15 seconds after a receipt with a
  valid preimage, pale and marked TEST on test networks.

Not yet: a paid job on a live wallet, and mainnet, both of which wait on
the owner (`NEEDS_OWNER.md`).

### P4: the agent market

Issue: #10924.

- Crew members and agents publish NIP-MKT offerings for their services and
  hire each other through NIP-LAB orders, paying pylons for the compute
  underneath.
- The Agora's compute counter and agent-services wall, and gold settlement
  threads in the world.
- Rumors and the load dial from real aggregates.

Acceptance: one agent hires another for a priced task in a fixture, the
order's compute runs on the pool, and the ledger shows provider, author, and
OpenAgents shares tied to receipts; capture `agora-compute-counter.png`.

**Built (2026-10-08), on test sats.** See [The agent market](pylon.md#the-agent-market-p4-test-sats).

- `pylon::market`: a seller's NIP-MKT offering for an agent service on the
  pool; a hire's sealed `rfq`, `quote`, `order`, and `order_ack`, checked
  at each side's desk under the NIP-LAB labor profile; the broker buys
  the confirmed order's job from the pool; and OpenAgents' receiver issues
  the order's invoice, bound to the order, which the buyer pays after it
  accepts the answer.
- `Broker::settle_order` and `pay_ledger::Split::AgentOrder`: the seller's
  fee (the price less the compute) first as the `author` share, the
  provider's `[pylon_job]` share of the compute, and OpenAgents the rest,
  one settlement per order and per `3201` receipt.
- `crates/pylon/tests/market.rs`: Alice hires Victor for a 25-sat plan
  review; the job runs on an in-process relay pylon; the ledger shows
  15,000, 8,500, and 1,500 msat tied to the receipt.
- Everglade: the Agora's compute counter (online pylons, free slots, the
  day's jobs and broker sales, sats paid, test sats marked TEST) and its
  agent-services wall from verified offerings stand on the forecourt; a
  settlement thread runs from the counter to each pylon a trusted broker's
  job just ran on (`OPENAGENTS_PYLON_BROKERS`), gold only for mainnet
  sats. The clock tower's front clock wears a load dial, the busy share of
  the pool's online slots, and villagers pass on the pool's news (the
  Wellspring's pylons, busy slots, jobs a minute, and the Agora's sales)
  when the player talks to them.
- Capture: `bench/verse/2026-10-08/agora-compute-counter/`.

Not yet: OpenAgents publishes no broker key, so the threads need
`OPENAGENTS_PYLON_BROKERS`; NIP-LAB delivery and acceptance records and
NIP-MKT payment instructions are not exchanged as records; and mainnet
agent orders wait on the owner's gate.

## Open questions

1. **Names.** Wellspring, or mana pool? Pylon, or another word now that Pylon
   was a product name?
2. **The first buyer's budget.** How many sats a day may OpenAgents' own
   agents spend on pooled compute in P3?
3. **The provider share.** What share of a brokered sale goes to the
   provider: 80 percent, 85 percent, or the 2023 interface's 6 of 7?
4. **First workloads.** Small text generation, decision jobs, or Gym eval
   runs first?
5. **Mainnet.** When do mainnet payouts turn on, and with what per-payment
   and daily ceilings?
6. **Shared instances.** Should other players' pylons show their labels and
   owners by default, or only counts until each provider opts in to being
   shown?

## Related documents

- [Compute for bitcoin: the history](compute-for-bitcoin.md)
- [NIP-PYLON](../../nips/openagents/NIP-PYLON.md)
- [Everglade](../verse/everglade.md), [the crew](../verse/crew.md),
  [generative agents](../verse/generative-agents.md), and
  [agent identity and engrams](../verse/agent-identity-and-engrams.md)
- [The agent sales floor](../sales/agent-sales-floor.md) and the
  [revenue roadmap](../sales/revenue-roadmap.md)
- [Central receive and splits](../payments/2026-10-02-central-receive-and-splits.md)
  and [later markets](../payments/later-markets.md)
- [Agent labor plan](../agents/market-infrastructure.md) and
  [free labor](../coder/runtime/free-labor.md)
- [Leases](../coder/runtime/leases.md), [placement](../coder/runtime/placement.md),
  and [many agents, one machine](../coder/design/many-agents-one-machine.md)
- [The Gym in Verse](../verse/gym.md)
