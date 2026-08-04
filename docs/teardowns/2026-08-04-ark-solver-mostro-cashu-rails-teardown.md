# Market rails teardown — Arkade, solvers, Mostro, Cashu, WDK

- Date: 2026-08-04
- Lane: Fast Follow research / protocol teardown
- Disposition: rail and profile donors for the Liquidity Market's next
  slices. One direct design counterpart to NIP-MKT discovery (the Arkade
  solver registry) is analyzed head-on.
- Primary local sources: the new `~/work/projects/arkade/` (19 arkade-os
  repos), `~/work/projects/mostro/` (7 MostroP2P repos), and
  `~/work/projects/cashu/` (nuts + cdk) lanes, plus the existing
  `projects/ark/` (ark-bitcoin "bark") and `projects/tether/` (`wdk`) lanes
- Spot pins: `arkd` `8b34e352` (MIT), `solver` `914079b` (no license),
  `solver-registry` `e21bd63` (no license), `skill` `ef366da` (no license),
  `arkade-unilateral-exit` `d9c949d` (MIT), `mostro` `94e736a` (MIT),
  `nuts` `3bc8b6d` (MIT), `cdk` `26d68d94` (Apache-2.0), `wdk` `61c6b2e`
  (Apache-2.0), `bark` `815faff3` (no license file)
- Companions:
  [Boltz](./2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md),
  [tbDEX](./2026-08-04-tbdex-liquidity-protocol-teardown.md),
  [Satora/LendaSwap outage](./2026-08-04-satora-lendaswap-outage-teardown.md)

Read-only review. No provider contacted, no funds moved. Repos without a
license file are ideas-only reference; no code is copied from them.

## Summary

This review closes the reference gaps the Satora teardown identified and
maps four future rails onto the NIP-MKT program:

1. **Arkade OS** (`arkade-os`) is the Ark implementation the Satora stack
   speaks to: the `arkd` operator server, five language SDKs, and — beyond
   what the satora lane showed — a production form of Striker's intent
   design: `solver` (market-maker daemon filling covenant-enforced swap
   offers from the arkd stream) and `solver-registry` (a git-based market
   discovery protocol). Arkade's Lightning ramps run through **Boltz
   submarine swaps** (`skill` README), making Arkade a third surface
   exposed to the Boltz outage class. [source]
2. **The Arkade Market Discovery Protocol v0** is the most direct external
   counterpart to NIP-MKT discovery yet reviewed, and it made the opposite
   transport choice: a git repo of JSON solver cards reduced by CI into
   per-network indexes, token-list style, with **no keys, signatures,
   relays, or messages in v0** — and a signed live-quote layer explicitly
   specced as "v1, dormant." Their execution path needs no interactivity
   because the covenant enforces terms and any solver can fill. [source]
3. **Mostro** is the production Lightning/Nostr P2P exchange (MIT, Rust):
   order lifecycle, bonds, range orders, hold invoices, disputes with
   admin/solver roles, key management — the semantics donor for MKT-P2P.
   [source]
4. **Cashu NUTs + cdk** carry mint quote/melt semantics for MKT-MINT, and
   **Tether WDK** defines the swidge-provider interface that is the
   cheapest wallet-distribution channel for the Immortal network. [source]

## 1. Two Ark implementations, kept deliberately separate

| | `arkade-os` (new lane) | `ark-bitcoin` / Second "bark" (existing `projects/ark/`) |
| --- | --- | --- |
| Server | `arkd` (Go, alpha, operator with strict fund-control boundaries, NBXplorer-based wallet) | `bark` server/client stack |
| Position | "Open execution engine for Bitcoin": Arkade Script compiler, emulator co-signer, intents/solvers | Wallet-first Ark implementation with BTCPay and FFI surfaces |
| Market surface | `solver`, `solver-registry`, `skill` (agent swaps), Satora integration | None comparable found at this review depth |
| Exit tooling | `arkade-unilateral-exit` (MIT): keyless static web executor for pre-signed exit packages, needs only an Esplora endpoint | Client-embedded |

The protocol is young and the implementations diverge; both lanes stay.
For the Liquidity Market, `arkade-os` matters first because Satora's leg,
the solver market, and the agent skill all live there. [source]

The `arkade-unilateral-exit` pattern deserves its own sentence: a user
holds a **pre-signed exit package** and can drive every transaction
on-chain from a static, keyless web page with no operator and no
infrastructure. That is the doomsday drill as a shipped product surface —
the same law the Satora teardown extracted, implemented a second,
independent time. [source]

## 2. The Arkade discovery spec versus NIP-MKT discovery

The solver-registry spec answers the same question as our `39600/39601`
heads — "which markets exist and at what terms?" — with a different trust
anchor: a reviewed git repo instead of signed events on relays.

| Dimension | Arkade registry v0 | NIP-MKT |
| --- | --- | --- |
| Publication | JSON card per solver, merged by PR, reduced by CI into one index per network | Signed addressable events per provider on relays |
| Authentication | The PR review; signatures optional until v1 | Every record signed at birth |
| Freshness | CI `generated_at`; no hand timestamps | `published_at` tags plus NIP-40 expiry |
| Liveness/quotes | Out of scope in v0; "v1, dormant" adds signed quote events | First-class: private RFQ → signed expiring Quote |
| Gatekeeping | "A registry, not the registry": forkable curation, clients follow a set and can pin cards directly | No universal provider list; clients choose relays and allowlist authorities |
| Execution coupling | None needed — covenant enforces terms, any watcher fills | Profile-owned; reservation classes with rail proofs |

Three judgments:

- **Their v1 converges toward our v0.** The spec itself says keys enter at
  the live-quote layer. NIP-MKT starts where they are heading, and their
  v0 shows what can be deferred when the rail (covenant enforcement)
  carries the trust that our signed records carry on rails that lack
  covenants. [source] [inferred]
- **The intent-market shape is real and NIP-MKT base does not cover it.**
  A maker-funded standing offer that *anyone* can fill is not an
  RFQ/Quote/Order negotiation; it is closer to NIP-69 public orders with
  enforcement. Whether this becomes an MKT-INTENT profile, folds into
  MKT-P2P, or stays rail-native on Arkade is a design decision for the
  profile drafts — recorded, not decided here. [proposal]
- **Their field vocabulary is directly harvestable** for our Offering and
  profile drafts: market identity is the **asset-id pair, never tickers**
  ("anyone can call an asset USDT"); amounts are **canonical decimal
  strings** because JSON numbers round past 2^53; `max = "0"` disables a
  side; `fee_bps` is a fill promise, not a fact; and the **price-feed
  pinning law** — the maker MUST price from the exact URL the solver
  validates against at fill time, with an RFC 6901 pointer schema, never a
  substitute feed. That last rule is a concrete instance of our oracle
  threat-table row. [source]

## 3. Mostro — the MKT-P2P donor

Production Rust daemon, MIT, actively maintained (rewards board, coverage
tracking). The protocol documentation enumerates the full action set:
buy/sell orders including **range orders**, bond invoices and
**bond-slashing**, fiat-sent/released steps, cancellation and cooperative
cancellation, disputes with taken/settled outcomes, **admin solver roles**
(`admin_add_solver`, admin settle/cancel), key management with per-trade
key rotation, and last-trade-index continuity for client recovery. [source]

MKT-P2P harvest when its draft starts: map Mostro's order lifecycle and
NIP-69 compatibility onto the negotiated spine without inventing a second
P2P network (the tbDEX teardown's build-order item 6); adopt bond and
dispute vocabulary as profile fields; respect their key-rotation privacy
practice in the identity section. The admin/solver dispute role maps onto
our arbiter disclosure law (disclose solver set and appeal path before
Order). [proposal]

## 4. Cashu — the MKT-MINT donor

`nuts` (MIT) is the spec authority: mint/melt quotes, keysets, proofs,
spending conditions. `cdk` (Apache-2.0) is the Rust implementation with a
mint daemon (`cdk-mintd`) usable in the adversarial lab later. MKT-MINT
stays a thin negotiation layer: NIP-87 owns discovery, the NUTs own quote
and redemption semantics, and the profile owns only the negotiated terms
and custody disclosure (mint custody class A3). No new mint protocol is
invented. [proposal]

## 5. WDK — the distribution interface

`@tetherto/wdk` (Apache-2.0, beta) is a manager that registers per-chain
wallet modules and protocol modules behind one interface; Satora shipped
`SatoraProtocol` as a WDK "swidge" provider, which is how WDK wallets got
BTC↔stablecoin swaps without bespoke integration. The harvest is the
interface position, not the code: an OpenAgents swidge provider on the
generated NIP-MKT SDK puts the whole multi-provider network behind the
same plug every WDK wallet already accepts. The satora lane's Apache
`wdk-protocol-bridge-satora-bitcoin` is the worked example of the
provider shape. [source] [proposal]

## 6. Consolidated harvest routing

| Finding | Routed to |
| --- | --- |
| Asset-id pair identity, decimal-string amounts, side-disable, fee-as-promise, price-feed pinning | MKT-SWP/PFI draft vocabulary (openagents#9311) |
| Intent-market shape (maker-funded, any-filler, covenant-enforced) | Profile-draft design decision: MKT-INTENT vs MKT-P2P fold (openagents#9311 note; extension profiles issue) |
| Covenant-enforced reserves as `hard`-reservation proof | Already routed to immortal#13 by the Satora review; the solver stack is the working example |
| Pre-signed exit packages (`arkade-unilateral-exit`) | Doomsday drill implementation shape for the client engine (immortal#12) and lab (immortal#18) |
| Ark rail leg (arkd VTXO swaps, both implementations) | New Ark-rail extension issue (immortal M13) |
| Mostro lifecycle/bonds/disputes | MKT-P2P draft + adoption issues (M13) |
| NUTs/cdk quote-melt semantics; `cdk-mintd` in the lab | MKT-MINT draft + adoption issues (M13) |
| WDK swidge provider interface | Distribution-surfaces issue (openagents) |
| Arkade agent `skill` (agents swapping over Arkade/Lightning/stablecoins) | Evidence for the agent-markets thesis; the OpenAgents SDK should expose the same capability against the open network instead of one operator |
| Boltz dependency inside Arkade Lightning ramps | Recorded: third Boltz-exposed surface; strengthens the M12 replacement case |

## 7. What we do not take

- **Git-repo discovery as the primary market wire.** The registry pattern
  is honest about being curation, and its own roadmap adds keys and
  events. Our discovery stays signed events on relays; a forkable curated
  overlay (NIP-51 lists) already covers the token-list use.
- **CI/GitHub as protocol infrastructure.** Their reducer runs on GitHub
  Actions; Immortal's rules prohibit GitHub-billed automation as protocol
  machinery, and a market index that requires a specific forge is a
  gatekeeper in disguise.
- **Unlicensed repos as code donors** (`solver`, `solver-registry`,
  `skill`, `ts-sdk`, `protocol`, `bark`): laws and vocabulary only.
- **Operator-priced trust.** `arkd` is alpha with an operator role;
  nothing here changes the custody gradient — Ark routes disclose the
  operator/exit model like every other rail.

---

*End of teardown. Research and candidate protocol only; no market,
provider, financial authority, or deployment is created by this document.*
