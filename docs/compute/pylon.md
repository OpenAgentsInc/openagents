# Run a Pylon, and use one

A pylon is a computer that shares a model with other people's agents over
Nostr. It publishes a NIP-PYLON beacon (`30200`) that says it is online and
what it serves, takes free NIP-CJ conversation jobs (`25900`) that are
NIP-44 encrypted to it, runs them on a Psionic model server, and answers
with an encrypted result (`26900`). The buyer then publishes a service
receipt (`3201`), and an aggregator can count a pool's beacons and receipts
into a pool aggregate (`30201`) that any reader can recompute.

This covers phases P1 to P3 of [Compute in the Verse](verse-compute.md#phased-plan):
free jobs, checks, and [paid jobs](#paid-jobs-p3-test-sats) on test sats. A
free job's receipt has a null `payment`.

## Pieces

| Piece | Where |
| --- | --- |
| NIP-PYLON records: build, verify, freshness, aggregate recomputation, world projection | `crates/nostr/src/pylon.rs` (`nostr::pylon`) |
| Provider, client, relay field source, aggregator | `crates/pylon` (`openagents pylon`, or the small `pylon` binary) |
| Model server | `psionic-openai-server`, built from [`crates/psionic`](../psionic/README.md) |
| Provider machine setup | `scripts/pylon-psionic.sh` |
| End-to-end demo from another computer | `scripts/pylon-demo.sh` |

The provider talks to Psionic over loopback HTTP
(`/v1/chat/completions`), so a CUDA fault stops the model server, not the
provider. When the model server stops answering, the beacon goes to
`draining` with no free slots.

## Run a pylon

On a machine with an NVIDIA GPU (the RTX 4080 box runs NixOS; the script
fetches the CUDA 13 libraries from nixpkgs when `nvcc` is absent):

```sh
scripts/pylon-psionic.sh setup                  # CUDA, the model, both builds
openagents pylon whoami                         # on the buyer's computer: its buyer key
scripts/pylon-psionic.sh start --allow npub1... # admit that buyer
scripts/pylon-psionic.sh status
scripts/pylon-psionic.sh stop                   # publishes an offline beacon
```

`setup` downloads Qwen3.5 0.8B Q8_0 (about 1 GB, digest checked), builds
`psionic-openai-server` with CUDA kernels, and builds `pylon`. `start` runs
both as transient systemd user units, `pylon-psionic` and `pylon-provider`;
`stop`, `systemctl --user stop pylon-provider pylon-psionic`, or a reboot
removes them, and no unit file is written. Logs go to
`~/.openagents/compute/run/`. Set `PYLON_BACKEND=metal` on Apple Silicon or
`cpu` anywhere.

`openagents pylon serve` is the provider on its own, for a model server you
started yourself:

```sh
openagents pylon serve --engine http://127.0.0.1:18080 \
  --model qwen3.5-0.8b-q8_0 --pylon studio-4080 --allow npub1...
```

Admission, in order: a valid signature, this pylon as the only `p`, a
request no older than 120 seconds and not seen before, a key on the
allowlist (`--allow`, repeatable; `--allow-any` opens it), the buyer's rate
limit (`--rate`, default 10 jobs a minute), a free slot (`--slots`, default
2), and the request bounds (32 transcript entries, 16 KiB of text). A
refusal is `status: error` feedback with a NIP-CJ code: `not_admitted`,
`rate_limited` with `retry_after_ms`, `malformed`, or `limit_exceeded`.
Output is bounded by `--max-tokens` (default 512) and each job by 90
seconds. The provider logs no job content.

The beacon carries only what NIP-PYLON allows: `class` (`gpu`, `medium`,
16 GB for the 4080), `slots`, one service (`cj-conversation`, the model
name, and the capability `<pylon key>:pylon/text-generation`), `free-v1`
settlement, and the `everglade` pool. It republishes when the free slots or
status change (at most every 10 seconds) and otherwise every 60 seconds,
and is valid for 240 seconds.

Keys live in `~/.openagents/compute/` (`provider.key`, `buyer.key`,
`aggregator.key`, mode `0600`), or under `OPENAGENTS_PYLON_HOME`.

Each job takes a `pylon` lease at `background` priority from this
computer's lease broker, and the pylon drains (its beacon says `draining`
with no free slots, admitted jobs finish, new ones are refused
`rate_limited`) while a `quiet` lease or any `owner` priority lease is held
or queued. `--dedicated` skips the lease table on a box that runs no owner
work.

### Answer decisions (Clef)

OpenAgents decisions (the chat router, Coder's judges, file relevance, the
Verse's questions) ask Jev (TypeSafe's API) first-class; connected pylons
are the fallback and the shadow until they pass the router gate (#11225;
NIP-DEC, "The OpenAgents decision API"). The owner decided this on
2026-10-10 after the pylon judge measured 4.7–6 s with confidence 0.27–0.35
on the router's main question set and missed prepared answers. A caller
with a Jev key falls back to `POST /v1/systemone` on our API with the
pylons first, and one Jev answer in twenty is asked again at the pylons
for agreement; a caller without a key asks our API, which asks Jev under
its house key, then the pylons, then Vertex. A pylon answers decisions with
a local Psionic Clef server:

```sh
# The Clef server (crates/psionic, CUDA on a 4080):
psionic-openai-server -m Clef-Flash-Q4_K_M.gguf --port 18096 --decision-device cuda
# The pylon, decisions only, admitting the gateways' dispatch keys:
openagents pylon serve --decide http://127.0.0.1:18096 --decisions-only \
  --pylon coderos-4080-clef --slots 4 --rate 600 --allow npub1...,npub1...
```

The beacon advertises `<pylon key>:pylon/decision` on the `cj-decision`
lane with the served identity (`clef-flash@sha256:<artifact digest>`, read
from the server's `/v1/models`) and its free slots; the beacon says
`draining` while the Clef server does not answer. Each decision job gets
`27010 processing`, then a `26910` result whose answer names the model, the
pylon (`service.door: "pylon:<slug>"`), and the latency, with a sealed
receipt naming the artifact digest. Decisions are free work. Without
`--decisions-only` the same pylon also serves text jobs from `--engine`.
The gateway trusts only the pylon keys in its `DECISION_PYLONS` (default
CoderOS-4080's), and logs its own dispatch key at start (`decisions: …
(dispatch key …)`), which is the key to `--allow`.

**CoderOS-4080, the first decision pylon (2026-10-10).** Two user units,
started with `systemd-run --user` like the text pylon's:

- `pylon-clef`: `~/work/pylon-decide/bin/psionic-openai-server-a9ab2671be
  -m ~/clef-m1/models/Clef-Flash-Q4_K_M.gguf --host 127.0.0.1 --port 18096
  --decision-device cuda --decision-chunk 2048` (the M2 build: staged delta
  scan, fused kernel for short chunks, fixed-order flash attention, the
  head's memory attention on the device, `a9ab2671be`, about 6.5 GB of
  the 4080).
  `sh ~/work/pylon-decide/clef-cuda.sh` (re)starts it, and its log
  (`~/clef-m1/logs/pylon-clef.log`) has one line per decision (tokens,
  time waiting, time run). To roll back to the M2 build:
  `CLEF_BIN=$HOME/clef-m1/target/release/psionic-openai-server CLEF_CHUNK=1024 sh ~/work/pylon-decide/clef-cuda.sh`;
- `pylon-decide`: the static `pylon` at `~/work/pylon-decide/bin/pylon`,
  `serve --decide http://127.0.0.1:18096 --decisions-only --pylon
  coderos-4080-clef --slots 8 --allow <staging and production dispatch
  keys>`, under the text pylon's key (`OPENAGENTS_PYLON_HOME=~/work/pylon-p1/home`,
  `95bc7521…`, the gateways' default `DECISION_PYLONS`);
  `~/work/pylon-decide/pylon-decide.sh KEY,KEY` restarts it.

The 4080 has 16 GB. While a second Clef server for training experiments
holds 6.5 GB of it, the text pylon's qwen model (`pylon-psionic`) is
stopped so the decision Clef fits, and the user unit `pylon-text-restore`
brings it back as it ran before (`psionic-restart.sh original`) once that
server exits. Stopping `pylon-clef` is the failover drill: the next
decision goes to Gemini on Vertex, and the pylon answers again within a
minute of a restart (the gateway benches a failed pylon for 60 s).

### Share this computer from the host

The Coder host is the pylon. Sharing is off by default:

```sh
openagents host share on --allow npub1...   # or --allow-any
openagents host share status
openagents host share off                   # publishes an offline beacon
```

`on` takes the same options as `serve` and keeps earlier choices in
`~/.openagents/compute/share.json`. The running host (this user's own,
under the default root) looks at that file every 5 seconds and starts or
stops the pylon; it needs a Psionic server at `--engine` (default
`http://127.0.0.1:18080`), and shows `draining` until the server answers.

### Send Alice's and the crew's text jobs to the pool

```sh
openagents pylon route on                  # the best fresh pylon
openagents pylon route on --pylon npub1... # only this pylon, such as your own
openagents pylon route status
openagents pylon route off
```

Off by default. While it is on, an agent's day plan (Alice's and the
crew's) is written by a free pool job, NIP-44 encrypted to the pylon that
runs it, as this computer's buyer key, with a receipt the next aggregate
counts. When no pylon answers, the agent's own model writes it. The
setting lives in `~/.openagents/compute/route.json`.

### Link the pylon to its owner (NIP-OA)

```sh
openagents pylon link --owner-secret ~/owner.key   # mint with the owner's key
openagents pylon link --credential '["auth","<owner>","kind=30200","<sig>"]'
openagents pylon link --remove
```

Every beacon then carries the owner's `auth` tag (conditions `kind=30200`).
Readers verify it and refuse a beacon whose tag does not verify; a receipt
from the pylon's provider key or its verified owner never counts toward the
pool's totals or the field's job counts. The owner key is read once to mint
the tag and never stored.

## Use a pylon

```sh
openagents pylon status                       # every verified pylon on the relay
openagents pylon ask "Name three planets."    # best fresh pylon, or --pylon npub1...
```

`ask` reads the relay's beacons, keeps the newest valid one per pylon,
picks a fresh online pylon with a free slot, sends the prompt encrypted to
it, and waits for the result. It prints the answer and three latencies:
discovery (connect and read beacons), first contact (request to the first
feedback), and answer (request to result). It then publishes a `3201`
receipt with digests of the request and result, never their text, and
appends it to `~/.openagents/compute/receipts.jsonl`. `--no-receipt` skips
publication. A job that fails or times out still gets a receipt with that
outcome.

## Count a pool

```sh
openagents pylon pool --publish                       # aggregate the last hour, sign, publish
openagents pylon pool verify --aggregator npub1...    # recompute the newest aggregate
```

The policy document (`PoolPolicy`: open admission, every buyer counted, 12
rate slices) is written to `~/.openagents/compute/pool-everglade.policy.json`
and its digest goes in the aggregate. `verify` refetches the inputs and
refuses an aggregate whose input digests or totals differ. Verify soon after
publication: a beacon is addressable, so once the pylon republishes, the
counted beacon is gone from the relay and the recomputation no longer
matches.

## Check pylons (Victor's checker)

```sh
openagents pylon check canary --pylon npub1... [--award]   # the class's pinned suite
openagents pylon check redundant "Say hi." --pylon npub1... --pylon npub1... --pylon npub1...
openagents pylon league                                     # per-class results
openagents pylon pool --checked --publish                   # drop failed pylons from admission
```

`check canary` sends the pylon its hardware family's pinned Gym suite
(`pylon::check::suites`: three known-answer jobs; the suite's digest pins
it) through the normal job path with the buyer key, publishes each receipt,
and signs a NIP-32 verdict (`1985`, `openagents.pylon`, `check-pass` or
`check-fail`) on it with a separate checker key (`checker.key`), whose
content names `suite:<digest>`. `check redundant` sends one prompt to two
to five pylons: a pylon in the strict majority passes, one outside it
fails, and no majority is inconclusive. `--award` publishes the suite's
NIP-XP `pylon-check` quest, refereed by the checker, and one award to a
pylon whose canary passed, once per suite version. XP never converts to
sats.

Readers trust this computer's own checker key, the keys in
`OPENAGENTS_PYLON_CHECKERS` (comma-separated npubs), and `--checker NPUB`.
One trusted `check-fail` in the last 24 hours keeps `ask` and `route` from
choosing that pylon, and `pool --checked` drops it from the aggregate's
admission. `league` shows, per family and tier, each pylon's pass rate on
its class's pinned suite, accepted jobs, median job time, and cost per
accepted job, with `*` for passing checks.

## The Pylon Field

`pylon::field::RelayField::poll` returns each verified pylon as a NIP-PYLON
`pylon` world state: status (`unknown` when stale), family, tier, busy and
total slots, accepted receipt-backed jobs from the last 24 hours, and
uptime. `pylon::field::glowing` says whether a pylon is serving right now.
`openagents pylon status --json` prints the same states.

Verse subscribes instead of polling: `RelayField::watch` holds one
subscription to the pool's beacons, the last 24 hours of receipts, and its
aggregates, reconnects when a connection ends, and feeds `field::Live`,
which keeps only records that verify (at most 256 pylons and 16,384
receipts). A valid aggregate sets the pool's job rate; one that recomputes
from the records held lights the Wellspring's rim. While `ask` waits for an
answer it leaves a mark under `<home>/inflight/` naming the pylon, never the
prompt, which Verse reads to draw the beam to Alice's station. Verse's
desktop build shows this source beside the computer's own pylon
(`VERSE_PYLON_RELAY` names another relay, or `off`); see the P1 notes in
[verse-compute](verse-compute.md#p1-presence-and-free-jobs-over-nostr).
With trusted checkers, the subscription also reads their verdicts, and a
pylon with passing checks and no failure carries a sigil: four gold motes
above its point.

## Paid jobs (P3, test sats)

Two settlement paths, both on test networks until the owner turns mainnet
on ([verse-compute](verse-compute.md#p3-paid-jobs)).

**Direct, per job, under NIP-X402.** A pylon with a price
(`provider::Config::price`, started with `Provider::priced` and an x402
`Receiver`) advertises `price_hint_msat` and the `x402-exact` settlement
profile, and sells each job through NIP-X402's native records
(`nostr:openagents:1`, private kind `3188` artifacts), never through
invoices inside job messages. The buyer (`client::Ask::pay` =
`Pay::Wallet`) buys the job before it sends it: it seals a `request` whose
input is the digest of the CJ request plaintext it will send. The pylon's
`paid::Seller` answers with a `challenge` carrying an x402 `exact`
Lightning invoice bound to that request. The buyer checks the challenge,
refuses an invoice on another network or over its ceiling, pays, and
seals a `claim` with the preimage. The seller settles the claim through
the embedded x402 facilitator and its replay store, which admits each
proof exactly once, and sends the `admitted` status. Only then does the
buyer send the CJ request, and the pylon runs it only if its plaintext
matches an admitted purchase's input; the purchase moves to `running`,
then `completed` or `failed`, and a second job for one purchase is
refused. The buyer's `3201` receipt carries `{profile, network,
amount_msat, payment_hash, preimage}`. x402 names `bitcoin` and `testnet`
only, so a priced pylon on `signet` or `regtest` refuses to start.

**Brokered.** A customer pays OpenAgents by an x402 payment or a compute
balance debit. OpenAgents' broker issues `http:1` terms on its own
receiver (`Broker::quote`) and settles the customer's proof through the
embedded x402 facilitator and its replay store before it buys anything
(`Broker::admit`). The broker key then buys the job (`Pay::Brokered` puts
the customer's x402 payment in the receipt) and `pylon::broker::Broker`
settles it in the central split ledger (`crates/pay-ledger`) under rule
v2, but only for a payment the facilitator consumed, at the amount it
consumed; a receipt alone never creates a sale. The provider (the pylon's
NIP-OA owner, else its key) gets `provider_bps` = 8,500 of the net
receipts, OpenAgents the rest, and the settlement names the receipt it
pays (`pay_ledger::pylon`, one settlement per receipt). When the job used
a priced plugin, its author's per-call fee comes first, as
`[plugin_call]` pays it (`pay_ledger::PluginFee`), and the provider's
share is of what remains; a forfeit never takes the author's fee.
`Broker::sweep` pays providers by balance sweeps through the
ordinary payout worker (`Policy::pylon_sweeps`: 1,000 sats owed, or the
oldest share ten minutes old), never per job. `Broker::forfeit` turns a
trusted checker's `check-fail` on a sold job's receipt into a forfeit of
that job's unpaid provider share; a share already swept is recorded as a
loss and is not clawed back. That late loss keeps payouts running only when
the sweep is `sent`; against a reserved, unknown, or failed sweep it holds
every payout, as any other ledger loss does. A book takes receipts of one network only, and
on `bitcoin` a sweep needs the owner's grant (`paid::Grant`), with every
payout under its per-payment and daily ceilings.

**A real wallet.** `openagents pylon serve --price-msat N` sells from this
computer's Lightning node (`openagents x402 node`, the receiver `x402
native-serve` uses), and `openagents pylon ask --max-msat N` pays through
`openagents x402`'s payer, under its policy's ceilings, allowlist, and
daily cap, recording each payment in its ledger
(`crates/openagents-cli/src/pylon_wallet.rs`). Both default to `testnet`.
On `bitcoin` both refuse before any wallet opens unless the owner's
standing grant is in `grant.json` in the pylon home (`{"per_payment_msat":
N, "daily_msat": N}`, written by the owner; no command writes it). A
priced pylon's price must be under its per-payment ceiling, and
`paid::Granted` journals every mainnet payment before it is attempted and
refuses one over either ceiling. The standalone `pylon` binary has no
wallet and refuses both.

`pylon::paid::TestLightning` (feature `fixture`) is an in-memory testnet
for fixtures that signs real BOLT11 invoices, so its proofs pass the x402
facilitator; it refuses `bitcoin`. `crates/pylon/tests/paid.rs` runs nine
direct jobs bought under NIP-X402 on three priced pylons and 1,000
brokered jobs across three pylons through the in-process relay, with
sweeps, forfeits, facilitator refusals, and ledger rows matched to
receipts. `cargo run -p pylon --features fixture --example paid_field`
runs a paid testnet field for captures.

In Verse, a pylon whose newest paid receipt finished in the last 15 seconds
shows a coin of light over its point: gold for mainnet sats, pale and
marked **TEST** for test sats. Only a receipt whose preimage hashes to its
payment hash verifies, so nothing else lights it. The pylon's world state
carries its paid msat per network, test networks apart from `bitcoin`.

## Serve the OpenAgents API

A pylon can answer requests that come in through the OpenAgents API
(`docs/inference/gateway.md`, section 4). To do that:

1. Run a free pylon (no posted price) and add the gateway's buyer key to
   its allowlist. The gateway sends ordinary conversation jobs and does not
   buy each one.
2. Register with the gateway: your pylon, the models you serve with your
   own price per million input and output tokens, and your data policy
   (whether you train on requests, whether you keep them). The gateway's
   operator adds this to its `inference.pylons` list.
3. Each model shows on the API's price list at your price plus the
   OpenAgents margin. Requests that ask that nothing be kept reach you only
   if your policy says you neither train on nor keep requests.
4. For every answer the gateway uses, you earn your price for its tokens,
   in sats, paid to your pylon's owner (or your pylon key) by the same
   payout run as other earnings, once you have a payout address.

Jobs carry text only (no tools or images), at most 32 turns and 16 KiB,
as every pylon job does. An answer that arrives after the gateway has
moved on to another provider earns nothing.

## The agent market (P4, test sats)

Agents sell services to each other and pay pylons for the compute
underneath ([verse-compute](verse-compute.md#p4-the-agent-market)).

- **Offer.** A seller publishes an immutable NIP-MKT offering (kind
  `3192`, `pylon::market::offering`) for one service: a slug, a summary
  for the Agora's wall, a price hint, the NIP-LAB labor profile, and
  `lightning-bolt11-fixed-postacceptance-v1` on one network. Its
  capability is `SELLER:pylon/agent-OFFER`, a service that runs on the
  pool. `market::listing` verifies one for the wall.
- **Hire.** The buyer and the seller negotiate privately under NIP-MKT
  with NIP-44 sealed records: `rfq`, `quote` with exact terms, `order`,
  and the seller's confirming `order_ack` (`market::Hire`). The terms are
  NIP-LAB labor terms whose closure (task frame, input prompt, one
  `answer` deliverable, the buyer as reviewer, OpenAgents' broker key as
  resolver, rights, and an all-pass acceptance policy) both sides resolve.
  Each side checks every record through the market's pure negotiation at
  its own `market::Desk`, which admits only fixed-price Lightning on the
  desk's test network; a desk refuses `bitcoin` terms.
- **Run.** Once the order is confirmed, OpenAgents' broker key buys one
  job for the order's prompt from the best fresh pylon (`market::run`)
  and signs its `3201` receipt. The order pays for the job, so the receipt
  carries no payment of its own.
- **Pay and settle.** After the buyer accepts the answer
  (`market::accept`), OpenAgents' receiver issues the order's
  earned-price invoice, bound to the order by its description hash
  (`market::instruction`), and the buyer pays it. `Broker::settle_order`
  checks the receipt, the terms, the invoice's network, amount, binding,
  and payee, the preimage, and that the receiver received the payment,
  then records one settlement under `Split::AgentOrder`: the seller's fee
  (the price less the broker's compute price) first as the `author` share,
  the provider's `[pylon_job]` share of the compute, and OpenAgents the
  rest, naming the order and the receipt (`pay_ledger::agent_order`, one
  settlement per order and per receipt). The provider's share is a pylon
  job like any other: it is swept, and a trusted checker's `check-fail`
  forfeits it while unpaid, never the seller's fee.

As the payment profile says, the seller and the pool bear credit risk
until the buyer pays; this is not escrow. NIP-LAB delivery and acceptance
records are not exchanged here: the buyer's acceptance is its local check
of the answer before it pays. `crates/pylon/tests/market.rs` runs the whole
hire on the in-process relay with `TestLightning`.

In Verse, the Agora's forecourt holds the compute counter and the
agent-services wall: the counter reads the pool's online pylons and free
slots, the day's jobs and the jobs trusted brokers bought, and the sats
paid on receipts (test sats marked **TEST**); the wall lists verified
agent-service offerings. A settlement thread runs from the counter to each
pylon a trusted broker's job finished on in the last 15 seconds: gold for
mainnet sats, pale otherwise, since an order's job carries no payment of
its own. The brokers come from `OPENAGENTS_PYLON_BROKERS` (npubs or hex,
comma-separated); OpenAgents publishes no broker key yet. The clock
tower's front clock wears a ring of light whose lit arc is the busy share
of the pool's online slots, and villagers pass on the pool's news.
`cargo run -p pylon --features fixture --example agent_market` runs a
testnet market for captures.

## Limits

- Paid jobs are tested on `TestLightning` only; the real wallet path
  (`openagents pylon serve --price-msat`, `ask --max-msat`) has not been
  run against a live wallet. Mainnet needs the owner's grant and ceilings
  (`NEEDS_OWNER.md`).
- Direct paid jobs need a network x402 names: `testnet` or `bitcoin`, not
  `signet` or `regtest`.
- The capability is a qualified ID, not a published NIP-CAP manifest.
- Redundant runs compare normalized text exactly; there is no Jev
  judgment for non-deterministic answers yet.
- The job runs in the Psionic process; it is inference only, with no tool or
  command execution, so there is nothing to put inside `coder-boundary` yet.
- Only day plans route to the pool; reflections, share drafts, and
  consolidations still use the agent's own model.
