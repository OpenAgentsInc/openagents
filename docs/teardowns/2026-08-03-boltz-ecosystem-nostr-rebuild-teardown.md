# Boltz Ecosystem Teardown — non-custodial Bitcoin layer swaps, and a Nostr rebuild

- Date: 2026-08-03
- Lane: Fast Follow research / product teardown
- Disposition: high-relevance **liquidity and atomic-swap** reference; recommended
  **native multi-provider swap marketplace** design using OpenAgents NIP drafts and
  official Nostr payment NIPs — **not** implementation, deploy, or product authority
- Local mirror: `~/work/projects/repos/boltz/` (31 public `BoltzExchange` clones)
- OpenAgents tree pin: `086dc611bb0cf12258b305007f6f66c8efa052a1`

Read-only architecture and product audit of the public Boltz Exchange
organization at commit-pinned local clones. Nothing in those clones was
modified. The audit did not install dependencies, start `boltzd`, open
channels, broadcast transactions, call production `api.boltz.exchange`, or
move funds. [source] [limitation]

## Summary

Boltz is a **non-custodial atomic swap service** between Bitcoin layers. A
user (or wallet, or merchant plugin) swaps chain Bitcoin, Liquid L-BTC,
selected EVM assets, Arkade VTXOs, and Lightning without giving Boltz custody
of both sides at once. The atomicity hinge is the classic HTLC / preimage
law: whoever learns the preimage can claim the locked funds; whoever times
out can refund. Modern Taproot swaps add **cooperative MuSig2 key-path
claims** so the happy path is a single-sig lookalike, with script-path
fallback when a party does not cooperate. [source] [public]

```text
  Wallet / Web App / BTCPay plugin / boltz-client
              |  REST + WebSocket (do not hand-roll)
              v
        Boltz Backend (AGPL)  ---- gRPC/HTTP ----  CLN + LND + hold
              |                                         |
         boltz-core (MIT)                          chain watchers
         swap trees · MuSig2 · claim/refund        Electrum · Elements
         EtherSwap / ERC20Swap ABIs                EVM · Ark nursery
              |
         PostgreSQL · rates · nurseries · sidecar (Rust boltzr)
```

The product is not "a Lightning node for hire." It is **liquidity as a
protocol surface**: create a swap, lock coins under a verifiable script or
contract, observe status, claim or refund with cryptography the client can
check. Official docs insist integrations use SDKs (`boltz-client`, Breez
SDK Liquid, `boltz-core`, boltz-rust) rather than the raw REST API, because
fund-loss lives in the edge cases, not the happy path. [source]

The OpenAgents decision is not "run Boltz" and not "fork the AGPL backend
into Cloud Run." The useful port is the **economic and cryptographic shape**:

1. **Non-custodial bridge between rails** (on-chain, Lightning, Liquid-class
   sidechains, later Cashu/Fedimint) as first-class product capability for
   agents, merchants, and owners.
2. **Client-verified scripts and invoices** — "Don't trust. Verify." is the
   product contract, not marketing.
3. **Explicit lifecycle states** with cooperative and non-cooperative exits.
4. **Multi-provider discovery** instead of a single HTTP operator: Nostr
   events for capacity, pairs, fees, and public-safe status; private
   channels for swap parameters that must stay off public relays.
5. **Settlement and receipts stay off the relay authority path** — same law
   already stated in NIP-LBR, NIP-AC, NIP-OC, and NIP-EV.

A shortest accurate statement:

> Rebuild Boltz-shaped non-custodial layer swaps as an OpenAgents-native
> marketplace of liquidity providers. Keep HTLC/Taproot/preimage physics
> client-side (prefer MIT crypto libraries or independent Effect ports). Use
> Nostr for identity, discovery, private negotiation, payment references, and
> evidence pointers. Do not put preimages, refund keys, invoices, or wallet
> material in public events. Do not adopt the AGPL backend as a dependency or
> a second payment authority.

## 1. Snapshot, provenance, and limits

