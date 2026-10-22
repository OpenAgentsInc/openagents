# New landing page: stack and loading plan

Date: October 22, 2026. Source snapshot: `be63f0d030`.
Status: architecture assessment with an initial isolated preview implementation. `/new-lander` serves the marketing page and `/new-lander/verse` serves the universe selector; `/` stays unchanged. This is not the full tutorial or a production deployment.

## Decision

**Build `/new-lander` with the existing Rust/Axum server, server-rendered HTML, Maud, and CSS. Load the interactive Verse separately, after the visitor chooses Enter the Verse.** Use the selected Effect/direct-WebGPU browser architecture for the game and creator experience, with existing Rust services behind it.

“Axiom” is interpreted here as **Axum**, the Rust HTTP framework already used by the website. Axum and WebGPU solve different problems: Axum serves the page and APIs; WebGPU renders a world after entry. They can be used together. [Axum documentation](https://docs.rs/axum/latest/axum/).

The marketing page needs readable product information, links, responsive layout, and a clear entrance. It should display those before a game engine, Wasm compilation, GPU adapter, chat canvas, or relay connection is ready. WebGPU is not a prerequisite for reading the company website.

## Design being served

The source is [Kiki's flow in cakbags](https://github.com/OpenAgentsInc/cakbags/blob/204c1a1cc9ae159bfc1018515056b96578f7b8f9/ux-flow-kiki.md). Her latest homepage note calls for the existing top-left identity and top-right controls, a conventional marketing hero and product suite, black and white, and a white glow. Interpret the voice transcript's repeated “sleep” as likely “sleek”; confirm the visual treatment with Kiki rather than using that transcription as interface copy.

- The homepage features **Enter the Verse** at the top, alongside the standard product-suite presentation.
- Entry first reveals a game-quality universe/world selector, informed by the Episode 289 network graphic and Kiki's Expanse-like space direction. Finite planets represent network nodes. The central OpenAgents company planet remains locked; its Gym and Pylons serve their audiences later.
- A Grid-style login card appears over that view. After login, choose exploration or the Haunted House tutorial.
- The tutorial uses voice and optional buttons, a mesh toggle, a bottom-left planetary map control, and shadowed sci-fi editing screens. Compile, inspect, then explicitly Publish or Revert.
- Play/Share cards lead into the tutorial's return to the planets, a personal planet award, voice setup of art style/time/day-night cycle, walking, invitations, and chat toggles.
- Buildings, levels, narrative tools, and further chat changes remain later scope. AAA quality is a creative target, not an existing capability or a consequence of choosing WebGPU.

Design interpretation: preserve place while tools open; let people direct changes; teach through play; keep navigation consistent while worlds have distinct art styles. These principles guide the technical boundaries below, without adding new tutorial steps.

## Enter the Verse: universe and world selector

**Kiki's visual direction, added October 22:** after choosing Enter the Verse, the visitor first sees a universe-like navigational graphic, as if choosing galaxies, systems, or planets in a cinematic space experience. The mood should recall *The Expanse*: immense space, believable depth, restrained light, and a purposeful navigation interface.

Reference: the user-provided Episode 289 screenshot at approximately **03:33**, named `Screenshot 2026-10-22 at 4.32.49 PM.png` on the owner's Desktop. It shows one large central circular network connected to smaller orbital groups, with luminous nodes and fine links on a dark field. The original image remains outside Git under the repository's large-file rule.

The [full transcript](../transcripts/289.md) describes other connected universes at **02:30**, then a shared ecosystem reached through simple interfaces at **03:00–03:30**. That is the source concept; using it as a navigable planetary entrance is Kiki's new design direction. The screenshot is a design reference, not evidence that this selector already runs in the product.

- **Composition:** a prominent central OpenAgents system/planet, with smaller surrounding systems, orbital groupings, luminous bodies, and fine connecting paths. Translate the graphic's nested circular structure into spatial depth; give the visitor a sense of a larger universe around the destinations.
- **Presentation:** dark space, carefully controlled glow, readable silhouettes and labels, and subtle color accents where they clarify groups or selection. The marketing page remains black and white; the universe and individual worlds can have their own art direction within a consistent sci-fi interface.
- **Arrival:** reveal the universe view before the login card. It is the first destination after the game-style loading transition, not a diagram the visitor must study before proceeding. Any camera reveal should be brief and respect reduced motion.
- **Navigation:** make the graphic function as a world selector. A focused or selected destination needs a clear name, selection state, and destination card; support a keyboard-accessible destination list alongside spatial navigation. Pan/zoom and a move from system overview to planet detail are proposed interactions for Kiki to review, not requirements for a full astronomy simulation.
- **Availability:** keep the OpenAgents company planet central and visibly locked. Other destinations must distinguish available worlds from locked or future ones. Do not suggest that every luminous point is an enterable world or that every network connection is live traffic.
- **Sequence:** the Grid-style login card overlays this universe view. After login, preserve the Explore versus Haunted House tutorial choice. Exploring the map does not enter the locked company planet. The bottom-left map control later returns to this same navigational view, where the creator's awarded planet becomes discoverable.
- **Scope:** the first view establishes place and offers destinations. It does not require Pylon statistics, agent-routing details, payments, or an infrastructure tour. Galaxy/system/planet naming and hierarchy still need Kiki's review; do not infer plot sales, planet counts, or capacity rules from the reference.
- **Original design:** borrow the reference's sense of connected systems and the show's atmosphere while creating OpenAgents' own planets, animation, typography, and interface. The reference video's speaker overlay, player controls, date label, and timeline do not belong in the product UI.

The [existing Kiki galaxy film shot](../verse/film/verse-shots/kiki-galaxy.json) is another spatial/mood reference: it describes a pullback to company and creator planets connected by threads of light. It is a cinematic shot definition, not an interactive selector, login implementation, or completed planetary navigation system.

## What exists now

| Surface | Evidence in the inspected repository | Consequence |
| --- | --- | --- |
| Public homepage | `/` belongs to `pages::grid_home`; `/home` redirects there. A public browser visit on October 22 showed the live Grid, rather than a product-suite hero. | A new marketing page is a different entry experience. Do not replace `/` during preview work. |
| Homepage loader | `grid_home::LOADER` calls `account()`, then `loadLayer().then(verse)`. The layer loads Pylon Wasm/glue; Verse loads `/everglade/everglade_web_bg.wasm` and its glue. It prepares chat around startup and mounts it after the world is ready; individual cards load on demand. | Reusing the entire loader would start the world and related modules before the new entry click. Reuse individual capabilities instead. |
| Current top bar | HTML supplies the OpenAgents power mark/name and account navigation when enabled; signed-in information comes from `/home/me`. The current world/chat layers also replace parts of that presentation. | Preserve identity and account behavior without requiring those canvases for marketing navigation. |
| Existing marketing-style pages | `pages::download::download_page` uses `UiPage`, `PageColumn`, Maud, and the shared HTML design components. The crates already depend on Axum and Maud. | An accessible Rust-rendered page is an existing path, not a new frontend stack. |
| Earlier landers | `lander5-web` is a small Rust/Wasm **WebGL2** procedural background. `/lander11` combines it with chat and a later Verse load. | Useful loading/lifetime references, but neither is the chosen direct-WebGPU runtime or the required product-suite page. |
| Browser creator runtime | `apps/haunt-preview` uses Effect, direct WebGPU, a preview worker, Kiln/OPFS, and Rust-backed decision handling. Its README describes local editing and states that public publication is later work. | Reuse its browser architecture and service boundaries; do not claim the complete login/tutorial/planet/publication flow already exists. |
| Shipping content | `live_content` supports manifest-backed text, browser-module files, and handler rendering. The deployment record documents live content updates on staging, including a measured 3.8-second median for a docs edit. | Reuse this delivery system, but add and measure the new page's content paths explicitly. That docs measurement is not a landing-page deploy benchmark. |

Source locations: [Grid page and loader](../../crates/openagents-web/src/pages/grid_home.rs), [page routing](../../crates/openagents-web/src/pages/mod.rs), [download page](../../crates/openagents-web/src/pages/download.rs), [legacy lander runner](../../crates/lander5-web/README.md), [lander11](../../crates/openagents-web/src/pages/lander11.rs), [creator preview](../../apps/haunt-preview/README.md), [live content](../../crates/openagents-web/src/live_content.rs), and [deployment record](../deployment/openagents-web.md#2026-10-21-content-deploys-from-livehost-on-staging-12435).

The public root returned HTML with `cache-control: no-cache` and a restrictive hashed-script/style CSP during inspection. This confirms the live shell's response, not its deployed source commit, total startup transfer, frame performance, or the state of every product. No sign-in or account mutation was performed.

## Stack by responsibility

| Responsibility | Recommendation |
| --- | --- |
| Marketing route and page | Axum handler in the existing web page router; Maud/HTML using the site's existing `UiPage`/component foundations where appropriate. No new server or framework migration. |
| Typography and visual treatment | CSS, existing brand assets/fonts, semantic sections, responsive product cards, and restrained glow. Use a static optimized image/SVG for the Verse preview first. Honor reduced motion. |
| Marketing behavior | Native links and browser behavior first. Reuse existing Rust-owned adapters where needed; do not introduce a second TypeScript marketing application or a new Wasm UI bundle for ordinary navigation. |
| Account and domain authority | Existing Rust authentication, sessions, permissions, storage, and publishing services. The marketing page does not create a game identity or open a microphone. |
| Planetary view and creator runtime | The browser-first Effect/direct-WebGPU approach selected in the [owned renderer audit](2026-10-22-owned-renderer-webgpu-and-effect.md), loaded after entry. WGSL and scene/material changes remain independently editable. |
| Game overlays and cards | Preserve the sci-fi appearance with accessible controls over the world. Reuse Rust Native card/sheet definitions and the appropriate adapter where available; identify adapter gaps before implementing duplicate cards. |
| Assets and deployment | Existing content-addressed/livehost delivery, separately versioned marketing content and game packages, and a working fallback for each page. No new hosting service is required. |

[Maud](https://maud.lambda.xyz/) generates HTML from Rust. It does not make a WebGPU application, and changing Maud markup still needs a Rust rebuild. Move frequently edited copy and approved content into runtime-loaded data/templates where the current content system supports it; do not promise compilation-free layout changes automatically.

The [agent contract](../../AGENTS.md) now names the owner-requested `apps/verse-selector` browser island alongside the existing creator-preview exception. Marketing and routing remain Rust. The selector uses Effect and direct WebGPU; it does not replace the Rust world/domain services or add a second account system.

## Preview addresses and ownership

- Marketing preview address: **`/new-lander`**. This explicit route does not revive redirects such as `/landing` or another numbered legacy lander.
- Universe selector address: **`/new-lander/verse`**. Selection is kept in its `world` query parameter. The remaining tutorial/planet flow is future work; this address is not a requirement to reload the document for each overlay.
- Add the page through `openagents-web/src/pages/new_lander.rs` and the existing page router, following current route ownership. If exposed in Design Lab, register sample states separately in `openagents-web-site`; its screens gallery is not the public page implementation.
- Keep `/`, `/home` redirects, `/chat`, account routes, hosted games, and existing landers unchanged. Do not redirect or advertise the new page from the current homepage yet.
- During review, exclude the preview from search indexing and the public sitemap. Give it its own metadata; do not switch the root's canonical URL. An unlisted/noindex page is not private access control.
- Kiki's approval of a preview is followed by an explicit promotion decision. Publishing this assessment to `main` does not deploy or promote the page.

## Loading sequence

1. **Serve the marketing page.** Send complete HTML with the hero, product overview, account links, and Enter the Verse. Load essential CSS/fonts and the appropriately sized poster. Core content and navigation work without JavaScript or a GPU.
2. **Wait for entry.** Do not request a GPU adapter, download Grid/chat/Kiln/Wasm/game packages, join the relay, or request microphone access while the visitor reads. A focus/hover enhancement can fetch a small manifest only if measurements justify it; avoid unconditional game preloads. Browser [preload declarations request early downloads](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/link), so they must respect this boundary too.
3. **Begin the game-style transition on click.** Open the preview experience, show the loading presentation, check capabilities, and fetch only the planetary first-view resources. Expose retry and a way back. Show actual known stages; percentages require a meaningful measured total.
4. **Show the universe selector, then login.** Render the connected-system/planetary overview described above before the Grid-style login card, as Kiki specifies. Reuse the existing account flow with an allowlisted return address and restore the scene and selected destination after an external provider redirect. Preserve context without promising that an OAuth provider itself renders inside a card.
5. **Reveal Explore or Tutorial.** Load the Haunted House and editing dependencies when needed, not all worlds during planetary startup. In the runtime, keep the simulation/renderer lifetime separate from overlay visibility so opening Edit does not reset the player.
6. **Load personal-world resources later.** Planet allocation, setup, and invitations follow tutorial completion. They need real authenticated services and persistence; a planet image or generated local scene alone does not supply those capabilities.
7. **Leave cleanly.** Returning to marketing releases game GPU resources, workers, relay subscriptions, and listening sessions. Moving from the house to the planetary map can retain bounded caches within the game session. A still-visible marketing background should not run a second renderer.

WebGPU requires a secure context and is not available on every browser/device configuration. Keep the marketing page useful when it is unavailable; explain the experience's support requirements at entry and offer a static preview or return. Do not add a second game renderer merely to make marketing universally readable. [WebGPU requirements](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API).

## Iteration and rendering performance

- **Marketing speed comes from small DOM/CSS/assets and delivery boundaries.** A GPU-rendered heading or all-Wasm landing page adds startup work without improving the core job. CSS/SVG can provide the black/white/glow direction.
- **Game speed comes from asset budgets and renderer design.** Direct WebGPU gives control, but does not guarantee AAA quality or faster frames than equivalent Rust/wgpu work. Use the existing worker/runtime direction; profile scene preparation, draw submission, shader cost, and residency separately.
- **Authoring speed comes from independent updates.** Copy/CSS should not rebuild the game. A material edit should not rebuild marketing or reinitialize player state. Use revisioned assets and keep the last working result during failed edits.
- **Keep DOM controls responsive.** Use workers where supported and useful for render/compile work, with coarse messages. Workers do not grant account isolation; user-authored games still need the separate-origin and bounded-command model in the renderer audit.
- **Reuse the deployment improvements already built.** The October 21 assessments described a monolithic build problem; the later deployment record documents content/module/image paths. Register the new page's assets and copy in those paths instead of treating the old full rebuild as inevitable. Production live-content enablement must be checked separately; staging behavior is not proof that production follows the same manifest.

## Smallest implementation after this assessment

1. Build the isolated marketing preview: current identity/account treatment, monochrome hero, verified product information, and the Verse entry. Do not imply unfinished products or earning features are available.
2. Add the universe/world selector and real login continuity, using the Episode 289 graphic and Expanse-like spatial direction above. Keep the central OpenAgents planet locked; do not invent live node/planet relationships or counts without data.
3. Integrate the real Haunted House tutorial and its editing/publish boundaries. Where features are absent, keep the page a clearly identified review build; do not expose dead production controls.
4. Add the planet award, basic voice setup, walking, invitations, and chat only as working capabilities. Kiki's stated stopping point precedes buildings, levels, and narrative tooling.

These are sequencing recommendations, not new assignments or launch dates. This document does not create implementation issues or alter the other agent's work.

## Checks before any future homepage promotion

- Cold-load `/new-lander` with JavaScript disabled and WebGPU unavailable: the product suite and navigation remain readable and usable.
- Inspect the network: zero game, chat, Kiln, relay, or microphone startup before entry; correct first-view assets afterward. Record transferred bytes rather than guessing from HTML size.
- Verify desktop/mobile layout, keyboard focus, reduced motion, retry/back paths, real login return, and cleanup after leaving the game. Use isolated browser profiles and local or dedicated test accounts.
- Review the first post-entry view with Kiki: connected systems and finite planets read as a navigable universe; the central company planet is locked; destination labels and the Explore/tutorial choice remain clear. Returning from a world reaches the same map, including the creator's awarded planet when available.
- Target marketing LCP at or below 2.5 seconds, INP at or below 200 milliseconds, and CLS at or below 0.1 at the 75th percentile, segmented by device. These are [Core Web Vitals targets](https://web.dev/articles/vitals), not measurements from this assessment; a single lab run cannot establish field percentiles.
- Measure game entry separately: click-to-loader, download, compile, first rendered planet, login readiness, tutorial entry, and edit-to-visible time. State hardware/browser/resolution and cold/warm cache conditions.
- Confirm `/` and old routes have unchanged behavior, preview metadata stays isolated, and Kiki reviews the actual flow. Root promotion is a separate requested change.

## Initial implementation scope

The owner subsequently requested implementation of the marketing homepage and universe selector. The Rust pages use original inline SVG/CSS on marketing, then load a generated browser module of about 102 KiB only after entry. The selector draws original lit planets, orbits, stars, and connecting paths with direct WebGPU; it supports destination selection, camera orbit/zoom, keyboard controls, and a still-view fallback. Future planets are labeled as future destinations; this curated preview does not claim live network membership, ownership, or population counts.

The central OpenAgents world stays locked. Blue Rush links to the existing studio page; Haunted House entry appears only when the server has a house build or hosted tenant. Configured account services use the existing login route with a return to the selector. The full voice tutorial, publishing, and personal planet award remain outside this implementation. Source, bundle regeneration, and the isolated-browser check are in [the selector package](../../apps/verse-selector/README.md).

Verification: TypeScript checks, two geometry/projection tests, the two focused `new_lander` Rust tests, and isolated Chrome checks for real WebGPU pixels, navigation, camera controls, mobile layout, no-GPU fallback, and script-free marketing pass. The broader route-owner scanner reports an existing unreadable `PATH` declaration in unchanged `pages/pylon_work.rs`; the new routes pass their direct ownership checks. No production account or public world was created.
