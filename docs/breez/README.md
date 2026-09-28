# Breez and Spark

This directory studies re-adding the Breez SDK (Spark implementation) and
making the OpenAgents app's Wallet a Spark wallet for people and agents. The
phone Wallet now runs it on mainnet (`crates/openagents-mobile/src/spark.rs`
and `wallet.rs`; epic [#9854](https://github.com/OpenAgentsInc/openagents/issues/9854)).
Computers keep `crates/wallet` (`ldk-node`) and `crates/x402`; see
[Bitcoin](../bitcoin/README.md).

## Owner decisions (2026-09-28)

1. Spark's trust model is acceptable for a person's main wallet. The app
   keeps a plain-language trust note (the Wallet's **i** button).
2. Separate agent seeds only if needed; postponed.
3. Code is not blocked on a NIP. The spend grant, request, and receipt
   format in [the wallet design](wallet-design.md#spending-grants) is the
   contract phase 1 implements; it becomes a NIP later.
4. No regulatory review.
5. The Breez API key is a basic validation key that can't be hidden,
   confirmed with the Breez team. It is committed in source, which replaces
   the design's build-time-input position and the history's lesson 4.
6. Mainnet on the phone. Spark has no Lightning test network, so regtest
   is for automated tests only. The phone's Mutinynet `ldk-node` wallet is
   replaced; `ldk-node` stays on computers and as the x402 receiver.
7. Buy bitcoin with dollars in the Wallet, through Breez's MoonPay and
   Cash App integrations ([#9865](https://github.com/OpenAgentsInc/openagents/issues/9865)).

- [Breez and Spark in this repository](history.md): every earlier use, from
  2025 to 2026, what worked, and why each ended, with commits.
- [Breez SDK review](sdk-review.md): capabilities, API surface, trust model,
  keys, fees, testing, and fit for a Rust-core iOS and Android app.
- [Breez and Spark](breez-vs-spark.md): who builds and runs what, Spark's use
  cases mapped to the Breez SDK, what each SDK lacks, and which to use.
- [USDC and USDT receive](stablecoin-receive.md): how Breez's cross-chain
  receive works through Flashnet Orchestra, its custody, fees, and failure
  paths.
- [Paying people](paying-people.md): how an npub resolves to a Spark address
  or Lightning address, publishing the phone's Spark address, QR routing,
  and contacts.
- [Wallet design](wallet-design.md): human wallets, agent wallets, spending
  grants, approval flows, peer payments, x402, key custody, testing, a phased
  plan, and open questions.

## Positions in brief

- Use the Breez SDK, linked directly into `crates/openagents-mobile`, for the
  phone Wallet. Spark's own SDK is TypeScript and does not fit.
- Replace the phone's Mutinynet `ldk-node` wallet with Spark; keep `ldk-node`
  on hosts as the x402 receiver.
- Spark is federated, not self-custody in the node sense. Fine for a phone
  spending wallet with plain disclosures; not for a treasury.
- A Spark wallet can pay x402 sellers but cannot receive x402 payments.
- Agents spend through phone approvals first, then their own bounded
  allowance wallets, then standing grants. Spark's operator-enforced token
  allowances are the best future primitive for dollar grants.
- Keep USDC and USDT receive off at first; it is new, mainnet only, and
  custodial during the swap.
