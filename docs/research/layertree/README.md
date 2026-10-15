# LayerTree

Research from 2026-10-15 on LayerTree, Lightning Labs' shared-UTXO design:
one Bitcoin output holds many users' balances as an off-chain Merkle-sum
tree, and any member leaves alone in one transaction. It covers what
LayerTree is, what it would take for OpenAgents to use it, and whether we
drive LND from Rust or keep to LDK.

| File | What it is |
| --- | --- |
| [analysis.md](analysis.md) | How it works (with diagrams), cost tables by tier and depth, the trust model (operator powers, data availability, eviction, the single-UTXO bottleneck), a comparison with Ark, channel factories, timeout trees, Lightning, Spark, statechains and Cashu-style mints, what the Lean proofs cover, what runs today, repo maturity, and claims against evidence |
| [integration-plan.md](integration-plan.md) | Where our Lightning code lives, four fits (many small balances, batched author and affiliate payouts, agent wallets that exit without us, Lightning receive via vouchers), what to do now, triggers for deeper work, experiments E0 to E4 with acceptance checks, and LDK in Rust against a Rust `lnd-sdk` driving LND, with the crate's surface |

## The short version

- **The design.** The tree's root tweaks the output's Taproot key, and the
  tapscript leaves are the rules. Every spend proves the old state, applies
  one rule and recreates the output under the new root. A solo exit is one
  transaction of about 1,020 vB at 65,536 members and about 1,150 vB at a
  million (FULL tier).
- **The catch.** It needs opcodes that are not active on mainnet or on any
  public test network (CCV, `OP_CAT`, `OP_TWEAKADD`, CSFS, `OP_TX`, the
  restored opcodes). Today it runs only in its Python simulator, its Go
  builders and its Lean model.
- **The bottleneck.** All activity goes through one UTXO, at about 25 to 64
  transitions per block per tree, and fee pinning is outside its model.
- **For OpenAgents.** Track it, reproduce it, and prototype the off-chain
  half in a sandbox crate (`crates/layertree-lab`). The best fit is turning
  owed author and affiliate earnings into credit leaves that payees can
  redeem without us. For the Lightning node, take a hybrid path: LND via a
  new Rust `crates/lnd-sdk` server-side, and LDK where the node must be
  embedded.

## Sources

- [layertree.tech](https://layertree.tech/), the interactive explainer
- [Short paper (PDF)](https://layertree.tech/paper/main.pdf) and
  [full paper (PDF)](https://layertree.tech/paper/main-full.pdf), revision
  `ba8422e`
- [github.com/lightninglabs/layertree](https://github.com/lightninglabs/layertree),
  read at commit `f6c4d29`
- [The announcement on X](https://x.com/lightning/status/2110812651882217677)

The PDFs and the upstream code are not committed here. The repository has
no LICENSE file yet, although its spec header says BSD-3-Clause.
