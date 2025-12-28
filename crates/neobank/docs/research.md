Below is a research-backed spec for turning **OpenAgents** into an **“agentic neobank”**—a banking-like control plane for *humans + fleets of agents* that stays **Bitcoin-first**, supports **stable unit-of-account**, and remains **modular on custody + compliance**.

I’m going to frame this around **three money layers** that are already emerging on Bitcoin/Nostr:

1. **BTC rails** (Lightning + on-chain) for native, permissionless machine-money
2. **Stablecoin rails on Bitcoin** via **Taproot Assets** (and Lightning) for “dollar UX”
3. **eCash rails** (Cashu/Fedimint) for **privacy + cash-like UX**, with Nostr-native discoverability & wallet state

---

## What the ecosystem is doing (why this matters)

### 1) Taproot Assets is becoming the “stablecoins on Bitcoin” path

Taproot Assets (Lightning Labs’ protocol) is explicitly aimed at issuing assets on Bitcoin and moving them over Lightning. Lightning Labs positions it as a multi-asset Lightning protocol on mainnet that can move stablecoins instantly for low fees. ([Lightning Engineering][1])

Recent protocol milestones matter for a neobank UX:

* **Grouped assets / group keys**: designed for stablecoins minted in multiple batches while remaining fungible; group_key is usable for Lightning flows (funding channels, invoices, receive). ([Lightning Engineering][1])
* **AddressV2 reusable addresses**: static “set-and-forget” addresses that can receive a specific stablecoin repeatedly; supports grouped assets and optional zero-amount addresses. ([Lightning Engineering][2])
* **Supply commitments**: on-chain attestations meant to make total supply auditable (useful for “proof-of-supply” style transparency). ([Lightning Engineering][3])
* **Burn** primitives exist (reducing supply irreversibly by sending to an unspendable script key). ([Lightning Engineering][4])

### 2) Tether is explicitly moving USDT onto Bitcoin + Lightning via Taproot Assets

Tether publicly announced bringing **USDt** to Bitcoin’s base layer and the Lightning Network, **powered by Taproot Assets** (Lightning Labs). ([Tether][5])
Lightning Labs frames this as enabling Lightning merchants/gateways to add USDT using the same infrastructure and explicitly calls out “AI agent transactions” as a target use case. ([Lightning Engineering][6])

Implication: even if OpenAgents never issues its own stablecoin, **supporting Taproot-Assets stablecoins** could quickly become table-stakes for “agentic commerce”.

### 3) eCash is being standardized in Nostr (discovery + wallet state + payments-as-receipts)

You already pasted the key NIPs, and they’re extremely aligned with your broader vision:

* **NIP-87** adds mint discoverability + recommendations:

  * Mints publish announcements (`kind:38172` Cashu / `kind:38173` Fedimint).
  * Users publish recommendation events (`kind:38000`) pointing to mint announcements. ([NIPs][7])
* **NIP-60** defines Cashu wallet state stored on relays:

  * Wallet event `kind:17375`, unspent proofs `kind:7375`, optional history `kind:7376`, encrypted via NIP-44. ([NIPs][8])
* **NIP-61** defines Nutzaps:

  * P2PK Cashu token where “payment is the receipt”; clients can filter by expected mints and track redemption via `kind:7376`. ([NIPs][9])
* **Cashu** itself is positioned as Chaumian ecash for Bitcoin: bearer tokens, privacy via blind signatures, instant/near-free transfers, mint+wallet architecture. ([Cashu][10])
* **Fedimint** is Chaumian eCash with a *federation of guardians* instead of a single custodian, interoperable with Lightning. ([Fedimint][11])

Implication: Nostr already has a credible path to **cash-like, private, programmable “app money”** for agents—especially for micropayments, tips, and privacy-sensitive flows.

### 4) “Crypto neobanks” are converging on the same stack: self-custody + stablecoins + cards + ramps

There’s a visible surge of products that “feel like a neobank” while keeping crypto-native balances, often bridging to cards and fiat rails. ([Bankless][12])

Two important infra patterns:

* **Stablecoin issuance as a service**: Bridge/Stripe “Open Issuance” pitches launching stablecoins quickly, with reserve management + compliance tooling, and mentions first use cases like Phantom launching a stablecoin. ([Stripe][13])
* **Cards-as-an-adapter**: providers like Rain pitch stablecoin-linked card programs via API. ([Rain][14])
  And there are self-custody card approaches (e.g., Gnosis Pay positioning a self-custodial Visa debit card linked to a smart account). ([Gnosis Pay][15])

