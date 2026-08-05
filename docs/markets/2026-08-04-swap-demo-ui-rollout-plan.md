# Swap demo UI rollout — one engine, two rendered surfaces

- Date: 2026-08-04
- Amended: 2026-08-04 for the Boltz web-app parity directive
  (openagents#9314). The amendment changes the custody clause, splits
  the shared source into *engine* and *components*, expands the
  component set, and adds the parity phases. Everything else stands.
- Status: owner-directed product-rollout plan for the Liquidity Market's
  first visible surfaces
- Owning issues: omega#244 (Omega market panel), openagents#9310 (web
  market demo), openagents#9314 (Boltz web-app parity), openagents#9309
  (generated TypeScript SDK — unchanged, see §8), immortal M12 ledger
  (protocol engine)
- Protocol base: [`docs/nips/MKT.md`](../nips/MKT.md) v0.1, implemented
  base on the Immortal relay
- Profile: [`docs/nips/MKT-SWP.md`](../nips/MKT-SWP.md) v1
- Parity evidence:
  [Boltz web-app UX parity teardown](../teardowns/2026-08-04-boltz-web-app-ux-parity-teardown.md)

## The thesis

Build the swap experience **once** and ship it on two surfaces:

1. **Omega desktop** — a market panel in the zero-base shell: the
   wallet-grade product surface, rendered with GPUI components extending
   Omega's existing `ui` crate.
2. **The web** — the user-facing swap product on `openagents.com`: the
   browser owns a WebSocket straight to `wss://relay.openagents.com`,
   the Immortal client crate does the profile and rail work in wasm, and
   there is **no server-side swap API or relay proxy**.

The shared source is the **engine and its exported typed session
view-model**, not one widget toolkit. Both surfaces render the same
contract; behaviour contracts hold both to the same laws. See §2.1 for
why the original "one GPUI component set, two surfaces" thesis narrowed.

The `/demo` document keeps its current job unchanged: a scripted,
protocol-real walkthrough in the `/dh` lineage, gated as it is today.

Every piece of infrastructure this needs already exists and was proven
this week. The plan below is assembly, not invention.

## 1. What already exists (verified 2026-08-04)

| Asset | Where | State |
| --- | --- | --- |
| GPUI on web (WebGPU/wasm) | `omega/crates/gpui_web` + `gpui_wgpu` | Supported target; omega#243 closed (settings-for-wasm, embedded assets) |
| Real-design-system wasm example | `omega/crates/gpui_web/examples/chat_web` (commit `6ec37d164f`, 2026-08-04) | Renders an Omega-shaped chat surface — thread rail, transcript with typed activity rows, disclosure header, composer — with real `ui::{Label, Button, Chip, Indicator}` and the parsed Aiur theme |
| Web build/serve pipeline | `apps/openagents.com/apps/diamond-hands/` | Standalone Rust crate → Trunk/Zig build pinned to exact Omega + Immortal commits → static HTML/JS/wasm staged into `../start/public/dh/` → served by the Cloud Run monolith behind an env gate (`OPENAGENTS_DIAMOND_HANDS_ENABLED`); browser-proof script included |
| Wasm-proven relay client | Immortal crate, non-`server` feature | NIP-11 pinned identity check, subscription, event verification, EOSE snapshot/live fold, reconnect; proven on `wasm32-unknown-unknown` |
| NIP-MKT base on the relay | Immortal M10 (complete) | Discovery heads `39600-39603` validated and served; wrapped private transport gated; NIP-11 advertises `nip-mkt`; deterministic contract export |
| Local market environment | Immortal #9 (complete) | `dev-relay.sh` + `dev-market-seed.sh`: seeded provider/requester actors driving a full RFQ→Quote→Order→Status→Close session no-spend |
| Client swap engine | Immortal #12 (fleet, in progress) | Submarine/reverse/chain flows, verify-before-fund, recovery — the engine both surfaces embed |
| Omega component inventory | `omega/crates/ui/src/components/` | ~60 components incl. `data_table`, `redistributable_columns`, `tab_bar`, `modal`, `progress`, `indicator`, `chip`, `banner`, `callout`, `disclosure`, `list`, `popover`, `tooltip`, `toggle`, `count_badge`, `avatar` |
| Product shell | `omega/crates/omega_zero_base`, `agent_ui` | The zero-base sidebar shell (channels, planning/work currently hidden) the market panel mounts into |

