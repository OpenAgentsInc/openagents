# Satora/LendaSwap teardown — a second swap coordinator goes dark

- Date: 2026-08-04
- Lane: Fast Follow research / protocol teardown
- Disposition: third architectural donor for the Liquidity Market. Harvest
  the EVM-HTLC leg, the counterparty-only recovery law, the intent/covenant
  quoting model, and the regtest environment shape. The outage itself is the
  strongest evidence yet for the NIP-MKT thesis.
- Primary local source: the `~/work/projects/satora/` reference lane
  (25 public `satoraHQ` repos under `projects/satora/repos/`, synced
  2026-08-04)
- Spot pins: `lendaswap-frontend` `f0c020c`, `satora-sdk` `ff56114`,
  `arkade-wallet` `261bf42`, `lendasat-sdk` `cfc813c`,
  `lendaswap-contracts` `28c6224`, `striker` `191ddce`, `doomsday` `b035a4f`
- Companions:
  [Boltz ecosystem and Nostr rebuild](./2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md),
  [tbDEX liquidity protocol](./2026-08-04-tbdex-liquidity-protocol-teardown.md)

Read-only teardown of local clones and public surfaces. No swap was
attempted, no credential presented, no funds moved.

## Summary

Satora (formerly Lendasat / LendaSwap, "Make Bitcoin Move") operates
non-custodial atomic swaps between Bitcoin — on-chain, Lightning, or Arkade
(Ark VTXOs) — and EVM tokens on Polygon, Ethereum, and Arbitrum, using
SHA-256 HTLCs on both legs. As of 2026-08-04 the swap coordinator
`api.satora.io` returns **502** while the marketing site, swap frontend, and
docs all still serve. Development was active through 2026-08-03 (the last
`satora-sdk` commit switched EVM→BTC claims to 0-conf). The product is
non-custodial in its settlement physics and centralized in its
availability: one company, one closed-source backend
(`docker-registry.satora.io/swap-daemon:main`, private registry), one API
host. When that host stops answering, every new swap stops, exactly as when
Boltz went down. [source] [public]

Satora's own regtest environment ships Boltz images as an internal
component of its Lightning leg, so the Boltz takedown plausibly cascaded
into this outage; the available evidence shows the dependency, not the
cause of the 502. [inferred] [limitation]

The pattern across Boltz and Satora is now established: **the atomic
settlement layer keeps working — script, preimage, timelock, refund — while
the discovery/quote/coordination layer is a single point of failure.** That
coordination layer is precisely what NIP-MKT moves onto relays and many
independent providers. This teardown extracts what Satora adds beyond the
Boltz and tbDEX harvests.

## 1. Outage evidence

| Probe (2026-08-04) | Result |
| --- | --- |
| `https://satora.io` | 200 |
| `https://swap.lendasat.com` | 200 |
| `https://docs.satora.io` | 200 |
| `https://api.satora.io` | **502** |

The SDK's status stream targets `wss://api.satora.io/ws`, and the regtest
environment names the backend image in a private registry, so the 502 host
is the coordinator for quote, swap creation, and status — not a marketing
surface. Latest repo activity: `satora-sdk` and `lendaswap-frontend` both
2026-08-03. No public shutdown announcement was found at review time; the
outage may be temporary. The architectural lesson does not depend on the
duration. [source] [public]

## 2. What Satora is

### 2.1 The swap architecture

Client SDKs (`@satora/swap`, wrapping the legacy
`@lendasat/lendaswap-sdk-pure`) hold the user's HD keys, preimages, and swap
state locally with pluggable storage. The backend coordinates quotes and
provides the counterparty liquidity. Settlement runs over HTLCs on both
legs:

- **EVM leg** (`lendaswap-contracts`, MIT, deployed on Polygon, Ethereum,
  Arbitrum): `HTLCErc20` locks ERC-20 tokens against a SHA-256
  `preimageHash` — deliberately compatible with Bitcoin HTLC scripts — with
  a `claimAddress` binding that prevents front-running and a minimal-storage
  design (one `bool` per swap; all parameters hash-verified on
  redeem/refund). Redeem and refund accept EIP-712 signatures, so the
  claimer needs no gas token, and Permit2 gives gasless approvals.
  `HTLCCoordinator` composes arbitrary calls with the HTLC in one
  transaction: `executeAndCreate` (e.g. Uniswap USDC→WBTC, then lock),
  `redeemAndExecute`, `refundAndExecute`. [source]
