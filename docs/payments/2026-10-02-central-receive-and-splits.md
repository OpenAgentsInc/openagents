# Central receive, splits, payouts, and the live flow view

Design, 2026-10-02. Nothing here is implemented yet. Umbrella issue
[#10200](https://github.com/OpenAgentsInc/openagents/issues/10200); the
per-piece issues are listed in [section 11](#11-issues).

## The owner's direction (verbatim, 2026-10-02)

> Yeah, so basically I don't want people setting up their own M.D.K. or X.4.2
> receives. I don't think that makes sense. Let's receive it all through our
> setup. If that's M.D.K., and we don't have to worry about liquidity, that's
> ideal, and then we can route the payments however needs to do, 'cause we
> have to think about we're gonna be doing rev share, so we gotta have some
> kind of routing of the payments that we control, sort of, according to
> splits that we figure out. There might be a little short-term bonus and
> stuff, but I want to see the full X.4.2 flow working and like see some
> actual stats moving around, and I want us to be able to visualize that in
> real time, like how we really have the stats flowing, kind of, in that
> simulation that we did on the homepage, but I want to see that real
> movement, like real plugin, real everything moving so. Figure out what we
> need to do for all that. Make any issues needed. Task out whenever.

It restates Episode 289 ([`docs/transcripts/289.md`](../transcripts/289.md),
02:00): "Lightning micropayments, where you get paid for all paid usage of
plugins that you create."

## Decisions in this document

| # | Decision |
| --- | --- |
| P1 | **One receiver: ours.** Every paid endpoint (the API, plugin invocations, author-hosted paid resources) issues its `402` from one OpenAgents-run Lightning node. Authors and providers never run a receiver; they register a payout destination. |
| P2 | **The receiver is `crates/wallet` with `--lsp mdk`**, run as a service on our infrastructure with a fresh seed. It is MoneyDevKit's `ldk-node` fork and MoneyDevKit's LSPS4 just-in-time liquidity, so there is no channel funding and no liquidity management. It is not the production `MdkTreasuryContainer` (section 1 has the evidence). |
| P3 | **A ledger owns money truth.** Every settled payment becomes one settlement row, split into shares by a versioned rule; payouts drain shares. The ledger is the only source for payouts, stats, and the live view. |
| P4 | **Payouts go to a registered destination,** a Spark address (preferred, since every user Wallet is Spark) or a Lightning address, batched per payee, from our wallets. |
| P5 | **The live view is driven by real events**: a public, privacy-safe flow stream from the ledger, drawn by the same picture as the Episode 289 `routes-future` scene, on `openagents.com/live` and as a desktop deck scene. |
| P6 | **Payouts never exceed receipts.** Bonuses are funded from OpenAgents' own share of the same call, so the ledger never promises sats it has not received. |

## 1. Which receiver: the MDK treasury, `mdkd`, or our own node

### What the x402 receiver must do

`crates/x402` asks a receiver for exactly one thing
(`crates/x402/src/server.rs:25`, trait `Receiver`):

```rust
fn invoice(&self, amount_msat: u64, request_hash: [u8; 32], expiry_secs: u32)
    -> Result<String, String>;
```

The validator that settles a proof (`crates/nostr/src/x402/invoice.rs`)
refuses an invoice with an inline description (BOLT11 tag 13 is
`InvoiceDescription` at line 156) and requires the description hash `h` to
equal the request hash, and the invoice to be signed by the `payTo` node
(`PaymentError::InvoicePayee`). `crates/wallet` implements it as
`receive_exact` (`crates/wallet/src/ldk.rs:315`): a description-hash invoice
for an exact amount, through the LSPS4 just-in-time route when the node lacks
inbound (`receive_via_lsps4_jit_channel`), signed by our own node, so the
node id stays a valid `payTo`.

### Option A: the production MDK treasury (`MdkTreasuryContainer`)

- It is an MDK `@moneydevkit/lightning-js` 0.1.84 `MdkNode` behind a small
  Bun HTTP server (issues #4698–#4700; moved to Cloud Run in #8531, commits
  `89898be350`, `2f53008161`, `ecbd1799de`). Its source left the monorepo in
  `21e82ce829` (2026-07-14, "retire money sites and wallet authority");
  the last version is
  `git show 21e82ce829~1:apps/openagents.com/services/mdk-treasury/src/server.mjs`.
- Its routes: `GET /offer` (variable-amount JIT BOLT11 plus BOLT12),
  `POST /donation-invoice` (variable-amount JIT), `GET /received/{hash}`,
  `POST /pay`, `GET /payments/{id}`, plus Spark funding routes.
- **It cannot issue x402 invoices.** The lightning-js API
  (`projects/moneydevkit/repos/lightning-js/index.d.ts`, rev `b37cf05`) offers
  `getInvoice(amount, description: string, expirySecs)` and the
  variable-amount JIT variants. Every one takes an inline description string
  and none takes a description hash, so every invoice it issues fails the x402
  description check. Variable-amount invoices also fail the exact-amount check.
- Received payments are kept in an in-memory `Map` "for container lifetime
  only" (server.mjs, `receivedPayments`), so a restart loses which payments
  arrived. Payout dispatch is behind a flag that is off
  ([bitcoin history](../bitcoin/2026-09-28-bitcoin-node-history.md#june-to-july-2026-mdk-treasury)).
- Its mnemonic is the campaign treasury's; no agent may run a second node on
  it while the container is live.

Verdict: **not the receiver.** Leave it running as it is; it is out of this
design's path. Moving its balance later is an owner decision, not a
dependency.

### Option B: MoneyDevKit's own daemon, `mdkd`

`projects/moneydevkit/repos/mdkd` (rev `491460e`) is MDK's Rust payment
server on their `ldk-node` fork. It does what lightning-js does not:
`POST /createinvoice` takes `descriptionHash` (`src/daemon/api/invoices.rs`),
it receives through LSPS4 (`src/mdk/node.rs:172`), it signs webhooks with
HMAC, keeps invoice metadata in SQLite and channel state in VSS, and
`POST /pay` pays BOLT11, BOLT12, LNURL, and BIP 353 destinations
(`bitcoin-payment-instructions`). Limits: amounts are whole sats
(`amount_sat`), it needs an MDK platform access token, and it would be a
second wallet implementation beside `crates/wallet`, reached over HTTP.

### Option C: `crates/wallet` with `--lsp mdk` (recommended)

- The same MDK `ldk-node` fork (0.7.0 plus the LSPS4 client, commit
  `3086f8557d`, issue #9829) and the same MDK LSP
  (`crates/wallet/src/config.rs:126`, `MDK_LSPS4_MAINNET`, taken from
  `mdkd`'s `NetworkInfra`).
- `receive_exact` already produces x402-exact invoices, `lookup` resolves a
  payment by hash in either direction, `pay` pays BOLT11 with a fee cap and is
  idempotent per invoice (`crates/wallet/src/lib.rs`).
- It is already the x402 receiver in `openagents x402 serve`
  (`crates/openagents-cli/src/x402.rs:240`) and the API design's chosen
  receiver ([API, section 5.1](../api/2026-10-02-openagents-api.md#51-receiving-payments)).
- No funding step: the first payment opens the channel. Outbound liquidity
  for payouts is what we have received.

**Recommendation: option C.** It is "MDK, and we don't have to worry about
liquidity": MDK's node code and MDK's LSP, with no inbound to buy. It needs no
new wallet code, keeps one Rust wallet across the CLI, the hosts, and the
receiver, and works with msat prices. `mdkd` stays the fallback if the
in-process wallet proves fragile as a long-running service; switching means
one `Receiver` implementation over its HTTP API, since the description-hash
call exists there.

### What is not yet proven

- **Mainnet LSPS4 receive.** Issue #9832 (prove a fresh `openagents wallet`
  receives an x402 payment on mainnet through MDK's LSPS4 channel) was closed
  as not planned for now. Against the staging LSP on Mutinynet the first
  channel took about 60 s and the LSP failed the first payment back after its
  45 s hold ([CLI, inbound liquidity](../cli/README.md#inbound-liquidity-from-an-lsp)).
  The receiver issue (R1) runs this check as part of bring-up and pre-opens
  the channel with one small payment before any public traffic.
- **LSP fee.** The LSPS4 provider takes about 2% of the forwarded amount on a
  just-in-time open (#9832). Splits are computed on the amount actually
  received, and the LSP fee is charged to OpenAgents' share, never to an
  author's fee.

## 2. Architecture

```text
 callers (curl + any LN wallet, `openagents x402 fetch`, lnget / Payment-scheme
          clients, prepaid oak_ keys)
     │  HTTPS
     ▼
 api.openagents.com  ── pay front (`openagents pay serve`, one process) ─────────────┐
 │  route table: /v1/messages, /v1/plugins/{id}/invoke, /x/{resource} (author proxy) │
 │  crates/x402: 402 challenge (x402 v2 + Payment scheme), proof check,             │
 │  one replay store, settle-before-execute                                         │
 │      │ receive_exact / lookup / pay                                              │
 │      ▼                                                                           │
 │  receiver wallet: crates/wallet, --lsp mdk (LSPS4 JIT from MoneyDevKit)          │
 │      │ settled payment (payment_hash, amount received)                           │
 │      ▼                                                                           │
 │  ledger (crates/pay-ledger, SQLite on durable disk, single writer)               │
 │    settlements ─► shares (rule vN) ─► accruals per payee ─► payouts              │
 │      │                                   │                                       │
 │      │ flow events (public, anonymized)  │ payout worker                         │
 │      ▼                                   ▼                                       │
 │  /flow/stream (SSE), /flow/snapshot,     Lightning address: LNURL-pay → BOLT11   │
 │  /stats (JSON)                            → receiver wallet `pay`                 │
 │                                          Spark address: payout Spark wallet       │
 │                                           (Breez SDK), refilled from the receiver │
 └──────────────────────────────────────────────────────────────────────────────────┘
     │ proxied over the private network
     ▼
 openagents.com/live (canvas), openagents.com/stats, desktop deck `routes-live`
```

Where it runs: one Debian host with the same deploy shape as the relay and
chat worker ([`deploy/README.md`](../../deploy/README.md)): a hardened
systemd unit (`deploy/systemd/openagents-pay.service`), an environment file
under `/etc` that names credential files and holds none, Caddy for TLS, and
the backup timer pattern in `deploy/backup/`. The wallet seed lives in Secret
Manager and is written to a root-only file at start. One process owns the
wallet, the replay store, and the ledger, because the x402 receiver requires
exclusive invoice authority and one replay store per `payTo`
([API 5.1](../api/2026-10-02-openagents-api.md#51-receiving-payments)).

## 3. Who gets paid

| Party | Earns | Source |
| --- | --- | --- |
| **Plugin author** | The plugin's whole per-call fee, every paid call that used the plugin (per-call x402 or a prepaid balance debit). | API decisions D7 and D9; Episode 289. The fee and payout destination ride in the signed EXT release (API gap G9: `fee_msat`, `payout`). |
| **OpenAgents** | The endpoint price (model, routing, hosting), minus the LSP fee, minus any bonus it funds. | API section 5.5. |
| **Author of a hosted paid resource** (an HTTP service an author runs, sold through our receiver) | The resource's price, minus the OpenAgents take set in the rule. | Replaces the author running `openagents x402 serve` with their own wallet (#9791 advertised such resources). |
| **Computer provider** | Nothing yet. API D4 runs Coder only on computers the caller's own user granted, so the provider is the payer. The rule reserves a `provider` role for runs on someone else's computer under [NIP-LAB](../../nips/openagents/NIP-LAB.md), at a later rule version. | API D4. |
| **XP** | Never sats. XP is evidence of accepted work and never converts ([NIP-XP](../../nips/openagents/NIP-XP.md)). The live view may show XP awards beside payments, as `routes-plugin` does, but the ledger never reads them. | NIP-XP. |

### Short-term bonus (launch bonus, rule `v1`)

- **First paid call bonus:** when an author's plugin earns its first paid
  call, the author gets a one-time bonus (default 1,000 sats).
- **Launch match:** for 60 days after rule `v1` takes effect, each paid call
  adds a bonus equal to 100% of the plugin fee, capped at 50,000 sats per
  author per calendar month.
- **Funding rule (P6):** a bonus share for a call can never exceed
  OpenAgents' share of that same call; whatever the cap leaves unpaid is
  dropped, not owed. The first-call bonus is drawn from OpenAgents' accrued,
  unpaid share, and is skipped (recorded as `bonus_unfunded`) when that is
  short.
- The numbers are defaults in the rule file; changing them is a new rule
  version, never an edit.

## 4. The ledger and split rules

### Tables (SQLite, one writer)

| Table | Key | Holds |
| --- | --- | --- |
| `settlement` | `payment_hash` (or `debit:{id}` for a balance debit) | resource, plugin id, price_msat, received_msat, lsp_fee_msat, rail (`lightning`, `balance`), payer alias, settled_at, rule_version |
| `share` | (`settlement`, `party`, `role`) | amount_msat, role (`author`, `resource`, `openagents`, `bonus`, `lsp_fee`, `provider`) |
| `payee` | party id (author npub, account id) | destination kind and value, where it came from (release, NIP-A3, profile, account), verified_at |
| `payout` | payout id | party, amount_msat, destination, rail, state, wallet reference (payment hash or Spark transfer id), attempts, times |
| `payout_item` | (`payout`, `share`) | which shares a payout drained |
| `balance` | account id | prepaid sats (D13): top-ups are settlements with role `balance_credit`; debits are settlements with rail `balance` |
| `rule` | version | the rule file's digest and effective time |

### Invariants (tested, and checked by reconciliation)

1. A settlement is written once per payment hash, before the purchased work
   runs (the x402 upfront flow settles before execution). A replayed proof
   finds the existing row and writes nothing.
2. For every settlement, the shares sum exactly to `received_msat`. Integer
   msat arithmetic; any remainder goes to `openagents`.
3. Shares are computed by the rule version in force at `settled_at`, and the
   version is stored on the row. A new rule never rewrites old shares.
4. A share is drained by at most one payout that did not fail.
5. Total paid out plus total accrued never exceeds total received (P6).
6. Every Lightning settlement's payment hash is a succeeded inbound payment
   in the receiver wallet (`lookup`), and its received amount matches.

### Rules

Rules are TOML files in the repository (`crates/pay-ledger/rules/v1.toml`),
loaded by digest, so a change is a reviewed commit:

```toml
version = 1
effective = "2026-10-15T00:00:00Z"

[plugin_call]            # POST /v1/plugins/{id}/invoke, or a plugin offer in a message
author = "fee"           # the whole declared fee (D9)
openagents = "rest"      # endpoint price, after the LSP fee

[hosted_resource]        # an author's HTTP resource sold through our receiver
resource_owner_bps = 9000
openagents = "rest"

[bonus]
first_paid_call_msat = 1_000_000
launch_match_bps = 10000
launch_match_until = "2026-12-14T00:00:00Z"
launch_match_cap_msat_per_month = 50_000_000
funded_by = "openagents_share"
```

### Reconciliation

A job every 10 minutes and a daily report compare the ledger with the
wallets: received (ledger) against inbound succeeded (receiver wallet),
paid out against outbound succeeded (both wallets), and holdings (Lightning
plus Spark balances) against accrued, unpaid shares plus OpenAgents' share.
A drift, a settlement with no wallet match, or a payout stuck in `unknown`
raises an alert line on the pay host's log and appears on `/stats` as
"reconciliation: drift" until resolved.

## 5. Payout destinations and payouts

### Where a payee's destination comes from (first match wins)

1. **The signed EXT release** for a plugin: `payout` (G9). Extend G9's value
   to accept a Spark address (`spark1…`) as well as a Lightning address or
   node key. Signed by the author, so it is pinned with the release.
2. **The author's NIP-A3 payment target**, kind `10133`, tag
   `["payto","spark","spark1…"]`, the target the phone's **Publish my Spark
   address** switch writes ([paying people](../breez/paying-people.md)).
3. **`lud16`** in the author's newest kind-0 profile.
4. **An API account's payout setting** (`PUT /v1/account/payout`), for hosted
   resource owners without a Nostr identity.
5. None: shares accrue and wait. The author's plugin page and `/stats` say
   "earning, no payout address yet".

Only signed, verified events count, as on the phone.

### How payouts go out

- **Batched per payee.** Shares accrue; a payout goes out when a payee's
  accrued amount reaches a threshold (default 100 sats for Spark, 1,000 sats
  for Lightning addresses, where routing fees bite) or once a day for anything
  above 1 sat. Past failures came from a single hot wallet paying many
  recipients ([Breez history, lesson 3](../breez/history.md)); batching keeps
  the send rate low.
- **Lightning address rail:** resolve LNURL-pay (the current MDK `ldk-node`
  fork in `projects/moneydevkit/repos/ldk-node` has `resolve_lnurl_to_invoice`
  in `src/types.rs`; check the revision `crates/wallet` pins, or reuse the
  phone's resolver in `crates/openagents-mobile/src/payees.rs`), check the
  invoice amount, then `pay` from the receiver wallet with a fee cap.
- **Spark rail:** a payout Spark wallet on the pay host (Breez SDK 0.26, as
  `crates/openagents-mobile/src/spark.rs` already uses), with its own seed.
  The payout worker keeps it topped up by paying a BOLT11 invoice it issues
  from the receiver wallet, then sends free Spark-to-Spark transfers. Spark
  stays off the x402 critical path (it cannot be the receiver,
  [Breez history](../breez/history.md)); it only pays out.
- **Restart-safe state machine:** `planned` → `sending` (the wallet reference
  is recorded before the send) → `sent` or `failed`, and `unknown` on a crash
  mid-send. An `unknown` payout is resolved only by looking up its recorded
  reference in the wallet, never by sending again.
- **Fees:** Lightning routing fees on a payout come out of OpenAgents' share
  for amounts at or above the threshold.

## 6. Paid endpoints on the central receiver

- **API endpoints** (API phase 1): the pay front serves the priced `/v1`
  routes. One route table maps a path to a price, an executor, and a split
  role. This is `openagents pay serve --routes FILE` (`crates/x402::front`,
  #10186): one wallet, one replay store, x402 and the HTTP `Payment` scheme
  on one invoice, and a `SettlementSink` hook called with each
  `Settlement` (payment hash, request hash, route, resource, role, plugin,
  price, received msat, scheme, time) before the route runs. Until
  `crates/pay-ledger` implements the sink, settlements go to an NDJSON log
  the ledger can import.
- **Plugin invocations** (API phase 2): `POST /v1/plugins/{id}/invoke`
  prices `endpoint + fee_msat` from the pinned release, runs the plugin's
  packet through `crates/plugin` (`invoke_with_receipt`), and writes the
  settlement with the plugin id and author before running it. Shipped in
  #10193 as a `registry` route of `openagents pay serve`
  (`crates/openagents-cli/src/pay_plugin.rs`): the quote pins the release,
  the settlement carries plugin, release, author, and fee, the ledger sink
  splits the fee to the author, and every call that reaches a route is a
  `call` record in the ledger. Only a program of one guest step that
  requires nothing is sold; a `snapshot-read` guest runs with an empty
  snapshot.
- **Author-hosted resources:** an author who has an HTTP service registers
  it (`openagents x402 serve --central` or `openagents x402 publish`), and
  we serve it at `https://api.openagents.com/x/{resource}`: our `402`, our
  invoice, our replay store, then a forward to the author's upstream with a
  signed header that says the call is paid. The author's process holds no
  wallet. `openagents x402 advertise` publishes the central URL and our
  `payTo`. Running one's own receiver stays possible for self-hosters (the
  protocol is open) but is no longer what our tools set up.

## 7. Stats and the live view

### The simulation it replaces

The Episode 289 deck shows two scenes
([`episode-289.md`](../../crates/openagents-deck/decks/episode-289.md),
lines 29 and 38):

- `routes-future` (`crates/openagents-desktop/src/route_future.rs`): a
  synthetic model grown month by month from October 2026 to December 2030
  over today's committed route map (`Map::committed()`,
  `crates/openagents-chat-app/src/route_map.rs:648`). Dots come from
  `RouteFuture::traffic()` (line 262): seeded slots pick a leaf; a white
  request dot travels out along `path_to(leaf)`, and a gold payment dot
  travels back when the leaf is a plugin, engine, or knowledge node.
- `routes-plugin` (`route_plugin.rs`): a scripted author earns XP and paid
  uses (`uses()`, line 429; `traffic()`, line 445).

Both draw through `MapPage::presenting` and `set_frame(layout, shown,
traffic: Vec<Pulse>, camera)` (`route_map.rs:239`, `:259`). The website has
no canvas today (`crates/openagents-web` serves one script, `static/ask.js`,
under `script-src 'self'`) and no stream or stats routes.

### The flow event (`openagents.flow-event.v1`)

One JSON object per event, public by design:

```json
{"v":1,"seq":1042,"at":1791043200123,"type":"payment",
 "resource":"plugin","plugin":"explain-error","node":"plugin:explain-error",
 "amount_sats":31,"rail":"lightning",
 "split":{"author":10,"openagents":21},
 "author":"npub1…","payer":"caller-7f3a"}
```

| `type` | When | Dot |
| --- | --- | --- |
| `call` | A call reached a route (free or paid) | white, out |
| `payment` | A settlement was written | gold, back to the hub |
| `share` | The split of that payment | gold, hub to author |
| `payout` | A payout was sent | gold, hub to the author's wallet |
| `bonus` | A bonus share | gold with a ring |
| `run` | A Coder run started or finished (from run cost records, #10161) | white |

Privacy rules: payers are an alias from a salted hash that rotates daily,
never a key, IP, or account; no request hash, payment hash, invoice,
message text, or destination string appears; authors appear by npub only
when they published the plugin under it (public already), otherwise as
`author-xxxx`; amounts are exact sats. Free calls carry no amount.

### Transport

- `GET /flow/stream`: server-sent events from the ledger's sequence, with
  `Last-Event-ID` resume.
- `GET /flow/snapshot`: the last 500 events, totals, and the topology (nodes
  with ids, kinds, parents, and positions computed by the same
  `route_map` layout code the desktop uses, so web and desktop agree).
- `GET /stats`: totals received and paid out, pending accruals, calls and
  earnings per plugin, payouts per author, 24-hour and 30-day series, and the
  reconciliation state.
- `openagents.com` proxies `/api/flow/*` and `/api/stats` to the pay host, so
  the page stays same-origin under its CSP.
- Later, not needed for the demo: an hourly signed digest on Nostr so anyone
  can check the totals (a new kind, proposed `3200`, which the registry shows
  as free).

The Rust `pay-host` binary projects a read-only `pay-ledger` database
(`OPENAGENTS_PAY_SOURCE_DB`) into a separate `OPENAGENTS_PAY_FLOW_DB`.
Set `OPENAGENTS_PAY_FLOW_SALT` to a stable 32-byte hex salt and optionally
`OPENAGENTS_PAY_LISTEN` (default `127.0.0.1:4400`).
`OPENAGENTS_PAY_PUBLICATIONS` reads signed public NIP-EXT listings or releases
as JSONL. The ledger's call records supply paid and unpaid requests, including
402 challenges. `OPENAGENTS_PAY_ROUTE_JOURNAL` optionally reads a route journal
file or directory for completed runs. Dollar run costs never become sats.
Amounts preserve exact msat as decimal sats, and reconciliation
stays `unknown` until a reconciler updates it. Configure the website with
`--pay-host http://HOST:PORT` or `OPENAGENTS_WEB_PAY_HOST`.

### Surfaces

- **`openagents.com/live`:** a canvas page (`static/flow.js`, same-origin)
  that draws the snapshot's topology and animates each event as the deck
  does: white out, gold back, gold on to the author. A ticker of totals sits
  under it. With no traffic it shows the last real events, never synthetic
  ones, and says when the last event was.
- **`openagents.com/stats`:** the numbers as tables and small series charts,
  linked from `/live`.
- **Desktop deck scene `routes-live`:** `RouteFuture`'s renderer fed by the
  stream instead of `traffic()`'s hash slots: each event maps to a leaf by
  node id, and its pulses run along `path_to(leaf)`. Episode decks can show
  `routes-future` (the projection) and `routes-live` (today) side by side.

Status (2026-10-02): `/live` (`crates/openagents-web/src/pages/live.rs`,
`static/flow.js`, #10197) and the deck scene `routes-live`
(`crates/openagents-desktop/src/route_live.rs`, the Episode 289 deck's
sixth slide, #10198) are on `main`, built against the schema above and the
fixture `docs/payments/fixtures/flow-stream.jsonl`. Both map events to dots
by the same table (tested in `route_live.rs` and `static/flow.test.js`),
resolve a short plugin node (`plugin:explain-error`) to the map's
`plugin:crates/plugin-explain-error`, and say the stream is unreachable
until `/api/flow/*` answers (#10195). The desktop reads
`https://openagents.com/api/flow` unless `OPENAGENTS_FLOW_URL` names another
base or `file:PATH` to replay a fixture.

## 8. End-to-end demo

The first real flow, in order:

1. The pay host is up on mainnet; one small payment has pre-opened the LSPS4
   channel (R1).
2. A real plugin, **Explain this error** (`crates/plugin-explain-error`),
   is published with `fee_msat = 10000` (10 sats) and the author's Spark
   address as `payout`.
3. A client calls `POST https://api.openagents.com/v1/plugins/explain-error/invoke`
   with no key and gets a `402` priced endpoint + 10 sats.
4. It pays and retries, twice: once with
   `openagents x402 fetch … --max-msat …` and once with an HTTP `Payment`
   scheme client (`lnget`), each funded by a real wallet.
5. The ledger shows one settlement per call, split `author: 10 sats`,
   `openagents: the rest minus the LSP fee`, plus the launch bonus.
6. The payout worker sends the author's accrual to their Spark address; the
   author's phone Wallet shows it arrive.
7. `openagents.com/live` shows the white dot out, the gold dot back, and the
   gold dot on to the author, within two seconds of each step; `/stats`
   totals tick; the desktop `routes-live` scene shows the same.
8. Reconciliation reports no drift.

The paying wallet and the author's phone are the owner's (real money), so
steps 4 and 6 are owner taps; everything else is agent work.

## 9. Risks and facts

- **Custody (fact list, no review, per the owner):** OpenAgents receives
  every payment and holds authors' accrued shares until payout, in a hot
  Lightning node and a hot Spark wallet. Authors are owed sats between
  settlement and payout. Spark balances depend on Spark's operators
  ([Breez decisions](../breez/README.md)). The owner has said no regulatory
  review (Breez decision 4); this list records the facts only.
- **Hot wallet loss:** a host compromise exposes both seeds. Mitigations:
  seeds only in Secret Manager and a root-only file, low balances by paying
  out daily, a sweep of OpenAgents' share to cold storage above a ceiling.
- **First-payment failure** on the just-in-time open (section 1): pre-open
  the channel; clients retry with a fresh challenge.
- **Channel capacity:** an LSPS4 channel is sized by the LSP; heavy traffic
  opens more channels, each with the LSP's fee.
- **Single host, single writer:** the ledger and replay store are on one
  durable disk with backups; a restore must not reuse an older replay store
  (it would accept a replayed proof).
- **Earlier failures:** every previous Lightning node died of liquidity or
  host operations ([bitcoin history](../bitcoin/2026-09-28-bitcoin-node-history.md)),
  and the Spark treasury died of payout load. This design buys liquidity
  from the LSP, batches payouts, and keeps Spark off the receive path.

## 10. Phases

| Phase | Delivers | Issues |
| --- | --- | --- |
| 1. Receive | The pay host, the receiver wallet on mainnet, the multi-route x402 front, the ledger with rule `v1`. | R1, R2, R3 |
| 2. Pay out | Fee and payout fields in releases, destination resolution, the payout worker on both rails, reconciliation, the launch bonus. | R4, R5, R6, R7, R8 |
| 3. Sell | Paid plugin invocation, author-hosted resources through the central receiver. | R9, R10 |
| 4. Show | The flow stream, `/stats`, `/live`, the `routes-live` deck scene. | R11, R12, R13, R14 |
| 5. Prove | The end-to-end demo with real money. | R15 |

## 11. Issues

Created in dependency order. Umbrella: [#10200](https://github.com/OpenAgentsInc/openagents/issues/10200). Coder-sized issues carry the `coder-sized` label (`openagents chat work --issues coder-sized`).

| Id | Issue | Piece | Coder-sized |
| --- | --- | --- | --- |
| R1 | [#10185](https://github.com/OpenAgentsInc/openagents/issues/10185) | Central receiver: pay host, `crates/wallet --lsp mdk` on mainnet, seed in Secret Manager, systemd unit, backups, LSPS4 first-receive check | No (operations) |
| R2 | [#10186](https://github.com/OpenAgentsInc/openagents/issues/10186) | Multi-route x402 front `openagents pay serve`: one wallet, one replay store, route table, settlement hook | No |
| R3 | [#10187](https://github.com/OpenAgentsInc/openagents/issues/10187) | `crates/pay-ledger`: settlements, shares, versioned rules, invariants | Yes |
| R4 | [#10188](https://github.com/OpenAgentsInc/openagents/issues/10188) | NIP-EXT G9: `fee_msat` and `payout` (Spark address, Lightning address, node key) in releases | Yes |
| R5 | [#10189](https://github.com/OpenAgentsInc/openagents/issues/10189) | Payout destination resolver | Yes |
| R6 | [#10190](https://github.com/OpenAgentsInc/openagents/issues/10190) | Payout worker: batching, Lightning-address and Spark rails, restart-safe states | No |
| R7 | [#10191](https://github.com/OpenAgentsInc/openagents/issues/10191) | Reconciliation job and report | Yes |
| R8 | [#10192](https://github.com/OpenAgentsInc/openagents/issues/10192) | Launch bonus (rule `v1` bonus section) | Yes |
| R9 | [#10193](https://github.com/OpenAgentsInc/openagents/issues/10193) | Paid plugin invocation `POST /v1/plugins/{id}/invoke` | No |
| R10 | [#10194](https://github.com/OpenAgentsInc/openagents/issues/10194) | Author-hosted resources through the central receiver | No |
| R11 | [#10195](https://github.com/OpenAgentsInc/openagents/issues/10195) | Flow event stream and stats JSON | Yes |
| R12 | [#10196](https://github.com/OpenAgentsInc/openagents/issues/10196) | `openagents.com/stats` page | Yes |
| R13 | [#10197](https://github.com/OpenAgentsInc/openagents/issues/10197) | `openagents.com/live` canvas view | No |
| R14 | [#10198](https://github.com/OpenAgentsInc/openagents/issues/10198) | Desktop deck scene `routes-live` | No |
| R15 | [#10199](https://github.com/OpenAgentsInc/openagents/issues/10199) | End-to-end demo with real money | No (owner taps) |

## Related

- [The OpenAgents API](../api/2026-10-02-openagents-api.md): D3, D7, D9,
  D12, D13, section 5, gap G9.
- [PayPerQ research](../research/ppq.md): a hosted Lightning 402 API's
  pricing and balances.
- [NIP-X402](../../nips/openagents/NIP-X402.md),
  [`crates/x402`](../../crates/x402), [`crates/wallet`](../../crates/wallet).
- [Breez and Spark](../breez/README.md), [paying people](../breez/paying-people.md).
- [Bitcoin and Lightning node history](../bitcoin/2026-09-28-bitcoin-node-history.md).
