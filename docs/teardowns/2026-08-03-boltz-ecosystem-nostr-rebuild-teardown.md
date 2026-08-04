# Boltz Ecosystem Teardown — non-custodial Bitcoin layer swaps, and a Nostr rebuild

- Date: 2026-08-03; expanded 2026-08-04
- Lane: Fast Follow research / product teardown
- Disposition: high-relevance **liquidity and atomic-swap** reference; recommended
  **native multi-provider swap marketplace** design using OpenAgents NIP drafts and
  official Nostr payment NIPs — **not** implementation, deploy, or product authority
- Local mirror: `~/work/projects/repos/boltz/` (31 public `BoltzExchange` clones)
- OpenAgents tree pin: `dc8a75fe4c7663ee48347702f0ed604a17286606`
- Companion teardown:
  [tbDEX liquidity protocol](./2026-08-04-tbdex-liquidity-protocol-teardown.md)

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

The broader 2026-08-04 synthesis adds tbDEX's provider-neutral negotiation
grammar and a custody/trust gradient. Boltz supplies the strongest atomic
profile; tbDEX supplies the common provider/RFQ/Quote/Order/Status/Close
shape for rails that retain social trust; Nostr supplies portable discovery
and encrypted messaging; Immortal begins as a hardened relay and grows into
the noncustodial coordination runtime for that multi-relay fabric. See
§§11–18. [inferred]

### Product and protocol language

[Episode 213](../transcripts/213.md) names five interlocking **Agent Markets**:
compute, data, labor, liquidity, and risk. Boltz/tbDEX work is therefore not a
sixth market and not the whole marketplace. Use this vocabulary consistently:

- **OpenAgents Agent Markets** — the umbrella economy across the five market
  lanes;
- **OpenAgents Liquidity Market** — the product lane where agents and humans
  provide, find, route, and earn from Bitcoin liquidity;
- **NIP-MKT negotiated-market fabric** — the reusable provider-neutral wire
  grammar beneath liquidity and, where appropriate, other bilateral markets;
- **noncustodial Bitcoin liquidity network** — the first technical system:
  multiple providers, relays, wallets, and settlement rails speaking that
  common protocol.

