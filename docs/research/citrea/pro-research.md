[![Citrea](https://tse4.mm.bing.net/th/id/OIP.-teiqOEzwYXj9p9_oz9QsgHaEK?cb=defcache2\&pid=Api\&defcache=1)](https://citrea.xyz/)

# Citrea × OpenAgents Integration Report

This report treats Citrea as a **new “rail + liquidity venue”** you can plug into OpenAgents’ **Neobank (TreasuryRouter)** and **Exchange** layers—while also exploring a bigger idea: **Citrea can let OpenAgents reuse its existing Schnorr/Nostr identity for on-chain control**, because Citrea ships a **BIP340 Schnorr verification precompile**. That is a *very* rare overlap between “Bitcoin-native identity” and “EVM execution.”

---

## 1) What Citrea is (and why it matters to OpenAgents)

### Citrea in one paragraph

Citrea is a **Bitcoin Layer 2** positioned as a **ZK rollup** that uses Bitcoin as the **settlement and data availability anchor**, while offering an **EVM-compatible execution layer** (so you can run Solidity apps). Its mainnet went live in late Jan 2026, with two flagship assets:

* **cBTC**: “Bitcoin on Citrea” minted via Citrea’s native **Clementine** bridge (BitVM + ZK, “trust-minimized” framing).
* **ctUSD**: a **native stablecoin for the Citrea/Bitcoin app ecosystem**, issued by **MoonPay** and powered by **M0**, aiming at compliance + global distribution. ([Citrea · Blog][1])

### Why this is strategically relevant for OpenAgents

OpenAgents’ roadmap explicitly wants:

* **USD-denominated budgets + receipts + auditability** (Neobank)
* **Liquidity + FX routing + hedging** (Exchange)
* **Sovereign agents** with keys operators can’t extract

Citrea is directly aiming at “Bitcoin capital markets”—**lending, trading, settlement** on a Bitcoin-anchored L2. ([Citrea · Blog][1])
So in OpenAgents terms: Citrea looks like a **high-power financial substrate** you can optionally route through when:

* You want **on-chain DeFi liquidity** (BTC↔USD swaps, lending/borrowing, structured products)
* You want **a stablecoin rail** that comes with **fiat on/off-ramps**
* You want **programmability for escrow/policy** that’s harder on LN/Cashu alone

---

## 2) Citrea’s architecture and trust model (what you’d be inheriting)

### 2.1 Rollup lifecycle: soft confirmations → Bitcoin-anchored finality

Citrea docs describe a transaction lifecycle with:

* **Soft confirmations** from a sequencer for fast UX,
* Periodic posting of **commitments / state differences** and then **ZK “batch proofs”** to Bitcoin,
* Finality derived from Bitcoin confirmation depth. ([Citrea][2])

A key detail for OpenAgents’ “receipts” and audit trails:

* Citrea states that **batch proofs are posted to Bitcoin** and include **state differences**, chunked to fit size constraints (they mention staying under ~400kB per Bitcoin tx and potentially multiple proofs per block). ([Citrea][2])

**OpenAgents implication:** You can treat Citrea as a rail whose “ultimate settlement proof” is **Bitcoin-backed** (with Citrea’s proof system in the middle). That’s conceptually aligned with your bias for Bitcoin as the root of trust, but it’s still a multi-component system you must model explicitly (sequencer, prover, bridge, etc.).

### 2.2 Clementine bridge (BTC ↔ cBTC): “trust-minimized” BitVM2 design

Citrea frames Clementine as a **native two-way peg** between BTC and cBTC, using **BitVM2** for *optimistic verification of ZK proofs on Bitcoin* without soft forks. ([Citrea][3])

The docs explicitly describe the trust posture as “**1-of-N honesty**” for safety:

* One honest **Signer** to keep funds on pre-approved paths
* One honest **Watchtower** to block wrong-chain claims
* One rational **Challenger** to prove invalid computation and slash collateral ([Citrea][3])

They also describe the **role decomposition** (users, signers, operators, watchtowers, challengers), plus peg-in/peg-out flows and dispute game mechanics. ([Citrea][3])

**OpenAgents implication:** This is closer to your “explicit rail risk profile” model than many BTC bridges, but it’s still non-trivial operationally. If OpenAgents treats Cashu mints as counterparty risk, Clementine becomes a different kind of risk: **protocol + committee + dispute game** risk.

### 2.3 Signer set transparency (practical due diligence hook)

Citrea publicly lists Clementine “signers/verifiers” and their public keys—names include **Chainway Labs, Galaxy, Nansen, Nethermind, Luxor, Finoa, Hashkey Cloud**, etc. ([Citrea][4])

**OpenAgents implication:** This is very compatible with a Neobank-style policy engine:

* default allowlist/denylist
* risk caps by “bridge/signer-set”
* tiered routing by transaction size or required guarantees

---

## 3) The two Citrea features that are “uniquely synergistic” with OpenAgents

If you only remember two things for integration design, make them these:

### 3.1 Citrea ships a BIP340 Schnorr verification precompile

Citrea enables a **Schnorr signature verification precompile** at address `0x0000000000000000000000000000000000000200`, explicitly referencing **BIP340** and noting it enables things like **scriptless cross-chain atomic swaps** and “Bitcoin-aware oracles & bridges.” ([Citrea][5])

This is huge because:

* **Nostr event signatures are Schnorr over secp256k1** (BIP340-style).
* OpenAgents’ sovereignty stack centers on **FROST/FROSTR** and Nostr identity.
* This creates a realistic path to **control Citrea smart accounts using the same Schnorr identity**, without needing to give agents an ECDSA EOA key.

### 3.2 Citrea also ships a secp256r1 (passkey) precompile (RIP-7212)

Citrea documents a **secp256r1** precompile at `0x0000000000000000000000000000000000000100`, referencing **RIP 7212**, and explicitly calls out **Secure Enclave / WebAuthn / passkeys** and **account abstraction smart wallets** as use cases. ([Citrea][5])

**OpenAgents implication:** You can map OpenAgents’ *graduated autonomy / approvals / guardian model* onto:

* **agent Schnorr key** (autonomous execution)
* **human passkey** (approval or recovery)
* enforced **on-chain** via account abstraction

This is a rare alignment between:

* your “operator can’t extract keys” ethos
* practical human approvals that don’t require seed phrases

---

## 4) ctUSD: why it looks like a “Neobank bridge” more than a typical stablecoin

Citrea positions **ctUSD** as “banking rails between on-chain Bitcoin collateral and off-chain fiat systems” in its mainnet announcement. ([Citrea · Blog][1])

The ctUSD announcement goes deeper:

* ctUSD is **1:1 backed by short-term U.S. Treasuries and cash equivalents** (as described by Citrea). ([Citrea · Blog][6])
* It is **issued by MoonPay** and powered by **M0**. ([Citrea · Blog][6])
* Distribution claim: MoonPay has “**30 million+ verified users**” and offers multiple payment methods (cards, Apple Pay, bank transfers, etc.). ([Citrea · Blog][6])
* They describe **virtual accounts (vIBANs)** via Iron (for ACH/wire), and integrations like **Helio** for merchant tooling/cards. ([Citrea · Blog][6])
* Availability claims include geographic restrictions (U.S. excluding NY; 160+ countries; excludes Canada/EEA). ([Citrea · Blog][1])

**OpenAgents implication:** ctUSD is not just “a token.” It’s presented as a **fiat interface layer**:

* That’s *exactly* what your Neobank layer needs when talking to enterprises that budget in USD and want receipts/invoices.
* But it also implies **compliance, KYC, jurisdiction constraints**, and a very different trust model than Cashu or LN.

---

## 5) Integration opportunities mapped to OpenAgents primitives

Below are concrete ways Citrea can plug into your architecture, grouped by your primitives.

---

# A) Neobank / TreasuryRouter: add a “Citrea rail”

### The idea

Add **Citrea** as an additional **Rail** alongside:

* `BTC_LN` (Lightning)
* `BTC_CASHU(mint_url)`
* future `USD_TA(...)` (Taproot Assets)
* **new**: `CITREA_EVM(chain_id=4114)` with assets like `cBTC` and `ctUSD`

Citrea provides:

* a BTC-like asset on an app layer (**cBTC** via Clementine) ([Citrea · Blog][1])
* a USD stablecoin rail with fiat interfaces (**ctUSD**) ([Citrea · Blog][6])

### What TreasuryRouter can do with it

**Policy routing examples:**

* Spend caps:

  * “ctUSD exposure max $X”
  * “cBTC exposure max Y sats”
* Vendor routing:

  * Pay DeFi venues / on-chain counterparties via Citrea
  * Pay compute providers via LN (default), *but* replenish LN liquidity by swapping on Citrea when needed
* Approval gating:

  * Small routine swaps auto-approved
  * New contracts / unlimited approvals require guardian sign-off

### Why this is compelling

It gives you a realistic path to:

* **USD budgets that actually settle in USD-like units** (ctUSD), not just “USD-denominated accounting”
* **programmable escrow** for agent markets (especially your future Exchange layer)

---

# B) Sovereign agents: “Schnorr-native smart accounts” on Citrea

This is the most interesting integration.

### Problem: EVM wants ECDSA EOAs; OpenAgents wants threshold Schnorr keys

Normally, an agent interacting with an EVM chain needs:

* an **ECDSA private key** (EOA), or
* an account abstraction smart wallet whose ownership can be proven by something else

OpenAgents prefers:

* threshold-protected keys (FROST/FROSTR)
* Nostr-native identity (Schnorr)

### Citrea unlock: verify Schnorr (BIP340) signatures in contracts cheaply

Citrea’s Schnorr precompile makes it practical to build a smart account where “owner auth” is a **BIP340 Schnorr signature check**. ([Citrea][5])

### Concrete design: OpenAgents Citrea Smart Account (meta-tx)

1. Deploy `OpenAgentsAccount` contract (ERC-4337 compatible or simple forwarding wallet).
2. For every action, the agent signs an **OpenAgents Authorization Message**:

   * `hash = H(domain_sep || chain_id || nonce || call_data_hash || policy_context_hash)`
   * signature is **Schnorr** produced by the agent’s FROST/FROSTR key
3. A relayer/paymaster submits tx, contract verifies signature via `0x...0200`, then executes call.

**Benefits:**

* Agent never needs to hold an ECDSA key.
* The same cryptographic identity used for Nostr can authorize on-chain actions.
* You can embed OpenAgents policy context (budget, purpose, trajectory id) directly into signed payloads, improving receipts/auditability.

### Layered approvals: combine Schnorr agent auth + passkey human auth

Citrea’s secp256r1 precompile enables passkey verification on-chain. ([Citrea][5])
You can enforce:

* **low-risk actions:** agent Schnorr signature only
* **high-risk actions:** agent Schnorr + human passkey co-sign
* **recovery mode:** human passkey can rotate agent key / freeze

This mirrors your **autonomy graduation** model, but “hard-enforced” at the asset layer.

---

# C) Exchange / FX / Routing: use Citrea liquidity venues as “makers of last resort”

Citrea mainnet messaging highlights:

* Trading via DEX access (they name Satsuma, JuiceSwap, Fibrous)
* Lending markets (they name Morpho; plus others “soon”)
* Structured products/yield via institutional partners ([Citrea · Blog][1])

Even if OpenAgents’ **Exchange** is primarily Nostr-native RFQ, Citrea can act as:

* a **pricing oracle venue** (where do markets clear?)
* a **hedging venue** for Treasury Agents managing BTC risk
* a **liquidity backend** when LN/Cashu liquidity is thin

### Integration patterns

1. **Treasury Agent backend strategy**

* Treasury Agent holds balances in ctUSD and cBTC on Citrea
* Quotes agent-to-agent FX on Nostr
* When filled, executes:

  * on-chain swap (cBTC↔ctUSD),
  * then pays LN invoice / delivers eCash / etc

2. **OpenAgents “Liquidity Router” plugin**

* TreasuryRouter asks multiple venues for quotes:

  * internal Nostr makers
  * Cashu mint-based OTC
  * Citrea on-chain swap route
* picks best after applying policy weights (price, trust, latency, compliance)

---

# D) Payments: bridging Lightning world ↔ Citrea world

Citrea’s docs explicitly list **“Lightning Integration”** and “Trustless Atomic Swaps” as future research topics, and the Schnorr precompile page calls out **scriptless cross-chain atomic swaps** as a direct use case. ([Citrea][7])

**OpenAgents implication:** There’s conceptual room for a “unified settlement story” where:

* micro-payments stay on LN (your default)
* larger settlement / escrow / hedging happens on Citrea
* trust boundaries are explicit and receipts are linkable

A practical near-term path (even before trustless atomic swaps are productized):

* a Treasury Agent can pay LN invoices and rebalance using Citrea liquidity

---

## 6) Technical integration checklist (what you would actually build)

### 6.1 Network parameters (wallet + infra)

Citrea is EVM-compatible and publishes chain info including **mainnet chain id** and RPC endpoints in its docs. ([Citrea][8])

You’d add Citrea to the OpenAgents wallet as an EVM chain (example shape):

```json
{
  "chainId": 4114,
  "chainName": "Citrea Mainnet",
  "rpcUrl": "https://rpc.mainnet.citrea.xyz",
  "nativeCurrency": { "name": "cBTC", "symbol": "cBTC", "decimals": 18 }
}
```

*(Use Citrea docs as source of truth for the exact RPC/explorer values.)* ([Citrea][8])

### 6.2 “Rail” abstraction changes in OpenAgents Neobank

Add:

* `Rail::CitreaEvm { chain_id: 4114, rpc_url, ... }`
* Asset IDs:

  * `AssetId::cBTC_CITREA`
  * `AssetId::ctUSD_CITREA`
* Risk metadata:

  * `trust_profile = { bridge=Clementine, stablecoin=MoonPay/M0, execution=Citrea zkEVM }`

### 6.3 Signing and custody model (most important part)

You have three options:

**Option 1 — Fastest/boring:** derive an ECDSA key for Citrea like a normal EVM wallet.
Downside: conflicts with “operator can’t extract keys” unless you implement MPC ECDSA.

**Option 2 — Best alignment:** Schnorr-controlled smart account (recommended).
Use Citrea Schnorr precompile (`0x…0200`) to verify BIP340 signatures and execute meta-transactions. ([Citrea][5])

**Option 3 — Hybrid:** Safe (multisig) + relayer.
Citrea explicitly supports Safe wallet deployments. ([Citrea][7])
You can enforce spending rules at the wallet layer, but the identity alignment is weaker than Option 2.

### 6.4 Account abstraction + paymasters (gas UX)

Citrea docs note ERC-4337 infra availability via **Pimlico bundler/paymaster** endpoints (at least on testnet per docs) and highlight AA ecosystem tooling. ([Citrea][7])

For agents, paymasters matter because:

* You can sponsor gas for approved actions (or charge gas in ctUSD off-chain)
* You can smooth UX so agents aren’t constantly rebalancing cBTC for fees

### 6.5 Receipts and audit trail integration

For every Citrea action (swap, lend, escrow):

* Write a **Receipt** object containing:

  * Citrea tx hash
  * block number + timestamp
  * asset deltas (ctUSD/cBTC)
  * policy rule id + budget bucket id
  * trajectory session id
* Publish receipt hash into Nostr trajectory events (or store in `REPLAY.jsonl`)

Because Citrea ultimately anchors proofs/data to Bitcoin, this is philosophically aligned with your “cryptographic receipts” framing, but you still want local/agent receipts for operational truth. ([Citrea][2])

---

## 7) Risk analysis: where Citrea helps vs where it adds complexity

### What Citrea improves (relative to “Lightning + Cashu only”)

* **Full programmability** for escrow, conditional settlement, portfolio logic
* **Deep DeFi composition** potential (swap/lend/structured products) ([Citrea · Blog][1])
* **Stablecoin with fiat rails** (ctUSD) that feels like a neobank interface ([Citrea · Blog][6])
* **Identity compatibility via Schnorr precompile** (rare, high leverage) ([Citrea][5])

### What it adds (risks / costs)

* **Bridge risk**: Clementine is a sophisticated mechanism with committees, operators, dispute games (even if “trust-minimized”). ([Citrea][3])
* **Execution risk**: smart contract risk + DeFi risk
* **Operational risk**: RPC/indexer uptime, chain upgrades (e.g., Tangerine upgrade introduced the precompiles) ([Citrea][5])
* **Compliance surface**: ctUSD availability and on/off ramps imply jurisdiction + KYC constraints (depending on how users acquire it). ([Citrea · Blog][1])

### OpenAgents-native mitigations

Citrea fits your architecture best if you:

* treat it as a **distinct rail** with explicit risk caps
* use **TreasuryRouter policy** to gate:

  * max exposure
  * contract allowlists
  * size thresholds requiring approvals
* start with **Treasury Agent specialists** rather than giving every agent direct Citrea autonomy on day one

---

## 8) Recommended integration plan (phased, realistic)

### Phase 1 — “Citrea as a quoted venue” (no agent custody yet)

* Build a **Citrea Liquidity Adapter** that can:

  * read on-chain prices (DEX quotes)
  * estimate swap costs + fees
* Use it inside Exchange RFQs as a *reference price* (don’t execute yet)
* Outcome: You learn liquidity/volatility without key-management complexity.

### Phase 2 — “Treasury Agent runs Citrea execution” (centralized execution, decentralized settlement)

* One (or a few) OpenAgents-run **Treasury Agents** custody Citrea positions (ctUSD/cBTC).
* They quote and execute swaps/lending, then settle to the rest of the network via LN/Cashu.
* Add strict policy:

  * per-venue allowlist
  * exposure caps
  * circuit breakers (if bridge halts / spreads widen)

### Phase 3 — “Schnorr smart accounts for sovereign agents” (the real unlock)

* Deploy **OpenAgentsAccount** on Citrea:

  * authorize via Schnorr precompile (`0x…0200`) ([Citrea][5])
  * optionally require human passkey co-sign via secp256r1 precompile for high-risk ops ([Citrea][5])
* Integrate ERC-4337 paymasters for gas abstraction (where available). ([Citrea][7])
* Now each agent can truly be a **sovereign on-chain actor** without introducing an extractable ECDSA key.

### Phase 4 — “Neobank-grade fiat interface”

* Treat ctUSD as the **enterprise-facing unit of account**:

  * ACH/wire → ctUSD balances → budget buckets → receipts/invoices (as per your Neobank plan)
* This is only worth doing once OpenAgents has real enterprise demand, because ctUSD distribution/compliance is a feature *and* a constraint. ([Citrea · Blog][6])

---

## 9) What I’d build first if the goal is “tie-ins to neobanks / sovereign agents / trading”

If you want one concrete “high ROI” integration bet:

### Build: OpenAgents Citrea Smart Account (Schnorr-controlled) + TreasuryRouter adapter

**Why:** It converts Citrea from “another chain we might support” into “a chain where OpenAgents’ native identity works.”

**MVP scope:**

* Deploy `OpenAgentsAccount` contract:

  * `execute(call, nonce, schnorr_sig)`
  * verify via Schnorr precompile
* Add a TreasuryRouter “Citrea rail” implementation for:

  * querying balances (ctUSD, cBTC)
  * sending meta-txs through relayer
  * producing receipts
* Add a minimal swap skill:

  * swap ctUSD ↔ cBTC (or vice versa) on one venue (initially whichever has best docs/liquidity)

Then, once this exists, “neobank” and “trading” become add-ons:

* Neobank: ctUSD budgeting + fiat ramps
* Trading: swap routing + risk controls + hedging + yield

---

## Appendix: Key Citrea facts used in this report (for quick reference)

* Citrea mainnet announcement (Jan 27 2026): positions Citrea as Bitcoin application layer for lending/trading/settlement; highlights cBTC via Clementine and ctUSD via MoonPay/M0. ([Citrea · Blog][1])
* ctUSD details (Jan 15 2026): 1:1 backing claim; MoonPay issuance/distribution; mentions rails like virtual accounts and merchant tooling. ([Citrea · Blog][6])
* Rollup lifecycle: soft confirmations + batch proofs posted to Bitcoin; state diffs + proof chunking constraints discussed. ([Citrea][2])
* Clementine bridge: BitVM2 framing; 1-of-N honesty model; roles; peg-in/peg-out flows; dispute game. ([Citrea][3])
* Clementine signer set: named entities + public keys published. ([Citrea][4])
* Precompiles: secp256r1 (RIP-7212) at `0x…0100` and Schnorr (BIP340) at `0x…0200`. ([Citrea][5])
* Account abstraction tooling: docs mention Pimlico (bundler/paymaster), Safe wallet availability. ([Citrea][7])

---

If you want, I can also sketch the **exact contract interface + message format** I’d use for the Schnorr-based OpenAgentsAccount so it cleanly maps to your existing **FROSTR signing** and **receipt/trajectory** objects (no new key types, no ECDSA, no “operator extraction” loopholes).

[1]: https://www.blog.citrea.xyz/citrea-mainnet-is-live/ "https://www.blog.citrea.xyz/citrea-mainnet-is-live/"
[2]: https://docs.citrea.xyz/essentials/architecture-and-transaction-lifecycle "https://docs.citrea.xyz/essentials/architecture-and-transaction-lifecycle"
[3]: https://docs.citrea.xyz/essentials/clementine-trust-minimized-bitcoin-bridge "https://docs.citrea.xyz/essentials/clementine-trust-minimized-bitcoin-bridge"
[4]: https://docs.citrea.xyz/advanced/clementine-signers "https://docs.citrea.xyz/advanced/clementine-signers"
[5]: https://docs.citrea.xyz/developer-documentation/schnorr-secp256r1 "https://docs.citrea.xyz/developer-documentation/schnorr-secp256r1"
[6]: https://www.blog.citrea.xyz/introducing-citrea-usd-ctusd-the-native-stablecoin-for-bitcoin-issued-by-moonpay-and-powered-by-m0/ "Introducing Citrea USD (ctUSD): The Native Stablecoin for Bitcoin, Issued by MoonPay and Powered by M0"
[7]: https://docs.citrea.xyz/developer-documentation/ecosystem-tooling/wallets-aa "https://docs.citrea.xyz/developer-documentation/ecosystem-tooling/wallets-aa"
[8]: https://docs.citrea.xyz/developer-documentation/chain-information "https://docs.citrea.xyz/developer-documentation/chain-information"
