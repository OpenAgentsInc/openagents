# Lightning Labs Wavelength, and where it fits our LND plan

Analysis, 2026-10-15. Status: **assessment; no code.** It reads
[lightninglabs/wavelength](https://github.com/lightninglabs/wavelength) at
`main` `85cfb979` (2026-10-15, after release `v0.1.2`), and relates it to
[the LND plan](2026-10-15-lnd-sdk-plan.md) and its epic
[#11762](https://github.com/OpenAgentsInc/openagents/issues/11762).

## Contents

1. [Summary](#summary)
2. [What it is](#1-what-it-is)
3. [What it has](#2-what-it-has)
4. [How it works](#3-how-it-works)
5. [Maturity: claims and evidence](#4-maturity-claims-and-evidence)
6. [Does it run on mainnet today](#5-does-it-run-on-mainnet-today)
7. [Fit with our payment surfaces and the LND phases](#6-fit-with-our-payment-surfaces-and-the-lnd-phases)
8. [Pros and cons](#7-pros-and-cons)
9. [Risks](#8-risks)
10. [Recommendation](#9-recommendation)
11. [Sources](#sources)

## Summary

- **What it is.** A self-custodial Bitcoin **wallet client**, written in Go:
  an Ark client, a Lightning swap client and an on-chain wallet in one daemon
  (`waved`) with a CLI (`wavecli`), gRPC and REST, an MCP server for agents,
  and embeddable SDKs (Go in-process, browser WASM, gomobile for iOS and
  Android). MIT licence.
- **What it is not.** It is not a Lightning node, and it is not operator
  software. The Ark operator (`lumosd`) and the swap server (`swapd`) are
  Lightning Labs services in separate repositories that are not public. Every
  Wavelength wallet depends on them.
- **Mainnet.** The code supports mainnet behind `--allow-mainnet`, with
  Lightning Labs' mainnet endpoints built in. Lightning Labs' own launch post
  says mainnet is **invite-only alpha**, with "full mainnet availability in the
  next release". Test networks (signet, testnet3, testnet4) are open.
- **Fit.** It does not replace any LND phase. Our server side needs a node we
  control, with our own liquidity, invoices and `tapd`. Wavelength is a client
  that depends on someone else's operator. Its natural place is **beside P4**:
  a candidate for agent-side and per-machine wallets that today would be an
  LDK node plus an LSP.
- **Recommendation: adopt later.** Keep the LND plan as written. Run one
  bounded signet spike now
  ([#11770](https://github.com/OpenAgentsInc/openagents/issues/11770)) to measure it
  as an agent wallet. Reassess on the triggers listed below: mainnet general
  availability, published operator terms, and live Taproot Assets
  stablecoins.

## 1. What it is

`wavelength` "is a self-custodial Bitcoin wallet system written in Go. It
unifies three layers that usually live in separate tools: an Ark client, a
Lightning swap engine, and an on-chain wallet" (README).

- **Type:** a daemon plus libraries. `waved` is a long-running,
  restart-safe process. `sdk/ark`, `sdk/swaps` and `sdk/wavewalletdk` embed
  the same daemon in-process over a private `bufconn` transport.
- **Language:** Go 1.26. About 810 non-test Go files and 650 test files.
- **Protocol:** Ark. Users hold **VTXOs**, virtual outputs in a tree of
  pre-signed transactions under an operator's batch ("round") transaction.
  Transfers happen in rounds or out of round (OOR) with operator co-signing.
  Any VTXO can be exited on-chain without the operator after a relative
  timelock (CSV).
- **Lightning** is reached through **atomic swaps**, not channels. A swap
  server pays and receives Lightning, and funds or claims a **vHTLC**: an Ark
  output with the same hashlock and timelock logic as a Lightning HTLC.
- **Bitcoin-only.** Bitcoin, Lightning and (in development) Taproot Assets.
  Nothing here touches EVM chains, so it fits our
  [rails rule](agent-payments.md).

### Dependencies

| Dependency | How it is used | Required? |
| --- | --- | --- |
| Lightning Labs Ark operator (`lumosd` + its indexer) | Rounds, OOR co-signing, the authoritative index of live VTXOs | **Yes.** Not public; Lightning Labs runs it |
| Lightning Labs swap server (`swapd`) | Holds incoming HTLCs, funds vHTLCs, pays outgoing invoices, keeps the credit ledger | For any Lightning send or receive (`swapruntime` build) |
| Loop | Lightning Labs' liquidity service, used by the swap server for Lightning liquidity (launch post). In this repo only Loop's FSM library is imported (`sdk/swaps`) | Indirectly, through `swapd` |
| `lnd` | **Optional** wallet backend (`--wallet.type=lnd`) for signing and chain access. `lndclient` and `lnd` packages are Go dependencies | No. The default `lwwallet` uses btcwallet plus Esplora; `btcwallet` uses Neutrino |
| `tapd` / `tap-sdk` v0.3.0 | `tapassets/`: asset boarding and asset VTXO trees | Only for Taproot Assets, which is not live for users yet |
| Esplora or Neutrino | Chain data; defaults point at Lightning Labs' mempool instances | Yes, unless the `lnd` backend is used |
| SQLite or PostgreSQL | Durable state | Yes |

## 2. What it has

| Feature | What the code has | State |
| --- | --- | --- |
| Ark client | Boarding, in-round and OOR transfers, refresh, cooperative leave, unilateral exit (`round`, `vtxo`, `oor`, `unroll`) | Shipped since `v0.1.0` (2026-07-21) |
| Unilateral exit | Per-target exit actor, CPFP fee bumping, package relay, a dependency planner (`unrollplan`, `txconfirm`) | Shipped; `v0.1.2` fixed several interrupted-exit cases |
| Fraud watching | Watches OOR ancestor outpoints and starts an exit if one is spent on-chain (`fraud`) | Shipped |
| Lightning send and receive | Swaps through vHTLCs with a durable FSM (`sdk/swaps`); the client issues its own BOLT-11 invoice with a route hint through `swapd` | Shipped (`swapruntime` build tag) |
| Same-Ark payments | Two clients of one operator settle a payment without touching Lightning | Shipped |
| Credits | A sat balance held in the **swap server's ledger**, keyed by wallet identity, for amounts below the operator's minimum VTXO size | Shipped. Custodial by design: the server ledger is authoritative |
| Wallet API | Seven verbs (`create`, `unlock`, `send`, `recv`, `activity`, `balance`, `exit`) over `wavewalletrpc`, gRPC and REST | Shipped (`wavewalletrpc` build tag) |
| Agent integration | `wavecli mcp serve` exposes wallet verbs as typed MCP tools. Create and unlock stay outside the agent channel | Shipped |
| Embedding | Go in-process SDK, browser WASM (`cmd/wavewalletdk-wasm`), gomobile for iOS and Android; the website advertises React Native, Swift and Kotlin SDKs | Go and WASM in this repo; the host SDKs are not in this repo |
| Access control | Macaroons and TLS (`rpcauth`); `v0.1.2` added narrower per-application access | Shipped |
| Fee accounting | Double-entry fee ledger (`ledger`) | Shipped |
| Seed restore | Recovery scans owner and OOR receive keys | Fixed in `v0.1.2` |
| Scheduled batches | Clients join operator rounds on a published UTC timetable | Shipped |
| Taproot Assets | Asset boarding, asset VTXO trees, asset holdings reported apart from Bitcoin (`tapassets`, PRs merged 2026-09 to 2026-10-14) | **In development.** "Stablecoins are coming soon" (launch post) |
| L402 | Activity rows carry L402 proof of payment for settled Lightning sends (`swapwallet/credit_projector.go`) | Paying L402 works as any Lightning send; nothing L402-specific beyond that |
| Lightning address, LNURL, BOLT12 | None found in the code | **Absent.** A wallet can't be a static payout destination |
| Metrics | Prometheus under `waved_` | Shipped |

## 3. How it works

### Receiving Lightning

From `docs/swap_system.md` §3:

1. The client allocates a claim script and registers it with the operator's
   indexer.
2. It asks `swapd` for a route hint and fee, then issues a BOLT-11 invoice
   **signed by its own key**, with a route hint through `swapd`.
3. The payer's HTLC reaches `swapd`, which **holds** it and funds a vHTLC on
   Ark (sender `swapd`, receiver the client).
4. The client finds the funded vHTLC through the indexer, claims it with the
   preimage in an OOR transfer, and so reveals the preimage.
5. `swapd` settles the held HTLC with that preimage.

The receiver must be online to claim. The payment is atomic: `swapd` can't
take the HTLC without the client's preimage, and the client can't claim
without `swapd` funding the vHTLC.

### The vHTLC and its exits

A six-leaf taproot tree: three collaborative leaves that include the
operator key (settled off-chain by OOR), and three unilateral leaves without
it (CSV-gated, settled on-chain). `lib/arkscript` enforces that no leaf lets
the operator spend alone. That is the basis for the "never has unilateral
control of funds" claim, and it is enforced in code, not just stated.

### What the user still trusts

- **Liveness.** VTXOs expire. The client must refresh them in a round before
  expiry, or exit. Automatic refresh waits for the operator's free window. An
  offline wallet can fall into a critical state, and `v0.1.2` notes cases
  where a unilateral exit was not fundable.
- **The indexer.** The operator's indexer is the only witness to on-Ark
  outputs a swap creates.
- **Credits.** Sub-floor amounts live as a row in Lightning Labs' swap
  server ledger. That part is custodial.
- **Exit cost.** A unilateral exit pays on-chain fees for the whole branch
  from the batch transaction to the leaf. For micro-balances that right is
  too expensive to use, the same problem the
  [LayerTree analysis](../research/layertree/integration-plan.md#1-custody-minimising-balances-for-many-agents-and-users)
  found.

## 4. Maturity: claims and evidence

| Claim | Source | Evidence | Verdict |
| --- | --- | --- | --- |
| "Self-custodial; users keep their keys" | README, launch post, website | Keys are derived and held in the client. `lib/arkscript` rejects any leaf the operator can spend alone. Unilateral exit code exists and has release-tracked fixes | **Supported**, with the trust limits above (liveness, indexer, credits) |
| "Exit to on-chain at any time without the operator" | Launch post, website | `unroll`, `unrollplan`, `txconfirm`, `fraud`; `v0.1.2` notes interrupted exits that needed fixing, and that an exit can be unfundable | **Supported in design**; operationally young |
| "Lightning without nodes, channels or liquidity" | Launch post | True for the user. Liquidity sits with `swapd` and Loop, which are not in this repo | **Supported**, by moving the dependency to Lightning Labs |
| "1 basis point service fee" | Launch post | Not in this repo; the fee is set by the server | **Unverified here**; the post says alpha pricing may change |
| "Stablecoins coming soon through Taproot Assets" | Launch post | `tapassets` and asset reporting merged 2026-09 to 2026-10-14; no user-facing asset send or receive in the wallet verbs yet | **In progress** |
| "Open source" | Launch post, Bitcoin Magazine | The client is MIT. The operator and swap server are not public | **Partly**: client only |
| "Full mainnet in the next release" | Launch post (2026-07-21) | `v0.1.2` (2026-09-10) shipped without it. Commits on 2026-10-15 add mainnet startup guards (refuse the dev password, REST gateway kept on loopback) | **Not yet**; work in progress |

### Repository facts (checked 2026-10-15)

| Measure | Value |
| --- | --- |
| First commit | 2025-11-13 (repository created 2025-11-12) |
| Commits on `main` | 3,198; 205 since 2026-09-15 |
| Contributors | 27 by author name; most commits from Olaoluwa Osuntokun (1,580), Andras Banki-Horvath (725 across two spellings), Elle Mouton (383) |
| Releases | `v0.1.0` (2026-07-21), `v0.1.1` (2026-08-05), `v0.1.2` (2026-09-10), each after 3 to 5 release candidates |
| Issues and PRs | 108 open issues; about 1,040 PRs |
| Stars and forks | 49 and 16 |
| Tests | Unit tests across packages, property tests (`rapid`), a `systest` integration suite against SQLite and Postgres, `p-models` |
| Licence | MIT, copyright Lightning Labs 2025-2026 |

It is a serious, well-staffed codebase from the LND team, eleven months old,
at version 0.1, under heavy change. The README says "RPC surfaces, on-disk
schema, and CLI commands may still change across minor versions."

## 5. Does it run on mainnet today

**Only by invitation, and the operator gates it.**

- The client supports mainnet: `--network=mainnet` is the default and is
  refused without `--allow-mainnet`. Mainnet Ark and swap gRPC endpoints are
  compiled in (`wavelength.lightning.finance:443`,
  `swap.wavelength.lightning.finance:443`). The mainnet REST names are "dark"
  until an infrastructure change lands (`waved/config.go`).
- The launch post (2026-07-21) says mainnet access is by invite during a
  closed alpha. One secondary outlet says mainnet is live by invite; the
  primary source says full availability comes in the next release. No
  release since has announced it.
- Signet, testnet3 and testnet4 have public operator and swap endpoints
  (`docs/signet.md`). That is where we can test without asking anyone.

## 6. Fit with our payment surfaces and the LND phases

| Surface | Today | Could Wavelength serve it? | Verdict |
| --- | --- | --- | --- |
| Central receiver (`crates/wallet` on `oa-pay-1`, x402 `payTo`) | LDK on MDK LSPS4 | In principle a `waved` could issue our invoices and receive through `swapd` at about 1 bp instead of MDK's ~2%. But our receipts would sit as VTXOs under Lightning Labs' operator, need online refresh, and depend on an invite-only alpha. Each receive also needs our process online to claim | **No.** P1 and P2 exist to own the receiver and its liquidity. Wavelength trades one hosted dependency for another |
| Agent credit top-ups (#11630) | Through the receiver | Same as above | **No change.** Stays P1 on LND |
| Pro over Lightning | MDK checkout, our invoice | Same | **No change** |
| Splits and payouts, affiliate (#11727), Pylon | `pay-ledger` pays Lightning addresses and Spark | A payee with a Wavelength wallet can be paid only with a fresh invoice per payout: there is no Lightning address or LNURL | **Not a payout rail for us.** Wavelength payees can join later if they get a Lightning address; nothing to build now |
| Per-machine `openagents x402 node` | LDK, Esplora, optional LSP | **Yes, a real candidate.** No channels, no inbound liquidity, MCP tools, macaroons, a seven-verb API, embeddable as WASM or mobile | **P4 candidate**, after a spike |
| Agent wallets (self-registration, "agents own their money") | LDK or Spark | Same as above; the MCP split (agents can spend, can't create or unlock) matches our approval model | **P4 candidate** |
| The person's Wallet (Spark via Breez) | Spark | Another operator-based design with similar trade-offs. Switching has no clear gain | **Keep Spark** |
| Taproot Assets stablecoins (P3, `tapd`; `tap-ldk` research) | Not live | Asset VTXOs are in development. If they ship, they offer stablecoins to agent wallets without asset channels | **Watch.** Server side stays `tapd` in P3 |
| x402, L402, MPP buyers | `lnget` fixture, x402 node | A Wavelength wallet pays any BOLT-11, so it can pay our `402` challenges | Covered by the spike |
| Campaign treasury | MDK container | Not relevant | **Untouched.** Never read, print or run a node on the treasury mnemonic |
| LayerTree trigger "Lightning Labs operator software" | Tracking only | Wavelength is client software for an Ark operator that needs no soft fork. The operator is not public | **Does not trip the LayerTree trigger** |

### Effect on the phases

- **P0 to P3: no change.** Wavelength needs no LND node of ours, and our LND
  node does not need Wavelength. A Wavelength user paying our LND invoice is
  ordinary Lightning traffic.
- **P4: one candidate added.** The plan keeps embedded nodes on LDK because "a
  Go daemon can't be embedded". Wavelength embeds a Go daemon in-process,
  and as WASM and gomobile. That weakens the argument, though it adds a
  third-party operator our LDK nodes don't have. The spike measures this.
- **No new owner decision for the LND plan.** One for later: whether agent
  wallets may depend on an operator run by Lightning Labs.

## 7. Pros and cons

| Pros | Cons |
| --- | --- |
| Self-custodial for VTXOs, enforced in script, with fraud watching and unilateral exit | The operator and swap server are closed services run by one company |
| No channels, inbound liquidity or LSP for the user | Liveness: VTXOs must be refreshed before expiry; offline wallets need care |
| Agent-ready: MCP tools, macaroons, a small typed API, keys kept out of the model | Mainnet is invite-only alpha; schema and RPCs may change between minor versions |
| Embeddable (Go, WASM, gomobile), or a separate daemon over gRPC and REST | Go only; we would drive it over gRPC from Rust like LND, or embed WASM in the web app |
| Built by the LND team; serious test and recovery engineering; MIT | Sub-floor amounts become custodial credits in Lightning Labs' ledger |
| About 1 bp alpha fee, against MDK's ~2% | Unilateral exit is uneconomic for micro-balances |
| A route to Taproot Assets stablecoins for agent wallets without asset channels | No Lightning address or LNURL; can't be a static payout destination |
| Bitcoin-only | Default chain data comes from Lightning Labs' Esplora unless we set our own |

## 8. Risks

| Risk | Note |
| --- | --- |
| **Operator dependence** | One operator per wallet. If Lightning Labs stops, slows or censors service, users must exit on-chain. Fees and policy are the operator's |
| **Alpha software** | Three releases in three months, with migrations; `v0.1.2` fixed fund-visibility and recovery bugs. Expect more |
| **Exit economics** | A balance worth less than its exit fee is protected only in theory. Most agent balances are small |
| **Liveness** | An agent machine that sleeps through its refresh window falls back to an exit it may not be able to fund |
| **Custody and regulation** | Our own custody is unchanged if we only use it client-side. If we held agents' Wavelength wallets for them, we would be the custodian. Treat that like #11761 |
| **Invite gating** | Mainnet use waits on Lightning Labs; plans can't assume a date |
| **Treasury mnemonic** | Nothing here uses it. No experiment may import it into a Wavelength wallet |

## 9. Recommendation

**Adopt later.** Reasons:

1. **It solves a different problem from the LND plan.** The LND plan makes
   us the operator of our own Lightning node and liquidity. Wavelength makes
   its users clients of Lightning Labs' Ark operator. The server side should
   stay on LND (P0 to P3).
2. **It is the best candidate yet for the agent-side wallet** (P4). It needs
   no channels or LSP, it has MCP tools, and it can be embedded. That is
   worth measuring now, on signet, at no cost.
3. **It is not ready for real money.** Mainnet is invite-only alpha, the
   operator is closed, and stablecoins are not live.

**Now:** one signet spike,
[#11770](https://github.com/OpenAgentsInc/openagents/issues/11770) (P4 candidate), experimental and bounded:
run `waved` on signet, drive it from Rust over gRPC, pay a test x402 `402`
and an L402-style invoice, receive Lightning, force an exit, and measure
latency, fees, refresh behaviour and restart recovery against the LDK x402
node. No mainnet funds; no production change.

**Reassess when any of these happens:**

- Wavelength mainnet reaches general availability (no invite).
- Lightning Labs publishes operator terms (fees, limits, round schedule,
  service commitment), or opens the operator.
- Taproot Assets stablecoin send and receive ship in the wallet verbs.
- Lightning address or LNURL receive arrives, which would make Wavelength
  payees reachable by our payout worker.

## Sources

- Repository: [lightninglabs/wavelength](https://github.com/lightninglabs/wavelength)
  at `85cfb979`: `README.md`, `ARCHITECTURE.md`, `INSTALL.md`,
  `docs/swap_system.md`, `docs/credit_system.md`, `docs/signet.md`,
  `docs/scheduled_batches.md`, `docs/wavewalletdk_integration.md`,
  `waved/config.go`, `go.mod`, `tapassets/AGENTS.md`,
  `sdk/wavewalletdk/mobile/AGENTS.md`,
  [release `v0.1.2` notes](https://github.com/lightninglabs/wavelength/releases/tag/v0.1.2)
- [Announcing Wavelength: Bitcoin on Easy Mode for Agents and Humans](https://lightning.engineering/posts/2026-07-21-wavelength-launch/)
  (Lightning Labs, 2026-07-21)
- [Wavelength website](https://wavelength.lightning.engineering/)
- [Bitcoin Magazine: Machines Can Now Pay Machines](https://bitcoinmagazine.com/technical/lightning-labs-launches-wavelength-bitcoin-on-easy-mode-for-developers-and-autonomous-agents)
  (2026-07-22)
- Ours: [the LND plan](2026-10-15-lnd-sdk-plan.md),
  [agent payments](agent-payments.md),
  [central receive and splits](2026-10-02-central-receive-and-splits.md),
  [LayerTree integration plan](../research/layertree/integration-plan.md),
  epic #11762 and #11751 to #11761, #11770, #11630, #11727
