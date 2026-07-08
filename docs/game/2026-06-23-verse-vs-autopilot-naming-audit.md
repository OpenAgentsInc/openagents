# Verse vs Autopilot: Desktop App Naming Audit

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-23

Status: opinionated canon/brand audit (docs-only; proposes, does not execute)

Scope: whether to rename the OpenAgents **desktop app** from "Autopilot" to
**"Verse"**, reserving "Autopilot" as the **business/agent** terminology.

This audit changes no code and no user-facing copy. The workspace rule against
changing user-facing copy without explicit owner direction is binding
(`openagents/CLAUDE.md`, "Do not change homepage copy, marketing copy,
onboarding copy, CTA text, or other user-facing copy unless the user explicitly
asks"). What follows is a recommendation and a migration map for an owner
decision, not a rename.

---

## Thesis (read this first)

The word **"Autopilot"** has, in the canon, always meant the thing that *does
the work*: the autonomous AI worker, the coding agent, the company operating
system that "sees the business, runs approved work, proves what happened"
(root-workspace `docs/autopilot/2026-05-07-autopilot-ceo-system-spec.md`). But
the desktop app shipping under that name in `apps/autopilot-desktop` has, over
the last weeks, become something categorically different: a **3D multiplayer
world** — Pylons as serving nodes, avatars, Khala crackling-arc energy,
receipt-backed settlement beams — that the codebase, its tests, its docs, and a
2026-06-20 owner directive already call **the Verse**. The app name now
*under-describes the product*, and the product name **"Autopilot"** is
simultaneously *overloaded* across five sibling repos. My recommendation is
**Option (a): rename the desktop app to "Verse"** — "Autopilot is what works;
the Verse is where you watch it work and where the machine-work economy lives" —
and **reserve "Autopilot" for the agent/business layer** (the worker, the coding
agent, the CEO/company-OS surface customers hire). The conceptual split is
clean, the codebase has already done ~80% of the renaming for us, and the main
cost is bundle-id/release-channel churn, not conceptual confusion. I'd hold the
public-copy flip until an owner signs off, but the internal/product direction
should commit to Verse now.

---

## Part 1 — What "Autopilot" means in canon

The canon is unusually explicit here. Across the private vision repo (`alpha`)
and the root-workspace business specs, "Autopilot" is consistently the
**autonomous worker / agent / business-operating layer** — never primarily a
"3D world" or "place."

### 1.1 Autopilot = the worker-shaped product surface

`alpha/autopilot/README.md` (lines 17-38) is the clearest single statement:

> "Autopilot should be treated as a **worker-shaped product surface**, not mainly
> as chat, tool use, or a workflow builder. ... Autopilot should look like a
> manager-facing remote worker product, while the architecture underneath stays
> honest about execution truth, capability grants, receipts, routing, and
> economic closure."

Its bottom line (lines 195-201):

> "**Autopilot should be the product surface for world-mounted machine workers.**
> Those workers should execute as host-owned bounded graphs of processors."

### 1.2 Autopilot = the company/CEO operating system

The root-workspace business specs push "Autopilot" all the way up to a
*business operating system*:

- `docs/autopilot/2026-05-07-autopilot-ceo-system-spec.md` (lines 13-30):
  > "a way to put the current OpenAgents operation on Autopilot by giving the
  > CEO one operating surface over the whole business ... Autopilot sees the
  > business. Autopilot tracks what matters. Autopilot proposes decisions.
  > **Autopilot runs approved work.** Autopilot proves what happened."

- `docs/autopilot/2026-04-27-autopilot-ai-company-os-spec.md` frames Autopilot
  as "the user-facing company OS ... command center ... work and agent
  supervision UI."

- `docs/autopilot/2026-05-03-autopilot-managed-and-scheduled-agents-vision-audit.md`
  (lines 14-30) records the **brand decision** directly:
  > "`Autopilot` should remain the only primary user-facing abstraction.
  > Scheduled agents, managed agents, Probe sessions, Pylon assignments,
  > Forge-style work orders, GCP workers, Daytona sandboxes, Psionic execution,
  > and Codex backends are implementation and operator concepts ... the normal
  > buyer and user should experience one product: Autopilot."

That last quote is the crux for this audit: the canon already insists Autopilot
is the *abstraction customers hire*, the worker brand — explicitly **not** a
list of surfaces or implementation homes. That is a strong argument for letting
the *place* (the desktop world) carry a different name so "Autopilot" stays the
clean name for the *worker*.

### 1.3 Autopilot in the roadmap = the remote-worker entry point

`alpha/ROADMAP.md` repeatedly treats Autopilot as a worker lifecycle, not a
world:

- Phase 3 is literally "Autopilot As World-Mounted Remote Worker" (line 639),
  defining "worker identity and role schema ... assignment lifecycle with
  plans, checkpoints, resumability, and deliverable contracts" (lines 644-651).
- The DRI matrix names `apps/autopilot-desktop` as the *product DRI* for the
  compute/data/labor/etc. market launches (lines 180-182) — i.e. the desktop is
  cast as the **client/entry point to the markets**, not as a product in itself.
- The launch calendar (lines 74-82): "Autopilot v0.1 | desktop entry point to
  the markets, with built-in wallet, built-in Nostr keypair."

So even where the roadmap couples "Autopilot" to the desktop, it does so as
"the *entry point*" — a worker/market client — which is exactly the role that a
distinctly-named world app ("the Verse") would inherit while "Autopilot" floats
up to the worker/business concept.

### 1.4 "Autopilot" is *already overloaded* — the naming is under stress

The workspace contract (`CLAUDE.md` "Workspace Model" / "Inferred Program")
shows "Autopilot" stretched across **five sibling repos** with different
meanings:

- `autopilot-deprecated/` — legacy React/Vite Autopilot source.
- `autopilot2/` — Cloudflare Workers TanStack Start foundation.
- `autopilot3/` — Convex-backed TanStack Start foundation (WorkOS AuthKit).
- `autopilot4-deprecated/` — deprecated Rust/Maud Autopilot.
- `autopilot-omega/` — the active `openagents.com` product surface.

Plus, inside `openagents`: `apps/autopilot-desktop/` (the world app) **and**
`clients/khala-ios/AutopilotRemoteControl` **and** the `apps/openagents.com`
"Autopilot, Forum, Sites" product surface. One word is doing the work of: a
deprecated web app (×2), a Convex foundation, a Cloudflare foundation, the live
`.com` product, a desktop world, a mobile remote, and the abstract worker
concept. **This is the smell of an overloaded brand.** Splitting off the
*world/place* sense ("Verse") relieves the overload without inventing a second
worker name.

---

## Part 2 — What the desktop app has actually become (the Verse)

The evidence that `apps/autopilot-desktop` is no longer a "business autopilot
dashboard" but a **3D multiplayer world** is overwhelming and already lives in
the repo.

### 2.1 An explicit owner directive: the Verse is the default surface

`apps/autopilot-desktop/AGENTS.md` (line 34, dated 2026-06-20):

> "**Default surface: the Verse (owner directive, 2026-06-20).** The app
> launches to the Verse: the chat/world surface centered on Pylons, Tassadar,
> and one chat bar. ... `model.verseEnabled` default-on and the launch build
> resolving the Verse bundle on unless an explicit fallback/debug kill switch is
> set. Fresh first paint must not show the shell target tabs, `Claude Code`, or
> `Codex`."

And the old dashboard is explicitly demoted (lines 42-46):

> "**The fallback shell is KEPT, just demoted.** ... Do not make it the real app
> entry again without a new owner directive."

The thing the app *opens to* — the product's first impression — is the Verse.
The "Autopilot dashboard" is now a `Cmd-K` advanced/debug surface.

### 2.2 The app renders a world, not a dashboard

`apps/autopilot-desktop/src/ui/view.ts` — `verseSceneVisualization(model)`
(≈lines 7091-7169) composes a layered 3D scene:

- `pylonNetworkVisualizationOptions(...)` — Pylons as positioned 3D nodes.
- `withVerseTrainingLayer(...)` — the central Tassadar run core.
- `withChatWorldMultiplayerLayer(...)` with a `localAvatarRef` — avatars.
- `withVerseKhalaEffectLayer(...)` — "EPIC #6017: the LOCAL Khala crackling-arc
  effect."
- `withVerseSpawnedSceneLayer(...)` — portal/isolated scenes.

Supporting world files (a representative slice of ~31 `verse-*`/world-named
source files):

- `src/shared/verse-khala-effect.ts` — turns a Khala turn receipt + avatar
  position into "a **crackling-arc layer**" rendered by the shared three-effect
  `trainingRunView`.
- `src/shared/chat-world-visualization.ts` — "PYLONS — live nodes ring the
  'network' hub" and "PAYMENTS — each PaymentParticle becomes an EVIDENCE-BOUND
  beam from the actor pylon → target pylon ... with a **settlement burst** on the
  target."
- `src/shared/chat-world-multiplayer.ts` — `ChatWorldStationRow`,
  `ChatWorldAvatarRow`, `ChatWorldAvatarPositionRow` (x/y/z/yaw/animation),
  `ChatWorldLocalChatMessageRow` (proximity radius). This is **multiplayer world
  state**, not dashboard widgets.

The render stack is `@openagentsinc/three-effect` (Three.js 0.184) +
`@openagentsinc/world-client` / `@openagentsinc/world-contract`, not DOM/CSS
HUD cards (`apps/autopilot-desktop/package.json`).

### 2.3 The product narrative is already a "world"

The `docs/game/` directory is an entire world-building program for the desktop
app:

- `2026-06-16-spatial-hud-agentic-mmo-wow-direction.md`
- `2026-06-17-agent-avatar-proximity-chatter-world-plan.md`
- `2026-06-17-openagents-world-asset-catalog.md`
- `2026-06-21-mmo-characters-per-account-verse-presence.md`
- `2026-06-21-verse-scene-graph-vs-react-three-fiber-audit.md`
- `2026-06-22-cloudflare-world-actor-command-authority-model.md`
- `2026-06-22-talk-to-khala-from-verse-audit.md`
- a `woc/` ("World of ClaudeCraft") MMO-reference subdirectory.

`docs/transcripts/README.md` (Episode 240) describes "a visual walkthrough of
the **walkable 3D Tassadar run board** and **multiplayer/Verse direction**."

`docs/khala/khala.md` ties the world metaphor to the inference network:
"the **Khala is the psionic link that joins many minds into one** ... settling
verified work in Bitcoin."

### 2.4 The codebase has already half-renamed itself

`package.json` scripts: `smoke:verse-launch`, `proof:verse-coding-overlay`,
`proof:verse-arc`, `test:verse-launch`, `smoke:forum-verse-reflection`. The
`openagents/CLAUDE.md` repo-layout section enshrines `apps/openagents-world`,
`packages/world-client`, `packages/world-contract`, and "live Verse world
projection" as first-class. **"Verse" is already the working name everywhere
except the app's title bar and bundle id.**

### 2.5 The one stubborn fact: the shipped identity still says "Autopilot"

`apps/autopilot-desktop/electrobun.config.ts`:

```ts
app: {
  name: "Autopilot",
  identifier: "com.openagents.autopilot.desktop",
  version: "1.0.1"
}
release: { baseUrl: "https://updates.openagents.com/desktop" }
```

Mobile sibling `clients/khala-ios/AutopilotRemoteControl/app.config.ts`:
`name: "Autopilot"`, `bundleIdentifier: "com.openagents.autopilot-mobile"`.

So the *content* is a Verse; the *label and distribution identity* are
Autopilot. That mismatch is the whole question.

---

## Part 3 — The core question and the clean split

> Is **Verse** the right name for the desktop *world app* (the **place**), with
> **Autopilot** reserved for the *agent/business* concept (the **worker** + the
> business-operating terminology customers hire)?

My answer: **yes**, and the canon almost writes the split for us.

**Autopilot is what works. The Verse is where you watch it work / where the
machine-work economy lives.**

- **Autopilot** = the autonomous worker, the coding agent, the CEO/company-OS
  surface, the thing a customer "puts their business on" and the thing that
  "runs approved work and proves what happened." It is a *capability and a
  brand of labor*. This is exactly what `alpha/autopilot/README.md`,
  `docs/autopilot/2026-05-07-autopilot-ceo-system-spec.md`, and the
  managed-agents vision audit already say Autopilot *is*.
- **Verse** = the 3D, multiplayer, walkable *world* where that work is
  *visualized, watched, and inhabited*: Pylons (serving/compute nodes), avatars
  (operators and agents), Khala energy (inference flowing through the network),
  settlement beams/bursts (Bitcoin moving on accepted work), the Tassadar run
  core. It is a *place and a spectacle*, the front door of the economy.

This mapping is conceptually crisp because "Autopilot" answers *what is being
done and by whom*, while "Verse" answers *where it happens and how you see it*.
They don't compete; they compose. You can truthfully say: "Hire **Autopilot**;
watch it work in the **Verse**." That is a better story than "Autopilot the app
contains an Autopilot world rendered by Autopilot workers," which is where the
current single-name overload lands you.

It also resolves a real product-truth hazard. The desktop's first paint is
explicitly *not* the worker dashboard anymore (it's the world), yet it carries
the worker's name. New users opening "Autopilot" and seeing a 3D MMO-style world
get a name/experience mismatch. Renaming the app to "Verse" makes the title
match the first paint, and frees "Autopilot" to mean the worker you can also
reach (behind Cmd-K, in `apps/openagents.com`, on mobile remote, via API).

---

## Part 4 — Options and tradeoffs

### Option (a) — Rename desktop → **Verse**; reserve "Autopilot" for the worker/business term *(recommended)*

- **Pros:** Title matches the shipped first paint and ~31 `verse-*` source
  files; relieves the five-repo "Autopilot" overload; gives the place its own
  memorable identity; lets "Autopilot" stay the clean worker/CEO-OS brand the
  canon already mandates; clarifies the customer story ("hire Autopilot, watch
  it in the Verse").
- **Cons:** Bundle-id and OTA-channel churn (see Part 5); "Autopilot" has
  shipped brand/release equity at v1.0.1; "Verse" carries metaverse baggage and
  is a crowded, low-distinctiveness word for SEO.

### Option (b) — Keep "Autopilot Desktop"; "Verse" stays a *mode/surface inside it*

- **Pros:** Zero migration cost; bundle id, release tags, OTA feed, mobile
  remote untouched; preserves the existing release equity; matches the literal
  current state (`model.verseEnabled` is a toggle, the shell is demoted-not-
  deleted).
- **Cons:** Perpetuates the exact mismatch this audit is about — the app *opens
  to* the Verse but is *titled* Autopilot; keeps "Autopilot" overloaded as both
  worker and place; makes the customer story muddier ("Autopilot is a worker,
  and also an app, that opens to a world called the Verse, which is a mode").
  This is the path of least resistance and the most conceptual debt.

### Option (c) — Something else: "OpenAgents" (or "OpenAgents Verse") as the world app; Autopilot purely as the agent

- **Pros:** Leans on the strongest existing brand (OpenAgents); the world *is*
  the OpenAgents economy; avoids minting "Verse" as a standalone consumer brand
  if metaverse baggage is judged disqualifying.
- **Cons:** "OpenAgents" is the *company/everything* brand; using it as the
  desktop-app title under-differentiates the world from the org, the website,
  and the markets; loses the crisp place-noun the codebase already uses
  internally ("the Verse"). A hybrid "OpenAgents Verse" (org-qualified product
  name, "Verse" as the short name) is the reasonable middle and my fallback if
  (a)'s standalone "Verse" is rejected on brand grounds.

**Recommendation: (a)**, with **(c)'s hybrid "OpenAgents Verse" as the public
brand and "Verse" as the short/internal name** if the owner wants org-anchoring.
Avoid **(b)** as a *resting* state: it's fine as the literal current
implementation (a toggle), but it should not be the long-term answer, because it
banks conceptual debt every release.

---

## Part 5 — Migration scope if we rename (cheap vs invasive)

This audit *proposes*; it does not execute. Per `openagents/CLAUDE.md`, no
user-facing copy flips without explicit owner direction. The map below is the
work an owner-approved rename would entail.

### Cheap / low-risk (internal, non-distribution)

- **Docs and internal naming:** `docs/game/` already uses "Verse"; minimal
  delta. The `verse-*` source files and `package.json` test scripts already
  carry the name — effectively *done*.
- **`apps/autopilot-desktop/AGENTS.md`:** already declares the Verse the default
  surface; a rename only tightens wording.
- **In-app display strings / window title:** the `name:` field in
  `electrobun.config.ts` is a one-line change *for the title*, decoupled from
  the bundle id.

### Medium / coordinated

- **Directory rename `apps/autopilot-desktop` → `apps/verse-desktop`** (or
  similar): mechanical but touches imports, scripts, CI paths, and
  `docs/DEPLOYMENT.md` references. Defer or do as a clean dedicated change; it's
  invasive only because of breadth, not difficulty.
- **GitHub release-tag convention / RC + stable channel labels:** new tag prefix
  for clarity; old tags stay valid history.
- **Marketing / promise copy:** route through `docs/promises/` (the repo's
  copy-gate home) before broadening any public "Verse" copy — and only on
  explicit owner sign-off.

### Invasive / needs a real migration plan (the actual cost)

- **Bundle id `com.openagents.autopilot.desktop`:** changing the identifier
  breaks OTA continuity and code-signing/notarization identity. On macOS a new
  identifier is effectively a new app for installed users. Practical answer:
  **keep the existing `com.openagents.autopilot.desktop` identifier** for the
  installed base and only change the **display name** to "Verse" — a name change
  does *not* require an identifier change. A fresh `com.openagents.verse.desktop`
  identifier would be a clean break with a forced reinstall; only do that with a
  deliberate migration/OTA-bridge plan.
- **OTA feed `updates.openagents.com/desktop`:** if the identifier stays, the
  feed and signing key (`apps/oa-updates`, the ed25519 release key + Apple
  Developer ID `HQWSG26L43`) keep working untouched. If the identifier changes,
  you need a parallel feed and a cutover. **Strong argument for "rename the
  display name, keep the identifier."**
- **Mobile `clients/khala-ios/AutopilotRemoteControl`:** it remote-controls the
  *node/worker*, not the world — so it arguably *should stay "Autopilot"*
  (it's a worker-control surface, which is precisely the term we're reserving).
  This is a feature of the split, not a cost: the mobile remote keeps the
  worker name because it controls the worker; the desktop world takes "Verse."
- **Relationship to `autopilot-omega` / `openagents.com`:** the `.com` product
  surface ("Autopilot, Forum, Sites" per `openagents/CLAUDE.md`) is *also* a
  worker/business surface, so it too keeps "Autopilot." Renaming the desktop
  world to "Verse" actually *clarifies* this seam: `.com` and mobile = Autopilot
  (the worker/business), desktop = Verse (the world). The only naming hygiene
  needed is to stop using "Autopilot" to mean "the desktop app."

**Net:** the genuinely cheap, high-value move is **change the display name to
"Verse" while keeping the bundle id and OTA feed**, plus tighten docs. The
directory rename and any identifier change are separable, larger, and optional.

---

## Part 6 — Risks and counterarguments

1. **"Autopilot" has real brand/release equity.** It's shipped at v1.0.1, it's
   the launch-calendar name (`alpha/ROADMAP.md`), and it's the term in investor/
   roadmap language. Counter: we're not *retiring* Autopilot — we're *promoting*
   it from "an app" to "the worker/business brand," which is a stronger, more
   durable position than "the title of one of several clients."

2. **"Verse" is crowded and metaverse-tainted.** It evokes Meta-era metaverse
   hype and is a weak, generic SEO token. Counter: in our context "Verse" reads
   as *the place where the machine-work economy lives*, grounded in real
   receipts, Pylons, and Bitcoin settlement — not avatars-for-their-own-sake.
   Still, this is the strongest argument for the **(c) hybrid "OpenAgents
   Verse"**, which inherits OpenAgents' distinctiveness while keeping the short
   place-noun.

3. **Churn risk during a live launch window.** The system is mid-launch
   (markets cadence, promise assault). A rename mid-flight risks confusing
   contributors and users. Counter: do it as a *display-name + docs* change
   (cheap, reversible) and **gate the public copy flip on owner sign-off and a
   quieter window**; keep the bundle id stable so there is zero install-base
   disruption.

4. **Two names = two things to explain.** A skeptic says one name is simpler.
   Counter: we already *have* two things (a worker and a world) wearing one
   name, which is the harder thing to explain. The split reduces total
   explanation cost.

5. **Contributors vs customers may diverge.** Contributors (Pylon operators)
   live in the world; customers hire the worker. Counter: that's an argument
   *for* the split — contributors get "the Verse" (their home/spectacle),
   customers get "Autopilot" (the labor they buy). Each audience gets the name
   that matches its relationship to the system.

---

## Part 7 — Recommendation

**Adopt Option (a): rename the desktop *world app* to "Verse," and reserve
"Autopilot" for the agent/business concept** (the autonomous worker, the coding
agent, the CEO/company operating system — the thing customers hire and the
thing that runs and proves work). Phrase the split as: **"Autopilot is what
works; the Verse is where you watch it work and where the machine-work economy
lives."**

Concretely:

1. **Commit the internal/product direction to "Verse" for the desktop world
   now.** The codebase, tests, `docs/game/`, and the 2026-06-20 owner directive
   already do this; ratify it in naming.
2. **Execute the cheap half immediately on owner approval:** change the Electrobun
   **display name** to "Verse" while **keeping the `com.openagents.autopilot.desktop`
   bundle id and the existing OTA feed/signing identity** (zero install-base
   disruption). Tighten `apps/autopilot-desktop/AGENTS.md` and docs.
3. **Keep "Autopilot" on the worker/business surfaces:** `apps/openagents.com`
   ("Autopilot, Forum, Sites"), the mobile `AutopilotRemoteControl` (it controls
   the *worker*), `autopilot-omega`, and all CEO/company-OS framing. Stop using
   "Autopilot" to mean "the desktop app."
4. **Defer the invasive bits** (directory rename `apps/autopilot-desktop →
   apps/verse-desktop`, any bundle-id change, release-tag prefix) to a dedicated,
   owner-approved migration change — not the public-copy flip.
5. **If standalone "Verse" is judged too crowded/metaverse-tainted, fall back to
   the (c) hybrid "OpenAgents Verse"** as the public brand, with "Verse" as the
   short/internal name. Do **not** settle on Option (b) (Verse-as-mode-only) as
   the permanent answer; it banks conceptual debt every release.
6. **Route any public copy change through `docs/promises/` and explicit owner
   sign-off**, per `openagents/CLAUDE.md`. This audit proposes; the owner
   decides the flip.

### Honest ambiguities

- **The canon never names the desktop world "Autopilot" on purpose** — it
  attaches "Autopilot" to the *worker* and casts the desktop as the *entry
  point/client* (`alpha/ROADMAP.md` DRI matrix). So the rename is *clarifying an
  existing intent*, not overturning canon. But the roadmap *does* lean on
  "Autopilot v0.1 desktop" as a phrase, so there is real shipped coupling to
  unwind in language.
- **"Verse" is not yet a sanctioned public brand** — it's an internal/working
  name and an owner-directive default surface, not (as far as this audit found)
  an owner-ratified *consumer brand*. Treating it as the public app name is the
  decision this audit is asking the owner to make; the codebase has made it de
  facto, but not de jure.
- **Mobile and `.com`** keeping "Autopilot" is my read of the split (they're
  worker/control surfaces), but an owner could reasonably want a single
  unified consumer name. The audit's split is principled, not the only option.

---

## Sources

- `/Users/christopherdavid/work/CLAUDE.md` — workspace contract; the five
  overloaded `autopilot*` repos and the "Inferred Program."
- `/Users/christopherdavid/work/alpha/autopilot/README.md` — Autopilot as
  worker-shaped product surface.
- `/Users/christopherdavid/work/alpha/ROADMAP.md` — Autopilot as remote-worker
  entry point; DRI matrix; launch calendar.
- `/Users/christopherdavid/work/docs/autopilot/2026-05-07-autopilot-ceo-system-spec.md`
  — Autopilot as company/CEO operating system.
- `/Users/christopherdavid/work/docs/autopilot/2026-05-03-autopilot-managed-and-scheduled-agents-vision-audit.md`
  — "`Autopilot` should remain the only primary user-facing abstraction."
- `/Users/christopherdavid/work/docs/autopilot/2026-04-27-autopilot-ai-company-os-spec.md`
  — Autopilot as the user-facing company OS.
- `apps/autopilot-desktop/AGENTS.md` — 2026-06-20 owner directive: the Verse is
  the default surface; the shell is demoted.
- `apps/autopilot-desktop/electrobun.config.ts` — `name: "Autopilot"`,
  `identifier: "com.openagents.autopilot.desktop"`, OTA `baseUrl`.
- `apps/autopilot-desktop/src/ui/view.ts` — `verseSceneVisualization`.
- `apps/autopilot-desktop/src/shared/verse-khala-effect.ts`,
  `chat-world-visualization.ts`, `chat-world-multiplayer.ts` — Pylons, avatars,
  Khala crackling arcs, settlement beams, multiplayer world state.
- `apps/autopilot-desktop/package.json` — `verse-*` test scripts; three-effect /
  world-client / world-contract deps.
- `clients/khala-ios/AutopilotRemoteControl/app.config.ts` — `name: "Autopilot"`,
  `bundleIdentifier: "com.openagents.autopilot-mobile"`.
- `openagents/CLAUDE.md` — repo layout (`apps/openagents-world`,
  `packages/world-client`/`world-contract`, "live Verse world projection");
  `apps/openagents.com` = "Autopilot, Forum, Sites"; copy-change and
  user-facing-copy rules; `docs/promises/` copy gate.
- `docs/game/*` — the world-building program (spatial HUD/MMO direction, avatar
  proximity, asset catalog, per-account Verse presence, Cloudflare world authority).
- `docs/khala/khala.md` — Khala as the psionic link / world metaphor.
- `docs/transcripts/README.md` — Episode 240 walkable 3D / multiplayer Verse
  direction.
