# The Aethyr merge

Date: 2026-10-24. Owner direction, same day: the engine is open source
except for `cakbags`; all new work lives in this repository; the
cross-device framework is called **Aethyr**; the Kiki MVP on the web launches
first and is rebuilt here on Aethyr, working on the web and in the mobile
app.

Decisions live in [`docs/adr/`](../adr/README.md). This document is the plan:
what moves, what we build, what we suggest deprecating, and what the owner
still has to decide. The two documents it builds on are imported unchanged:
the [consolidation audit](imported/2026-10-24-consolidation-audit.md) (the
inventory of what moves where) and the
[GPU Native exploration](imported/2026-10-24-gpu-native.md) (the case for one
GPU-drawn framework).

## What we know for sure

| Decided | Record |
| --- | --- |
| All earlier decision records are deprecated | [0001](../adr/0001-one-decision-record-folder.md) |
| Everything new lives here; Aethyrforge moves here; `cakbags` stays private | [0002](../adr/0002-everything-in-openagents.md) |
| The framework is Aethyr, GPU native, with thin hosts | [0003](../adr/0003-aethyr.md) |
| One Rust engine core over a `webgpu.h`-shaped interface, with batched web calls | [0004](../adr/0004-engine-core-and-gpu-interface.md) |
| Effect controls the UI on the web, through a view-tree protocol | [0005](../adr/0005-effect-controls-the-ui.md) |
| One design system | [0006](../adr/0006-one-design-system.md) |
| One device profile and detail controller | [0007](../adr/0007-capability-and-detail.md) |
| Kiki on the web first, then mobile demos, then mobile zones | [0008](../adr/0008-launch-order.md) |
| Zones, and the Grid as the coding-agent zone | [0009](../adr/0009-zones-and-the-grid.md) |
| Mobile zones run in a web view now and on Aethyr natively later; the Grid is built into the shell | [0010](../adr/0010-mobile-zone-loading.md) |
| GPU-driven rendering; Slang shaders shipped as WGSL | [0011](../adr/0011-rendering-and-shaders.md) |
| One token crate: `oa-tokens` absorbs `aethyr-tokens`; Coder Noir roles win where they mean the same thing | [0012](../adr/0012-one-token-crate.md) |

## Where things go

| What | Where |
| --- | --- |
| Engine Rust crates (`af-kernel`, `af-gpu`, `af-render-lab`, `af-mmorpg-core`, `af-world-server`) | `crates/aethyr/`, its own Cargo workspace (Rust 1.98.1, `wasm32-unknown-unknown`), excluded from the root workspace like `crates/psionic` |
| Aethyr web: the Effect controllers, the React DOM renderer, the web host, the engine's TypeScript renderer and runtime until Rust parity, editor, player, preview, labs, and their tests | `apps/aethyr/`, one npm package that absorbs `apps/verse-ui` |
| View-tree schema, design tokens, device profile | Rust crates in the root workspace (`crates/aethyr-ui`, `crates/aethyr-profile`; tokens in `crates/oa-tokens` per [0012](../adr/0012-one-token-crate.md)), so the website, the engine, and the mobile app can all link them |
| Zone launcher, web runner, and host bridge for the phone | `apps/openagents/crates/mobile` (Rust) with thin UIKit and Kotlin hosts |
| Engine specification and research | `docs/aethyr/` |
| Licensed assets | Never in Git ([private assets](../verse/private-assets.md)) |

The Kiki routes keep serving their pinned releases throughout. A pin changes
only after a release built from this repository passes the same browser
checks (`scripts/check-haunted-house.mjs`, `scripts/check-verse-*.mjs`,
`scripts/check-medieval-town.mjs`) in candidate mode.

## The Kiki rebuild

Tracked as one epic with task issues. In order:

1. **Bring the Kiki-relevant engine code over** (universe, house, player,
   preview, editor services, renderer, runtime, kernel, render lab, `af-gpu`,
   tests, verifiers, deploy scripts) with no behavior change. Proof: equal
   `kernel.wasm` and bundle hashes against the same Aethyrforge commit.
2. **Aethyr UI protocol:** the view-tree and intent schema in Rust, generated
   TypeScript types, the Effect controller runtime, and the React DOM
   renderer on the design system ([UI protocol](ui-protocol.md)).
3. **Design tokens crate** emitting the CSS the website uses today
   (`crates/aethyr-tokens`).
4. **Kiki surfaces on Aethyr:** the entry and destination flow, the universe
   HUD, the house studio, the player, and invitations, each moved to an Effect
   controller and the shared renderer, phone width first.
5. **Device profile and detail controller**, wired first to the current
   renderer's quality switches.
6. **Releases built from this repository**, deployed by digest to the existing
   editor and preview services, then pinned.
