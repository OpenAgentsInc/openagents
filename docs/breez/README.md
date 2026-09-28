# Breez and Spark

This directory studies re-adding the Breez SDK (Spark implementation) and
making the OpenAgents app's Wallet a Spark wallet for people and agents. It is
research and design only; no code uses Breez or Spark on `main` today. The
current Bitcoin code is `crates/wallet` (`ldk-node`) and `crates/x402`; see
[Bitcoin](../bitcoin/README.md).

- [Breez and Spark in this repository](history.md): every earlier use, from
  2025 to 2026, what worked, and why each ended, with commits.
- [Breez SDK review](sdk-review.md): capabilities, API surface, trust model,
  keys, fees, testing, and fit for a Rust-core iOS and Android app.
- [Breez and Spark](breez-vs-spark.md): who builds and runs what, Spark's use
  cases mapped to the Breez SDK, what each SDK lacks, and which to use.
- [USDC and USDT receive](stablecoin-receive.md): how Breez's cross-chain
  receive works through Flashnet Orchestra, its custody, fees, and failure
  paths.
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