### 1.1 Exact source identity (primary pins)

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Org | `https://github.com/boltzexchange` | Public ecosystem home |
| Local mirror root | `~/work/projects/repos/boltz/` | Audited trees |
| `boltz-backend` | `4d131ef8562eea25ab687bcc75a17ce899110b66` (2026-07-27, v3.13.0, AGPL-3.0) | Production swap operator, REST/WS API, nurseries, rates |
| `boltz-core` | `a932d49c4daaeae3d7940dc1519bf77ef92e6dc1` (2026-07-24, v5.0.0, MIT) | Reference swap scripts, Taproot trees, MuSig2, EVM ABIs |
| `boltz-client` | `746f73c5ecbd3621f628f60108a404ef26f0de95` (2026-07-27, MIT) | Go daemon + gRPC: autoswap, rebalance, nodeless LN receive |
| `boltz-web-app` | `dd9c2df26db54a2554dc1e628b095ce856c0d9de` (2026-07-30, v2.2.1, AGPL-3.0) | Public UI at boltz.exchange |
| `hold` | `14c3568d2b9be7af23df69a4dc579dd198428f1d` (2026-04-30, MIT) | CLN hold-invoice plugin (Rust) for reverse swaps |
| `docs` (org docs site) | `0d7cb95ac48742f05a097ab47df44c64bbdc519c` | VitePress docs home |
| Hosted API | `https://api.boltz.exchange/` (+ Tor onion) | Live mainnet operator surface |
| Product site | `https://boltz.exchange/` | End-user swap UI |

Selected content digests at those commits:

| File | SHA-256 |
| --- | --- |
| `boltz-backend/README.md` | `7cba9dd25c78568b00db8d10d178fe91bed21c047d2fade590658d772b6cf24c` |
| `boltz-backend/docs/lifecycle.md` | `5145ad7c6c13767b92ec1522909182d01d8c868171da2232d03e93663cd75d76` |
| `boltz-backend/docs/dont-trust-verify.md` | `5c5db4f45750af657c9d62987ee2ee8dc203ea6e947b6928a5cad39e782f7003` |
| `boltz-backend/docs/api-v2.md` | `3cbcf4298d8b3fbece10d1376ae1103b6fd2bdd0d71a90aff3af7ce8bbe7b0ed` |
| `boltz-core/README.md` | `72a843899c1a846bb8a06a8cef938a0465fb7726946a48512bf88ef1fcf08f08` |
| `boltz-client/README.md` | `7f0220f922e32a0754d42aa249ff64c7cef678e3d1f99dbcd8a7ea279337b4f0` |
| `boltz-web-app/README.md` | `ad1fe960d99efa8cbcf065369e58d48f9e8b318447d043730c576aafa92cccdf` |
| `hold/README.md` | `af5c6725eed6c1feadd944bb74993ef2374920dbf30ff0bf0bad56d489ade24a` |

### 1.2 Full org inventory (31 public repos, 2026-08-03)

Grouped by role. Every row was present under the local mirror after a fresh
shallow clone of the org listing.

| Role | Repos |
| --- | --- |
| **Operator core** | `boltz-backend`, `boltz-middleware` (older reference), `docs` |
| **Crypto / scripts** | `boltz-core` (MIT TS), `boltz-core-angular` (historical bitcoinjs fork), `bitcoin-ops`, `bolt11`, `bolt12-wasm`, `covclaim` (claim covenants) |
| **Clients** | `boltz-client` (Go), `boltz-python`, `boltz-web-app`, `boltz-frontend` (legacy), `boltz-demo` |
| **Node plugins / ops** | `hold` (CLN hold invoices), `channel-bot`, `channel-creation-plugin`, `cln-backup`, `canary` |
| **Merchant / product** | `btcpay-plugin-liquid`, `btcpay-plugin-arkade`, `boltz-partner-dashboard`, `arkade-landing`, `maintenance-page` |
| **Chain / infra libs** | `go-electrum`, `regtest`, `legend-regtest-enviroment`, `proton-bridge` (fork surface), `umbrel-apps`, `logo`, `slides` |

Scale signals (source file counts, excluding `node_modules` / `target` /
`.git`): backend ~647 TS files plus multi-crate Rust (`boltzr`, cache, EVM,
utils); `boltz-core` ~82 TS + Solidity contracts; `boltz-client` ~183 Go
files; web app ~600 TS/TSX files; `hold` ~45 Rust files. [source]

### 1.3 Evidence labels

- **`[source]`** — tracked source or docs at a pinned commit above
- **`[public]`** — hosted docs or product pages cited without local clone proof
- **`[history]`** — Git history on a pinned tree
- **`[inferred]`** — reasoned from several observations
- **`[limitation]`** — what this audit cannot prove

### 1.4 Audit limits

This audit is source-and-docs only. It does not prove production liquidity
depth, fee schedule fairness, 0-conf policy under live miner behavior,
Tor endpoint availability, partner-program payouts, cryptographic security of
MuSig2 under adversarial clients, or that every listed repo is still in
production use. `boltz-middleware` and `boltz-frontend` look historical
relative to backend + web-app. Live mainnet was not exercised. [limitation]