- **Bitcoin legs**: on-chain script HTLCs, Lightning (with Boltz components
  inside the leg per the regtest environment), and Arkade — Ark-protocol
  VTXOs giving instant pre-confirmations with batched on-chain settlement.
  [source]
- The last pre-outage commit made EVM→BTC claims proceed at 0-conf instead
  of waiting for a block — a live example of a confirmation policy changing
  as a provider-side default rather than a quoted, taker-visible term.
  [source]

### 2.2 The surrounding ecosystem

- **Lendasat** — the origin product: peer-to-peer Bitcoin-collateralized
  loans with collateral contracts on Bitcoin. Its whitepaper ships in the
  lane. [source]
- **doomsday** — a standalone recovery tool: "If Lendasat disappears, use
  this tool to recover your contract funds." Both counterparties derive
  their contract keys from client-held encrypted backups, coordinate **over
  Nostr using derived nsec/npub identities**, build a cooperative PSBT, and
  broadcast — no company infrastructure required. [source]
- **Striker** — a design for an intent-based swap layer on Arkade: makers
  deposit liquidity once into a covenant and sign quotes off-chain; the
  coordinator polls makers and returns the best signed quote; the taker
  locks funds in a covenant that guarantees at least the quoted amount;
  settlement is atomic; makers can be filled while offline; the coordinator
  never custodies funds. Self-described as NEAR Intents rebuilt on Bitcoin.
  Its README records why the previous on-chain-orderbook model failed:
  prices welded to specific outputs made every requote an on-chain action.
  [source]
- **Arkade** — wallet PWA plus TypeScript/Rust SDKs speaking to any `arkd`,
  and escrow samples (Lightning⇄Ark 2-of-2 and 2-of-3) showing coordinated
  hold/escrow constructions on the Ark rail. [source]