“Decentralized exchange” is too narrow because the product includes swaps,
Lightning channel/JIT liquidity, mint/federation gateways, credentialed
on/off ramps, provider discovery, and recovery. “Liquidity pool” is also
misleading unless referring to actual provider inventory: Immortal does not
pool or custody funds. Boltz contributes the atomic-swap profile; tbDEX
contributes the broad provider negotiation grammar; neither names the whole
product. [source] [proposal]

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
| LP identity | Boltz company + node pubkeys | Nostr service pubkey (person or NIP-SA agent), owner binding where applicable, and wallet-selected NIP-32/39/85 assertions |
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
| [NIP-32](https://github.com/nostr-protocol/nips/blob/master/32.md) / [NIP-39](https://github.com/nostr-protocol/nips/blob/master/39.md) / [NIP-85](https://github.com/nostr-protocol/nips/blob/master/85.md) | Namespaced public claims, external identity control, and wallet-selected assertion providers; none is settlement or solvency proof |
| [NIP-51](https://github.com/nostr-protocol/nips/blob/master/51.md) / [NIP-65](https://github.com/nostr-protocol/nips/blob/master/65.md) / [NIP-66](https://github.com/nostr-protocol/nips/blob/master/66.md) | Curated provider/monitor lists, participant relay sets, and multi-monitor relay liveness |
| [NIP-69](https://github.com/nostr-protocol/nips/blob/master/69.md) | Existing P2P fiat order vocabulary and interop precedent; do not overload it for atomic swaps |
| [NIP-87](https://github.com/nostr-protocol/nips/blob/master/87.md) | Cashu mint and Fedimint discoverability; reuse rather than duplicate |
| [NIP-89](https://github.com/nostr-protocol/nips/blob/master/89.md) | Discover applications that handle the focused market events |
| [NIP-99](https://github.com/nostr-protocol/nips/blob/master/99.md) | Human-facing provider/service listings; not the executable quote wire |
| [NIP-98](https://github.com/nostr-protocol/nips/blob/master/98.md) | HTTP auth if an LP still exposes a Boltz-compatible REST facade |

NIP-90 is explicitly marked **unrecommended** upstream with a direction to
prefer use-case-specific microstandards. It remains a historical OpenAgents
compatibility surface, not the swap or liquidity protocol. See
[`NIP90-MIGRATION.md`](../nips/NIP90-MIGRATION.md).

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
| Legacy NIP-90 / NIP-LBR v1 | Compatibility only; swaps may fund admitted labor outcomes but never reuse the DVM wire |
| NIP-AC credit | Fees and inventory float without free-floating loans |
| L402 / paid APIs | Reverse submarine can fund prepaid API access |
| Khala token counters | Exact token rows stay exact; do not mix with sat ledgers |
| Omega / Desktop | UI for treasury swap intents; no renderer crypto authority |

## 7. Candidate packets (not admitted work)

These are research-sized packets for later owner admission. They are not
issues, roadmap rows, or deploy authority.

1. **MKT/SWP-Spec-0** — Draft a focused negotiated-market base and MKT-SWP
   profile with kinds reserved only after review, a status enum aligned to
   Boltz lifecycle, and public-safe/private field rules. Do not use NIP-90.
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
| Immortal relay source | `2a04142b0a791e862a2112c7230d186d088bde17` (`origin/main` audited 2026-08-04) |
| OpenAgents source | `dc8a75fe4c7663ee48347702f0ed604a17286606` (`origin/main` audited 2026-08-04) |

OpenAgents NIP drafts consulted: `docs/nips/README.md`, `AC.md`, `LBR.md`,
`SA.md`, `HP.md`, `OC.md`, `EV.md`, `WI.md`, `WK.md`, `DS.md`, `AT.md`,
`PROPOSED.md`. Official NIPs consulted from Immortal's pinned official lane:
NIP-01, 17, 32, 39, 40, 43, 44, 47, 51, 57, 59, 60, 61, 65, 66, 67, 69,
70, 73, 85, 87, 89, 90, 94, 98, and 99. The full Block lane was reviewed;
its market-relevant disposition is in §13.2. The local tbDEX v0.2 whitepaper,
its TeX source, the archived tbDEX protocol/SDK repositories, the current
Mostro protocol, Cashu NUTs, Fedimint, Arkade, and Lightning bLIPs were also
consulted for the 2026-08-04 expansion.

## 11. Full ecosystem decomposition: what decentralizes, what does not

The 31 repositories are not 31 pieces of a protocol. They divide into a few
portable laws, operator processes, client surfaces, and packaging artifacts.
The decentralized design should replace the central coordination boundary,
not force every operational tool onto Nostr.

| Boltz ecosystem slice | Repositories | Decentralized successor |
| --- | --- | --- |
| Primary operator | `boltz-backend`, older `boltz-middleware` | Many independently keyed LP daemons; each owns inventory, node access, reservations, watchers, and quotes |
| Cryptographic verifier | `boltz-core`, `bitcoin-ops`, `bolt11`, `bolt12-wasm` | Shared client/LP verifier SDK behind an OpenAgents-owned adapter; quote binds exact algorithm/version |
| Wallet and automation clients | `boltz-client`, `boltz-python`, `boltz-web-app`, legacy frontend/demo | Local-first market router that discovers providers, compares quotes, verifies scripts, and recovers without one API |
| Lightning mechanics | `hold`, channel-creation plugin | Provider-local CLN/LND/LSP adapters; never relay code |
| Merchant bridges | BTCPay Liquid and Arkade plugins | Merchant adapter chooses a provider set and exposes settlement state without taking protocol custody |
| Chain observation | `go-electrum`, backend nurseries, `covclaim` | Independent rail watchers plus wallet verification; Nostr carries signed observations, not chain truth |
| LP operations | `channel-bot`, `canary`, `cln-backup` | Private provider operations; optionally advertised as separate services, never market authority |
| Partner metrics | partner dashboard | Derived signed attribution and redacted receipts; no privileged API-key ledger |
| Conformance lab | `regtest`, predecessor environment | Multi-provider adversarial lab with relay partitions, reorgs, RBF, stale quotes, crash/recovery, and refunds |
| Packaging and brand | Umbrel/Proton forks, landing, maintenance, logo, slides | Distribution and provenance only; no protocol role |

`covclaim` supplies a particularly useful warning: a watcher that notices a
claimable covenant is not an offline proof that the covenant was safe. The
client must verify the covenant before funding. The same law applies to every
provider event: later observability cannot repair an unverified lock.

## 12. What Immortal contributes now—and is committed to become

The current Immortal source and production handoff establish a specific relay,
not a hypothetical exchange server:

- one Rust binary and one Postgres database;
- NIP-01 event/signature admission, prepared SQL, bounded filters and query
  cost, per-IP/per-pubkey limits, and commit-before-`OK`;
- stable historical/live boundaries through database ingest sequence and
  LISTEN/NOTIFY; fail-closed connection shutdown when a process cannot remain
  current;
- ephemeral kinds `20000–29999` excluded from storage;
- relay identity, NIP-42 authentication, recipient-gated gift wraps, COUNT,
  full-text search, relay-managed groups, authenticated management, and
  bounded Blossom media;
- pinned official, Block, and OpenAgents NIP source lanes with manual fixture
  and conformance policy; no GitHub workflows or GitHub-billed runners;
- production deployment at `relay.openagents.com` backed by Cloud Run and one
  Cloud SQL database, with the prior revision retained for rollback.

The production cutover record reports 6,882 source rows examined, 6,792
stored, 90 expired, zero rejected or unresolved, and successful publish/read,
NIP-42, COUNT, and load proofs at cutover. That is useful evidence of a real
relay and migration path. It is not evidence of market liquidity, provider
diversity, or decentralization. [source]

### 12.1 Relevant implemented protocol surface

Immortal advertises the official NIPs its configured runtime actually serves,
including base/event metadata, deletion, expiration, COUNT, search, relay
lists, media, and—when configured—private messaging/authentication, groups,
management, and HTTP auth. The Block server contract implements or safely
degrades ownership/admission, agent observer traffic, encrypted memory,
metrics, personas, reminders, projects, archival, identity/DM/read-state, and
workspace presentation features according to configuration.

As of this audit pin, it does **not yet** implement a swap state machine,
provider catalog, quote reservation, liquidity verifier, NIP-66/67/77
multi-relay fabric, or an OpenAgents market draft as an executable server
contract. That is an honest current-state gap, not the intended boundary.

The owner-directed target is every NIP pinned across Immortal's official,
Block, and OpenAgents lanes, plus focused new NIPs for missing market
primitives. Immortal will absorb every useful noncustodial Boltz/tbDEX
coordination function that fits its one-binary/one-Postgres laws: validation,
catalog/indexing, private session routing, provider-signed reservation state,
state-machine enforcement, timers, evidence verification, recovery,
monitoring, and compatibility APIs. Client and provider code still verifies
and controls its own funds; the relay must not hold wallet/LP spend keys,
balances, unreleased preimages, node or bank credentials, or final settlement
authority. The design separation is **custody from coordination**, not market
functionality from Immortal.

### 12.2 Relay topology, not “the relay”

```text
                       public Offering replicas
                ┌──────── relay A (Immortal) ────────┐
wallet/router ───┼──────── independent relay B ───────┼── LP directories
                └──────── independent relay C ───────┘
                     |                         |
            taker DM inbox relays       provider DM inbox relays
                     \──── encrypted RFQ/Quote/Order ────/

wallet + LP verify rails directly; relays never declare settlement
```

- Use NIP-65 for small participant-selected read/write sets.
- Use NIP-66 observations from several monitors; never trust one monitor or
  make its absence fatal.
- Add NIP-67 completeness hints or conservative pagination before treating a
  catalog snapshot as complete.
- Consider NIP-77-style sync only for public catalogs and receipts, not secret
  sessions.
- Keep a high-integrity authenticated Immortal deployment if useful, but do
  not weaken its write policy merely to call a marketplace permissionless.
  Permissionless protocol participation can span multiple relays with
  different policies.

## 13. The three NIP lanes: complete implementation, role-correct use

The pinned lanes are complementary, and every pinned NIP is now an Immortal
implementation target. The tables below describe relevance to this liquidity
market, not the limit of implementation. Relay semantics belong in server
handlers; client-only semantics belong in the native/browser client;
operator/provider/executor profiles belong in bounded configured modules.
Only a currently executable relay behavior is advertised. Deprecated or
unrecommended NIPs receive compatibility implementations and fixtures rather
than becoming the basis for new market design.

### 13.1 Official lane

| Function | Preferred official primitives |
| --- | --- |
| Event and expiry | NIP-01, NIP-40, NIP-70 |
| Private negotiation | NIP-17, NIP-44, NIP-59; ephemeral relationship keys |
| Wallet operation | NIP-47; local CLN/LND remains valid |
| Provider and trust discovery | NIP-32, NIP-39, NIP-51, NIP-73, NIP-85, NIP-99 |
| Relay discovery and routing | NIP-43, NIP-65, NIP-66, NIP-67 |
| Adjacent rail discovery | NIP-87 for Cashu/Fedimint |
| P2P fiat interop | NIP-69 and Mostro semantics |
| Client handlers | NIP-89 |
| HTTP compatibility | NIP-98 |

NIP-15 is not the new foundation; upstream recommends NIP-99. NIP-96 is
deprecated in favor of Blossom. NIP-90 is unrecommended and is now
compatibility-only under
[`NIP90-MIGRATION.md`](../nips/NIP90-MIGRATION.md).

### 13.2 Block lane

| Draft | Market use |
| --- | --- |
| NIP-OA | Bind an agent service key to an owner's signed attestation |
| NIP-AA | Explicit relay admission and revocation for scoped/private relays |
| NIP-AP | Provider-agent persona/team catalog, not financial identity |
| NIP-AO | Ephemeral private telemetry/control for provider operations |
| NIP-AM | Durable bounded metrics with no claim of solvency |
| NIP-AE / NIP-ER | Encrypted provider memory and recovery reminders |
| NIP-RS | Wallet/router read state across devices |
| NIP-IA | Archive public catalogs and receipts |

NIP-MP, GS, CW, DV, and WP may support project, Git, pagination, DM
projection, and presentation surfaces, but they do not become settlement
authority. NIP-PL's current fail-closed handler is temporary: the full-lane
program must add its specified executor/decryption/dispatch behavior inside
Immortal's one-binary boundary and advertise it only after fixture and actual
transport proof. Market safety must not depend on an unconfigured executor.

### 13.3 OpenAgents lane

| Draft | Market use |
| --- | --- |
| NIP-SKL | Versioned rail-adapter skill manifests and independent attestations |
| NIP-SA, AD, AS, AV | Provider/taker agents, delegated authority, sessions, and bounded activity |
| NIP-WI / NIP-WK | Owner-authorized treasury action before an agent may lock funds |
| NIP-EV | Lock, claim, refund, invoice/payment-hash, and independent verification evidence |
| NIP-OC | Accepted outcome and closeout references; never payment authority |
| NIP-HP | Host/provider capability; never proof of live inventory |
| NIP-AT, GB, AL, TP | Private refund/claim alerts, guidance, automation, and triage |
| NIP-PP | Honest claim state for a proposed market capability |
| NIP-DS / LBR v1 | Historical compatibility and adjacent data/labor markets |
| NIP-AC | Optional admitted credit later; not required for first-pass swaps |

BT contribution credit is not part of the first market pass. Security
hardening NIPs can verify an implementation, but should not be overloaded into
the quote wire.

## 14. Unified market architecture: Boltz physics plus tbDEX negotiation

The market must use the strongest guarantee available on each route rather
than impose one custody ideology on every provider.

| Guarantee class | Provider examples | What the client must verify or accept |
| --- | --- | --- |
| Atomic | Boltz-style submarine/reverse/chain LP | Script/tree, payment hash, amounts, timelocks, confirmations, claim/refund exits |
| Coordinated escrow | Mostro-style Lightning hold coordinator | Hold state, bond, solver/arbiter set, release/dispute/timeouts |
| Federated custody | Fedimint federation + gateway | Federation identity/threshold/modules, gateway terms, redemption route |
| Mint custody | Cashu mint | Mint key/keysets/NUTs, redemption rail, operator/solvency/censorship risk |
| Regulated PFI | Bank/stablecoin/payment provider | Legal identity, credential policy, settlement/reversibility window, custody, recourse |
| Custodian/prime service | Named custodian or broker | Segregation, authorization, withdrawal, insurance, insolvency and jurisdiction |

Every signed Quote should bind at least:

- exact input/output asset, network, amount, fees, expiry, and route legs;
- provider and service keys plus Offering and RFQ digests;
- guarantee and custody class for every leg;
- reservation type and proof/reference, with expiry;
- confirmation/finality/reversibility rules;
- credential types and issuers, disclosed data classes, purpose and retention;
- dispute/solver/recourse path;
- settlement adapter and evidence profile versions;
- privacy and relay/inbox expectations.

The wallet then chooses a route using its own policy. There is no universal
rank. Price is one dimension alongside finality, custody, credential cost,
latency, privacy, and recourse.

### 14.1 Focused market microstandard

The smallest common base is tbDEX-shaped:

```text
ProviderProfile → Offering → private RFQ → signed expiring Quote
                → signed Order → sequenced OrderStatus → Close
```

Call the candidate base **NIP-MKT: Negotiated Markets** and define narrow
profiles:

- **MKT-SWP** — atomic Bitcoin/Lightning/Liquid/Ark swaps;
- **MKT-P2P** — NIP-69/Mostro-compatible fiat trades;
- **MKT-PFI** — credentialed on/off ramps;
- **MKT-MINT** — Cashu/Fedimint gateway and redemption;
- **MKT-LSP** — channel and just-in-time liquidity;
- later **MKT-RISK** — bonds/guarantees/insurance only when a real
  underwriter and claims authority exist.

The base owns signatures, correlation, idempotency, expiry, reservation,
cancellation, sequencing, errors, privacy, and evidence references. Profiles
own rail semantics and fixtures. No kinds are allocated by this teardown.

### 14.2 Public and private event law

**Public and replicated:** provider profile, human-facing NIP-99 listing,
Offering, supported profile/adapter versions, bounded availability, relay
lists, public attestations, and optional redacted receipts.

**Pairwise encrypted by default:** RFQ, Quote, Order, Status, invoices,
addresses, scripts before funding, account/payment method, credentials,
reservation proof, disputes, and recovery traffic.

**Never placed on relays:** wallet seeds, NWC connection secrets, node
macaroons, preimages before safe revelation, private refund/claim keys,
MuSig2 secret nonces, raw PII, bearer credentials, or unredacted bank data.

Relay acceptance is not provider acceptance; provider acceptance is not
capacity reservation; reservation is not funding; funding is not finality;
Close is not settlement unless the rail-specific verifier proves it.

## 15. Liquidity bridges and interoperability

| Rail/ecosystem | Concrete bridge |
| --- | --- |
| Bitcoin and Liquid | Bitcoin Core/Elements plus Electrum/Esplora-compatible watchers; client verifies consensus-visible evidence |
| Lightning | LND/CLN, BOLT11/BOLT12, hold invoices, NIP-47 for scoped remote wallet operations |
| LSP market | bLIP-50 transport, bLIP-51 channel purchase, bLIP-52 JIT channels; do not invent parallel terms |
| Ark | Arkade/Ark adapters and the existing BTCPay Arkade reference; expose operator and exit assumptions |
| Mostro | Translate existing NIP-69 order and current encrypted action semantics into MKT-P2P; preserve bonds, ratings, disputes, and privacy modes |
| Cashu | NUT-compatible mint/wallet adapters and NIP-87 discovery; quote mint and redemption risk explicitly |
| Fedimint | Federation/gateway adapter and NIP-87 discovery; surface guardian threshold and gateway exposure |
| Fiat and stablecoins | Provider-specific adapters plus consented credential presentation; never call reversible settlement atomic |
| Existing Boltz clients | Optional REST/WSS compatibility facade backed by one or more LP sessions; client still verifies |

Provider-held spend authority, private inventory control, node credentials,
rebalancing, and rail submission remain provider processes. Immortal should
nevertheless implement the noncustodial half of those workflows: provider
registration, signed capacity and quote-reservation contracts, routing,
public-chain and invoice verification, timers, status/evidence indexing,
recovery coordination, and compatibility APIs. That division lets Immortal
eat the coordination surface without becoming custodian or settlement truth.

### 15.1 Liquidity truth

An address balance or signed capacity claim is insufficient: funds may be
encumbered or promised elsewhere. Prefer quote-scoped proof:

- funded HTLC/Taproot output;
- pending hold invoice or channel/JIT commitment;
- mint/federation quote with a verifiable state transition;
- escrow or custodian reservation reference;
- bounded guarantee/bond/insurance where the rail cannot be atomic.

History, uptime, NIP-85 assertions, and independent metrics help select whom
to ask. They never substitute for the current reservation.

### 15.2 Route composition

A router may find Lightning → Liquid → fiat or Cashu → Lightning → Bitcoin.
Two atomic legs do not automatically make an atomic route. A composed route
must share an enforceable secret/timelock construction, be fully pre-funded,
carry a guarantee for intermediate exposure, or disclose the exact sequential
risk window. The router must reject any route whose worst-case refund and
timeout graph it cannot explain.

## 16. Threat model

| Threat | Required control |
| --- | --- |
| Fake/stale Offering | Short expiry, replaceable head, independent probes, Quote rebinds current terms |
| Double reservation | Idempotency key, provider sequence, reservation digest/expiry, enforceable penalty only where real |
| Public front-running and amount leakage | Pull public Offerings; pairwise encrypted RFQs and Quotes |
| Sybil/reputation capture | Wallet-selected provider/issuer/monitor lists; no global score |
| Credential harvesting | Disclose after shortlist; audience/purpose/nonce/expiry binding; retention and deletion contract |
| Relay omission/censorship | Multi-relay replication, local session log, conservative pagination, direct/HTTP fallback |
| Relay correlation | Ephemeral trade keys, separate inboxes, padding/timing defenses, Tor where required |
| LP or wallet crash | Persist-before-publish outbox, deterministic replay, monotonic status, independently executable refund |
| Reorg, RBF, 0-conf loss | Pair-specific policy and local chain verification; fail closed |
| Price-oracle manipulation | Several signed sources, bounded staleness, exact quote binding; oracle never settles |
| Fiat chargeback/non-delivery | Explicit reversibility window, escrow/guarantee/recourse, never “atomic” copy |
| Arbiter capture | Solver set and authority disclosed before Order; portable evidence; competing coordinators |
| Privacy compromise | NIP-44 limitations documented; separate high-risk channel when forward secrecy is required |

## 17. Honest current-state map and committed destination

| Capability | Current evidence | Status for this design |
| --- | --- | --- |
| Hardened relay | Immortal M1–M7 and production cutover evidence | Exists; one operator deployment |
| Browser/native verified reader | Immortal transport-neutral client and WASM proof | Reusable substrate; not a market router |
| Signed projection/outbox | OpenAgents signed workroom projection | Reusable pattern |
| Typed liquidity model | OpenAgents inert request/offer/fill/receipt skeleton | Shape only; explicitly no provider, price, fill, or settlement |
| Historical labor market | NIP-90/LBR helpers and prior receipts | Compatibility evidence; NIP-90 expansion frozen |
| Swap provider | None in OpenAgents | Missing |
| Multi-provider router | None | Missing |
| Liquidity/custody assertions | Draft components only | Missing and must not be inferred from relay events |
| Real cross-rail settlement | External Boltz/Mostro/mint/federation/PFI systems | Adapter research, not OpenAgents authority |

Every “missing” entry above is queued implementation scope except live capital
and authorities that inherently belong to independent providers or settlement
rails. The target is a complete Liquidity Market and negotiated-market fabric,
not a relay that only transports someone else's market. Current-state honesty
controls claims and advertisement; it does not narrow the roadmap.

Ignore retired product surfaces as a design ceiling. Reuse their verified
contracts and evidence laws where they help; do not resurrect their claims or
let them imply a live market.

## 18. Falsifiable first program

1. **NIP-MKT envelope and MKT-SWP fixtures.** Define exact public/private
   fields and state transitions for one Bitcoin↔Lightning regtest profile.
2. **Two independent LP daemons.** Different keys, inventories, and relay
   sets; no shared matcher or hot wallet.
3. **Local-first router.** Discover, shortlist, send private RFQs, compare
   Quotes, persist before publish, and verify every lock/invoice locally.
4. **Owned crypto adapter.** Pin a compatible MIT implementation or
   independent port; bind version/digest in the Quote and conformance corpus.
5. **Adversarial lab.** Relay loss, stale catalog, provider crash, duplicate
   Order, conflicting Status, RBF, reorg, timeout, non-cooperative refund, and
   secret-leak scans.
6. **Compatibility bridge.** One Boltz REST/WSS adapter after the native wire
   works, proving old clients can reach a provider without making the facade
   canonical.
7. **Mostro and LSP profiles.** Reuse their current protocols rather than
   creating isolated liquidity islands.
8. **Mint/federation profile.** NIP-87 discovery plus explicit custody/exit
   terms.
9. **Credentialed PFI pilot last.** Only with a real provider and legal owner;
   prove selective disclosure, retention, failure, dispute, and reversibility.

The first success gate is deliberately small and hard:

> One wallet completes the same regtest swap against either of two independent
> LPs, over independently selected relay sets, then safely refunds when one LP
> and one relay disappear. The relay holds no keys or funds, and every terminal
> claim is independently re-derived from the underlying rail.

That proves dispersed coordination without confusing it with dispersed
liquidity or settlement. Production comes only after the failure journey is
as good as the happy path.

---

*End of teardown. Design evidence only. The implementation mandate is active;
each capability still requires separately fixture-proved packets, authority
reconciliation, and live verification before it is claimed or advertised.*
