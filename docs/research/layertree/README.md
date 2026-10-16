# LayerTree

Research from 2026-10-15 on LayerTree, Lightning Labs' shared-UTXO design:
one Bitcoin output holds many users' balances as an off-chain Merkle-sum
tree, and any member leaves alone in one transaction. It covers what
LayerTree is, what it would take for OpenAgents to use it, and whether we
drive LND from Rust or keep to LDK.

| File | What it is |
| --- | --- |
| [analysis.md](analysis.md) | How it works (with diagrams), cost tables by tier and depth, the trust model (operator powers, data availability, eviction, the single-UTXO bottleneck), a comparison with Ark, channel factories, timeout trees, Lightning, Spark, statechains and Cashu-style mints, what the Lean proofs cover, what runs today, repo maturity, and claims against evidence |
| [upstream-code.md](upstream-code.md) | A walkthrough of the upstream Go packages roasbeef linked (`tree`, `script`, `txn`, plus `msg`) in our own words: the data structures, what each script does in each opcode tier, how transactions are built and signed, how a leaf exits, and how light clients follow the tree |
| [2026-10-15-mainnet-tee-spec.md](2026-10-15-mainnet-tee-spec.md) | **LayerTree on mainnet today, with a TEE enforcing the rules.** The exact missing opcodes; an enclave co-signer in a GCP Confidential VM; a pre-signed, timelocked unroll that needs no enclave; light clients; key lifecycle; threat model; verification mapped to upstream's Lean theorems; migration to real covenants as one rotation; OpenAgents uses; phases, costs and owner decisions |
| [integration-plan.md](integration-plan.md) | Where our Lightning code lives, four fits (many small balances, batched author and affiliate payouts, agent wallets that exit without us, Lightning receive via vouchers), what to do now, triggers for deeper work, experiments E0 to E4 with acceptance checks, and LDK in Rust against a Rust `lnd-sdk` driving LND, with the crate's surface |

## Public signet run

The [TDX verification report](2026-10-16-signet-verification.md) records real
signet creation, join, update, exit, and refresh transactions confirmed at
heights 326595–326599, with Google-backed image/key/channel checks and
public mirror readbacks. Its linked archive contains the public captures
and actual confirmation and fee measurements. The week-long run and
physical-loss recovery remains in progress under #11793. The separate
guardian HALT recovered all five exact 1,000-sat payouts at signet height
326616; see the report and [final capture](captures/signet-halt-paid.jpg).

![Confirmed five-member signet tree](captures/signet-tree.jpg)

## Regtest views

![LayerTree balances from the regtest run](captures/tree.jpg)

The [capture gallery](captures/README.md) shows confirmed regtest transitions,
a mined theft detected by three followers, and the timed unroll. Every image
names its log, event, and transaction where applicable. Attestation uses a
test vendor key; the later split/merge operations and failed-attestation
views are labelled simulations. These are not signet or hardware results.
The [Verse viewer](../../verse/layertree-visualizer.md) and four Video Lab
scene documents reproduce the stills and clips.

## The short version

- **The design.** The tree's root tweaks the output's Taproot key, and the
  tapscript leaves are the rules. Every spend proves the old state, applies
  one rule and recreates the output under the new root. A solo exit is one
  transaction of about 1,020 vB at 65,536 members and about 1,150 vB at a
  million (FULL tier).
- **The catch.** It needs opcodes that are not active on mainnet or on any
  public test network: CCV, `OP_CAT`, `OP_TWEAKADD`, CSFS, `OP_TX`, the
  BIP-441 restored opcodes and the EC-ops draft. It does not need CTV.
  Today it runs only in its Python simulator, its Go builders and its Lean
  model.
- **The mainnet route without a soft fork.** roasbeef's suggestion on
  2026-10-15 was a TEE that "enforces all the rules 'faithfully'". Our
  [spec](2026-10-15-mainnet-tee-spec.md) does this. An enclave key signs
  only transitions that pass LayerTree's rules, and a pre-signed unroll
  pays every member after a delay if the enclave stops. Exits after a halt,
  proofs and fraud detection stay trustless. Preventing theft becomes
  trust-the-TEE, so the spec caps the value at risk. The sandbox
  [`crates/layertree-tee-lab`](../../../crates/layertree-tee-lab) tests it.
  Its tests include property tests named after upstream's Lean theorems.
- **The bottleneck.** All activity goes through one UTXO, at about 25 to 64
  transitions per block per tree, and fee pinning is outside its model.
- **For OpenAgents.** Track it, reproduce it, and prototype in a sandbox
  crate outside the default build (`crates/layertree-tee-lab`, which now
  holds the off-chain tree and the TEE predicate). The best fit is turning
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
- roasbeef's follow-up thread on X, 2026-10-15 (pasted by the owner). He
  said "all primitives in place and tested, but no clients yet": light
  clients that follow spends, or leaves from a third party checked by
  Merkle sum. He linked the `tree`, `script` and `txn` packages. Asked by
  @AtlantisPleb whether it could go on mainnet now, he answered: "you could
  have some like TEE that enforces all the rules 'faithfully'". Julian
  (@_julian256_) noted that the GSR proposal has every opcode it needs
  except CCV, and said he plans to run it on his Core-based `0xc2`
  implementation.

The PDFs and the upstream code are not committed here.

**Licence, checked 2026-10-15 at `f6c4d29`:** there is still no LICENSE
file, and the GitHub API reports `license: null`. The spec header says
`License: BSD-3-Clause`. Until a LICENSE file lands we write our own code
from the spec, and we only read upstream's published test vectors at test
time.

**What changed since the first pass (same day):** upstream is still at
`f6c4d29`, so nothing changed upstream. What is new on our side:

- the thread above and the TEE route;
- the package walkthrough;
- a corrected commit count (516 commits, all by one author, from
  2026-09-16; the first pass said about 70);
- the BIP-158 nuance in the light-client section of upstream-code.md;
- the sandbox crate, whose tree matches upstream's vectors byte for byte.