## 2. What Boltz is

### 2.1 Product thesis

Boltz sells **trust-minimized conversion between Bitcoin representations**.
The user never deposits into a hot wallet that promises a later withdrawal.
Instead:

- One side locks under a hash-locked (and time-locked) condition the client
  can re-derive.
- The other side pays Lightning or locks the destination chain under the
  **same** preimage hash.
- Revealing the preimage completes both sides; timeout refunds the locker.

That is Submarine Swaps in the original sense (chain ↔ Lightning), plus
chain↔chain and newer rails (Liquid, EVM tokens, Arkade). [source] [public]

### 2.2 Swap types and state machines

From `boltz-backend/docs/lifecycle.md` at the backend pin:

#### Normal Submarine (Chain → Lightning)

1. Client (often) supplies a BOLT11 invoice and a refund public key.
2. Server returns a lockup address / Taproot swap tree the client must
   verify.
3. Client locks chain coins.
4. Server pays the Lightning invoice; learns the preimage from settlement.
5. Server claims the lockup (cooperative MuSig2 key-path preferred;
   script-path fallback).
6. Failure paths: `invoice.failedToPay`, `transaction.lockupFailed`,
   `swap.expired` → client refunds with cooperative signature when offered,
   else script-path after timeout.

Important status vocabulary: `swap.created`, `invoice.set`,
`transaction.mempool`, `transaction.confirmed`, `invoice.pending`,
`invoice.paid`, `transaction.claim.pending`, `transaction.claimed`. [source]

#### Reverse Submarine (Lightning → Chain)

1. **Client generates the preimage**, sends only the hash to the server.
2. Server creates a **hold invoice** (CLN `hold` plugin is first-class).
3. Client pays Lightning; HTLC hangs until preimage is known.
4. Server locks chain coins under the same hash.
5. Client claims chain (cooperative claim) and reveals preimage to server so
   the invoice can settle.
6. If the server cannot lock: cancel HTLC, user keeps Lightning funds, no
   fee. If the user never claims: server refunds its lockup after expiry
   (`transaction.refunded`). [source]

#### Chain Swaps (Chain → Chain)

Both legs on-chain (e.g. BTC ↔ L-BTC). Same preimage law, dual lockups,
Taproot-only cooperative claims, plus **renegotiation** when the client
over/under-pays: quote endpoints can salvage the swap instead of forcing an
immediate refund when limits and time remain. [source]

### 2.3 Trust model: "Don't trust. Verify."

`dont-trust-verify.md` is explicit product law:

- Never trust lockup addresses or invoices from the API without local
  re-derivation.
- Verify Taproot trees / redeem scripts: preimage hash, pubkeys, timeout
  height, opcodes.
- Verify Lightning invoices: payment hash equals client-chosen hash; amount
  matches calculated send amount after fees.
- Verify EVM contract addresses against known `EtherSwap` / `ERC20Swap`
  deployments.
- Prefer SDK state machines over raw HTTP. [source]

This is the same class of law OpenAgents already applies to receipts and
evidence: **wire data is untrusted until a local oracle re-derives it**.

### 2.4 Cooperative Taproot claims (modern path)

API v2 examples show the claim dance:

1. Status reaches `transaction.claim.pending` (or reverse equivalent).
2. Client fetches claim details (preimage, nonces, sighash).
3. Client checks `sha256(preimage)` against the invoice payment hash.
4. Client builds MuSig2 with local key + server key, tweaked by the swap
   tree.
5. Client posts `pubNonce` + `partialSignature`.
6. Happy path never reveals the script on-chain. [source]

Non-cooperation still works via script path after timeouts. That dual path
is load-bearing for a Nostr rebuild: **network flakiness must not equal fund
loss**.

### 2.5 Operational surfaces beyond the web UI

| Surface | Role |
| --- | --- |
| `boltz-client` | Unattended rebalancing; accept LN without a public node (Liquid swaps); gRPC control plane |
| BTCPay Liquid plugin | Merchant LN via Liquid nodeless mode or node rebalance mode |
| BTCPay Arkade plugin | Merchant VTXO accept + Lightning via submarine into Arkade |
| Partner dashboard | Referral volume stats via API key |
| `hold` | Production reverse-swap prerequisite on CLN |
| `regtest` / legend env | Full multi-chain dev stack |
| `canary` / `channel-bot` | Node health and liquidity ops |

