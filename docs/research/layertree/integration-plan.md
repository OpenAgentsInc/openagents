# LayerTree: OpenAgents integration plan

How LayerTree could fit OpenAgents' Bitcoin payments, what to do now, what
would trigger deeper work, and a staged experiment plan with acceptance
checks. It also weighs the two Lightning node paths (LDK in Rust against a
Rust `lnd-sdk` that drives LND) for LayerTree and for payouts generally.
The design itself is in [analysis.md](analysis.md).

## Decision in brief

- **Track LayerTree. Do not build on it for production.** It needs soft
  forks that are not active anywhere, not even on a public signet.
- **Run it now where it runs**: the upstream Python simulator, Go builders
  and Lean model, at a pinned commit, in a scratch clone.
- **Prototype the off-chain half in Rust** in a sandbox crate,
  `openagents/crates/layertree-lab`: the tree, proofs, path maintenance and
  vouchers, checked byte for byte against upstream test vectors. Then run the
  real payout ledger through it in shadow mode, with no money and no chain.
- **Go deeper only on a trigger**: a public test build with one tier's
  opcodes, activation signalling on mainnet, Lightning Labs operator
  software, or a licence file plus an external review.
- **Lightning node path: a hybrid.** LND, driven by a new Rust
  `crates/lnd-sdk`, for server-side operator nodes, liquidity, Taproot
  Assets and any LayerTree operator work. LDK stays wherever the node must be
  embedded: per-machine `openagents wallet`, agent wallets and mobile.
  Production payout rails stay as they are until `lnd-sdk` passes its
  regtest gate.

## Where our Bitcoin and Lightning code lives today

