# Can and should OpenAgents remove Effect Native entirely?

- Date: 2026-08-04
- Class: evidence audit, requested before any removal work
- Status: audit and recommendation only. This document deletes nothing,
  changes no code, and authorizes nothing by itself. Owner direction
  2026-08-04: "if we want Effect just use basic Effect, no Effect Native
  anymore — want things simpler; web UI for all the omega-adjacent stuff
  should use GPUI not Effect Native, and TanStack Start only sometimes."
- Baseline: monorepo `origin/main` at `1cc29d4318` (2026-08-04). Sibling
  repos measured: `~/work/effect-native` at `f53838e` (2026-07-16),
  `~/work/omega` at `9d388fdcd0` (2026-08-04).
- Method: every number below was measured on the baseline commit with
  `rg`/`find`/`wc`/`git log`, not estimated. Where something could not be
  measured, that is stated.

## 1. The question and the answer

**Question.** Should the repo drop the Effect Native UI framework — the
"one typed component set, thin swappable renderers" layer — keeping plain
Effect for services/logic, using GPUI for omega-adjacent surfaces, and
TanStack Start "only sometimes" for document/form-shaped web surfaces?

**Answer: yes — remove the Effect Native component layer (Option B),
executed with a desktop freeze rather than a desktop rewrite (§4,
Option C sequencing).** The evidence says the mandate is already dead in
practice and the removal is far cheaper than the AGENTS.md contract makes
it look:

1. **Mobile already removed it**, by owner direction, a week ago. Commit
   `017ae3dedb` (2026-07-27), "Rebuild the mobile app on arcade patterns,
   in plain React Native": *"Effect Native is gone from this app… no view
   is described as data any more."* `apps/openagents-mobile` has **zero**
   `@effect-native` imports today (measured §3.4).
2. **Desktop, the only heavy consumer (98 files, ~52k EN-coupled LOC), is
   already on a deprecation path.** `docs/sol/MASTER_ROADMAP.md` owner
   decision 3: *"Omega is the primary Desktop, IDE, and company workroom
   destination… The current Electron application stays supported until the
   Omega cutover gate,"* and decision 5 calls the monorepo's holdings
   "current Electron **migration** code." Rewriting its Effect Native views
   only to delete the app at cutover would be waste; freezing them is not.
3. **The upstream framework is stalled.** `OpenAgentsInc/effect-native` is
   not an empty reservation (the workspace `CLAUDE.md` description is
   stale) — it is a real repo with 143 commits, 13 packages, 394 TS files,
   84,902 LOC — whose last commit is `f53838e`, **2026-07-16**, nineteen
   days before this audit with zero activity since.
4. **The web usage is small and bounded**: ~31 files / 12,401 LOC in
   `apps/openagents.com/apps/start`, concentrated in roughly ten page
   modules out of 72 route files. Everything else in the web app is
   ordinary React on TanStack Start already.
5. **New work already ignores it.** The SWAP parity packets landing this
   week (`packages/mkt-swp-destination`, `packages/swap-i18n`) depend on
   `effect` only — no `@effect-native` anywhere.

What must NOT be removed, because it is a different thing wearing a
similar name: **plain Effect** (`effect` 4.0.0-beta.94 via catalog) is the
services/logic substrate of the whole TypeScript estate — 286 importing
files in desktop, 191 in pylon, 25 in start, 11 in mobile — and
`packages/effect-start` (419 LOC) and `packages/effect-boundary` (167 LOC)
depend only on `effect`, not on Effect Native. The owner is explicitly
keeping this layer, and nothing in this audit touches it.

The one decision the owner must make explicitly, not by drift, is the
accessibility/SEO posture of GPUI-on-web surfaces (§6): `gpui_web` has no
accessibility adapter at all, and "web UI for all the omega-adjacent stuff
should use GPUI" collides with the SWAP-0 record that chose DOM for the
page a user moves money on.

## 2. What exists, measured

### 2.1 Three things called "Effect Native" — kept rigorously apart