Boltz is therefore an **ecosystem**: operator, crypto library, wallet UI,
merchant adapters, and node tooling — not a single repository. [source]

## 3. Architecture deep dive

### 3.1 Backend (`boltz-backend` v3.13.0, AGPL)

Observed layout under `lib/`:

| Area | Responsibility |
| --- | --- |
| `api/` + `api/v2/` | REST surface, static assets, swap info cache |
| `service/` | Pair logic, invoice expiry, Elements, renegotiation, signer control |
| `swap/` | Nurseries: UTXO, Lightning, Invoice, Ethereum, Ark; SwapManager; overpayment protection; node switch/fallback |
| `lightning/` | LND + CLN clients, routing fees, pending payment tracker |
| `rates/` | FeeProvider, RateProvider, lockup fee tracking |
| `chain/` + `wallet/` | Chain backends and wallet abstraction |
| `db/` | Persistence (PostgreSQL in regtest/prod scripts) |
| `grpc/` | Operator gRPC + JWT |
| `sidecar/` + Rust crates (`boltzr`, `boltz-evm`, `boltz-cache`, …) | Performance / EVM / CLI helpers compiled beside TypeScript |

The **nursery** metaphor is important: each rail has a long-running watcher
that advances swaps as chain or Lightning events arrive. That is the real
product engine — not the HTTP handlers. [source] [inferred]

License: **AGPL-3.0**. Any network-deployed derivative must share source
under AGPL. For OpenAgents this is a hard adoption boundary: treat the
backend as **behavior evidence**, not a vendored service. [source]

### 3.2 Crypto core (`boltz-core` v5.0.0, MIT)

ESM TypeScript library (Node ≥ 20.10):

- `swapTree` / `reverseSwapTree` Taproot constructions
- claim / refund transaction builders
- MuSig2 helpers
- preimage and swap detectors
- optional `boltz-core/liquid` entry (peer deps `liquidjs-lib`,
  `secp256k1-zkp`)
- Solidity `EtherSwap`, `ERC20Swap`, router contracts

This is the **portable MIT gift** of the ecosystem: scripts and claim logic
that wallets already share. OpenAgents can study or depend on it with
attribution under ordinary MIT rules; it still does not grant product
authority or a live LP. [source]

### 3.3 Client daemon (`boltz-client`, MIT)

Go module with:

- `cmd/boltzd`, `cmd/boltzcli`
- `internal/autoswap` — policy-driven rebalancing
- `internal/lightning` + LND/CLN adapters
- `internal/onchain`, electrum, mempool, esplora
- `internal/nursery` — local swap lifecycle
- `pkg/boltz`, `pkg/boltzrpc` — library + protobuf API
- Liquid wallet stack (`lwk`, `bdk` subtrees)

This is the **server-side integration reference** Boltz recommends for
merchants and node operators. Conceptually it is closer to a Pylon-shaped
"local always-on agent for money rails" than to a pure browser dapp.
[source] [inferred]

### 3.4 Web app (`boltz-web-app` v2.2.1, AGPL)

Vite/TypeScript SPA: swap config, status machine, hardware wallet helpers,
i18n, workers for preimage hashes, rescue flows, product pages. Same verify
discipline as the docs (`utils` validation patterns referenced from
dont-trust-verify). License is AGPL; UI is evidence, not a shell to fork.
[source]

### 3.5 Hold invoices (`hold`, MIT)

Rust CLN plugin: create/list/settle/cancel hold invoices; gRPC; SQLite or
Postgres; expiry deadline relative to shortest HTLC; MPP timeout. Reverse
submarine swaps are not optional theater — without hold invoices the
server cannot safely lock chain after partial Lightning payment. [source]

### 3.6 API shape (v2, SDK authors only)

Documented flows use:

- `POST /v2/swap/submarine` | `reverse` | `chain`
- WebSocket `swap.update` channel
- Claim/refund cooperative endpoints per swap id
- Pair limits, fees, 0-conf policy via pairs endpoints
- Optional webhooks (docs), partner referral stats

Mainnet base: `https://api.boltz.exchange/`. Docs ship `llms.txt` /
`llms-full.txt` for agent consumption — Boltz already expects LLM
integrators, and still forbids naive raw-API integration. [source]

### 3.7 What Boltz is not

- Not a general DEX for arbitrary tokens
- Not a custodial exchange account system
- Not a Lightning Service Provider in the pure forwarding sense (though it
  operates large nodes)