---

## The core design insight for OpenAgents

**OpenAgents-as-neobank isn’t “a bank.” It’s a programmable treasury + payments router for agents**, with:

* **Self-custody by default** (agent/human keys; threshold-protected)
* **Stable unit-of-account** where it matters (budgets, invoices, payroll, pricing)
* **Pluggable regulated adapters** (fiat ramps, card issuance, compliance signers) where required
* **Everything auditable** (trajectory logs + signed payment receipts + event trails)

That’s exactly the “interface on top of regulated rails” play—but in your case, the “users” include autonomous software entities.

---

# Spec: OpenAgents Agentic Neobank

## 1) Product definition

### What it is

**OpenAgents Neobank** = a **Treasury OS** for people and agent fleets:

* Creates and manages **agent operating accounts**
* Holds **BTC + stable assets on Bitcoin + eCash balances**
* Enforces **budgets, approvals, spending policies**
* Routes payments across **Lightning / Taproot Assets / eCash / fiat adapters**
* Exposes receipts + statements for humans, APIs for agents

### What it is not

* Not fractional reserve.
* Not “deposit-taking and lending” unless you *choose* to add regulated yield products later.
* Not a closed ledger: your “ledger” is **verifiable receipts** + **local accounting views**, not a proprietary bank database.

---

## 2) Money rails you support

### Rail A: Native Bitcoin (baseline)

**Use for:** machine-to-machine micropayments, censorship resistance, settlement finality.

* **Lightning** (you already have this in the vision: NIP-57 zaps, L402, Spark)
* **On-chain BTC** (treasury settlement, large transfers, “birth/death” events, long-term reserves)

### Rail B: Stablecoins on Bitcoin via Taproot Assets

**Use for:** stable budgeting, enterprise procurement, payroll, predictable pricing, “neobank UX”.

Key capabilities you should design around:

* Hold/send/receive Taproot Assets on-chain
* Pay/receive over Lightning using Taproot Assets
* Track assets by **group_key** for stablecoin fungibility across issuance batches ([Lightning Engineering][1])
* Use **AddressV2** for reusable stablecoin receive addresses (critical for “account numbers”) ([Lightning Engineering][2])
* Optional: verify supply lineage and (if issuer) publish supply commitments ([Lightning Engineering][3])

### Rail C: eCash (Cashu + Fedimint) via Nostr standards

**Use for:** privacy, cash-like UX, tips, low-trust small-value payments, offline-ish flows.

* Mint discovery & reputation: **NIP-87** ([NIPs][7])
* Wallet state portability: **NIP-60** ([NIPs][8])
* “Payment is receipt”: **NIP-61** Nutzaps ([NIPs][16])
* Optional Fedimint support as a different trust model (federation guardians) ([Fedimint][11])

---

## 3) Account model

### Entities

* **Human Operator**: funds the system, sets policy, reviews exceptions
* **Agent**: autonomous actor with its own identity + wallets + budget
* **Guardian**: recovery / safety signer (threshold share)
* **Policy Signer** (optional): marketplace/compliance signer enforcing constraints before co-signing

### Accounts

Think “neobank subaccounts”, but implemented as **wallet partitions**:

* **Treasury Account** (org-level): long-term holdings, top-ups, reserves
* **Operating Account** (per agent or per workload): day-to-day spending
* **Escrow Account** (per transaction class): pay-after-verify, disputes
* **Payroll/Rewards Account**: bounties, skill revenue splits, contributor payouts

### Asset balances per account

* BTC (on-chain + LN)
* Stable asset(s) (Taproot Assets): e.g., USDT-on-Taproot-Assets (once live), or your own stablecoin asset
* eCash: Cashu proofs / Fedimint notes (with explicit mint trust lists)

---

## 4) Key management + sovereignty

### Default key topology (recommended)

* **2-of-3 FROST** for each *account*:

  * Share A: agent runtime enclave (or agent’s secure module)
  * Share B: policy signer (can enforce budgets / allowlists)
  * Share C: guardian/recovery (human-controlled)

This matches your “operators cannot extract keys” primitive.

### Separate keys for eCash P2PK

NIP-60/61 explicitly expects a separate wallet key for P2PK receiving (not the main Nostr key). Your spec should comply by:

* deriving a dedicated **eCash P2PK key** from the same root seed but a separate path, and
* threshold-splitting it the same way as other keys. ([Lightning Engineering][17])

---

## 5) Payment routing & policy engine