**(a) The framework thesis.** `docs/effect-native/` — 13 documents,
4,571 lines total (`wc -l docs/effect-native/*.md`): the framing doc, the
EN-0…EN-9 conversion roadmap
(`2026-07-08-effect-native-one-ui-substrate-analysis.md`, 435 lines), the
Foldkit/React-Native/three-effect/styling comparisons, the SwiftUI
renderer audit, the web-absorption burndown, the React-web harmonization
gap analysis (773 lines, 2026-07-14 — the newest design doc), and the
living `DEMAND_REGISTER.md`. The binding repo contract is
`AGENTS.md:1016-1037`: *"the entire repo converts to Effect Native, ASAP…
React/TanStack Start and React Native are renderer adapters and serving
hosts only — never the architecture."* The framework's public home is
`OpenAgentsInc/effect-native`.

Correction to the task framing and to the workspace `CLAUDE.md` (which
says the repo is "LICENSE only as of 2026-07-08; do not change it yet"):
the sibling checkout `~/work/effect-native` is a **full framework repo**
— 143 commits, 13 packages (`core`, `tokens`, `render-dom`, `render-rn`,
`render-canvas`, `khala-ui`, `gallery`, `devtools`, `platform-desktop`,
`platform-electron`, `platform-mobile`, `site`, `testkit`), 394 TS files,
84,902 LOC, plus `GAPS.md`, `ROADMAP.md`, `INVARIANTS.md`, and a
`formal/` directory. Last commit `f53838e` on **2026-07-16**; nothing
since. Removing Effect Native means *archiving a real public repo*, not
withdrawing an empty reservation.

**(b) Plain Effect.** `effect` v4 (`4.0.0-beta.94`, catalog-pinned). Used
everywhere as services/layers/Schema/error handling. Not proposed for
removal; measured only to show it is separable. Note that
`@effect-native/core/effect` is literally a one-line re-export
(`apps/openagents.com/packages/effect-native-core/src/effect.ts`):

```ts
export { Duration, Effect, Exit, Fiber, Schema, Scope, Stream, SubscriptionRef } from 'effect'
```

so every import of that subpath is plain Effect wearing an Effect Native
badge, removable with a mechanical codemod.

**(c) The vendored implementation.** Seven workspace packages under
`apps/openagents.com/packages/effect-native-*`, vendored (unbuilt TS
snapshot, not npm) from upstream commit `467bde0` (catalog
`effect-native/v43`, vendored 2026-07-15) per
`apps/openagents.com/packages/effect-native-vendor.json`, guarded by
`apps/openagents.com/scripts/check-effect-native-vendor.test.ts` (drift =
hard fail) and `scripts/check-effect-native-vendor-freshness.ts`
(staleness = warning). Measured size (`find …/src -name '*.ts*' | xargs
wc -l`):

| Package | Files | LOC |
| --- | --- | --- |
| `effect-native-core` | 4 | 7,838 |
| `effect-native-render-dom` | 6 | 7,520 |
| `effect-native-render-rn` | 2 | 8,324 |
| `effect-native-render-canvas` | 11 | 2,791 |
| `effect-native-tokens` | 3 | 2,510 |
| `effect-native-gallery` | 5 | 4,810 |
| `effect-native-khala-ui` | 7 | 1,251 |
| **Total vendored** | **38** | **35,044** |

Repo-wide, 331 `@effect-native/` occurrences appear in TS sources
(`rg -c '@effect-native/' … | sum`). The vendor pin is already one commit
behind upstream HEAD.

### 2.2 The docs footprint