7. **Mobile:** the zone launcher, the web runner on iOS and Android, the
   native bridge for `aethyrforge-host`, the Haunted House as the first
   demo, and the Grid as a built-in zone.
8. **Native `af-gpu` backend and render-lab benchmarks** on real GPUs: the
   start of the native runner.

## Suggested deprecations

Stop extending these. Each is removed when its replacement passes its checks.

| Deprecate | Replacement | Remove when |
| --- | --- | --- |
| Aethyrforge as a place for new work | This repository | The import lands; then archive it |
| Every decision record outside `docs/adr/` | `docs/adr/` | Now: add a deprecation note to each |
| The name "Verse Engine" | Aethyrforge (engine) and Aethyr (framework); Verse stays the product | Now, in new docs |
| Verse v1 renderers that call `wgpu` directly (`verse`, `verse-gfx`, `verse-pbr`, `everglade-web`) | Aethyr on `af-gpu` | Each zone is ported; maintenance only until then |
| The WebGL2 path in `everglade-web` | WebGPU only | Everglade is ported |
| iOS `medieval_town` (second native renderer of the town) | The native zone runner | The native runner draws the town |
| `apps/haunt-preview` and the `haunt.rs` fallback | The pinned house release | The house pin has been stable for an agreed period |
| `apps/verse-selector` and `new_lander.js` | One universe selector, from the engine | `/new-lander/verse` runs on it |
| `apps/verse-ui` as a separate package | `apps/aethyr` | Step 4 above |
| Rust Native's per-platform widget adapters as the growth path | One GPU adapter plus platform bridges | Aethyr's GPU UI passes the accessibility bar |
| New investment in the GPUI fork | Aethyr on the desktop | The desktop phase; fixes only until then |
| Two UI schemas (`ui.screen.v1` and the Rust Native view schema) | One view tree ([0005](../adr/0005-effect-controls-the-ui.md)) | Step 2 above |
| Per-call Rust-to-JavaScript WebGPU imports | Batched pass replay | The batched import lands |
| Python and TypeScript asset tools in Aethyrforge `tools/assets` | One Rust asset compiler | Each tool's Rust replacement matches its output hashes |
| The C Town experiment code | Its recorded results | At import |
| The cross-origin SDK fetch and the "private adapter" machinery in the host protocol | Linking `aethyrforge-host` at build time; the protocol stays as a security boundary | New releases no longer need it |
| Chat entry points in Kiki surfaces | Kiki's own flows | Done on the web (`c4560e65`) |
| `docs/audits/2026-10-22-owned-renderer-webgpu-and-effect.md` and `docs/verse/engine/roadmap.md` | [0004](../adr/0004-engine-core-and-gpu-interface.md) and this plan | Now: mark them historical |
| Headless Linux Chrome as GPU performance evidence | Real-GPU hosts that record the adapter | Now |

## Decisions for the owner

Nothing below is built until decided. Recommendations are marked.

1. **History.** Bring Aethyrforge's full history in with an unrelated-history
   merge so cited commits keep resolving (recommended), filter it, or squash.
   And may the third-party tweet digest and the X archiver stay in public
   history? The Kiki rebuild copies files with their source commit recorded,
   so it doesn't wait on this.
2. **License.** Apache-2.0 for engine code, matching this repository
   (recommended). Should the archived Aethyrforge repository become public?
3. **Mobile controller.** Run the same Effect controller on mobile in an
   embedded, interpreted JavaScript engine (recommended), or write mobile
   controllers in Rust ([0005](../adr/0005-effect-controls-the-ui.md)).
4. **Language exceptions.** `apps/aethyr` replaces the three current
   TypeScript exceptions (`apps/haunt-preview`, `apps/verse-selector`,
   `apps/verse-ui`). Is a transitional Python exception allowed for the asset
   tools until the Rust compiler replaces them?
5. **Accessibility bar** before a GPU-drawn surface replaces a DOM or native
   one. Recommended: VoiceOver, TalkBack, and NVDA can reach and operate every
   control.
6. **The bare Verse world** in the mobile app's Verse tab is labeled "The
   Grid" today. What is it called now that the Grid is the coding-agent zone?
7. **Verse v1.** Maintenance with gradual porting (recommended). Does WebGL2
   support end with the last port?
8. **iOS `medieval_town`.** Freeze it until the native runner replaces it
   (recommended), or remove it now.
9. **Licensed-kit identifiers.** May vendor mesh names (`SM_*`) stay in public
   engine code under the licensed-kit rule, or do they move into private asset
   manifests?
10. **Unreal-dependent tools.** Publish Aethyrforge `tools/reference` and the
    Unreal exporters, or drop them.
11. **Cutover.** When Aethyrforge `main` stops taking work, and whether its
    `codex/kiki-mobile-touch` branch is merged or dropped.
12. **Store review.** Is a legal reading of the current Apple and Google rules
    on downloadable zones needed before the first mobile submission?