### Core component: `TreasuryRouter`

A deterministic policy router that decides:

* **which rail** to use (BTC LN vs stable LN vs on-chain vs eCash)
* **which asset** (BTC vs USD stable)
* **which limits** apply (daily, per-merchant, per-task, per-provider)
* **when approvals** are required
* **how receipts** are recorded and published

### Policy rules (examples)

* Under $5 equivalent: allow eCash or Lightning automatically
* Under $200: allow stablecoin LN if invoice is stable-denominated
* Over $200: require human approval or guardian co-sign
* Only pay compute providers with:

  * successful past verification
  * minimum reputation threshold
  * matching job spec hashes (your existing “pay-after-verify” pattern)

### Receipts and statements

Every payment yields:

* a **cryptographic receipt** (preimage / txid / taproot-assets proof / cashu proof ref)
* a **trajectory link**: “this spend happened during this agent session; here’s why”

That’s the “bank statement” equivalent for autonomous systems.

---

## 6) Nostr-native discoverability and configuration

### What you already get “for free” from NIP-87/60/61

* **Mint discovery + social trust** for eCash (recommendation graph) ([NIPs][7])
* **Wallet portability** (state in relays) ([NIPs][8])
* **Receipt-style payments** (nutzaps) that are naturally content-addressed and auditable ([NIPs][16])

### What you should add for Taproot Assets (proposed OpenAgents NIP)

Taproot Assets has “Universes” for distributing proofs/lineage (like a git repo for asset proofs). ([Lightning Engineering][18])

You should standardize Nostr events for:

* **Universe endpoint announcements**
* **Asset registry** (asset_id, group_key, decimals, issuer info)
* **Recommended asset lists** (like NIP-87 but for stable assets)
* **Agent payment profile** including:

  * LN endpoints (BTC + stable)
  * Taproot Assets AddressV2 receive addresses
  * supported stable assets list
  * accepted eCash mints (by NIP-87 IDs)

This keeps “neobank configuration” censorship-resistant and agent-readable.

---

## 7) Stablecoin strategy options

You asked: “if we want our agents to issue their own dollar-denominated stablecoin.”

There are two very different interpretations—design for both:

### Option 1: *Use* stablecoins (recommended for v1)

* Support USDT-on-Taproot-Assets (and others as they come)
* You’re an interface + router, not an issuer
* Much easier go-to-market; still “agentic neobank”

This aligns with Tether’s direction (USDT on Bitcoin + Lightning via Taproot Assets). ([Tether][5])

### Option 2: *Issue* OAUSD (a Taproot Assets stablecoin)

If you want your own dollar stablecoin, model it like a “bank subsidiary” where the *issuer* is a tightly controlled module, not every agent.

#### OAUSD: issuance spec (high level)

**Asset type:** Taproot Assets grouped asset (fungible)
**Issuer control:** threshold-controlled “Issuer Group Key”
**Proof distribution:** Universe server(s) + optional Nostr announcements
**Supply transparency:** enable supply commitments for the asset group (on-chain attestations) ([Lightning Engineering][3])
**Burn/redemption:** burn OAUSD when redeemed, if you use a burn-on-redemption model ([Lightning Engineering][4])

#### Minimum viable issuer workflow

1. **Deposit/Reserves**: fiat USD enters reserve account (via regulated partner)
2. **Mint**: issuer mints OAUSD to user/agent AddressV2
3. **Transfer**: OAUSD moves over Lightning or on-chain
4. **Redeem**: holder sends OAUSD back; issuer burns and pays out USD

#### Critical design decision: “free transfer” vs “issuer-mediated”

Taproot Assets supports creative multisig/co-sign arrangements (per the protocol positioning), but you should assume **most users want free transferability** like cash. ([Lightning Engineering][18])
If you introduce issuer co-sign on every transfer, you get compliance control but you lose a lot of “unstoppable” properties.

My recommendation for the spec: support both *asset policy profiles*:

* **Bearer-stable**: transfers are permissionless; compliance gates happen at **on/off ramps**
* **Managed-stable**: transfers require a policy co-sign (enterprise mode)

---

## 8) Fiat adapters (how it becomes a “neobank” for normal users)

To feel like a neobank, you need the “last mile”:

* cards
* on/off ramps
* statements
* fraud controls

The industry pattern is: **don’t build these from scratch—plug in regulated providers**.

Examples of existing infra directions:

* Card programs marketing stablecoin-backed cards via API (e.g., Rain’s positioning). ([Rain][14])
* Stablecoin issuance + reserve/compliance tooling via platform providers (e.g., Bridge/Stripe Open Issuance). ([Stripe][13])

**OpenAgents spec should treat these as adapters**, not core protocol dependencies:

* `FiatRampAdapter`: KYC + bank transfer in/out
* `CardAdapter`: virtual/physical cards, merchant settlement
* `ComplianceAdapter`: sanctions screening, risk scoring, reporting

Agents can *invoke* these adapters, but budgets and approvals remain enforced by your threshold/policy layer.

---

## 9) The “agentic neobank” killer features (what’s uniquely yours)

### A) Programmable, enforceable budgets for autonomous entities

Traditional neobanks give humans controls. You need:

* per-agent daily caps
* per-task caps
* per-provider allowlists
* approval workflows
* velocity-aware throttles (“if APM spikes and failure rate rises, clamp spend”)

### B) Receipts that include “why”

A bank statement says *what* you spent. An agentic neobank must also say:

* **which agent**
* **which trajectory**
* **which tool result verification**
* **which policy allowed it**
* **who co-signed it (if threshold)**

This is a huge enterprise wedge.

### C) Multi-rail routing as a first-class primitive

Pick the best rail per context:

* LN BTC for tiny machine payments
* Taproot Asset stable LN for “USD pricing” at scale
* eCash for privacy / content tips / offline-ish workflows

---

## 10) Concrete v1 scope (buildable spec)

### v1 goals

1. **Multi-asset wallet for agents**:

   * BTC (LN + on-chain)
   * Taproot Assets stablecoin (hold/send/receive)
2. **TreasuryRouter + Policy Engine**:

   * budgets, approvals, allowlists
3. **Receipt ledger**:

   * local DB + exportable statements
   * Nostr-linked receipts (optional publishing)
4. **eCash support for Nostr-native tips and microflows**:

   * implement NIP-87 discoverability
   * implement NIP-60 wallet state
   * implement NIP-61 nutzaps

### v1 non-goals

* issuing your own stablecoin (unless you have reserves + compliance ready)
* lending/yield
* full global fiat bank transfer coverage

---

## 11) OpenAgents protocol additions (proposed)

To make this “neobank” interoperable across Nostr apps, define 2–3 new event kinds (names illustrative):

1. **AgentPaymentProfile** (replaceable)

* lists:

  * BTC LN receiving endpoints
  * Taproot Assets stable receiving AddressV2
  * accepted assets (by asset_id / group_key)
  * accepted eCash mints (via NIP-87 references)

2. **AssetRegistry** (replaceable)

* maps asset identifiers → metadata:

  * issuer pubkey
  * group_key
  * decimals
  * universe endpoints
  * policy profile (bearer vs managed)

3. **UniverseAnnouncement** (replaceable)

* universe URL + supported assets + sync hints

This is the missing piece to make Taproot Assets feel as “discoverable” as eCash does via NIP-87.

---

## 12) “How these NIPs help the broader vision” (direct answer)

### NIP-87 = decentralized “bank branch discovery” for eCash

It gives you **mint directories, social trust, and spam resistance** through recommendations and explicit mint announcements. That’s exactly what a neobank needs for choosing counterparties—except it’s open and relay-native. ([NIPs][7])

### NIP-60 = portable, relay-synced “checking account state”

Wallet state living in relays means:

* an agent can migrate between runtimes/providers and still “bring its wallet”
* multiple clients can interoperate without proprietary databases
  It’s the Nostr version of “log into your bank account from any device,” but encrypted. ([NIPs][8])

### NIP-61 = receipts-native payments (perfect for agents)

Nutzaps make the payment itself the receipt. That’s *exactly* what your agent economy needs: payments that can be attached to work, content, or proofs, and then audited later. ([NIPs][16])

Together they form a **Nostr-native money substrate** for the smallest, most frequent interactions—while Taproot Assets becomes the stablecoin substrate for larger dollar flows.

---

## 13) Reality check: regulation is tightening (design for optional compliance)

If you ever issue a stablecoin, you’re increasingly in scope of stablecoin issuer regimes (US, EU, Singapore, etc.). For example:

* The US **GENIUS Act** overview from CRS describes a regulatory framework with tailored capital/liquidity/risk management rules for stablecoin issuers. ([Congress.gov][19])
* EU **MiCA** creates uniform EU rules and covers issuers of ART/EMT tokens, including authorization and supervision. ([ESMA][20])

So the spec should explicitly support **two deployment modes**:

* **Sovereign mode**: self-custody + open rails, minimal compliance assumptions
* **Enterprise/compliant mode**: policy signers + KYC ramps + reporting modules

(That’s how you keep the “open OS” ethos *and* sell to enterprises.)

---

## A practical naming + positioning

If you want this to land in the market without sounding like “crypto bank”:

* **OpenAgents Treasury**
* **OpenAgents Agent Accounts**
* **OpenAgents Money Router**
* **OpenAgents Neobank (Agents-first)**

The differentiator: **“banking controls for autonomous entities”**.

---

* [Reuters](https://www.reuters.com/world/asia-pacific/singapore-trial-tokenised-bills-bring-stablecoin-laws-central-bank-chief-says-2025-11-13/?utm_source=chatgpt.com)
* [Reuters](https://www.reuters.com/business/finance/amundi-warns-us-stablecoin-policy-could-destabilise-global-payments-system-2025-07-03/?utm_source=chatgpt.com)
* [Investors](https://www.investors.com/news/shift4-payments-stablecoin-settlement-launch-merchants-four-stock/?utm_source=chatgpt.com)
* [investopedia.com](https://www.investopedia.com/fiserv-to-add-new-stablecoin-for-its-network-by-end-of-year-11759129?utm_source=chatgpt.com)
* [axios.com](https://www.axios.com/2025/03/19/stablecoin-stable-sea?utm_source=chatgpt.com)

[1]: https://lightning.engineering/posts/2025-6-24-tapd-v0.6-launch/ "Announcing Taproot Assets v0.6: Bitcoin’s Decentralized FX Network Has Arrived ⚡  | Lightning Labs"
[2]: https://lightning.engineering/posts/2025-12-16-tapd-0.7-launch/?utm_source=chatgpt.com "Announcing Taproot Assets v0.7: The Set-and-Forget ..."
[3]: https://lightning.engineering/api-docs/api/taproot-assets/mint/mint-asset/ "MintAsset | Lightning Labs API Reference"
[4]: https://lightning.engineering/api-docs/api/taproot-assets/taproot-assets/burn-asset/ "BurnAsset | Lightning Labs API Reference"
[5]: https://tether.io/news/tether-brings-usdt-to-bitcoins-lightning-network-ushering-in-a-new-era-of-unstoppable-technology/ "
      Tether Brings USDt to Bitcoin’s Lightning Network, Ushering in a New Era of Unstoppable Technology - Tether.io
    "
[6]: https://lightning.engineering/posts/2025-01-30-Tether-on-Lightning/ "A New Era for Stablecoins: Tether Is Coming to Bitcoin and Lightning ⚡ | Lightning Labs"
[7]: https://nips.nostr.com/87 "NIP87 - NIP-87 - Ecash Mint Discoverability"
[8]: https://nips.nostr.com/60?utm_source=chatgpt.com "NIP60 - NIP-60 - Cashu Wallets"
[9]: https://nips.nostr.com/61 "NIP61 - NIP-61 - Nutzaps"
[10]: https://cashu.space/ "Cashu - Open-source Ecash"
[11]: https://fedimint.org/docs/GettingStarted/What-is-a-Fedimint "What is a Fedimint | Fedimint"
[12]: https://www.bankless.com/read/crypto-neobanks "The Crypto Neobanking Surge"
[13]: https://stripe.com/blog/introducing-open-issuance-from-bridge "Introducing Open Issuance from Bridge: A new platform to launch your own stablecoin"
[14]: https://www.rain.xyz/cards "Launch Stablecoin-Powered Cards in Weeks | Rain"
[15]: https://gnosispay.com/card "Gnosis Pay"
[16]: https://nips.nostr.com/61?utm_source=chatgpt.com "NIP61 - NIP-61 - Nutzaps"
[17]: https://docs.lightning.engineering/the-lightning-network/taproot-assets/faq?utm_source=chatgpt.com "FAQ - Builder's Guide - Lightning Labs"
[18]: https://docs.lightning.engineering/the-lightning-network/taproot-assets/taproot-assets-protocol "Taproot Assets Protocol | Builder's Guide"
[19]: https://www.congress.gov/crs-product/IN12522?utm_source=chatgpt.com "Stablecoin Legislation: An Overview of S. 919, GENIUS Act ..."
[20]: https://www.esma.europa.eu/esmas-activities/digital-finance-and-innovation/markets-crypto-assets-regulation-mica?utm_source=chatgpt.com "Markets in Crypto-Assets Regulation (MiCA)"
