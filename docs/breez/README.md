# Breez and Spark

This directory studies re-adding the Breez SDK (Spark implementation) and
making the OpenAgents app's Wallet a Spark wallet for people and agents. The
phone Wallet runs it on mainnet (epic
[#9854](https://github.com/OpenAgentsInc/openagents/issues/9854)), and since
2026-10-02 computers run the same wallet from the same seed
([#10202](https://github.com/OpenAgentsInc/openagents/issues/10202)): the
shared code is `crates/spark-wallet` (`openagents-spark`), used by
`crates/openagents-mobile` and by `openagents wallet`. `crates/wallet`
(`ldk-node`) and `crates/x402` remain only as the x402 receiver, under
`openagents x402 node`; see [Bitcoin](../bitcoin/README.md).

## Owner decisions (2026-09-28)

1. Spark's trust model is acceptable for a person's main wallet. The app
   keeps a plain-language trust note (the Wallet's **i** button).
2. Separate agent seeds only if needed; postponed.
3. Code is not blocked on a NIP. The spend grant, request, and receipt
   format in [the spend protocol](spend-protocol.md), narrowed from
   [the wallet design](wallet-design.md#spending-grants), is the contract
   phase 1 implements; it becomes a NIP later.
4. No regulatory review.
5. The Breez API key is a basic validation key that can't be hidden,
   confirmed with the Breez team. It is committed in source, which replaces
   the design's build-time-input position and the history's lesson 4.
6. Mainnet on the phone. Spark has no Lightning test network, so regtest
   is for automated tests only. The phone's Mutinynet `ldk-node` wallet is
   replaced; `ldk-node` stays on computers and as the x402 receiver.
   **Superseded for the person's wallet on 2026-10-02 by decision 8.**
7. Buy bitcoin with dollars in the Wallet, through Breez's MoonPay and
   Cash App integrations ([#9865](https://github.com/OpenAgentsInc/openagents/issues/9865)).

## Owner decision (2026-10-02)

8. **Spark on every device.** Computers (OpenAgents Terminal, the
   `openagents` command, the desktop app) run the same Spark wallet as the
   phone, from the same seed, so the person has one balance everywhere.
   `ldk-node` stays only as the x402 receiver, hidden from people under
   `openagents x402 node`. This supersedes decision 6 for the person's
   wallet ([#10202](https://github.com/OpenAgentsInc/openagents/issues/10202)).
   The trigger: asking the chat on a computer for the balance printed the
   computer's `ldk-node` id, network, chain server, channels, and inbound
   liquidity.

   How it works:

   - `openagents wallet balance`, `address`, `receive`, `send`, and
     `history` run the shared wallet (`crates/spark-wallet`) on mainnet with
     the committed Breez key, and answer in plain words: "Your balance is
     ₿12,000 (0.00012000 BTC)." No answer names a node, network, chain
     server, channel, liquidity, or msat.
   - The wallet lives in `~/.openagents/spark`: the seed as a 0600 file in a
     0700 folder, and Breez's records as a JSON file
     (`openagents_spark::store`), because the computers' Cargo workspace
     cannot link Breez's SQLite store beside `ldk-node`'s (one
     `libsqlite3-sys` per lock file). The phone keeps Breez's SQLite store.
   - The seed reaches a computer two ways. `openagents wallet link` asks the
     phone through the computer's OpenAgents host (NIP-HOST
     `wallet.link.list` and `wallet.link.answer`); the phone shows the
     computer's name and a six-digit code that the computer also prints, and
     only after the owner approves (with Face ID or the passcode) seals the
     seed with NIP-44 to a one-time key that only that command holds.
     `openagents wallet restore` takes the recovery words typed without
     echo. Nothing prints the seed or the words.

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
- [Agent spend protocol, phase 1](spend-protocol.md): the grant, request, and
  receipt formats, refusal codes, ledger, and revocation that phase 1 runs
  (an agent asks; the owner approves each payment on the phone).
- [Amounts](amounts.md): BIP 177 display (`₿12,345`) with a legacy BTC
  toggle, the shared `bitcoin-amount` formatter, which protocol names keep
  `sat`/`msat`, and the audit of every surface
  ([#9881](https://github.com/OpenAgentsInc/openagents/issues/9881)).

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