| Surface | Code | Node | Role |
| --- | --- | --- | --- |
| Central receiver and x402 | [`crates/wallet`](../../../crates/wallet) (`openagents-wallet`) | MoneyDevKit's `ldk-node` 0.7.0 fork with LSPS4 JIT liquidity (`--lsp mdk`) | Receives every first-party payment; the x402 receiver and payer ([central receive and splits](../../payments/2026-10-02-central-receive-and-splits.md)) |
| Splits, earnings, payouts | [`crates/pay-ledger`](../../../crates/pay-ledger) (`payout.rs`, a `Rails` trait), [`crates/gateway/src/earnings.rs`](../../../crates/gateway/src/earnings.rs) | through `crates/wallet` and the payout Spark wallet | Accrues msat shares to plugin authors, resource owners and affiliates; pays Lightning addresses and Spark in batches per payee, with thresholds |
| Affiliate payouts | #11698, [#11727](https://github.com/OpenAgentsInc/openagents/issues/11727) | same rails | Attribution, commission terms, payouts-due list; the owner sends payouts for now |
| Agent accounts | [OpenAgents API D13](../../api/2026-10-02-openagents-api.md) | the central receiver | Accounts without signup draw on a prepaid sats balance topped up by Lightning invoice (custodial) |
| The person's Wallet | [`crates/spark-wallet`](../../../crates/spark-wallet) | Breez SDK, Spark | One balance on phone and computers |
| Campaign treasury | MDK `MdkTreasuryContainer` | MDK (`lightning-js`) | Outbound payout and custody bridge (the MDK/Nexus bridge) |
| Taproot Assets | [`~/work/tap-ldk`](https://github.com/OpenAgentsInc/tap-ldk) + [`~/work/ldk-node`](https://github.com/OpenAgentsInc/ldk-node) fork | OpenAgentsInc `rust-lightning` fork | Demo-grade native LDK Taproot Assets wallet; README last updated 2026-05-30 |
| Swap network | [`~/work/immortal`](https://github.com/OpenAgentsInc/immortal) `immortal-provider` | operator-declared bitcoind + CLN or feature-gated LND REST | Liquidity-provider daemon; its own minimal rail clients |
| History | [bitcoin node history](../../bitcoin/2026-09-28-bitcoin-node-history.md) | `oa-lnd` (LND v0.20.1, Feb to Jul 2026), `nexus-ldk-mainnet-1` | Both swept and deleted in the 2026-07-30 cost cleanup |

Payments are Bitcoin-only: Lightning, Taproot Assets and Bitcoin-based
stablecoins. Cashu is out for now
([agent payments](../../payments/agent-payments.md)). LayerTree is
Bitcoin-native, so it fits that rule.

## Fit assessment

### 1. Custody-minimising balances for many agents and users

**What it would replace.** Agent prepaid balances (D13) and owed earnings in
`pay-ledger` are custodial liabilities today: we hold the sats, and the
ledger says whose they are.

**What LayerTree offers.** OpenAgents as a tree operator would hold no key
over member balances. Each agent or user would get a leaf it can exit alone,
and grafting a 1,024-member cohort costs under 1 vB per member.

**The catch is the size of the balances.** A solo exit costs about ₿10,200
at 10 sat/vB (1,020 vB). Most agent balances are tens to thousands of sats,
so for them a unilateral exit is a right they could never afford to use. Two
mechanisms still protect them. Eviction and cohort rewrites pay them on-chain
at 150 to 200 vB each. And a full-tree mass exit is throughput-bound at about
25 to 64 transitions per block.

**Verdict: good fit for mid-size balances (about ₿100,000 and up) and for
operator capital efficiency; weak for micro-balances.** For micro-balances,
custodial accounting with a Lightning cash-out is still the practical answer.

### 2. Batched payouts to affiliates and plugin authors

**Today.** The payout worker drains each payee's accrued shares to a
Lightning address or Spark once a threshold is reached. That costs nothing
on-chain, but until a payout goes out the payee holds only our promise.

**The LayerTree shape.** Give each payee a **credit leaf**: owned by
OpenAgents, with the payee's key as exit key, in a tree that carries
`CLAIM^x`, `UPDATE^x` and the exact exit. For each settlement period, sign a
**cumulative voucher** for the payee's running total of owed earnings.

- Owed earnings become claims the payee can enforce without us: redeem the
  latest voucher, then exit (about 2,656 vB).
- Per payment the chain sees nothing. A period settles with one cohort graft
  (under 3 vB per member) or one `REWRITE^x_h` (about 164 vB per leaf).
- It turns `pay-ledger`'s liabilities into something close to a public proof
  of liabilities, because the tree is a Merkle-sum tree.

**Costs.** OpenAgents must pre-fund credit leaves, which is capital locked
per payee. We take on lease management and tree operations. And every
voucher is a tracked liability, which `pay-ledger` already models.

**Verdict: the most interesting fit.** It upgrades "trust our ledger" to
"redeem without us" for earnings owed to authors and affiliates, and it can
be modelled entirely off-chain today.

### 3. Agent wallets that can exit without us

The tower exit `EXIT^t` lets an agent sign one authorisation in advance, so
a tower (the agent owner's machine, or a third party) can build and rebuild
the exit while the agent is offline. That matches agents that run
intermittently. The agent still needs a confirmed fee coin (except under
FULL's fee-from-payout exit) and a backed-up join path.

**Verdict: fits the "agents own their money" direction.** It depends
entirely on soft forks and on an operator; per-agent LDK nodes or Spark do
this today without a fork.

### 4. Lightning receive via vouchers

An agent with no node of its own receives a payment: the operator settles
the HTLC, then signs a cumulative voucher into the agent's credit leaf.
Nothing happens on-chain per payment, and the agent can be offline. That is
the same user story as our x402 receiver for agents without nodes, but
non-custodial after the voucher. The plain flow leaves the agent trusting us
for one payment. The pending-leaf flow closes that gap, but the agent (or
its tower) must be live before each window's deadline.

**Verdict: a strong fit for agent micropayments once it can run.** The
nearest thing deployable today is the LSPS4 JIT receive we already use.

### Summary

| Fit | Value to us | Runs today | Main blocker |
| --- | --- | --- | --- |
| Many small balances | medium (mid-size balances only) | no | soft forks; solo-exit fee floor |
| Batched payouts to authors and affiliates | **high** | the off-chain model, in simulation | soft forks; pre-funding capital |
| Agent wallets that exit without us | medium-high | no | soft forks; fee coins |
| Lightning receive via vouchers | high | no | soft forks; an operator LN stack |

## What to do now

1. **Track it.** Watch `lightninglabs/layertree` releases, spec version bumps,
   the `SLASH` issue, a LICENSE file, any Inquisition or Mutinynet
   deployment of a tier's opcodes, and Lightning Labs operator tooling. Pin
   the commit we evaluated (`f6c4d29`) in the crate README.
2. **Run upstream where it runs** (experiment E0 below).
3. **Prototype the off-chain half in Rust** (E1, E2). No consensus or node
   dependency, no money.
4. **Do not** fork, vendor or copy upstream code until a LICENSE file
   exists. Reading the spec and checking against their published test
   vectors is fine.

## Triggers for deeper work

| Trigger | What it unlocks | Next step |
| --- | --- | --- |
| A public test build (Inquisition, Mutinynet or a Lightning Labs signet) carrying one tier's opcodes, MIN (`OP_CAT` + `OP_TWEAKADD`) most likely | real transactions on a test chain | E3: regtest/signet exits, joins, rewrites, throughput |
| Lightning Labs ships operator software (lnd, tapd or Loop integration) | a reference operator to interoperate with | E4 on LND via `lnd-sdk` |
| Activation signalling for CCV, `OP_CAT` + `OP_TWEAKADD`, or GSR + `OP_TX` + CSFS on mainnet | production planning | custody and legal review; payout pilot design |
| A LICENSE file plus an external review or audit | code reuse and trust in the bytecode | consider porting script generators to Rust |
| A native Merkle-sum opcode proposal | exits near 580 vB at 1M members | re-run the cost model for micro-balances |

## Experiment plan

All experiments live in the `openagents` monorepo, in a sandbox crate
`crates/layertree-lab`. LayerTree is not Taproot Assets work, so it does not
belong in `tap-ldk`. `immortal` has strict dependency allowlists and a
different product. Payments, wallet and payout surfaces are owned by
`openagents`. The crate stays out of the default build and the product
binaries until a trigger fires.

### E0: reproduce upstream (days)

Clone `lightninglabs/layertree` at `f6c4d29` into a scratch directory
outside every repo. Run the CI list (`tools/layertree_ref.py`,
`test_scripts.py`, `test_mid.py`, `test_min.py`, `test_ccv.py`,
`test_msu.py`, `test_chan.py`, `gen_golden.py`, then a clean
`git diff spec/`), `go test ./...` and `cd formal && lake build`. Delete the
clone afterwards.

**Acceptance:**

- Every suite passes, and `gen_golden.py` leaves no diff.
- `spec/scripts/sizes.md` reproduces EXIT_1 at 1,020 vB (D=16) and
  1,149 vB (D=20).
- The Lean axiom audit prints 72 theorems with no `sorryAx`.
- The result is recorded in the crate README with the commit hash.

### E1: the off-chain tree in Rust (one to two weeks)

`crates/layertree-lab`, written fresh from the spec:

- the 116-byte leaf, tagged hashes, Merkle-sum nodes and `enc64`;
- a dense tree with fold, sibling paths and `E_h`;
- path maintenance from a transition witness (spec 9.7);
- the voucher, cumulative voucher and `EXIT^t` message hashes, with
  BIP-340 signing on the workspace's existing `secp256k1`/`bitcoin` crates;
- `TreeSPK(r, m)` computation for the FULL tier;
- a simulated ledger of transitions (JOIN, EXIT, UPDATE, CLAIM, REWRITE)
  that checks conservation, with no script execution.

**Acceptance:**

- Roots, leaf encodings, voucher messages and tree keys match upstream
  `spec/test-vectors.json` byte for byte. The test fetches the vectors
  from a pinned URL or a local scratch path; they are not vendored until
  there is a licence.
- Property tests: conservation over random transition sequences; path
  maintenance after every step; voucher non-replay after `seq` advances.
- A negative test: a taptree with an extra leaf fails the `m` recomputation
  check.

### E2: payout ledger in shadow mode (one week)

Feed `pay-ledger` settlement and payout shapes into the E1 model. Use a
synthetic ledger generated from aggregate counts (payees, accrual
distribution, payout cadence), never private ledger rows, and model:

- one credit leaf per payee with a cumulative voucher per period;
- settlement by `JOIN_8`/`JOIN_10` cohort grafts or `REWRITE^x_6`;
- leases aligned per cohort, and renewals.

**Acceptance:**

- A report table: on-chain vB and sats per period at 2, 10 and 50 sat/vB;
  capital pre-funded; how long a forced dissolution takes at κ = 25 and 64.
  It compares the current Lightning-address/Spark rails, LayerTree credit
  leaves and plain on-chain batch payouts.
- A break-even line: the smallest payee balance for which a unilateral
  claim plus exit costs under 10% at each feerate.
- Conservation holds across the simulated year. No private data appears in
  fixtures.

### E3: on a chain (trigger-gated)

Start this only when a node build with one tier's opcodes exists (an
Inquisition or Mutinynet deployment, or a published patched regtest).

**Acceptance:**

- EXIT_1, JOIN_h, `REWRITE_h`, `EVICT_SUBTREE_h` and `DETACH_h` confirm on
  regtest or signet, and their serialised sizes are within 1% of
  `spec/scripts/weights.md`.
- The measured unconfirmed chain limit is 25 under the old ancestor rules
  and 64 under cluster mempool.
- An exit rebuilt after a competing spend confirms within the paper's
  `Δ(q)` bound at β = 0.
- A member with only its leaf and join-time path exits after the operator
  process is deleted.

### E4: Lightning receive into a credit leaf (trigger-gated)

On regtest, an operator node settles an HTLC, then signs a cumulative
voucher into a credit leaf; then run the pending-leaf variant.

**Acceptance:**

- Ten payments produce one on-chain transaction or none.
- The recipient redeems the latest voucher alone after the operator stops,
  and the amounts conserve.
- The one-payment exposure of the plain flow shows up in a test, and the
  pending-leaf flow removes it.

The operator node for E4 is LND, driven by `lnd-sdk` (see below), because
Lightning Labs' own LayerTree tooling will most likely target lnd first.

## Lightning node path: LDK in Rust, or a Rust `lnd-sdk` driving LND

The owner asked us to weigh this:

> "We've prioritized LDK Rust stuff in the past, but consider we may just
> build a steering apparatus to drive the Go-based LND, to use all their same
> liquidity infrastructure and battle-tested node software, just using Rust:
> lnd-sdk or something."

### The two paths

**Path 1, LDK in Rust (current direction).** `crates/wallet` embeds
`ldk-node` (MoneyDevKit's fork, for LSPS4). `tap-ldk` and our `ldk-node`
fork pursue native Taproot Assets channels over LDK, explicitly "without
running LND or `tapd` as a wallet sidecar". One Rust process holds the node.

**Path 2, a Rust `lnd-sdk`.** This is a typed client and supervisor that
drives LND and Lightning Labs' stack: `tapd` for Taproot Assets, Loop, Pool,
Lightning Terminal (`litd`) and LNC. Reference clones are in
`~/work/projects/lightninglabs/repos/` (`lnd`, `taproot-assets`, `loop`,
`lightning-terminal`, `lnc-core`, `lightning-node-connect`; Pool is not
cloned yet).

- **Transport:** gRPC over `tonic` (already in our `Cargo.lock` at 0.12)
  with TLS pinned to the node's certificate and a macaroon interceptor; REST
  as a fallback where gRPC is awkward.
- **Supervision:** start or attach to `lnd`/`tapd`/`litd` processes, unlock
  the wallet, wait for readiness through `StateService`, check health,
  reconnect with backoff, and stop gracefully.
- **Interfaces:** typed Effect-style service traits with typed errors, so
  Coder, the Worker-facing services and the payout worker call it like any
  other Rust service. In practice it implements `pay-ledger`'s `Rails` and
  `openagents-wallet`'s `LightningWallet`, so callers do not change.

### Weighing them

| Factor | Path 1: LDK in Rust | Path 2: `lnd-sdk` + LND |
| --- | --- | --- |
| Node maturity | LDK is solid, but our forks (`rust-lightning`, `ldk-node`) carry our own changes | LND is the most deployed implementation; we ran `oa-lnd` v0.20.1 on mainnet from Feb to Jul 2026 |
| Liquidity infrastructure | LSPS clients (MDK LSPS4 today); we source liquidity ourselves | Loop (submarine swaps), Pool (channel leases), Lightning Terminal, LNC remote access, a large LSP ecosystem |
| Taproot Assets | `tap-ldk` is a demo: one asset channel and payments in both directions work, but its README lists live force-close, sweep recovery, a proof courier, grouped assets and RFQ checks as not done; no update since 2026-05-30 | native in `tapd` today (asset channels, RFQ, universe); `tap-ldk` already proved interop against `litd` |
| LayerTree | we would port everything ourselves | the reference implementation is Go on btcd, lnd and taproot-assets, and the author's design sketch ties it to Loop and a liquidity marketplace, so operator tooling will most likely land in Lightning Labs' Go stack first; `lnd-sdk` would track it closely |
| Moving parts | one Rust binary; the node is a library | a Go binary per node (lnd, plus tapd/litd), its database, SCB/`channel.backup` handling, and version upgrades |
| Secrets | the seed in our process | the seed in lnd, plus **macaroon custody**: an admin macaroon is full spend authority, so we bake scoped macaroons (invoice-only, read-only, pay with limits) and keep them in Secret Manager |
| Embedding (mobile, agent wallets, per-machine CLI) | yes | no; a server-side process only |
| Bitcoin-only rails | fits | fits |

### Fit with our rules

- **Bitcoin-only rails:** both paths fit.
- **`immortal`'s no-sidecar rule.** `immortal/AGENTS.md` rule 1 allows "one
  binary and one Postgres database per product" and no broker, cache or sync
  engine, but "the provider may connect to the operator's declared bitcoind
  and Lightning rail nodes". LND as a declared rail node is therefore
  allowed, and the provider already has a feature-gated LND REST rail with a
  pinned certificate (owner-approved `tokio-rustls`, 2026-08-05). The
  tension is with dependencies: `immortal` crates have owner-approved
  allowlists and forbid third-party Bitcoin crates, so they cannot depend on
  an `openagents` `lnd-sdk` built on `tonic`/`prost`. `immortal` keeps its
  own minimal rail client, and `lnd-sdk` serves `openagents`.
- **`tap-ldk`'s thesis.** Path 2 contradicts `tap-ldk`'s stated goal
  (Taproot Assets without LND or `tapd`). Choosing `tapd` for server-side
  Taproot Assets makes `tap-ldk` embedded-wallet research, not the
  production asset path. That is an owner-level call, flagged below.
- **The MDK/Nexus payout bridge.** Nothing here replaces it. `lnd-sdk` would
  be a third implementation behind `pay-ledger`'s `Rails` trait, next to the
  `crates/wallet` LDK node on MDK LSPS4 and the Spark payout wallet. The MDK
  treasury container keeps its role.

### Recommendation

**Use the hybrid.**

- **For LayerTree experiments:** E0 to E2 need no node. For E3 and E4, use
  LND through `lnd-sdk` as the operator's Lightning node, because Lightning
  Labs' reference code and any operator tooling will be Go on lnd, btcd and
  taproot-assets.
- **For payouts generally:** keep production as it is (`crates/wallet` on MDK
  LSPS4, plus Spark). Build `lnd-sdk` as the server-side operator rail and
  prove it on regtest. Promote it to a production payout rail only for a
  concrete need: Taproot Assets stablecoin payouts through `tapd`, or Loop
  and Pool liquidity once payout volume makes LSPS4 fees or limits bind.
- **Keep LDK where embedding matters:** `openagents wallet` on each machine,
  agent wallets, and mobile, where a Go daemon is not an option.

### The `lnd-sdk` crate

**Home:** `openagents/crates/lnd-sdk`. Payments and wallet surfaces are owned
by `openagents`; `tap-ldk`'s purpose excludes LND; `immortal`'s allowlists
exclude it.

**Modules:**

| Module | Contents |
| --- | --- |
| `config` | endpoints, network, TLS certificate pin, macaroon source (file or Secret Manager path; never logged), timeouts |
| `transport` | `tonic` channel over rustls with certificate pinning, a macaroon metadata interceptor, deadlines, retry and backoff for idempotent calls only |
| `proto` | code generated by `tonic-build`/`prost` from a pinned snapshot of `lnd` (`lnrpc`, `routerrpc`, `invoicesrpc`, `walletrpc`, `stateservice`, `walletunlocker`) and `taproot-assets` (`taprpc`, `tapchannelrpc`, `rfqrpc`, `universerpc`) `.proto` files, which are MIT-licensed upstream |
| `lightning` | typed calls: `get_info`, `wallet_balance`, `channel_balance`, `list_channels`, `open_channel`, `close_channel`, `add_invoice`, `lookup_invoice`, `subscribe_invoices`, `send_payment` (`routerrpc.SendPaymentV2` with a fee cap), `track_payment`, `estimate_route_fee` |
| `onchain` | `new_address`, `list_unspent`, `send_coins`, fee estimation (`walletrpc`) |
| `tap` | `list_assets`, `list_balances`, `new_addr`, `send_asset`, asset channel `fund_channel`, asset invoices and payments (`tapchannelrpc`), RFQ quotes |
| `macaroon` | bake and verify scoped macaroons: invoice-only for receivers, read-only for dashboards, pay-with-limits for the payout worker |
| `supervisor` | spawn or attach to `lnd`/`tapd`/`litd`; unlock; readiness via `SubscribeState`; health loop; reconnect; graceful stop; version check against the pinned protos |
| `service` | the traits callers use (`LightningRail`, `AssetRail`, `NodeHealth`) with typed errors, plus adapters implementing `pay_ledger::payout::Rails` and `openagents_wallet::LightningWallet` |
| `fake` | an in-process fake of each trait for unit tests of callers |

**First RPCs, in order:**

1. `GetInfo`, `WalletBalance`, `ChannelBalance` (health and balances).
2. `AddInvoice`, `LookupInvoice`, `SubscribeInvoices` (receive).
3. `SendPaymentV2` with a fee limit, `TrackPaymentV2` (pay, idempotent on
   the payment hash, to match the payout worker's recovery rules).
4. `ListChannels`, `OpenChannel`, `CloseChannel`.
5. `tapd`: `ListAssets`, `ListBalances`, `NewAddr`, `SendAsset`, then the
   asset channel calls.

**Testing:**

- **Unit:** the `fake` module, plus a `tonic` mock server for transport,
  macaroon and error mapping.
- **Integration:** a Polar-style regtest fixture, `scripts/lnd-sdk-regtest.sh`,
  running bitcoind plus two `lnd` nodes plus `tapd` (or `litd`) in Docker. It
  funds wallets, opens a channel, and runs invoice, pay, track and an asset
  send. These tests are opt-in (ignored by default) so normal crate tests
  stay fast. Reuse what `tap-ldk`'s `litd` compatibility demo already learned.
- **Contract:** the payout worker's existing state-machine tests run
  against the `lnd-sdk` adapter on regtest, including `unknown`-state
  recovery without a second send.

**Before building:** check existing Rust LND clients (for example
`tonic_lnd` and its forks) and pick generation from pinned protos unless one
of them is current, small and maintained. We need control over the proto
version and the dependency set.

## Owner decisions this raises

- Whether server-side Taproot Assets moves to `tapd` through `lnd-sdk`,
  making `tap-ldk` embedded-wallet research only.
- Whether to start `crates/lnd-sdk` now (regtest-only first milestone), or
  wait for a concrete payout need.
- Whether credit-leaf "redeem without us" earnings (fit 2) is worth a
  custody and legal review once a trigger fires.

None of these is needed for E0 to E2, which can start now.
