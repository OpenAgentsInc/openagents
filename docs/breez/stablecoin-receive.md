# USDC and USDT receive

Date: 2026-09-28

Scope: how the Breez SDK's "Receive USDT/USDC" feature works, who holds the
money at each step, what it costs, and whether the OpenAgents Wallet should
turn it on. Sources are the SDK source at `754ae7959` (0.26.0), the SDK guide,
and Flashnet's Orchestra documentation. Nothing was run.

## What Breez announced

Breez announced that users "can now receive USDC or USDT from 30+ networks,
including Ethereum, Base, Solana, and Tron," landing in "your bitcoin or
dollar balance." This review did not find the announcement post itself. The
release notes are the evidence that it shipped: 0.25.0 (2026-09-10) added
"Cross-chain: Orchestra receive" (PR #964), and 0.26.0 (2026-09-23) carries it
(<https://github.com/breez/spark-sdk/releases>). Sending USDC and USDT shipped
earlier, in 0.17.1 (2026-06-29).

## How it works

There is one provider: **Orchestra, run by Flashnet**. Flashnet is also one of
the three default Spark operators. A second provider, Boltz, is compiled but
not registered; PR #1105 says "The Boltz service is not operational, and it is
unclear when or whether it will be again."

The receive flow, from `crates/breez-sdk/core/src/cross_chain/orchestra.rs`
and `crates/flashnet/src/orchestra/`:

1. The app lists routes with `get_cross_chain_routes`. Orchestra publishes
   them at `GET v1/orchestration/limits`.
2. The app calls `receive_payment` with
   `ReceivePaymentMethod::CrossChain { route, amount, destination, fee_mode, max_slippage_bps, target_overpay_bps }`.
3. The SDK creates an amountless Spark invoice with no expiry, for BTC or
   USDB, and asks Orchestra for a quote from `{source chain, USDC or USDT}` to
   `{Spark, BTC or USDB}` with that invoice as the recipient.
4. Orchestra returns a **deposit address that it controls** on the source
   chain. The SDK returns it as `payment_request`: an EIP-681 URI on EVM
   chains, a bare address on Solana and Tron. Orchestra's quote is firm for 2
   minutes; paying the address creates the order.
5. The payer sends USDC or USDT to that address from any wallet or exchange.
6. Orchestra detects the deposit, converts, and pays the Spark invoice. The
   SDK polls status every 30 seconds and keeps probing for 24 hours past the
   quote's expiry. A late deposit is repriced at the live rate.

The Orchestra base URL and API key come from Breez's configuration server
(`bs1.breez.technology`), with no bundled fallback. Requests carry
`affiliate_id = "breez_sdk"`.

**Destination.** The funds land as sats or as USDB, Spark's USD token. When
`destination` is unset, the SDK picks the wallet's active stable-balance token
if the route supports it, otherwise BTC.

**Networks.** The guide lists USDC on 19 EVM chains, USDT on 23 EVM chains,
USDC and USDT on Solana, and USDT on Tron. The live list comes from Orchestra.
TON receive is filtered out because Orchestra needs a refund address there.

## Who holds the money

| Step | Holder | Trust |
| --- | --- | --- |
| Payer sends USDC or USDT | Flashnet's deposit address on the source chain | Custodial. Flashnet says "Flashnet settles without taking custody of user funds," but the deposit address is provider-controlled, and its docs also say over-limit funding "can be held." This review treats this leg as custodial. |
| Conversion and payout | Flashnet | Flashnet decides the rate within the slippage bound. Its workers can return AWS Nitro enclave attestation. Which bridges or liquidity venues it uses is not documented. |
| Funds on Spark | The user's Spark wallet | The usual Spark trust model. USDB adds issuer trust. |

**USDB.** Stable Balance holds USDB, "the 6-decimal USD-pegged token on
Spark." This review did not find who issues USDB, what backs it, or how it is
redeemed. Until that is known, a USDB balance carries an unknown issuer risk
on top of Spark's.

**Compliance.** Flashnet screens every order's source and destination through
Elliptic. High-risk orders go to review; orders tied to sanctioned addresses
are rejected (<https://docs.flashnet.xyz/orchestra/security/risk-and-compliance.md>).
The Breez docs mention no end-user KYC. A screened order can be held.

## Fees and limits

- The receiver pays nothing; the sender's deposit covers fees. In the default
  `FeesExcluded` mode the SDK pads the requested deposit for provider fees
  plus a 15 bps overpay buffer.
- Slippage defaults to 100 bps, allowed 10 to 500.
- Orchestra charges a base fee plus optional partner fees, and may add sweep,
  network, and rounding fees. **No published percentage was found**, and
  Breez's affiliate cut is not published. Letting integrators set their own
  fee is open (breez/spark-sdk#1108).
- Each route publishes optional minimum and maximum amounts, and "a route can
  enforce a tighter bound than it publishes." Issue #1138 reported failures
  at $5 on Ethereum, Tron, BNB, and HyperEVM, and at $50 on Ethereum and
  Tron.

## Failure and refunds

This is the weakest part.

- `RefundNeeded` exists only for sends. The receive path sends
  `refund_address: None`.
- Orchestra then refunds to the detected source address "only when that is
  safe, and never to an exchange's shared sender address." Otherwise recovery
  "may need manual review" by Flashnet.
- A real crediting bug was reported and fixed in 0.25:
  "USDT delivered on Solana, confirmed on-chain, never credited" (#1124).

A payer who sends from an exchange and hits a failed order has to reach
Flashnet. Neither OpenAgents nor Breez can move those funds.

## Constraints

- Mainnet only. Regtest has no stablecoins.
- Requires background tasks, so it does not work in Breez's server mode.
- Incompatible with a signing-only external signer.
- Token-leg (USDB) sends have no idempotency guarantee.

## Position for the OpenAgents Wallet

1. **Do not turn it on in the first release.** It is two weeks old, mainnet
   only, untestable on regtest, custodial for the swap, and has a manual
   refund path. It also adds a sanctions-screening counterparty to a flow the
   app would present as its own.
2. **Offer it later as an explicit "Receive from another network" action**,
   never as the default receive, with these disclosures on the screen:
   Flashnet holds the deposit until it pays out; failed deposits from
   exchanges may need Flashnet support; the fee is in the quote.
3. **Land in BTC, not USDB, by default,** until USDB's issuer and backing are
   documented. A dollar balance is a separate owner decision.
4. **Never route an agent's income or a grant's funding through it
   automatically.** An agent cannot resolve a manual-review refund.
5. **Test with small mainnet amounts behind an owner flag**, recorded as a
   dated measurement: one EVM route, Solana, and Tron, including one
   deliberately under-minimum deposit to observe the refund path.

The dollar-denominated use case the owner cares about, getting paid in
dollars, can also be met by sending USDB or BTC directly between Spark
wallets, which needs no bridge. Cross-chain receive matters only when the
payer holds USDC or USDT elsewhere.