- Not a Nostr-native protocol today — discovery and session state are HTTP
  and operator-hosted WebSocket
- Not multi-operator by protocol: one API host is the LP, even if clients are
  open source

## 4. Laws worth harvesting

| Law | Boltz form | OpenAgents translation |
| --- | --- | --- |
| Atomicity | Same preimage hash both legs | Keep cryptographic link; never "promise later" |
| Client verification | Re-derive address/invoice/contract | Same as receipt digest verification |
| Dual exit | Cooperative MuSig2 + script timeout | Network loss ≠ fund loss |
| Explicit states | Named lifecycle statuses | Typed swap state machine in schema |
| SDK boundary | Ban raw API for app integrators | Ban ad-hoc payment code paths |
| Hold before lock (reverse) | Invoice pending until preimage | Do not lock destination before source risk is bounded |
| Renegotiation | Chain swap quote salvage | Prefer repair over abandon when safe |
| 0-conf policy | Pair limits; RBF rejection; mainchain caution | Fail closed on unconfirmed risk |
| Secrets offline | Preimage client-local on reverse | Never publish preimage/refund key on relays |
| Ops nurseries | Per-rail watchers | Separate watchers from admission API |

## 5. Rebuilding a Boltz-class system with Nostr

This section is a **rebuild architecture**, not a claim that OpenAgents has
shipped it. It maps Boltz roles onto official NIPs (local clone
`~/work/projects/repos/nips`) and OpenAgents drafts under
[`docs/nips/`](../nips/README.md).

### 5.1 Design goal

**Multi-provider, non-custodial layer swaps** where:

- Any LP can advertise pairs, limits, fees, and uptime.
- Takers discover LPs on Nostr and complete swaps with client-side verify.
- Payment of Lightning legs uses the taker's or LP's wallet via NIP-47 or
  local node control — not a platform hot wallet as custodian of both sides.
- Public events carry **refs and digests**; private events or local storage
  carry scripts, nonces, and partial signatures as needed.
- Closeout produces NIP-EV evidence + NIP-OC/NIP-AC settlement references
  without treating relay presence as payment finality.

### 5.2 Role mapping

| Boltz role | Single-operator today | Nostr multi-provider rebuild |
| --- | --- | --- |
| LP identity | Boltz company + node pubkeys | Nostr pubkey (person or NIP-SA agent) with NIP-32 attestations |
| Pair catalog | `GET` pairs on one API | Addressable **Swap Pair** / capacity events (see §5.4) + NIP-HP capacity for host-bound LPs |
| Create swap | `POST /v2/swap/...` | Signed **Swap Offer** response to a **Swap Intent** (private NIP-17/59 or gift-wrapped) |
| Status stream | Operator WebSocket | Append-only **Swap Progress** events to agreed relays; client also watches chain/LN locally |
| Invoice pay | Wallet / node | NIP-47 `pay_invoice` / `make_invoice`, or local CLN/LND; optional NIP-57 receipt when public tip-shaped |
| Hold invoice | `hold` plugin on LP CLN | LP-side requirement remains; not replaceable by Nostr |
| Claim/refund crypto | `boltz-core` in client | Same (MIT lib or Effect port) in taker + LP daemons |
| Fees | Operator schedule | Quoted in offer; bound into intent digest; NIP-AC if credit-financed |
| Merchant accept LN | BTCPay plugins | Merchant agent + LP offers; Forum/Work optional for human ops |
| Partner referrals | API keys + dashboard | NIP-32 labels + NIP-OC contributor tags — not a second ledger |

### 5.3 Official NIPs to use (do not reinvent)

