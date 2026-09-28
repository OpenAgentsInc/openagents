# Wallet design: people, agents, and spending grants

Date: 2026-09-28

Status: **Design proposal.** Nothing here is implemented. It proposes how the
OpenAgents app's Wallet becomes a Breez SDK (Spark) wallet, how agents hold or
borrow spending power, and what must be true before real money moves. It
builds on the [SDK review](sdk-review.md), the
[Breez and Spark comparison](breez-vs-spark.md),
[stablecoin receive](stablecoin-receive.md), and the
[history](history.md). Owner decisions it needs are in
[open questions](#open-questions).

## Goals and non-goals

Goals:

- A person's Wallet on iOS and Android that can hold, send, and receive
  bitcoin over Spark, Lightning, and on-chain, with the key on the phone.
- Agents that can pay for things (x402 tools, other agents' labor, tips)
  without the owner handing over the whole wallet.
- Spending authority that is explicit, bounded, expiring, revocable, and
  receipted, in the style of NIP-HOST grants.
- Peer-to-peer payments between people and agents.

Non-goals for this design:

- A treasury, payout service, or any custody of other people's money by
  OpenAgents. Those failed before and belong to a separate decision.
- x402 **receive** on Spark. It does not work; see [x402](#x402).
- Paying quest rewards. The
  [agent-trainer leveling spec](../verse/agent-trainer-leveling.md) keeps sats
  rewards future until its Rewards preconditions hold.

## Principals and keys

| Principal | Key | Where it lives | What it can do with money |
| --- | --- | --- | --- |
| Person (owner) | Spark wallet seed, new and separate | Phone: this-device-only Keychain item or Keystore-wrapped file, handed to Rust at open, memory only | Everything, subject to Spark's trust model |
| Phone (device) | Existing device key (`com.openagents.app.device`) | Phone Keychain | Signs grants and receipts. Holds no funds. |
| Agent | The agent's own Nostr key | The host that runs it, or the agent's own store | Only what a grant or its own allowance wallet allows |
| Host | Host key (NIP-HOST) | The owner's computer | Runs agents. Host access never implies spending, as NIP-HOST and NIP-SOV already say. |

Positions:

1. **The Spark seed is new, random, and separate** from the device, world,
   and Mutinynet wallet keys, following the existing
   [Phone wallet invariants](../../INVARIANTS.md#phone-wallet). It is not
   derived from a Nostr key or NIP-06 phrase. An earlier unified Nostr and
   Spark identity profile (`fc539974a8`) was removed; do not revive it.
2. **The seed cannot live in secure hardware.** Spark signs with secp256k1;
   the Secure Enclave and most Keystores support only P-256. Wrap it the way
   the phone wallet does today: random entropy in its own Keychain item
   (`WhenUnlockedThisDeviceOnly`, not synchronized) on iOS, and an AES key in
   Android Keystore encrypting the entropy in non-backed-up storage.
3. **Backup is the 12-word mnemonic,** revealed under the same rule as the
   device nsec: an explicit request after a confirmed warning, never in a
   packet, log, or file. Spark recovery needs the seed **and** live operators;
   see [disclosures](#disclosures) for the outage case.

## Wallet kinds

### Human wallet

One Spark wallet per person on the phone, account number 1 on mainnet (the
SDK default) and 0 on regtest. The Rust core owns it through
`breez-sdk-spark`; SwiftUI and Kotlin render screens built in Rust, as the
other tabs do.

Screens, in order of delivery: balance and pending; receive (Spark address,
BOLT11, on-chain deposit, QR and copy); send (scan or paste anything `parse`
accepts, fee quote, confirm); history; backup and reveal; settings (network,
Lightning address, disclosures). Later: grants, agent wallets, and dollars.

### Agent wallets

An agent spends in one of four ways. They differ in who holds the key and
who enforces the limit, and the design keeps all four because each covers a
case the others cannot.

| Mode | Key holder | Enforced by | Works with the phone off | Loss bound if the host is compromised |
| --- | --- | --- | --- | --- |
| A. Approval request | Phone | The person, per payment | No | Nothing moves without a tap |
| B. Standing grant | Phone | The phone's grant ledger | No, unless woken | The grant's remaining budget |
| C. Allowance wallet | Host | Host software; the balance is the hard bound | Yes | The allowance wallet's balance |
| D. Operator allowance (tokens) | Phone signs once; spender key on host | Every Spark operator | Yes | The allowance's remaining total |

**A. Approval request.** The agent asks the phone to pay a specific
invoice, address, or LNURL. The phone shows the request and the person
approves or refuses it. This is the default for anything new.

**B. Standing grant over the human wallet.** The person signs a grant that
lets the phone pay an agent's requests without a tap, within caps. The phone
still holds the key and executes. It is fast when the phone is reachable.
iOS gives a woken app about 30 seconds and does not guarantee delivery, so
this mode is best effort. It never falls back to a weaker mode.

**C. Allowance wallet.** The agent has its own Spark wallet on its host,
funded by transfers from the human wallet that a grant bounds. It pays with
no phone involved. Host software enforces per-payment and per-purpose rules,
but a compromised host can spend the whole balance, so **the balance is the
real limit**. The seed is generated on the phone for account `100 + n` of a
separate agent seed, not the person's seed (see
[open questions](#open-questions)), sent to the host encrypted to the host
key, and kept on the phone so the person can sweep it back.

**D. Operator-enforced allowances.** Spark's token allowances (see
[Breez and Spark](breez-vs-spark.md#what-sparks-sdk-has-that-breez-does-not-expose))
let the person's wallet sign an allowance for an agent's key with a
per-transaction ceiling, a lifetime total, an expiry, and a recipient
allowlist. The operators enforce it on every pull and honor revocation as a
permanent tombstone. The funds stay in the person's wallet until pulled. This
is the only mode in which neither our host code nor the phone enforces the
limit. It covers tokens only, so it applies to a USDB balance, not sats.
**Unverified:** that allowances run on mainnet. The Breez SDK does not expose
them; its `crates/spark` compiles the RPC.

Positions:

- **Ship A first, then C, then B, then D.** A needs no standing authority. C
  is the only way an agent can pay while the phone sleeps, and its risk is
  easy to state. B adds convenience over A. D waits on USDB and on upstream
  support.
- **Do not put agent sub-wallets on the person's own seed.** Account numbers
  make that possible, but a host would then need the person's seed, or we
  would have to write a signer that holds a single hardened account key.
  A separate agent seed on the phone gives the same recovery with a smaller
  blast radius.
- **An agent never holds the person's seed.** Breez's client signing lets a
  keyless process prepare a payment for the phone to sign; that is the
  transport for mode A when the agent prepares, not a reason to move keys.

## Spending grants

A grant is a signed statement from the wallet's owner that an agent may cause
payments within stated bounds. It follows NIP-HOST's shape and inherits its
rules: the issuer is the only source of authority, a grant is not a bearer
credential, every use is signed by the grantee, and revocation advances an
epoch. It is also a concrete wallet policy in NIP-SOV's terms ("Treasury,
earnings, and purchases") and uses NIP-POL approvals for mode A.

### Grant

`openagents.spend-grant.v1`, signed by the phone's device key and encrypted to
the agent's key in a kind-`3188` private artifact envelope, as host grants
are.

| Field | Meaning |
| --- | --- |
| `v`, `requires` | Version and required features. Unknown fields refuse. |
| `grant` | 32-byte ID and reply mailbox. |
| `issuer` | The phone's device key. |
| `owner` | The person's Nostr key, when different from the issuer. |
| `grantee` | The agent's key. |
| `wallet` | Spark identity public key and network of the wallet the grant draws on (the human wallet for A and B, the allowance wallet for C). |
| `mode` | `request`, `standing`, `allowance`, or `operator_allowance`. |
| `unit` | `sat` or a token identifier. Exact units, never inferred, per the shared `spend_microunits` rule. |
| `per_payment_max` | Ceiling for one payment, fees included. |
| `period` and `period_max` | Rolling window (for example 24 hours) and its ceiling. |
| `total_max` | Lifetime ceiling. |
| `fee_max` | Absolute and proportional fee ceilings. A quote above either refuses. |
| `rails` | Subset of `spark`, `lightning`, `lnurl`, `onchain`, `token`. `onchain` and cross-chain are off unless named. |
| `payees` | Allowlist of Spark addresses, Lightning node keys (x402 `payTo`), LNURL domains, and CAP capability IDs. Empty means none, not any. |
| `purposes` | Subset of `x402_purchase`, `labor_payment`, `tip`, `transfer`. |
| `auto_approve_max` | For `standing`: payments at or below it proceed without a tap; above it, the phone asks. |
| `epoch` | The grantee's current epoch. |
| `issued_at`, `expires_at` | At most 30 days apart, as for host grants. |

Only the owner, on the phone, creates or widens a grant. A request, a model,
a capability definition, or an approval cannot widen one. Approvals only
narrow.

### Spend request

`openagents.spend-request.v1`, signed by the grantee.

| Field | Meaning |
| --- | --- |
| `request` | 32-byte ID and the idempotency key. Different bytes under the same ID return `conflict`. |
| `grant`, `epoch` | The grant it draws on. |
| `payment` | The exact payment request: BOLT11, Spark address or invoice, or LNURL with amount. |
| `amount`, `fee_max` | Exact amount and the caller's fee ceiling, never above the grant's. |
| `purpose` | One of the grant's purposes. |
| `context` | References to the task, x402 purchase record, or MKT order, for the receipt and the approval sheet. |
| `issued_at`, `expires_at` | Short-lived; never past the payment request's own expiry. |

### Receipt

`openagents.spend-receipt.v1`, signed by the phone (modes A and B) or the
allowance wallet's host (mode C).

| Field | Meaning |
| --- | --- |
| `request`, `grant` | What it answers. |
| `outcome` | `paid`, `refused` (with a code), `pending`, or `unknown`. |
| `payment_id`, `amount`, `fees` | From the SDK. Unknown fees stay unknown. |
| `proof` | The Lightning preimage or Spark transfer ID. Sent only in the encrypted reply; general traces carry a digest. |
| `remaining` | Budget left in the period and in total. |
| `at` | Time of the outcome. |

Refusal codes follow NIP-HOST's style: `expired`, `revoked`, `stale`,
`over_payment_cap`, `over_period_cap`, `over_total_cap`, `fee_too_high`,
`payee_not_allowed`, `purpose_not_allowed`, `rail_not_allowed`,
`declined_by_owner`, `phone_unreachable`, `insufficient_funds`.

### Ledger and enforcement

- The phone keeps one durable ledger per wallet. It **reserves** a request's
  amount plus fee ceiling before calling the SDK, and releases or settles the
  reservation from SDK evidence. Pending and unknown payments stay reserved,
  as NIP-SOV requires. A restart never resets a period.
- Enforcement runs in a `PaymentObserver` registered on the wallet, so a send
  that bypasses the grant path by mistake is still checked. The observer
  refuses any payment the ledger has not reserved.
- `send_payment` receives the request ID as its `idempotency_key`, so a
  retried request cannot pay twice.
- The ledger is the source of truth for the grants screen. Breez's payment
  list is evidence, not the budget.

### Revocation

- **A and B:** revoking a grant or all of an agent's grants advances the
  agent's epoch atomically. Queued requests under the old epoch refuse as
  `stale`.
- **C:** revocation stops top-ups and tells the host to stop. Because the
  host holds the key, the phone also sweeps the allowance wallet back to the
  human wallet; the person sees the sweep result. A compromised host can race
  the sweep. That is the stated risk of mode C.
- **D:** the phone calls `revokeTokenAllowance`; the operators tombstone it.

## Approval flows

1. The agent's host sends a spend request over the relay path it already uses
   for NIP-HOST, addressed to the phone's mailbox.
2. The push gateway wakes the phone with no payment details in the
   notification body.
3. The phone checks the request against the grant and ledger. If the grant is
   `standing` and the amount is at or below `auto_approve_max`, it pays.
   Otherwise it shows an approval sheet.
4. The sheet shows, from typed data: the agent and host, the task, the purpose,
   the payee as decoded from the payment request (not as the agent described
   it), the amount, the quoted fee, and what the grant has left. It says in
   plain words if the payee is new.
5. Approve requires Face ID or the device passcode above a threshold the
   person sets. Approve pays once. It never raises a cap.
6. The phone replies with a receipt. The host records it with the task's
   evidence.

A request that expires while the phone is unreachable refuses as
`phone_unreachable`. It is never retried under a different mode.

## How agents on hosts spend

Coder and other agents already have buyer-side policy in `crates/x402`
(`~/.openagents/x402/policy.json`: ceilings, allowlist, daily cap, ledger).
That policy stays the host's first check. The new part is where the money
comes from, behind the existing `LightningWallet` trait's pay side:

- **Phone payer adapter (modes A and B).** `pay(invoice, max_fee_msat)`
  becomes a spend request to the phone and waits for the receipt. It returns
  the preimage, which x402 settlement needs.
- **Spark allowance adapter (mode C).** A Breez SDK wallet on the host.
  `pay` quotes with `prepare_send_payment`, refuses a quote above
  `max_fee_msat`, sends with the request ID as idempotency key, and returns
  the preimage from `PaymentDetails::Lightning.htlc_details`.
  `receive_exact` returns an unsupported error; see [x402](#x402).
- **`ldk-node` wallet (unchanged).** Hosts that receive x402 payments keep
  `crates/wallet`.

The host's x402 policy and the phone's grant are independent limits; the
lower wins. A task's configuration can never name a grant with more
authority than its owner gave.

## Peer-to-peer payments

- **Person to person:** Spark address (free, instant, both on Spark) or
  Lightning (anyone). Contacts come from the SDK's contact list and from Nostr
  profiles.
- **Person to agent and agent to person:** the same rails. An agent's
  allowance wallet has a Spark address the person can pay to top it up.
- **Discovery:** publish a Spark address and, later, a Lightning address in
  the person's or agent's Nostr profile. Breez's LNURL server supports NIP-57
  zaps.
- **Lightning address domain:** start without one. When we add one, **self-host
  Breez's open-source LNURL server** on an OpenAgents subdomain rather than
  pointing a CNAME at Breez's hosted server, because the LNURL server can
  return invoices that do not pay the user, and it would carry our name.
  Running it is a service with uptime duties; it needs its own issue.
- **Zaps are separate from x402 and labor payment,** as NIP-X402 requires.

## x402

**Paying an x402 seller from Spark works in principle.** The seller's invoice
is ordinary BOLT11. The Spark wallet pays through the SSP, and the payment
record carries a preimage. Before relying on it, confirm on mainnet that the
preimage is always populated and that the quoted fee is a hard ceiling. Note
that x402 admits only `bc` and `tb` invoices, and Spark has no testnet and no
Lightning on its hosted regtest, so **x402 payment from Spark can be tested
only on mainnet** or on the local `spark-itest` cluster with a custom
validator network.

**Receiving x402 payments into a Spark wallet does not work,** for three
independent reasons:

1. **The payee is not ours.** A Spark BOLT11 invoice is created and signed by
   the SSP's Lightning node, which serves every Spark user of that provider.
   NIP-X402 requires `payTo` to be a receiver key with exclusive
   invoice-issuance authority and says "A shared custodial key under which
   untrusted tenants can issue invoices is not compatible with this method."
   `crates/nostr/src/x402.rs` refuses an invoice whose payee differs from
   `payTo`.
2. **The description hash is not exposed.** x402 needs exactly one signed
   description hash equal to the request digest. Spark's protocol crate
   supports a description hash, but `ReceivePaymentMethod::Bolt11Invoice`
   takes only a text description.
3. **The receiver does not hold the preimage path alone.** The operators hold
   preimage shares and release them against the SSP's transfer; the
   facilitator's trust model assumes the receiver's own node.

Exposing the description hash upstream would fix only reason 2. What would
make Spark an x402 receiver is a **new x402 scheme for Spark itself**, for
example `exact` with method `spark-invoice`, where `payTo` is a Spark identity
key and proof is an operator-attested transfer to that key. That is a new
profile needing an upstream proposal and a NIP-X402 change, not a wallet
change. **Position:** x402 receive stays on `crates/wallet` with MoneyDevKit's
LSPS4 channel (#9832). A person who earns through x402 on a host can sweep to
their Spark wallet.

## Relation to the current wallet

Today the phone's Wallet tab runs `crates/wallet`'s `ldk-node` wallet on
Mutinynet signet, with its own Keychain item (`833528ed81`). The CLI's
`openagents wallet` runs the same crate on mainnet or testnet as the x402
receiver and payer.

| Surface | Position |
| --- | --- |
| Phone Wallet tab | **Replace** the Mutinynet `ldk-node` wallet with Breez Spark. The Mutinynet coins are test coins, so nothing migrates. Keep the Mutinynet path only behind a developer setting until the Spark regtest wallet passes its tests, then delete it. |
| Phone Keychain | New item for the Spark seed. Delete the Mutinynet item when that path is deleted. |
| `openagents wallet` on hosts | **Keep** `ldk-node` as the x402 receiver and sovereign node. |
| Host agent spending | **Add** the phone payer and Spark allowance adapters beside the `ldk-node` wallet. |
| Invariants | The [Phone wallet](../../INVARIANTS.md#phone-wallet) rows must be rewritten in the same change that ships Spark: Spark on regtest only at first, mainnet only behind the gate below, the seed rule unchanged in substance. |

Running two Lightning implementations is the price of Spark's offline receive
and small-balance ergonomics on the phone, and of `ldk-node`'s exclusive
payee key on hosts. Neither can do the other's job.

## Key custody on the phone

- The seed follows the phone wallet's current rules: its own item, this device
  only, memory only in Rust, typed errors that never carry it, and no `Debug`
  on the holder type.
- Breez's encrypted real-time sync is **off** at first. It adds a Breez
  server that stores encrypted records. Turn it on only when a second device
  is a real feature.
- Export Spark's unilateral-exit state periodically into the app's encrypted
  store, and offer an export to Files. A seed alone does not recover funds
  during an operator outage.
- The Breez API key is a build-time input from a gitignored file, never
  source. It is extractable from any shipped binary, as for every Breez app,
  so plan rotation. Earlier OpenAgents code embedded a key in source three
  times; see [history](history.md#lessons-for-a-re-add).
- Agent allowance seeds are generated on the phone, kept in their own
  Keychain item, and sent to a host encrypted to its key. The phone never
  receives an agent's own Nostr secret.

## Disclosures

The app states, on the Wallet's first screen and in settings:

- Spark is not a self-run Lightning node. Three companies (Lightspark, Breez,
  and Flashnet) run the operators; two of them together must cooperate for
  off-chain payments, and your safety depends on at least one having deleted
  old keys, which no one can check.
- If the operators stop, you can still withdraw on-chain yourself, but it can
  take days and needs a separate on-chain payment for fees.
- Lightning payments go through Lightspark.
- Keep amounts you would be comfortable holding in a phone wallet.

## Testing and mainnet gating

| Stage | Network | Covers | Does not cover |
| --- | --- | --- | --- |
| Unit and fixture tests | None | Grant parsing, ledger, reservations, refusals, revocation, the observer | Any SDK call |
| Hosted regtest (Lightspark, faucet, no API key) | Regtest | Spark send and receive, deposits, withdrawals, tokens, grants over Spark transfers, allowance wallets | Lightning, USDB, USDC and USDT |
| Local cluster (`spark-itest`: Docker bitcoind, operators, `ldk-server`, `sspd`) | Regtest | Lightning send and receive, x402 pay against a local validator | Production operators, Lightspark's SSP |
| Mainnet canary | Mainnet | Lightning through Lightspark, x402 pay, LNURL, preimage and fee behavior | Nothing is free here |

Mainnet gate for the human wallet, all required:

1. The owner turns on mainnet with a build flag and an explicit setting.
   No request, packet, or stored value switches networks.
2. A balance warning above a configured amount (the app cannot refuse
   incoming Spark transfers).
3. The disclosures above, acknowledged once.
4. A dated measurement record of the canary: amounts, fees, preimages,
   timings, and one failure case each.
5. An invariant change naming its tests.

Agents spend on mainnet only after the human wallet has passed its gate and
the grant ledger has regtest tests for every refusal code. Cross-chain
receive and Stable Balance have no test network and follow
[their own position](stablecoin-receive.md#position-for-the-openagents-wallet).
Nothing here touches any treasury wallet or the workspace's `.secrets`.

## Phased plan

| Phase | Delivers | Gate to leave it |
| --- | --- | --- |
| 0 | These documents; owner answers to the open questions; a NIP draft for spend grants in `nips/openagents/` | Owner accepts positions |
| 1 | Spark human wallet on iOS on regtest: balance, Spark and on-chain receive and send, history, backup and reveal. Rewritten phone-wallet invariants. `cargo tree -d` and archive size recorded. | Regtest tests pass; build size acceptable |
| 2 | Android parity (Keystore-wrapped seed) | Same tests on Android |
| 3 | Mainnet gate for the human wallet; Lightning send and receive; LNURL pay | Canary measurement recorded |
| 4 | Mode A approval requests; phone payer adapter for `crates/x402`; receipts in task evidence | Every refusal code tested; one mainnet x402 purchase paid from the phone |
| 5 | Mode C allowance wallets on hosts, with sweep on revocation | Regtest sweep race tested; mainnet canary |
| 6 | Mode B standing grants | Wake and expiry behavior measured |
| 7 | Lightning address on a self-hosted LNURL server; Nostr profile publishing | Service issue with an owner and uptime plan |
| 8 | Dollars: USDB balance, mode D operator allowances, optional cross-chain receive | USDB issuer documented; allowances confirmed on mainnet |

Quest purses from the leveling spec come after phase 4 at the earliest and
only when that spec's Rewards preconditions hold.

## Open questions

Each question lists this design's position. The owner decides.

1. **Is Spark's trust model acceptable for the person's main wallet?**
   Position: yes for a phone spending wallet with small balances and plain
   disclosures; no for any treasury or pooled funds.
2. **Replace or keep the Mutinynet phone wallet?** Position: replace on the
   phone; keep `ldk-node` on hosts for x402 receive.
3. **One seed or two for agents?** Position: a separate agent seed on the
   phone with one account per agent, not the person's seed.
4. **Which agent mode ships first?** Position: A (approval requests), then C.
5. **Should grants become a NIP?** Position: yes, drafted as a wallet-policy
   profile under NIP-SOV and NIP-POL before code, with NIP-HOST's refusal
   and epoch rules. Offer NIP-47 later as a transport only; a wallet
   connection is not a grant.
6. **Breez's hosted services:** API key yes; real-time sync off; Lightning
   address self-hosted when added; cross-chain off at first.
7. **Dollars:** Position: land in BTC by default until USDB's issuer and
   backing are documented; then offer USDB with operator allowances.
8. **Who pays fees on agent payments?** Position: the grant counts fees
   against its caps; the agent's request carries a fee ceiling.
9. **Regulatory review:** agent payments on a person's behalf, peer payments,
   and dollar balances may raise money-transmission questions, as the leveling
   spec notes for rewards. Position: get review before phase 5 (allowance
   wallets hold value on hosts) and before phase 8.
10. **Upstream work:** ask Breez to expose the BOLT11 description hash and
    Spark token allowances. Position: file both as upstream issues; do not
    fork.

## Unverified

- That `PaymentDetails::Lightning.htlc_details.preimage` is populated for
  every successful send.
- That Spark token allowances run on mainnet operators.
- USDB's issuer, backing, and redemption.
- Binary size and dependency conflicts from adding `breez-sdk-spark` beside
  `ldk-node` in `crates/openagents-mobile`.
- iOS background-wake reliability for standing grants.
- Breez API key terms, quotas, and rotation.