- **Distribution surfaces** — a BTCPay Server plugin, an embeddable iframe
  widget, a Tether WDK "swidge" provider (`SatoraProtocol` plugs into WDK
  wallets as a standard swap provider), and wallet integrations
  (arkade-wallet's LendaSwap feature, layerzwallet, alby-hub fork). [source]

## 3. Harvest map — what Satora adds beyond Boltz and tbDEX

Boltz contributed BTC/Lightning/Liquid settlement physics; tbDEX
contributed the negotiation grammar and conformance vectors. Satora's
additions:

| Item | What it is | Where it lands |
| --- | --- | --- |
| EVM-HTLC leg | MIT contracts proving the BTC↔EVM atomic pair: SHA-256 preimage compatibility across legs, claim-address front-running protection, hash-verified minimal storage, gasless EIP-712 redeem/refund, Permit2 | The stablecoin leg neither prior donor covered. A future **MKT-SWP-EVM** extension (or v2 of the SWP draft) reserves the vocabulary: chain id, contract address, token, claim/refund signature mode, confirmation policy. Reference verifier shapes for the profile's EVM evidence class |
| Gasless-claim pattern | Taker redeems with an EIP-712 signature; no gas token needed on the destination chain | Quote-level field in the EVM extension: who pays execution costs is a disclosed term |
| 0-conf policy change | Provider-side default changed the day before the outage | Confirms an MKT law already drafted: confirmation and RBF policy are quoted, taker-verified terms with provenance labels — never a provider default the taker discovers later |
| Doomsday recovery law | Counterparty-only fund recovery from client-held backups, coordinated over Nostr identities, with cooperative PSBT signing | The strongest validation yet of the MKT recovery rules. Adds a concrete acceptance test: **the doomsday drill** — every executable profile must prove both parties can reach the correct terminal state with the coordinator permanently gone, using only persisted signed records and the counterparty channel. Their reach for Nostr as the recovery rendezvous is independent confirmation of the fabric choice |
| Striker quoting model | Off-chain signed quotes + covenant-guaranteed minimum + maker-offline fills + no-custody coordinator | Direct input to reservation semantics: a covenant-enforced reserve is a rail-proof for a `hard` reservation (stronger than a signed claim); signed off-chain quotes match the MKT `firm` quote; the orderbook-of-outputs failure is a recorded anti-pattern for any on-relay orderbook idea |
| Arkade escrow samples | 2-of-2 / 2-of-3 Lightning⇄Ark escrow constructions | Primitives for the A1 (coordinated hold/escrow) custody class on the Ark rail; Ark bridge already listed in the tbDEX teardown's ecosystem table |
| Regtest devenv | Compose environment: bitcoin regtest + electrs, LND + CLN, arkd + wallet, Boltz components, the swap backend | Shape reference for the Immortal adversarial lab (immortal#18) and local dev env (#9, shipped): the external-nodes-beside-the-relay pattern, including an EVM leg when the extension lands |
| Distribution surfaces | BTCPay plugin, iframe embed, WDK swidge provider interface | Adoption channels for the Immortal-based network: the same three surfaces can sit on the generated SDK / Boltz-compatible facade so merchants and WDK wallets consume the multi-provider market without new integration work |
| Nostr identity in product | `lendasat-sdk` exposes `Wallet::npub`; doomsday coordinates via derived npubs | They already used Nostr keys as durable user identity in a financial product — precedent for MKT identity being the wallet's Nostr key, not an account at the coordinator |
| Lendasat loan design | BTC-collateralized loans with client-recoverable collateral contracts | Candidate future profile adjacent to NIP-AC agent credit — recorded, not scheduled |

## 4. What we do not harvest

- **The coordinator model**: closed-source backend in a private registry,
  one API host, company-operated liquidity. This is the failure the
  Liquidity Market exists to remove — replaced by public Offerings, private
  RFQ/Quote over relays, and many independent providers.
- **License-restricted repos as code donors**: `striker`, `doomsday`,
  `arkade-wallet`, `lendasat-sdk`, and `regtest-devenv` carry no license
  file — ideas and laws only, no copied code. The MIT contracts and SDK and
  the Apache WDK bridge are the only permissible code-reference donors,
  with attribution. [source]
- **DEX composition on the critical path**: `executeAndCreate`'s arbitrary
  call execution is powerful and widens the attack surface (the taker's
  verification must now cover the composed call). If the EVM extension ever
  admits composition, it is a separately disclosed, separately verified
  term — not a default.
- **Account-model semantics into the relay**: gas sponsorship, permit
  flows, and EVM addresses stay in profile adapters and wallet code; the
  relay keeps validating events, never chain state.

## 5. The pattern, stated once

| | Boltz | Satora | The NIP-MKT replacement |
| --- | --- | --- | --- |
| Settlement | Non-custodial scripts; kept working | Non-custodial HTLCs; kept working | Unchanged — profiles bind the strongest rail guarantee |
| Discovery/quotes | One company API | One company API (closed backend) | Public heads on relays; any provider can advertise |
| Negotiation | Provider REST/WS | Provider REST/WS | Signed private RFQ/Quote/Order over gift wrap; portable across providers |
| Liveness | Company infrastructure | Company infrastructure | Many providers, many relays; one failure narrows the market instead of closing it |
| Recovery | Client refund paths | Client refund paths + a separate doomsday tool | Protocol-level: persisted signed records + counterparty channel; the doomsday drill is an acceptance test, not an afterthought |

Two independent, well-engineered, non-custodial swap services have now
demonstrated the same availability failure within days of each other. The
settlement layer was never the problem. The market layer is the product.

## 6. Roadmap impact

Concrete deltas proposed against the current programs (dispositions
recorded here; the owning issues carry the work):

1. **MKT-SWP draft scope (openagents#9311):** keep v1 BTC/Lightning as
   drafted, and reserve the EVM-leg vocabulary (chain id, contract, token,
   signature-mode, confirmation-policy fields) so an MKT-SWP-EVM extension
   can land without a breaking revision. The BTC↔stablecoin pair is the
   demand Satora served and the Episode 266 thesis ("stablecoins won")
   already anticipates.
2. **Doomsday drill (immortal#12, #18):** add the coordinator-permanently-
   gone recovery scenario as a named acceptance case in the client engine
   and the adversarial lab — both parties reach the correct terminal state
   from persisted records plus the counterparty channel alone.
3. **Reservation rail-proofs (immortal#13, #9311):** admit covenant-
   enforced reserves (Striker-style) as a `hard`-reservation proof class in
   the profile vocabulary.
4. **Lab shape (immortal#18):** reuse the regtest-devenv service topology
   as the reference for the external-nodes matrix; add the EVM leg when the
   extension exists.
5. **Distribution follow-through (openagents#9310 and later):** BTCPay
   plugin, iframe embed, and a WDK swidge provider are the three adoption
   surfaces to build on the generated SDK once the network runs — recorded
   as future candidates, not scheduled work.
6. **Reference lanes to pull next:** the `arkade-os` org (`arkd`, wallet
   daemon, protocol docs) — the satora lane carries Arkade clients but not
   the Ark server implementation the samples speak to; and `tetherto/wdk`
   as the wallet-distribution interface reference.

---

*End of teardown. Research and candidate protocol only; no market,
provider, financial authority, or deployment is created by this document.*