| NIP | Use in rebuild |
| --- | --- |
| [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md) | Event envelope, replaceable/addressable kinds |
| [NIP-07](https://github.com/nostr-protocol/nips/blob/master/07.md) / [NIP-46](https://github.com/nostr-protocol/nips/blob/master/46.md) | Human signing; remote signer for LP daemons |
| [NIP-17](https://github.com/nostr-protocol/nips/blob/master/17.md) + [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md) + [NIP-59](https://github.com/nostr-protocol/nips/blob/master/59.md) | Private swap negotiation, claim nonces, rescue material distribution to the rightful party only |
| [NIP-47](https://github.com/nostr-protocol/nips/blob/master/47.md) | Wallet Connect: `pay_invoice`, `make_invoice`, `lookup_invoice`, balance, notifications — the Lightning hand without embedding node credentials in the swap UI |
| [NIP-57](https://github.com/nostr-protocol/nips/blob/master/57.md) | Optional public zap receipts when a payment is meant to be socially visible; **not** a substitute for HTLC proof |
| [NIP-60](https://github.com/nostr-protocol/nips/blob/master/60.md) / [NIP-61](https://github.com/nostr-protocol/nips/blob/master/61.md) | Cashu wallet state and nutzaps as **adjacent rails** for fee payment or later Cashu↔LN bridges (out of Boltz core, in scope for OpenAgents multi-rail) |
| [NIP-69](https://github.com/nostr-protocol/nips/blob/master/69.md) | Precedent for pooled P2P order discovery (`kind:38383`); pattern for public orderbooks — adapt tags for BTC layers rather than fiat methods |
| [NIP-89](https://github.com/nostr-protocol/nips/blob/master/89.md) / [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md) | App handlers and DVM-style service announcements for "I provide submarine swaps" machines |
| [NIP-98](https://github.com/nostr-protocol/nips/blob/master/98.md) | HTTP auth if an LP still exposes a Boltz-compatible REST facade |

### 5.4 OpenAgents NIP drafts to compose

| Draft | File | Role in rebuild |
| --- | --- | --- |
| NIP-SA | [`SA.md`](../nips/SA.md) | LP or taker as sovereign agent; guardian gates for large locks |
| NIP-AC | [`AC.md`](../nips/AC.md) | Outcome-scoped credit to **pay swap fees or bootstrap liquidity** on bolt11/bolt12/Cashu rails; settlement receipts `39244` |
| NIP-LBR | [`LBR.md`](../nips/LBR.md) | "Execute this swap policy" as ref-only labor when a human hires an agent operator; bonds without invoices on the wire |
| NIP-DS | [`DS.md`](../nips/DS.md) | Optional: sell historical anonymized swap-parameter datasets; payment tags already list Lightning rails |
| NIP-HP | [`HP.md`](../nips/HP.md) | Host records + capacity statements for LP nodes (`cap` for `swap_submarine`, `swap_reverse`, pairs, max sat) |
| NIP-EV | [`EV.md`](../nips/EV.md) | Evidence receipts for lockup txid, claim txid, invoice payment hash (hash only), verification by a second key |
| NIP-OC | [`OC.md`](../nips/OC.md) | Accepted outcome + closeout indexing settlement refs — never moves sats itself |
| NIP-WI / NIP-WK | [`WI.md`](../nips/WI.md), [`WK.md`](../nips/WK.md) | When a swap is part of larger Work (treasury rebalance, merchant payout), intent → admission stays authoritative |
| NIP-AT | [`AT.md`](../nips/AT.md) | Private attention for "swap needs refund" / "claim pending" |

### 5.5 Proposed event sketch (candidate only — not a published NIP)

Kinds below are **illustrative placeholders** for a future `NIP-SWP` draft.
They must be reserved through the normal OpenAgents NIP process before any
implementation treats them as wire truth. [inferred]

```text
SwapPairAnnouncement  (addressable)
  - LP pubkey
  - pairs: BTC/LN, L-BTC/LN, BTC/L-BTC, …
  - min/max sat, fee schedule digest, max 0-conf, expiry defaults
  - endpoints: nostr-only | rest-facade URL hash | both
  - hold_invoice: true/false, node implementation hints (public-safe)
  - a/cap → NIP-HP capacity

SwapIntent  (ephemeral or gift-wrapped)
  - taker pubkey
  - pair, amount, direction (submarine | reverse | chain)
  - payment_hash (reverse: client-chosen; submarine: from invoice)
  - refund_pubkey / claim_pubkey as required by direction
  - invoice ref or bolt11 hash pointer (never preimage)
  - expiry, max_fee, idempotency key

SwapOffer  (gift-wrapped to taker)
  - binds intent id
  - lockup address / swap tree digest / claim_pubkey
  - server timeouts, exact fees
  - optional REST session token hash for hybrid LPs

SwapProgress  (addressable unique-d per swap)
  - status enum aligned with Boltz vocabulary
  - txid digests, confirmation counts
  - zero_conf_rejected flags
  - no preimages, no partial signatures in public content

SwapEvidence  (NIP-EV profile)
  - producer-signed digests of claim/refund txs
  - independent verifier checks chain + payment_hash binding
```

**Hard exclusions on public relays** (same spirit as NIP-LBR):

- preimages and invoice payment secrets
- refund/claim private keys and MuSig2 secret nonces
- full bolt11 strings when they embed sensitive metadata (prefer hash
  pointers + NIP-47 local pay)
- wallet seeds, macaroons, NWC connection secrets
- raw proprietary node RPC payloads

### 5.6 End-to-end flows on Nostr

#### A. Reverse submarine (LN → chain) — agent cashes out to cold storage

1. LP publishes `SwapPairAnnouncement` + NIP-HP capacity.
2. Taker agent creates preimage locally; publishes or gift-wraps `SwapIntent`
   with `payment_hash`, claim pubkey, amount.
3. LP answers `SwapOffer` with hold-invoice reference (or NIP-47-makeable
   invoice description hash).
4. Taker pays via NIP-47 `pay_invoice` to its own wallet connection.
5. LP locks chain; publishes `SwapProgress=transaction.mempool`.
6. Taker verifies lockup script against offer digest; cooperative claim via
   private messages or hybrid API; reveals preimage only to LP over private
   channel / claim API.
7. Both sides emit NIP-EV evidence; optional NIP-OC closeout if this was Work.

#### B. Normal submarine (chain → LN) — fund an agent budget

1. Taker has or creates a BOLT11 (or bolt12) invoice to its node/NWC.
2. Intent includes invoice hash + refund pubkey.
3. LP offer returns lockup; taker verifies and pays chain.
4. LP pays invoice; proves preimage on claim path; taker may verify
   `sha256(preimage)` before cooperative claim signature.
5. NIP-AC envelope may have financed the on-chain leg; repayment uses AC
   rails after success.

#### C. Merchant nodeless LN (BTCPay-shaped)

1. Merchant agent is a NIP-SA with policy "accept LN up to X via reverse or
   Liquid reverse."
2. Each invoice creates a reverse swap with a preferred LP set (not one
   global API).
3. Failure cancels hold invoice; customer is never told "paid" without claim
   or explicit risk policy.
4. Accounting stays in merchant ledger; Nostr carries public-safe progress
   only if the merchant opts in.

### 5.7 Hybrid compatibility

Many LPs will keep a Boltz-compatible REST facade for wallet SDKs already in
the wild (Breez, boltz-rust, etc.). Nostr then provides:

- **discovery and reputation** (pairs, uptime attestations, NIP-32)
- **multi-LP competition** on fees
- **identity** portable across apps

…while the swap session itself may still be HTTP+WS to that LP. That is a
valid phase-1: **Nostr directory + Boltz-class session**, not pure-relay
atomic swaps on day one. [inferred]

### 5.8 What must remain non-Nostr

| Component | Why |
| --- | --- |
| HTLC / Taproot script physics | Consensus and Lightning protocol, not relay policy |
| Hold invoice plugin / node | Requires real LN implementation |
| Chain watchers / electrum / compact filters | Latency and correctness |
| Rate oracles | External; publish digests, do not invent prices on relays |
| Actual settlement of Lightning | NIP-57 is a receipt of a payment story, not the payment rail itself |
| Custody of LP inventory | LP hot wallets stay LP-operated; platform must not aggregate custody |

## 6. OpenAgents disposition

### 6.1 Harvest

- Submarine / reverse / chain **state machines** as typed Effect Schema
- Client-side verify checklists as executable oracles (tests)
- Cooperative + timeout dual-path claim design
- Nursery/watcher separation from admission API
- Merchant "nodeless LN" product shape for Pylon/Desktop treasury tools
- MIT `boltz-core` (and boltz-rust outside this org) as optional crypto
  dependencies behind an owned adapter
- LLM-oriented docs posture (`llms.txt`) for operator docs OpenAgents publishes

### 6.2 Reject / do not adopt

- AGPL `boltz-backend` or `boltz-web-app` as OpenAgents runtime dependencies
- Single-operator REST as the only discovery plane
- Putting swap secrets on public relays
- Treating zap receipts as swap finality
- Treating LP capacity announcements as grants or budgets
- Replacing Google Cloud / OpenAgents settlement authority with a Boltz
  instance
- Copying partner-program economics as default protocol fees

### 6.3 Relationship to existing OpenAgents rails

| Existing rail | Interaction |
| --- | --- |
| Pylon / own-capacity coding | Orthogonal; swaps fund or rebalance agent budgets |
| NIP-90 / NIP-LBR markets | Swaps can settle labor fees; labor can operate LP bots |
| NIP-AC credit | Fees and inventory float without free-floating loans |
| L402 / paid APIs | Reverse submarine can fund prepaid API access |
| Khala token counters | Exact token rows stay exact; do not mix with sat ledgers |
| Omega / Desktop | UI for treasury swap intents; no renderer crypto authority |

## 7. Candidate packets (not admitted work)

These are research-sized packets for later owner admission. They are not
issues, roadmap rows, or deploy authority.

1. **SWP-Spec-0** — Draft `docs/nips/SWP.md` with reserved kinds, status enum
   aligned to Boltz lifecycle, and public-safe tag lists; link from
   `docs/nips/README.md` only after review.
2. **SWP-Verify-1** — Effect Schema + tests porting `dont-trust-verify`
   checks for one Taproot submarine pair using MIT `boltz-core` behind an
   adapter (no network).
3. **SWP-NWC-2** — NIP-47 pay/make invoice glue for reverse/normal demos on
   regtest; secrets never logged.
4. **SWP-LP-Announce-3** — NIP-HP capacity profile + replaceable pair
   announcement events; consumer directory UI sketch in Desktop or Forum.
5. **SWP-Hybrid-4** — Optional: one external Boltz API as a single LP backend
   behind the Nostr directory (custodial risk = that LP only; document
   clearly).
6. **SWP-Merchant-5** — Policy template for nodeless LN accept via reverse
   swaps for owner-operated commerce experiments.
7. **SWP-Receipt-6** — NIP-EV evidence profile for payment_hash, lockup
   outpoint, claim txid; NIP-OC closeout when swaps are Work-scoped.

## 8. Comparison table

| Axis | Boltz (today) | Nostr rebuild (target) |
| --- | --- | --- |
| Custody | Non-custodial per swap | Same |
| LP model | Single primary operator | Many LPs, competitive discovery |
| Session transport | HTTPS + WSS | Gift-wrap + optional HTTPS facade |
| Identity | Implicit API client | Nostr pubkeys + optional SA |
| Lightning control | Node / WebLN / SDK | NIP-47 + local nodes |
| Public proof | Optional explorer links | NIP-EV digests + optional NIP-57 |
| Credit | Not core | NIP-AC envelopes |
| License of reference operator | AGPL backend | Independent implementation |
| Client crypto | boltz-core MIT | Same class, owned adapter |

## 9. Central finding

Boltz is the strongest open **Bitcoin-layer atomic swap product** in this
catalog: clear lifecycle docs, a hard verify culture, cooperative Taproot
claims, reverse swaps with real hold invoices, merchant plugins, and a MIT
crypto core other wallets already share. [source]

OpenAgents should not become "another Boltz host." It should become a
**Nostr-native marketplace and agent-capable client for Boltz-class swaps**,
composing NIP-47/57/60 for money movement, NIP-17/44/59 for private
sessions, NIP-HP/SA for LP identity and capacity, and NIP-EV/OC/AC for
evidence and settlement references — while the sats still move under HTLC
and Lightning physics no relay can fake. [inferred]

## 10. Source manifest (secondary pins)

| Repo | Commit |
| --- | --- |
| `boltz-python` | `36c5420f7c4b6376fc2177bf6af83ae6a6126c95` |
| `btcpay-plugin-liquid` | `9168cad4ae13659006d8d6bdd43bcf485c1aa355` |
| `btcpay-plugin-arkade` | `fcbc7b6bb1ac30159f80045156d789f6a0dedac2` |
| `regtest` | `ae600ed3190a7f1f8859d2dc6da0c8c85a245a59` |
| `canary` | `663fa7fac7e2b5a179553802b51bf40dec835391` |
| `covclaim` | `4eddc1a5308e1104cee765fc36c248c00af6cdd2` |
| `channel-bot` | `5bbf23b0762d4a055ecdebd776011b937dcd68b3` |
| `go-electrum` | `d6484ac8e978cf87db8abe17a06a152db2aad8a3` |
| `bolt12-wasm` | `1e693509772e13593ae0cffcffad394ad9abdc9b` |

OpenAgents NIP drafts consulted: `docs/nips/README.md`, `AC.md`, `LBR.md`,
`SA.md`, `HP.md`, `OC.md`, `EV.md`, `WI.md`, `WK.md`, `DS.md`, `AT.md`,
`PROPOSED.md`. Official NIPs consulted from
`~/work/projects/repos/nips`: `47.md`, `57.md`, `60.md`, `61.md`, `69.md`,
plus NIP-01/17/44/59 by reference.

---

*End of teardown. Design evidence only. Implementation requires separately
admitted packets, authority reconciliation, and live verification.*