`rg -l -i 'effect native|effect-native' docs/` matches **270 files**. The
overwhelming majority are dated receipts and historical records that
should not be rewritten. The *living* authority documents that would need
amending are: `AGENTS.md` (UI-layer clause 1016-1037; mobile clause
1068-1073), `docs/sol/MASTER_ROADMAP.md` (R5 exit criterion "Effect
Native/Electron", line 404), `docs/effect-native/README.md` + status,
`docs/effect-native/DEMAND_REGISTER.md`,
`docs/markets/2026-08-04-swap-demo-ui-rollout-plan.md` (§2.2 and §4.1 say
the web swap product "is Effect Native"), and the workspace-root
`CLAUDE.md` (stale on the repo's contents and on the reservation
pattern). `AGENTS.md:1006-1007` is also stale independently of this
audit: it says "`packages/autopilot-ui` stays: the live
`apps/openagents.com/apps/web` app imports it" — `apps/web` was deleted
in `67adbe523c` (2026-07-14, 471 files, 184,306 deletions) and no
`packages/autopilot-ui` exists in the workspace tree.

## 3. Actual adoption, per app (measured)

Summary table (`rg -l` file counts; LOC via `find … | xargs wc -l`):

| Surface | Stack today | Size (src) | Files importing `@effect-native` | EN-coupled LOC | Plain-`effect` files |
| --- | --- | --- | --- | --- | --- |
| `apps/openagents.com/apps/start` | TanStack Start 1.168.26, React 19.2.7, Vite 8 | 325 files / 47,317 LOC | 31 | 12,401 | 25 |
| `apps/openagents.com/apps/diamond-hands` | Rust/GPUI → wasm (Trunk) | 1,160 Rust LOC | — | — | — |
| `apps/openagents.com/apps/market-demo` | Rust/GPUI → wasm (Trunk) | 1,520 Rust LOC | — | — | — |
| `apps/openagents-desktop` | Electron 43 + React 19.2.7 + Vite | 832 files / 247,362 LOC | 98 | 65,805 (52,405 real component/View usage) | 286 |
| `apps/openagents-mobile` | Expo 57 / RN 0.86, plain RN | 47 files / 10,773 LOC | **0** | 0 | 11 |
| `apps/aiur` | TanStack Start 1.168.26 | 54 files / 4,735 LOC | 0 imports (dep on `tokens` only, 2 refs) | ~0 | 2 |
| `apps/forum` | headless Effect stub | 2 files / 35 LOC | 0 | 0 | 2 |
| `apps/pylon` | headless Effect service | 700 files / 204,650 LOC | 0 | 0 | 191 |
| `packages/ui` | plain React 19 + CSS kit | 44 files / 4,346 LOC | 1 (`Theme` type in `theme-bridge.ts`) | ~30 | — |
| `packages/omega-effectd` | Node Effect engine | — | 7 (all via the `/effect` re-export alias) | 2,940 | — |

### 3.1 Web: `apps/openagents.com/apps/start`

The only web app. (`apps/openagents.com/apps/web` — the Foldkit legacy —
was deleted by `67adbe523c` on 2026-07-14; `apps/astro` by `abf4eaa311`
on 2026-07-15. TanStack Start is the single web surface, exactly the
place the owner says TanStack fits.) It has 72 real route files. The
Effect Native surface is ~10 co-located page modules rendered through
`@effect-native/render-dom`: `-splash-page` (+ `-splash-khala-canvas`,
the one `render-canvas` consumer), `-forum-page`, `-portal-page`,
`-khala-effect-native-page`, `-trace-page`, `-landing-en-page`,
`-privacy-effect-native-page`, `-components-page`/`-components-khala-page`
/`-components-storybook-page`/`-components-workbench-page`,
`-share-page`, `-stage1-effect-native-page`, plus their tests — 31 files,
12,401 LOC total. The other ~62 routes (blog, docs, admin, forge, pylons,
qa, stats, business, aisdk, agents, …) are ordinary React/TanStack.

### 3.2 Desktop: the heavy consumer, already marked as migration code

`apps/openagents-desktop` is Electron 43 + React 19 + Vite. It boots a
real Effect Native app: `src/renderer/boot.ts` uses `makeReactDomRenderer`
from `@effect-native/render-dom/react`; views are authored as `View`
trees with typed intents (top imported symbols from `@effect-native/core`:
`resolveIntentRef` ×20, `IntentReporter` ×15, `IntentRef` ×15, `View`
×11, `ComponentValueBinding` ×10). Split of the 98 importing files:

- 66 files (**52,405 LOC**) import the real component/View API
  (`from "@effect-native/core"`),
- 25 files import **only** the `/effect` alias — i.e. plain Effect,
  a one-line codemod (§2.1(b)),
- boot/renderer glue uses `render-dom` (5 imports) and `tokens` (12).

Two facts cap the value of a rewrite: the desktop is *already* a hybrid
(direct deps on `@base-ui/react`, `@shadcn/react`, Radix, Lexical,
`@xyflow/react`, Embla, Rive — none of them behind the EN contract), and
MASTER_ROADMAP owner decisions 3-5 make Omega (Rust/GPUI, separate repo)
the desktop destination with this app supported only "until the Omega
cutover gate." Four repo tests assert the EN boundary and would need
retargeting on any change (`tests/electron-boundary.test.ts:361`,
`tests/owner-ux-rules.test.ts`, `tests/harness-maintenance.test.ts`,
`tests/desktop-preferences.test.ts`).

### 3.3 The contradictions already in-tree

The codebase stopped believing the AGENTS.md mandate before this audit:

- **Mobile** (`AGENTS.md:1068-1073` still says "authored in Effect
  Native… styling as typed style objects on the shared
  `@effect-native/tokens` vocabulary") has zero EN imports since
  `017ae3dedb` (2026-07-27, owner-directed plain-RN rebuild) and
  `9b7559f0ae` (docs correction #9265).
- **GPUI wasm documents ship from the web app**: checked-in artifacts
  `apps/…/start/public/demo/market_demo_web_bg.wasm` (11,789,997 B raw;
  4,282,305 B gzipped — measured with `gzip -c | wc -c`) and
  `public/dh/diamond_hands_web_bg.wasm` (13,126,709 B raw; 5,458,053 B
  gzipped), served by `workers/api/src/cloudrun/start-ui.ts` behind
  `OPENAGENTS_MARKET_DEMO_ENABLED` (set `"true"` in
  `workers/api/scripts/cloudrun/env-production.yaml:122`) and
  `OPENAGENTS_DIAMOND_HANDS_ENABLED` (intentionally absent in
  production; `/dh` stood down per
  `docs/hardening/2026-08-04-gpui-on-web-addendum.md` §13). Both crates
  pin omega rev `b4fac63a2c90d77d6630d9df7eb9cb8a5983bac8`; diamond-hands
  additionally pins immortal `cb13db4a`.
- **SWAP-0 (#9315)** and
  `docs/markets/2026-08-04-swap-demo-ui-rollout-plan.md` §2.2 already
  narrowed "one GPUI component set on both surfaces" to "one *engine*,
  two rendered surfaces" — and §2.2's stated reason the web side was to be
  Effect Native was *"The repository's product-UI mandate is Effect
  Native"*, i.e. the mandate itself, not a technical preference. The SWAP
  packets that have actually landed are plain Effect.
- **The upstream framework stalled on 2026-07-16** while the monorepo's
  last substantive EN-package commit is `d449eea72e` (2026-07-25) and all
  UI momentum since has been GPUI (`/demo`, `/dh`, the hardening program,
  omega `chat_web`).

### 3.4 Verification commands (representative)

```sh
rg -l '@effect-native/' apps/openagents-mobile -g '*.ts*'        # → empty
rg -l 'from "@effect-native/core"' apps/openagents-desktop -g '*.ts*' | xargs wc -l | tail -1   # → 52,405
rg -l '@effect-native/' apps/openagents.com/apps/start -g '*.ts*' | xargs wc -l | tail -1       # → 12,401
git log --format='%h %ad %s' --date=short -1 ~/work/effect-native  # → f53838e 2026-07-16
```

## 4. The three candidate end-states

### Option A — keep Effect Native

Keep the AGENTS.md mandate; revive the stalled upstream; re-convert
mobile (undoing an explicit owner direction from a week ago); continue
converting the web app's remaining ~62 routes; carry the vendor loop
(pin, guard test, freshness script, GAPS register) indefinitely.

- **Benefit:** preserves the one-component-set thesis and the 85k-LOC
  upstream investment; the desktop's 52k LOC of View code keeps its
  contract; the React-harmonization design work (773-line gap analysis)
  stays live.
- **Cost:** contradicts the owner's 2026-08-04 direction outright;
  requires re-adopting on mobile against `017ae3dedb`; the thesis's core
  payoff (one set across web/mobile/desktop) is unreachable because
  mobile is plain RN and desktop's destination is GPUI-in-omega — the
  framework would serve exactly one platform (web), which is the platform
  React already serves. **Not recommended.**

### Option B — the owner's proposal (remove the component layer)

Remove Effect Native as a component/renderer framework. Keep plain
Effect for all services/logic. GPUI (omega-pinned wasm crates) for
omega-adjacent web surfaces; TanStack Start + plain React for
document/SEO/form surfaces; plain RN for mobile; desktop resolved by the
Omega cutover.

- **Benefit:** matches the direction; deletes a stalled dependency, the
  vendor loop, and a second component vocabulary; new SWAP/product work
  already looks like this.
- **Cost:** the desktop question (52,405 LOC of View code), the ten web
  page modules (12,401 LOC), tokens consumers (aiur, `packages/ui`),
  doc/contract amendments, and the a11y decision in §6.

### Option C — Option B with a desktop freeze (recommended)

Identical end-state to B, with one sequencing rule: **do not rewrite the
desktop's Effect Native views**. Freeze them — keep `core`, `render-dom`,
and `tokens` as desktop-internal frozen code (no new EN surfaces
anywhere, vendor loop retired, no upstream bumps), and let the Omega
cutover gate delete the whole app, EN layer included, as already planned.
Convert only the live web surfaces and the trivial alias/tokens
consumers now. This avoids ~52k LOC of throwaway rewrite while still
ending the framework. Everything below assumes C as the execution of B.

## 5. The GPUI-for-web alternative, assessed honestly

### 5.1 What is proven (working code, artifacts measured)

- `omega/crates/gpui_web` is a real browser platform backend: 9 files,
  2,816 Rust LOC, delegating rendering to `gpui_wgpu` (4,356 LOC,
  WebGPU). Zero `todo!`/`unimplemented!` — gaps are honest no-ops.
  Pointer/wheel/keyboard/drag-drop/focus/resize/DPR/dark-mode wired; IME
  composition and clipboard paste/write implemented; real Web-Worker
  multithreading (forked `wasm_thread`); Fetch HTTP from workers.
- **Omega's real design system runs in a browser tab**: the `chat_web`
  example (721 LOC, commit `6ec37d164f`, 2026-08-04) renders the real
  `ui` crate (28,210 LOC) and the real Aiur theme. This — not hello-world
  — is the meaningful maturity signal.
- **The monorepo already ships the pattern**: `/demo` and `/dh` are
  pinned-rev Rust crates built by Trunk into static documents under
  `apps/start/public/`, gated in the Cloud Run adapter. Wasm sizes
  measured in §3.3 (11.2-12.5 MB raw, 4.1-5.2 MB gzipped). The untracked
  omega `chat_web` debug artifact is 16,071,727 B (built with
  `data-wasm-opt="0"`, so not a fair shipping number; an optimized
  measurement does not exist yet — stated rather than estimated).
- Theming: `theme` (5,847 LOC) and `ui` compile clean to wasm.

### 5.2 What is NOT proven, and what is structurally absent

- **Accessibility: absent, with no in-tree path.** gpui core carries
  1,525 LOC of AccessKit machinery (`crates/gpui/src/window/a11y.rs` et
  al.), and the macOS/Windows/X11/Wayland backends implement
  `a11y_init`/`a11y_tree_update`; `gpui_web` implements **neither** (the
  empty defaults at `crates/gpui/src/platform.rs:944,947` apply), and no
  web AccessKit adapter exists anywhere. The page is an opaque `<canvas>`
  plus one invisible 1px input; the a11y tree gpui builds every frame is
  discarded.
- **No DOM semantics**: no native text selection (canvas even sets
  `user-select: none`), no Ctrl-F, no SEO/indexing, no link previews, no
  browser translation, no print, quotable only by screenshot.
- **Input gaps**: `update_ime_position` is an empty body
  (`gpui_web/src/window.rs:783` — CJK candidate windows render at page
  top-left); `read_from_clipboard()` hardcoded `None`
  (`platform.rs:362`); keyboard layout hardcoded `"us"` with
  `DummyKeyboardMapper`; touch is pointer-as-mouse only (no TouchEvent,
  no gestures, `pointer_type` never read) — **mobile web is effectively
  unserved**. i18n/RTL: unmeasured, but with a US-only key map and no
  browser translation, assume unsupported until proven.
- **Reach and toolchain**: WebGPU required, no WebGL fallback (~70% of
  browsers per the addendum §3); nightly Rust + `build-std` + wasm
  atomics; COOP/COEP cross-origin isolation on the serving origin; no CI
  in omega at all (no `.github/workflows/` directory), so the web target
  can silently break.
- **Ownership risk**: 22 commits ever on `crates/gpui_web`; 21 are
  upstream Zed's (first: #50228, 2026-02-26), one is ours (the example).
  Zed publicly slowed GPUI in 2026. We inherit the backend either way,
  but we do not maintain it and have no second maintainer.
- **`market_ui` does not exist.** The rollout plan names
  `omega/crates/market_ui` as the GPUI component set; `find`/`rg` across
  omega and immortal finds no such crate (0 LOC). The GPUI component
  library for markets is planned, not built.

**Blunt summary:** GPUI-on-web has cleared the hard technical part and is
a legitimate choice for gated, demo-class, omega-adjacent documents — the
role it already plays at `/demo` and `/dh`. A GPUI-everywhere-on-web
claim that ignores the missing accessibility adapter, US-only keyboards,
absent touch, 70% reach, and multi-megabyte first paint would not be
honest, and this audit does not make it.

## 6. The accessibility/SEO decision the owner must make (a decision, not a footnote)

The owner's phrase "web UI for all the omega-adjacent stuff should use
GPUI" has two readings, and they diverge exactly where the repo already
fought this out:

- **Reading 1 (bounded — consistent with the existing record):** GPUI
  canvas documents are the pattern for *gated demos and
  omega-workbench-shaped surfaces* (`/demo`, a future browser Omega
  subset), while public evidence pages, money-moving product surfaces,
  and anything with SEO/forms stays DOM. This is what the hardening
  addendum §4/§12 and SWAP rollout §2.2 already decided, and what the
  stand-down of `/dh` (§13) reinforced.
- **Reading 2 (maximal):** ungated public product surfaces may ship as
  GPUI canvas. That means shipping, today: no screen reader access of any
  kind, no text selection or Ctrl-F, no indexing, ~70% browser reach with
  a GPU requirement, broken CJK IME positioning, US-only keyboard
  mapping, no real touch support, and a 4-5 MB gzipped first load —
  every one of these measured or verified in §5.2, none of them fixable
  in-tree today (the a11y adapter does not exist anywhere).

**The decision to record:** which reading governs, and specifically
whether the SWAP web product (P4-P6, issues #9315-#9323) — a page users
move money on — is DOM (plain React/TanStack after EN removal) or canvas.
This audit recommends Reading 1: keep the §2.2 engine split, re-spec the
web host from "Effect Native" to plain Effect + React, and hold the line
that no ungated public surface is canvas-only until an AccessKit web
adapter exists. Note also that `/demo`'s gate is enabled in production
(`env-production.yaml:122`) — if Reading 1 is chosen, `/demo` remains
fine *as a labeled demo*; it should just never be silently promoted to
"the product."

## 7. Per-surface routing table

"Packets" = independently shippable, revertible work units of roughly one
focused PR each.

| Surface | Today | Recommended stack | Why | Migration cost |
| --- | --- | --- | --- | --- |
| `apps/start` — ~62 non-EN routes (blog, docs, admin, forge, dashboards, promises, login…) | TanStack Start + React | unchanged | already the owner's "sometimes"; document/SEO/form-shaped | 0 |
| `apps/start` — ~10 EN page modules (§3.1) | EN `render-dom` inside TanStack | plain React components in place | 12,401 LOC, all web-only; removes `render-dom`/`khala-ui`/`gallery`/`render-canvas` consumers | 3 packets (splash+landing+privacy; forum+portal+share+trace; khala+components workbench) |
| `/demo`, `/dh` wasm documents | Rust/GPUI pinned to omega `b4fac63a` | unchanged (gated GPUI documents) | owner-directed lineage; demo-class scope | 0 |
| SWAP web product (P4-P6, #9315-#9323) | spec says "Effect Native" — no UI code landed yet | plain Effect engine binding + React/TanStack host (per §6 Reading 1) | money-moving page needs DOM a11y; only spec text changes | 1 packet (doc + issue re-spec) |
| Omega desktop + future market panel | GPUI native (`market_ui` unbuilt) | GPUI in omega | owner decision; omega#244 | n/a (omega repo) |
| `apps/openagents-desktop` | Electron + EN views (52,405 LOC) + hybrid React | **freeze**: EN packages become desktop-internal, no new EN surfaces, deleted whole at Omega cutover | avoids throwaway rewrite; MASTER_ROADMAP decisions 3-5 already sentence the app | 1 packet (containment + guard retarget) |
| `apps/openagents-mobile` | plain Expo/RN | unchanged | EN already removed 2026-07-27 | 0 |
| `apps/aiur` | TanStack Start + `@effect-native/tokens` | swap to a plain tokens package | 2 references | shared packet with tokens extraction |
| `packages/ui` | React kit; EN `Theme` type in `theme-bridge.ts` | depend on the extracted plain tokens package (or inline the type) | 1 file | shared packet |
| `packages/omega-effectd` | 7 files importing the `/effect` alias | `import { … } from "effect"` | pure re-export | trivial codemod packet |
| `apps/pylon`, `apps/forum`, workers, all other packages | plain Effect | unchanged | zero EN usage | 0 |

Total new work excluding the desktop rewrite deliberately avoided:
**~7-8 packets** (§9).

## 8. What breaks and what it costs

Named consumers requiring change under Option B/C:

| Consumer | Coupling | LOC touched | Disposition |
| --- | --- | --- | --- |
| `apps/openagents-desktop` (66 files real EN) | View trees, intents, `render-dom` boot, 4 boundary tests | 52,405 | **frozen, not rewritten**; deleted at Omega cutover |
| `apps/openagents-desktop` (25 files, `/effect` alias only) | plain Effect re-export | mechanical | codemod now (safe even under freeze) |
| `apps/start` (31 files) | `render-dom`/`khala-ui`/`render-canvas`/`gallery`/`tokens` | 12,401 | convert to plain React, 3 packets |
| `packages/omega-effectd` (7 files) | `/effect` alias | 2,940 file-LOC, mechanical | codemod |
| `apps/aiur` (2 refs) + `packages/ui` (`theme-bridge.ts`) | `tokens` / `Theme` type | small | retarget to extracted tokens package |
| Vendored packages (7, 35,044 LOC) | — | deleted: `render-rn` (consumerless since mobile removal), `render-canvas`, `gallery`, `khala-ui` after web conversion; `core`/`render-dom`/`tokens` survive only inside the frozen desktop until cutover | staged deletion |
| Guards: `check-effect-native-vendor.test.ts`, `check-effect-native-vendor-freshness.ts`, vendor manifest | vendor loop | ~few hundred | retired when vendoring ends |
| Contracts/docs: `AGENTS.md`, `MASTER_ROADMAP.md` R5, `docs/effect-native/` (13 docs), `DEMAND_REGISTER.md`, swap rollout plan §2.2/§4.1, workspace `CLAUDE.md` | mandate text | doc edits | amend/mark historical |
| `OpenAgentsInc/effect-native` (85k LOC public repo) | upstream | — | owner decision: archive (not "withdraw an empty reservation" — see §2.1(a)) |

Not broken, explicitly: plain `effect` everywhere,
`packages/effect-start`, `packages/effect-boundary`, all headless
services, mobile, the GPUI documents, and every non-EN web route.

## 9. Sequenced removal plan (if Option B/C is chosen)

Each packet independently shippable and revertible; order matters only
where noted. **None of this is authorized by this audit.**

1. **EN-R0 — Decision + contract amendment.** A Sol-lane decision doc
   recording the 2026-08-04 owner direction; amend `AGENTS.md:1016-1037`
   (UI layer → "plain Effect for logic; React/TanStack for DOM surfaces;
   GPUI for omega-adjacent documents; no new `@effect-native` imports"),
   fix the stale `AGENTS.md:1006` `apps/web` clause, amend the mobile
   clause 1068-1073 to match `017ae3dedb` reality, and record the §6
   accessibility decision explicitly. MASTER_ROADMAP R5 wording updated.
2. **EN-R1 — Alias + tokens codemod.** `@effect-native/core/effect` →
   `effect` in `packages/omega-effectd` (7 files) and desktop's 25
   alias-only files; extract `effect-native-tokens` as a plain
   `@openagentsinc/design-tokens` package (2,510 LOC, no framework
   coupling exists in it); retarget `apps/aiur` and
   `packages/ui/src/workbench/theme-bridge.ts`.
3. **EN-R2 — SWAP re-spec.** Amend
   `docs/markets/2026-08-04-swap-demo-ui-rollout-plan.md` §2.2/§4.1 and
   comment on #9315-#9323: web host is plain Effect + React/TanStack; the
   engine boundary (the part that mattered) is unchanged.
4. **EN-R3/R4/R5 — Web route conversion**, three packets per the §7
   clusters, each converting pages to plain React in place and deleting
   its EN imports. After R5, `render-canvas`, `khala-ui`, `gallery` have
   zero consumers.
5. **EN-R6 — Vendor prune.** Delete `effect-native-render-rn` (already
   consumerless), then `render-canvas`/`gallery`/`khala-ui`; shrink the
   vendor manifest to the desktop-frozen set (`core`, `render-dom`,
   `tokens` if not yet extracted); retire the freshness script; keep the
   drift guard only for the frozen set.
6. **EN-R7 — Desktop containment.** Move the frozen `core`/`render-dom`
   under `apps/openagents-desktop/` (or mark them frozen in place),
   retarget the four boundary tests, and add a repo guard (oxlint or
   assure-repo rule) rejecting new `@effect-native` imports outside the
   desktop. Desktop EN code is thereafter deleted by the Omega cutover,
   not by this program.
7. **EN-R8 — Docs + upstream disposition.** Banner
   `docs/effect-native/README.md` and the 12 sibling docs as historical
   (superseded by EN-R0's decision doc); close `DEMAND_REGISTER.md`.
   Owner actions (via `NEEDS_OWNER.md`): archive
   `OpenAgentsInc/effect-native` on GitHub; amend the workspace-root
   `CLAUDE.md` (separate repo/commit) which currently describes both the
   Effect Native thesis as current and the repo as empty.

Revert story: R1/R2 are text-level reverts; R3-R5 revert per-route;
R6-R7 revert by restoring packages from git. Nothing crosses repos in a
single packet.

## 10. Open questions for the owner

1. **Desktop:** confirm the freeze (Option C) over a rewrite — is it
   acceptable that the Electron app keeps frozen Effect Native internals
   until the Omega cutover deletes it? (52,405 LOC rides on this.)
2. **§6 accessibility/SEO reading:** Reading 1 (GPUI for gated
   demos/workbench surfaces, DOM for public/product/money pages) or
   Reading 2 (canvas allowed on ungated public surfaces)? This audit
   recommends Reading 1 and treats the SWAP web product as DOM.
3. **Web EN routes:** convert all ten in place, or retire any of them
   outright (e.g. `landing-en`, `stage1`, the components galleries exist
   largely to showcase Effect Native itself)? Retirement is cheaper than
   conversion where the page's purpose was the framework.
4. **`OpenAgentsInc/effect-native`:** archive the public repo (85k LOC,
   143 commits), or leave it dormant/public? Archiving is honest;
   dormant invites drift back.
5. **Tokens:** keep the design-token vocabulary as a plain package (this
   audit's recommendation — it is the one EN piece with cross-surface
   consumers and no framework coupling), or fold tokens into per-app CSS?
6. **`/demo` production gate:** it is on
   (`env-production.yaml:122`) — deliberate, given `/dh` was stood down?

## 11. Recommended follow-up issues (not created by this audit)

1. *Withdraw the Effect Native UI mandate* — EN-R0: decision doc,
   AGENTS.md/MASTER_ROADMAP amendments, §6 a11y decision recorded.
2. *Codemod `@effect-native/core/effect` → `effect`; extract tokens as a
   plain package* — EN-R1 (omega-effectd, desktop alias files, aiur,
   packages/ui).
3. *Re-spec SWAP web host to plain Effect + React* — EN-R2 on
   #9315-#9323 and the rollout plan.
4. *Convert or retire the ten Effect-Native web page modules in
   `apps/start`* — EN-R3..R5, three route-cluster packets.
5. *Prune consumerless vendored renderers and retire the vendor
   freshness loop* — EN-R6.
6. *Freeze desktop Effect Native internals and guard against new
   `@effect-native` imports* — EN-R7.
7. *Mark `docs/effect-native/` historical; owner: archive
   `OpenAgentsInc/effect-native`; amend workspace `CLAUDE.md`* — EN-R8
   (+ `NEEDS_OWNER.md` entries for the GitHub archive click-path).
