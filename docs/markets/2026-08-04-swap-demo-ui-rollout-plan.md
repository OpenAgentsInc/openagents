# Swap demo UI rollout — one GPUI component set, two surfaces

- Date: 2026-08-04
- Status: owner-directed product-rollout plan for the Liquidity Market's
  first visible surfaces
- Owning issues: omega#244 (Omega market panel), openagents#9310 (web
  market demo), openagents#9309 (generated TypeScript SDK — unchanged,
  see §8), immortal M12 ledger (protocol engine)
- Protocol base: [`docs/nips/MKT.md`](../nips/MKT.md) v0.1, implemented
  base on the Immortal relay

## The thesis

Build the swap UI **once**, in Rust, as GPUI components extending Omega's
existing `ui` crate — then ship it on two surfaces from the same source:

1. **Omega desktop** — a market panel in the zero-base shell: the real
   wallet-grade product surface where keys live and funds move.
2. **The web** — a GPUI/WebGPU wasm document on `openagents.com`, built
   and served exactly the way the Diamond Hands `/dh` document already
   was: the browser owns a WebSocket straight to
   `wss://relay.openagents.com`, the Immortal client crate does the
   protocol work in wasm, and there is **no server-side swap API or relay
   proxy**. The web demo is protocol-real, not a video or a mock.

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

The split follows custody, not features. **The web demo shows the market;
Omega operates in it.**

| Concern | Web demo document | Omega desktop |
| --- | --- | --- |
| Discovery: providers, offerings, custody dimensions | Yes — live from the relay | Yes — same components |
| Quote comparison, reservation classes, rung labels | Yes — live demo session | Yes |
| Session timeline (Status seq, gaps, forks, Close) | Yes — watching a scripted/no-spend session | Yes — own sessions |
| Issue RFQs / accept Quotes | Demo mode only: ephemeral in-browser key, no-spend seeded provider, amounts labeled DEMO | Yes — real sessions |
| Hold keys, fund swaps, claim/refund | **Never.** No wallet, no funds, no custody in the browser demo | Yes — wallet-grade, verify-before-fund enforced by the engine |
| Exit packages / doomsday drill | Rendered as evidence (badge + explainer) | Produced, persisted, and drillable |
| Provider operator view (inventory, reservations) | No | Later phase, same component set |
| Purpose | Public trust surface: *watch the open market work, verify everything yourself* | The product: *participate* |

This split also keeps every promise the protocol makes: relay acceptance
is transport only, a browser page never becomes a wallet, and nothing on
the web surface can overclaim settlement because it never touches funds.

## 3. The shared component crate: `omega/crates/market_ui`

New crate, GPUI-only, no networking — it renders typed NIP-MKT session
state produced by the Immortal client crate. It must build for native and
`wasm32` (chat_web proves the pattern). Components extend existing `ui`
primitives; the component gaps below are the "new components extending
our other Omega gpui components":

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

Rules carried from the protocol into every component: tickers are labels
and never identity; amounts display from decimal-string atomic units;
unknown states do not advance a timeline; a `completed` Status renders as
a claim until evidence upgrades its rung.

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

**Boundary with the Effect Native mandate:** the openagents.com product
surfaces remain Effect Native/TypeScript; this wasm document is a bounded
demo artifact in the `/dh` lineage — a static, gated, self-contained
page — not a change to the web app's architecture. The Effect-native
product integration path stays #9309 (generated SDK), which later powers
any retained product-route market surface.

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

P0/P1 are pure UI + existing infrastructure and can ship this week; the
demo goes online at P1 (watch-only) and gets interactive at P2.

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

## 9. What this plan does not do

No funds, keys, or custody in any browser surface; no server-side swap
API; no provider operator UI on the web; no mainnet path before the M12
lab passes; no claim that a demo session is settlement. The demo's
honesty is the marketing.