## 2. Web versus Omega — the responsibility split

**Amended 2026-08-04 (openagents#9314).** The split no longer follows
custody, because Boltz parity makes the web a user-facing swap product.
It follows **operator role**: the web is the requester's product, Omega
is the requester's product *plus* the provider's operating surface.

| Concern | Web swap surface | Omega desktop |
| --- | --- | --- |
| Discovery: providers, offerings, custody dimensions | Yes — live from the relay | Yes |
| Quote comparison, reservation classes, rung labels | Yes | Yes |
| Session timeline (Status seq, gaps, forks, Close) | Yes — own sessions | Yes — own sessions |
| Issue RFQs / accept Quotes | Yes — real sessions | Yes — real sessions |
| Hold keys, fund swaps, claim/refund | **Yes — self-custodially.** Keys and preimages are generated in the browser and never leave the device | Yes — wallet-grade, with Omega's identity and signer plumbing |
| Verify-before-fund | Enforced by the engine; funding disabled until every check passes | Same engine, same enforcement |
| Exit packages / doomsday drill | Built and persisted **before** every funding broadcast; the Rescue surface is the drill made usable | Produced, persisted, and drillable |
| Provider operator view (inventory, reservations, session queue) | **No** | Later phase, GPUI components |
| Custody environment | Weaker by construction — an origin compromise reaches key material. Stated in the product, with the desktop path visible | Stronger — OS-level storage, no browser origin exposure |
| Purpose | The product for anyone with a browser: *swap, and verify everything yourself* | The product for operators and for larger amounts |
| `/demo` | Unchanged: scripted protocol walkthrough, no keys, no funds, DEMO labels | — |

### 2.1 The custody boundary change, recorded

Until 2026-08-04 this plan said the web surface never holds keys. The
Boltz parity directive requires the opposite, and **it is still
non-custodial**: the browser generates and holds the refund/claim keys
and preimages that make unilateral recovery possible. That is precisely
what makes a Rescue page meaningful. Parity means *self-custodial
in-browser key material*, never *server-held funds*.

The invariants that replace the old blanket clause:

- Key material and preimages are generated in the browser, are never
  transmitted to a relay, to `openagents.com`, or to any provider, and
  never appear in a signed record, a receipt, a log, or a fixture — the
  forbidden-material list in [`MKT-SWP` §14](../nips/MKT-SWP.md) is
  binding on this surface.
- The relay and `openagents.com` never receive spend authority. There is
  no server-side swap API and no relay proxy.
- No page claims settlement its evidence does not prove. A `completed`
  Status renders as one signer's claim until an admitted verifier raises
  the rung.
- The exit package is built, persisted, and digest-checked against the
  Swap Contract pair **before** funding is offered, for every direction.
- The web surface loads no third-party script and states honestly that a
  browser is a weaker custody environment than the desktop app.

### 2.2 Why the shared source narrowed from components to the engine

The original thesis was one GPUI component set on both surfaces. A
GPUI/WebGPU canvas is right for a demo and wrong for the page a user
moves money on: no DOM means no screen reader, no text selection, no
browser translation, no indexable content, and no rendering where WebGPU
is unavailable. The repository's product-UI mandate was Effect Native at
the time this was written. Both could not hold.

**Amended 2026-08-05 (#9325).** The tension resolved by removing one side
of it: the Effect Native mandate was withdrawn and the framework deleted.
The web swap host is now **plain React on TanStack Start** — same DOM
conclusion, reached for the same accessibility reason, without the second
component vocabulary. The narrowing from components to the engine below is
unaffected; it was always the load-bearing part.

What must not give is implementing verify-before-fund twice. The profile
and rail logic — script and tree parsing, output-key re-derivation,
invoice checks, MuSig2 transcript checks, timeout ladders, exit packages,
the typestate fund-authorisation flow — exists once, in the Immortal
client crate, and builds for `wasm32`. `@openagentsinc/nip-mkt` covers
the NIP-MKT **base** in Effect/TypeScript but is deliberately opaque to
profile content.

So: **everything that can authorise funding stays behind one engine
boundary**, exposed to the web host through an Effect Schema contract;
the host owns rendering, storage, entropy, and relay transport; and the
artifact shared with Omega is the exported typed session view-model. The
rejected alternative — promoting the wasm document to the product
surface — is recorded with its costs in the parity teardown §10. Route
naming and the engine-binding shape remain owner calls, taken on the
SWAP-0 issue.

## 3. The component set

Two implementations, one contract. `omega/crates/market_ui` is the GPUI
set (Omega desktop, plus the `/demo` document); the web set is authored
in plain React against the same exported session view-model (amended
2026-08-05, #9325 — it read "in Effect Native"). Neither
does networking — both render typed NIP-MKT/MKT-SWP session state
produced by the Immortal client crate. The GPUI crate must build for
native and `wasm32` (chat_web proves the pattern).

### 3.1 Market components (both surfaces)

Components extend existing `ui` primitives; the component gaps below are
the "new components extending our other Omega gpui components":

| New component | Extends | Renders |
| --- | --- | --- |
| `OfferingCard` | `chip`, `label`, `indicator`, `disclosure` | One `39601` Offering: pair (asset-ID identity, ticker as label only), direction, bounds, profile+version, provider ref, freshness |
| `ProviderBadge` | `avatar`, `count_badge`, `tooltip` | Provider Profile head: status, supported profiles, NIP-32/85 assertions shown as *claims with named asserters* |
| `QuoteCompareTable` | `data_table`, `redistributable_columns` | Competing Quotes: price terms, fees (as promises), expiry countdown, reservation class, custody dimensions — sortable, best-execution first |
| `ReservationBadge` | `indicator`, `tooltip` | `none` / `soft` / `hard` with proof class (signed claim vs covenant-enforced reserve) |
| `CustodyStrip` | `disclosure`, `callout` | The six custody dimensions (`funds_control` … `credential_exposure`) as a compact strip that expands; never a single score |
| `RungLabel` | `chip` | Provenance labels `pledged → reserved → measured → verified → paid → settled`; renders the narrowest rung the evidence proves, never inferred upward |
| `SessionTimeline` | `list`, `progress` + chat_web's typed activity-row pattern | Per-signer Status sequences as rows; **sequence gaps render as visible gaps, equivocation forks render as forks** — never silently resolved |
| `VerifyChecklist` | `list`, `indicator`, `banner` | The verify-before-fund law as UI: lock script/tree, amounts, payment hash, timelocks, claim/refund paths — each an explicit check with pass/fail, funding disabled until all pass |
| `ExitPackageBadge` | `chip`, `popover` | Whether a pre-signed unilateral-exit/recovery artifact exists for the session (the doomsday drill made visible) |
| `ExpiryCountdown` | `label`, `indicator` | NIP-40 expiry with client-side enforcement state |
| `ReceiptCard` | `callout`, `chip` | A `39603` Public Market Receipt: outcome, role, redaction notice, "one signer's claim" framing |
| `SwapFlow` | `modal`, `tab_bar`, `toggle` | The guided requester flow: offering → RFQ → quotes → order → timeline → close |

### 3.2 Parity components (added 2026-08-04 for openagents#9314)

These come from the
[Boltz web-app UX parity teardown](../teardowns/2026-08-04-boltz-web-app-ux-parity-teardown.md).
They are the difference between a protocol walkthrough and a product a
user can swap on.

| New component | Extends | Renders |
| --- | --- | --- |
| `SwapWidget` | `SwapFlow`, `modal`, `toggle` | The whole product on one card: send/receive selection, amounts, destination, fee disclosure, one primary action |
| `AssetSideSelector` | `chip`, `list`, `popover` | Per-side asset choice folded from live `39601` Offerings; unreachable directions greyed **before** selection with the reason, plus a direction toggle |
| `AmountField` | `label`, `indicator` | Both-sides-editable atomic-unit entry with authoritative-side tracking, a MAX bound by the Offering maximum, and a persisted BTC/sats denomination — never auto-switching units while the user types |
| `PrimaryActionButton` | `button`, `banner` | The disabled-with-a-reason law: label, colour, disabled, and content computed independently; the refusal names the single most proximate cause in the user's current denomination |
| `FeeBreakdown` | `disclosure`, `callout` | `provider_fee`, `miner_fee_budget`, `lightning_routing_fee_budget`, who pays each, the rounding rule, and `amount_equation` — framed as the fill **promise** §3.3 makes it |
| `PriceFeedProvenance` | `popover`, `chip` | When a Quote pins a feed: URL, pointer, observation, age limit, response digest, and staleness state (§3.4) |
| `DestinationField` | `label`, `indicator`, `tooltip` | Address/invoice entry with per-asset validation, paste-driven route switching, QR scan, and typed parse failures that keep their discriminant |
| `InvoiceChecks` | `VerifyChecklist`, `list` | The §7.2 local invoice parse: network, payment hash, amount, expiry, minimum final CLTV, route-hint policy, description commitment; amountless invoices refused |
| `RescueKeyCeremony` | `modal`, `list`, `indicator` | Secret-store creation and **verified** backup — re-upload or word quiz, never a checkbox — gated **before** the first funding broadcast in either direction |
| `RescuePage` | `data_table`, `VerifyChecklist`, `banner` | Recovery with every provider, handler, and relay gone: local secret store plus persisted exit packages, cooperative path first, unilateral script path as the guaranteed fallback |
| `RefundPanel` | `ExpiryCountdown`, `callout` | Refund state with the height bound and the wall-clock estimate shown **separately** (§6) — never converting an estimate into consensus authority |
| `SessionHistory` | `data_table`, `RungLabel` | Local signed-record store: actionability-first ordering, resumable in-flight sessions, and an import path for every export we emit |
| `TypedErrorMessage` | `banner`, `callout` | The §17 identifiers mapped to localisable messages — never a counterparty's prose |
| `BuildProvenance` | `label` | Which build is holding the user's keys: release tag and commit, on every page that holds key material |

### 3.3 Rules carried into every component

Tickers are labels and never identity; amounts display from
decimal-string atomic units; unknown states do not advance a timeline; a
`completed` Status renders as a claim until evidence upgrades its rung;
per-signer sequence gaps render as gaps and equivocation forks render as
forks; funding is disabled until every verify-before-fund check passes;
and no error surface ever renders an upstream string.

## 4. The web demo document: `apps/openagents.com/apps/market-demo`

Clone the diamond-hands crate pattern (its README is the runbook):

- Standalone Rust crate, pinned by exact Git commit to Omega (for
  `market_ui`, `ui`, `theme`, `gpui_web`) and Immortal (client crate).
- `build-static.sh` → Trunk/Zig → static HTML/JS/wasm staged under
  `../start/public/markets/` (or `/demo/markets/`); the ordinary Start
  build copies it into the Cloud Run bundle.
- Served by the monolith behind `OPENAGENTS_MARKET_DEMO_ENABLED` (same
  fail-closed gate shape `/dh` uses). Route naming is an owner call
  recorded at flip time; until then the gate stays off in production and
  the demo runs on dev/staging.
- The browser opens its own WebSocket to `wss://relay.openagents.com`
  (and one fallback relay when available), does the NIP-11 check, and
  renders live `39600/39601` heads plus a demo session.
- Demo interactivity: a throwaway in-browser key (generated per visit,
  clearly labeled), RFQs against the **no-spend seeded provider actor**
  (immortal #9/#14) running against the public relay's demo market;
  every amount and outcome badge carries a DEMO label; the page states
  plainly that no funds exist on this surface.
- A "verify this yourself" panel shows raw signed events with copyable
  IDs — the demo doubles as protocol documentation.

**Boundary with the product web surface** (amended 2026-08-05, #9325 — this
read "Boundary with the Effect Native mandate"): the openagents.com product
surfaces remain DOM/TypeScript — TanStack Start and plain React; this wasm document is a bounded
demo artifact in the `/dh` lineage — a static, gated, self-contained
page — not a change to the web app's architecture. Whether a GPUI canvas
may ever be an *ungated* public or money-moving surface is an OPEN owner
decision recorded in `AGENTS.md`; `gpui_web` ships no accessibility adapter
today, so the answer is not assumed here. The product integration path stays #9309 (generated SDK), which later powers
any retained product-route market surface.

### 4.1 The web swap product (added 2026-08-04 for openagents#9314)

Shipped, this crate stays what it is. The **product** surface is
separate and is plain React on TanStack Start, per §2.2 (amended 2026-08-05,
#9325 — it read "is Effect Native"):

- Plain React components render the exported session view-model.
- `@openagentsinc/nip-mkt` supplies base-envelope validation and NIP-59
  transport; the wasm engine binding supplies everything that can
  authorise funding.
- Entropy comes from WebCrypto in the host, because the engine has no
  randomness source of its own and takes key material as bytes.
- Local storage holds the user's own signed records, exit packages, and
  encrypted secret store — with schema versioning from the first commit.
- Strict content-security posture; no third-party script on a surface
  that holds keys.
- Route naming, the gate shape, and the engine-binding surface are owner
  calls taken on SWAP-0. Until then the surface runs on dev/staging.

## 5. The Omega market panel (omega#244, expanded)

- `crates/market_panel` (or extend `agent_ui`): mounts `market_ui`
  components in the zero-base shell as a **Markets** section (sibling of
  the currently hidden Planning/Work sections; it can ship behind the
  same hidden-section mechanism until owner flip).
- Embeds the Immortal client crate natively: real key management via
  Omega's existing identity plumbing (`omega_identity`,
  `omega_signer_broker`) — market keys are Nostr keys, consistent with
  the doomsday/Nostr-rendezvous law.
- Phase order inside the panel: watch (same as web) → demo swap
  (no-spend) → real regtest swap once immortal #12 lands → mainnet only
  after the M12 lab (#18) passes.
- The panel is also where provider-side surfaces eventually land
  (inventory, reservation book, session queue) using the same
  components — the web never grows an operator view.

## 6. What the online demo shows (the script)

1. **The market is alive**: provider cards and offerings streaming from
   the relay, freshness ticking, one provider deliberately `paused` to
   show status honesty.
2. **Negotiation**: visitor picks an offering, sends a demo RFQ, two
   seeded providers answer — one `firm/hard`, one `indicative/soft` — and
   the compare table makes the difference legible.
3. **The laws, visibly enforced**: accept a quote → the VerifyChecklist
   runs → the timeline advances with per-signer sequences → one scripted
   provider emits a sequence gap so viewers see a gap rendered as a gap.
4. **Terminal honesty**: Close renders as the signer's claim; the
   ReceiptCard shows the redacted public receipt; the RungLabel never
   jumps to `settled`.
5. **The punchline** (one banner): *Boltz and Satora went dark because
   one company's API was the market. This market is signed events on
   relays — any provider can join, any client can verify, and this page
   is just one window onto it.*

## 7. Phases

| Phase | Deliverable | Depends on |
| --- | --- | --- |
| P0 | `market_ui` crate + `swap_web`-style example rendering the exported fixture corpus (static data, no network); runs native and wasm | Nothing — start now |
| P1 | Live discovery: the example reads real `39600/39601` heads from `relay.openagents.com`; market-demo crate + staging serve behind the gate | P0; relay already serves MKT base |
| P2 | Interactive demo session against the seeded no-spend provider on the public relay; Omega panel mounts the same views | P1; immortal #14 actor pointed at the public relay |
| P3 | Omega real regtest swap with VerifyChecklist/ExitPackageBadge live; web demo gains the "watch a real regtest swap" replay | immortal #12, #18 |
| **P4** | **Parity shell**: engine binding behind an Effect Schema contract, the React widget shell and typed state, asset/direction selection from live Offerings, amount entry, destination entry and validation — regtest only, no mainnet gate | P1; immortal #12 (landed); SWAP-0..2 |
| **P5** | **Parity depth**: multi-provider quote compare with expiry and reservation class, verify-before-fund as a real checklist, key generation and the rescue ceremony, the Rescue page, History with import, and per-signer status rendering | P4; immortal #14; SWAP-3..6 |
| **P6** | **Product surface**: nav, routes, build provenance, i18n scaffolding, and a coordinator-absent recovery proof (the §12.1 doomsday drill run as an acceptance test) before any mainnet flip | P5; immortal #18; SWAP-7..8 |

P0/P1 are pure UI + existing infrastructure and can ship this week; the
demo goes online at P1 (watch-only) and gets interactive at P2. P4-P6 are
the parity program; no mainnet path opens before the doomsday drill
passes at P6 and the M12 lab (immortal#18) is green.

Out of parity scope: stablecoins, DEX hops, bridge legs, slippage
tolerance, and gas top-up. Today's Boltz has all of them; MKT-SWP v1
requires `evm_leg` to be absent or null and Offering `evm_extension` to
be `unsupported`, so that surface waits for a future `mkt-swp-evm`
extension rather than being built against a rail the profile refuses to
execute.

## 8. Issue routing

- **omega#244** — expands to own `market_ui` + the panel + the wasm
  example (comment posted; the panel and the web demo share one
  component source, so the crate lives in Omega).
- **openagents#9310** — repurposed by this plan: the *online demo* is the
  GPUI wasm document above; #9310's Effect-native demo scope narrows to
  the later product-route integration on the generated SDK (#9309
  unchanged). Recorded on the issue.
- **New crate/app work** lands as: `omega/crates/market_ui`,
  `omega/crates/market_panel`, `apps/openagents.com/apps/market-demo`.
- Serving-gate and route naming: owner decision at P1 flip, recorded in
  the deploy runbook like `/dh`'s.

### 8.1 The parity program (openagents#9314)

| Issue | Scope | Phase |
| --- | --- | --- |
| [#9315](https://github.com/OpenAgentsInc/openagents/issues/9315) SWAP-0 | Widget shell, typed state, engine boundary, primary-action law | P4 |
| [#9316](https://github.com/OpenAgentsInc/openagents/issues/9316) SWAP-1 | Asset/direction selection from Offerings; rate and fee panel | P4 |
| [#9317](https://github.com/OpenAgentsInc/openagents/issues/9317) SWAP-2 | Destination entry and validation | P4 |
| [#9318](https://github.com/OpenAgentsInc/openagents/issues/9318) SWAP-3 | Multi-provider quote compare, expiry, reservation, custody, verify-before-fund | P5 |
| [#9319](https://github.com/OpenAgentsInc/openagents/issues/9319) SWAP-4 | Key material, rescue ceremony, coordinator-absent Rescue page | P5 |
| [#9320](https://github.com/OpenAgentsInc/openagents/issues/9320) SWAP-5 | Session store, History, resume, export **and** import | P5 |
| [#9321](https://github.com/OpenAgentsInc/openagents/issues/9321) SWAP-6 | Per-signer Status: gaps, forks, rungs, loss accounting | P5 |
| [#9322](https://github.com/OpenAgentsInc/openagents/issues/9322) SWAP-7 | Routes, nav, build provenance, settings, surrounding surfaces | P6 |
| [#9323](https://github.com/OpenAgentsInc/openagents/issues/9323) SWAP-8 | i18n scaffolding and the typed error-message table | P6 (author early) |

## 9. What this plan does not do

**Amended 2026-08-04.** The first clause changed; the rest stands.

- **No server-held funds and no server-held keys.** Browser key material
  is self-custodial, generated on the device, and never transmitted —
  see §2.1. The old blanket "no keys in any browser surface" clause is
  superseded by that invariant, not weakened by it.
- No server-side swap API and no relay proxy.
- No provider operator UI on the web.
- No mainnet path before the M12 lab (immortal#18) passes **and** the
  §12.1 coordinator-absent recovery drill passes on the web surface.
- No claim that any session is settlement, and no rung inferred upward.
- No third-party script on a surface that holds key material.
- No stablecoin, DEX, or bridge surface under MKT-SWP v1.
- No feature copied from `boltz-web-app`. It is AGPL-3.0; the parity
  teardown carries the behaviour, and every component here is written by
  us against our own design system.

The demo's honesty is the marketing. So is the product's.
