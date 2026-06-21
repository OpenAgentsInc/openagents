# Verse hands-off Pylon/Tassadar audit

- Date: 2026-06-20
- Scope: Autopilot Desktop, the Verse/game-world surface, Pylon network state,
  Tassadar training state, and first-run launch posture.
- Primary sources: current `main`, open GitHub issues #5523, #5526, #5527,
  #5528, #5556, #5510, #5512, #5520, #5521, `docs/launch/`, `docs/game/`,
  `apps/autopilot-desktop/`, and `apps/openagents-world-spacetimedb/`.

This is a status and launch-path audit. It intentionally treats the open issues
and current source as the live truth. Older launch/game audit docs are context,
not authority.

## Implementation log

- 2026-06-20: #5819 landed the P0 home change. The real Desktop runtime now
  starts on the Chat/Verse pane, the Verse scene/payment/character-creation
  bundle defaults on with an explicit kill switch, onboarding warms on first
  paint, and focused tests assert no `Claude Code` or `Codex` target tabs on
  the default surface.
- 2026-06-20: #5820 demoted the advanced coding chrome from first paint. The
  default Chat/Verse root now renders without sidebar, status HUD, shell target
  tabs, Spawn/Sessions/Swarm/Deploy labels, or repo/worktree/cloud-code copy;
  Cmd-K/Advanced opens the command palette and the explicit shell open path
  lands on the Code composer.
- 2026-06-20: #5821 split default Verse chat from the advanced
  Blueprint/session path. Visible Send now dispatches `RespondToVerseInput`;
  the Bun host builds a bounded public Tassadar context pack from
  `/api/public/tassadar-run-summary`, `/api/public/pylon-stats`,
  `/api/public/activity-timeline`, and product-promise state before calling the
  OpenAgents model gateway. `SpawnChatTurn` remains only on the explicit
  Blueprint command-palette action.
- 2026-06-20: #5822 put the Tassadar training run into the default Verse scene.
  The chat-world visualization now rewrites the center node into a Tassadar run
  core, adds public-ref-bearing lifecycle stages for assignment, trace/workload,
  exact replay, verdict, settlement, recipient confirmation, and blockers, and
  pins training motion to `motionPolicy.evidence = required`. First paint also
  warms the public training run and promise-gate projections.
- 2026-06-20: #5823 bound the local Pylon identity into the default Verse as
  "My Pylon Base." The Desktop identity-choice projection now preserves the
  public `pylonRef`, first paint warms identity/onboarding/operator readiness,
  and `projectPylonBase` splits local readiness, mana, blockers, and
  receipt-backed settled sats from fleet-wide Pylon growth.
- 2026-06-20: #5824 resolved payment endpoints against public world positions.
  Payment particles now prefer SpacetimeDB Pylon station and avatar coordinates
  when a public `ChatWorldMultiplayerProjection` is present, while unresolved
  endpoints stay visibly fallback-labeled and receipt-backed.
- 2026-06-20: #5825 connected Autopilot Desktop to the public
  `openagents-world` SpacetimeDB projection. The Desktop webview now has a
  flag-gated generated-binding client with reconnect/backoff, token persistence,
  public row projection, region join, safe avatar-position write planning, and
  live station/avatar entities composed into the Verse scene.
- 2026-06-21: #5909 added the first Verse bulletin board as a real world item,
  backed by the server-owned public Tassadar summary. `/api/public/tassadar-run-summary`
  now returns a plain-language `bulletin` envelope with headline, board lines,
  metrics, latest activity, and public source refs. Autopilot Desktop fetches
  that summary with training runs, maps it into a `bulletin_board` world item in
  the actual Verse scene, and opens a concise overlay only when the player walks
  up to it. The reusable Three/Foldkit primitive landed in `three-effect` #11.
- 2026-06-21: the old web `/tassadar` live scene was retired so it cannot be
  mistaken for the Verse again. Plain `/tassadar` now renders a compact retired
  notice pointing at Autopilot Desktop Verse, the public summary API, and the
  proof replay route. The self-fetching legacy `oa-tassadar-run` custom element
  and its page-wiring test were deleted; `/tassadar/replay/...` remains live.

## Executive read

The Verse is conceptually correct and now has real implementation seams. After
#5819-#5825 it is the default first surface, includes Tassadar training state,
shows a distinct local Pylon base, resolves receipt-backed payment motion to
real station/avatar positions when public world rows exist, and subscribes to
the public SpacetimeDB world projection. It is not yet the complete hands-off
experience the owner is asking for.

What exists:

- A flag-gated 3D world behind the Desktop chat pane.
- Fresh Desktop runtime startup now lands on that Chat/Verse surface by
  default.
- Live Pylon projection from `/api/public/pylon-stats`.
- Evidence-bound payment particle projection from
  `/api/public/activity-timeline` and its SSE stream.
- Payment endpoint resolution that prefers public SpacetimeDB Pylon
  station/avatar positions and labels unresolved endpoints as fallback rather
  than claiming a fake world location.
- A character-creation overlay projection tied to real onboarding status.
- A training scene and Training Live pane that already render a
  `three-effect` run visualization.
- A default Verse scene with a central Tassadar run core and public-ref-backed
  training/benchmark stages.
- A distinct "My Pylon Base" layer in the Verse scene, derived from public-safe
  local identity, onboarding, training-operator readiness, live Pylon state, and
  receipt-backed payment particles.
- A SpacetimeDB `openagents-world` module with public projection tables,
  interaction tables, service-only projection reducers, and browser-safe
  interaction reducers.
- A Desktop SpacetimeDB client that subscribes to public world rows, joins the
  active Tassadar region with a browser-safe display identity, feeds live
  stations/avatars into the Verse scene, and falls back to Worker/Pylon/activity
  feeds when unavailable.
- Default Verse chat with Tassadar/OpenAgents model responses over public
  Pylon/training context.
- Blueprint/Tassadar chat-step scaffolding that can show semantic signature
  routing and exact-replay verdicts.

What is wrong for the requested product direction:

- The fallback shell still exposes `Current`, `Claude Code`, and `Codex` when
  explicitly opened. Those controls are no longer first paint, but they still
  need further advanced/developer framing.
- The explicit Blueprint chat command still spawns a coding session through
  the selected coding adapter. That is now advanced behavior, but it still
  needs careful framing so it does not become the mental model of the app.
- The Verse is still controlled by build flags and lives behind the Chat pane,
  not as the default mental model of the app.
- The default Verse training layer is still fed by Desktop public Worker
  projections, not live SpacetimeDB training-run rows. This is intentional until
  a later authority change promotes SpacetimeDB beyond public scene enrichment.
- The live SpacetimeDB client renders public stations/avatars/chat, but Desktop
  does not yet have a user movement UI loop that continuously publishes guarded
  avatar positions.
- The open issue backlog still frames DE-3 as "Autopilot product surface -
  coding agent." The owner's current launch intent is different: Pylons,
  Tassadar, training visibility, and an autopilot world first; coding controls
  should be available but no longer own first paint.

The next cut should make the app open directly into the Verse: your Pylon as
base, Tassadar as the center training run/model surface, live Pylons and sats as
world events, and one chat bar that talks to Tassadar/OpenAgents. Code, cloud
sessions, Codex, Claude, swarm, repo/worktree, deploy, and shell controls move
behind an advanced Code/Supervise path.

## Product target

Hands-off means:

1. The user opens Autopilot Desktop.
2. The first surface is the Verse, not a blank shell and not a code dashboard.
3. The user's Pylon is visible as their base.
4. Tassadar is visible as the central run/model entity, with current training
   state and proof status.
5. Other live Pylons are visible from the public Pylon projection.
6. Settled sats and public activity move only when backed by public refs.
7. The user can type into one chat bar and talk to Tassadar/OpenAgents.
8. The app automatically keeps Pylon presence/readiness/training projections
   fresh.
9. Coding/cloud/session tools remain reachable, but they are not the default
   story and not the default target.

The user-facing name for this mode should be **the Verse**.

## Current implementation facts

### Autopilot Desktop

Relevant files:

- `apps/autopilot-desktop/src/ui/model.ts`
- `apps/autopilot-desktop/src/ui/view.ts`
- `apps/autopilot-desktop/src/ui/update.ts`
- `apps/autopilot-desktop/src/ui/subscriptions.ts`
- `apps/autopilot-desktop/src/shared/chat-world-flags.ts`
- `apps/autopilot-desktop/src/shared/chat-world-scene.ts`
- `apps/autopilot-desktop/src/shared/chat-world-visualization.ts`
- `apps/autopilot-desktop/src/shared/character-creation-onboarding.ts`
- `apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts`
- `apps/autopilot-desktop/src/shared/chat-world-game-layer.ts`

Working pieces:

- `initialModel.verseEnabled` defaults on.
- `VITE_CHAT_WORLD_SCENE` mounts a full-bleed world behind chat.
- `VITE_CHAT_WORLD_PAYMENTS` overlays evidence-bound payment beams/bursts.
- `VITE_AGENT_CHARACTER_CREATION` enables the character-creation overlay.
- `VITE_CHAT_WORLD_MULTIPLAYER` follows the Verse launch default and subscribes
  Desktop to the public `openagents-world` projection when enabled.
- `VITE_CHAT_WORLD_HOTBAR`, `VITE_CHAT_WORLD_REPUTATION`,
  `VITE_CHAT_WORLD_MANA_HUD`, and `VITE_CHAT_WORLD_HAND_TRACKING` exist as
  game-layer flags and pure projections.
- `subscribePylonScene` polls `https://openagents.com/api/public/pylon-stats`
  and dispatches a projected `ChatWorldPylonScene`.
- `subscribePaymentParticles` subscribes to
  `https://openagents.com/api/public/activity-timeline/stream`, with polling
  fallback.
- `subscribeSpacetimeWorld` connects to
  `https://spacetime.openagents.com`, database `openagents-world`, uses the
  generated SpacetimeDB bindings, subscribes the active Tassadar region rows,
  stores the SDK token locally, reconnects with backoff, and dispatches an
  explicit disconnected projection on outage.
- `activityEventToParticle` refuses to emit payment motion without payment kind,
  endpoints, and at least one source ref.
- `chatSceneVisualization` renders the Pylon network through
  `trainingRunView`, composes the Tassadar training run layer, overlays public
  SpacetimeDB station/avatar entities, then overlays payment layers when
  enabled.
- `chatWorldPaymentLayer` now resolves payment endpoints through
  `ChatWorldMultiplayerProjection` when available: `pylonRef` maps to station
  positions, `actorRef`/`avatarRef` maps to avatar positions, and unresolved
  endpoints are explicitly labeled fallback while still carrying the receipt
  source ref in the click label.
- `planChatWorldAvatarPositionWrite` validates client-side avatar movement
  before a browser path can call `set_avatar_position`: region present, finite
  coordinates, bounds, mode allowlist, region continuity, rate limit, and
  movement-jump limit.
- `characterCreationOverlay` projects onboarding status into character-creation
  beats and a compute/mana bar.
- `trainingPane` and `trainingFullscreenPane` already render `trainingRunView`
  for the run projection.

Remaining launch mismatch:

- The fallback shell exposes `Current`, `Claude Code`, and `Codex` as target
  tabs when explicitly opened.
- The explicit Blueprint flow still speaks in sessions, adapters, worktrees,
  and coding turns. That is useful infrastructure, but it is not the requested
  first experience and no longer owns the default Send button.

### Pylon network and payments

Working pieces:

- `projectChatWorldPylonScene` maps public Pylon stats into live Pylon nodes,
  online/offline/wallet-ready/assignment-ready states, heartbeat pulse speed,
  products, and growth tier from cumulative settled sats.
- `pylonGrowthTier` maps settled sats into crystal scale/facet/brightness hints.
- `PaymentParticle` carries sender, receiver, sats, real-bitcoin flag, color,
  size, source refs, timestamp, and text.
- `withChatWorldPaymentLayer` forces `motionPolicy.evidence = "required"` so
  beams/bursts cannot animate without refs.
- `projectPylonBase` detects the local public `pylonRef`, matches it against
  live Pylon nodes, projects online/presence/wallet/assignment readiness into
  mana, and keeps one clear identity-missing blocker when no local ref exists.
- `withPylonBaseLayer` adds a distinct `My Pylon Base` node connected to the
  Tassadar core while leaving fleet Pylons in their own ring.
- My-Pylon growth now comes only from matching payment particles with public
  source refs. Fleet growth remains the network-wide settled-sats tier.

Missing for hands-off:

- The local avatar movement UI loop is not yet mounted, so Desktop joins the
  region and renders live rows but does not continuously publish owner movement.
- The UI does not yet make Pylon readiness, wallet readiness, assignment
  readiness, and compute/mana feel like one base status.

### Tassadar and training

Relevant files/docs:

- `apps/autopilot-desktop/src/bun/training-runs.ts`
- `apps/autopilot-desktop/src/ui/view.ts` training panes
- `docs/launch/2026-06-17-tassadar-training-run-visual-language.md`
- `docs/launch/2026-06-20-llm-computer-training-run-definition.md`
- `docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md`

Working pieces:

- Desktop can load training projections and show Training / Training Live.
- The default Verse scene now shows a Tassadar run core and the benchmark flow
  stages from public Desktop training projections.
- The visual grammar already distinguishes run projection, public activity,
  dashboard, promise gates, operator readiness, evidence packet, closeout, and
  selected-node overlays.
- The launch docs correctly require evidence-bound motion, distinct encodings
  for online/assigned/verified/settled/recipient-confirmed, and no anonymous
  pulses.
- The model spec correctly states that canonical spelling is Tassadar and that
  the current green proof is the exact executor substrate, not a trained
  product model.

Missing for hands-off:

- Training operations panels are still operator-oriented and live in Training /
  Training Live rather than refined support drawers.

### SpacetimeDB world

Relevant files:

- `apps/openagents-world-spacetimedb/README.md`
- `apps/openagents-world-spacetimedb/src/lib.rs`
- `apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs`
- `apps/openagents-world-spacetimedb/scripts/project-activity-timeline.mjs`
- `apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts`

Working pieces:

- The module is deployed conceptually as `openagents-world` at
  `https://spacetime.openagents.com`.
- Public projection tables exist for training runs, entities, edges, proof refs,
  settlement refs, world events, cursors, and bridge health.
- Public interaction tables exist for world regions, Pylon stations, avatars,
  avatar positions, Pylon attention, local chat, bubbles, emotes, and intents.
- Service-only reducers can project world regions, Pylon stations, Pylon
  avatars, and system messages.
- Browser/user reducers are constrained to interaction state and cannot create
  product truth.
- Bridge scripts can project public Tassadar summary and activity timeline data.
- Desktop has pure row types, subscription query strings, projection logic,
  generated-binding connection code, reconnect/backoff, token persistence,
  region join, and explicit disconnected fallback.

Missing for hands-off:

- Desktop currently reuses the generated bindings under
  `apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings`; a later
  cleanup should make binding generation/ownership explicit for Desktop.
- Desktop binds a browser-safe display identity for `join_region`, but a richer
  local avatar customization/profile flow is still missing.
- There is no live position write loop from the Desktop character/spawner into
  `avatar_position`.
- Focus beams and richer proximity chat rendering still need the same treatment
  as stations, avatars, and payment endpoints.

## Open issue audit

### #5526 - DE-3 Autopilot product surface

This issue still frames the surface as a coding agent. That was correct for the
June 19 push, but it is now the wrong first-screen priority.

Keep:

- Desktop release readiness.
- Built-in model/chat readiness.
- Mission briefing, decision queue, and labor products as backend/autopilot
  capability.

Deprioritize from launch first paint:

- `autopilot.cloud_coding_sessions.v1`
- composer/repo/worktree-first flows
- swarm/batch coding sessions
- Claude Code and Codex target tabs
- deploy/cloud/code labels in the shell

The coding agent should remain reachable under Code/Advanced. It should not be
the first sentence the app speaks.

### #5527 - DE-4 Pylon network + multi-earning node

This is central to the Verse.

Priority for the Verse:

- Pylon identity as the user's base.
- Pylon readiness as mana/compute.
- Pylon network state as the live world population.
- Pylon earnings and settlements as growth/progression.
- Multiple earning modes as explicit base capabilities, not a CLI checklist.

The acceptance receipts for #5527 should feed the Verse directly. Every
self-serve install, heartbeat, wallet readiness, assignment readiness, earning
mode, and settlement should produce a public-safe projection that the world can
show.

### #5528 - DE-5 Training / Tassadar pipeline

This is the other center of the Verse.

Priority for the Verse:

- Tassadar run core visible by default.
- Benchmark/training flow visible in the scene:
  assignment -> trace -> replay -> verdict -> accepted/rejected -> settlement.
- Public distributed run state shown as live Pylon participation, not just a
  table.
- Model ladder / Percepta direction shown honestly as future/pending unless
  receipt-backed.

The current training pages are useful, but the default Verse should answer:
"What is Tassadar doing right now, and how are Pylons helping?"

### #5556 - NodeNext typecheck debt

This is not a product feature, but it matters before the Verse becomes default.
The issue says `apps/autopilot-desktop` has typecheck debt outside deploy gates.
That means a green deploy check does not yet prove the whole Desktop TypeScript
surface is type-clean. Before defaulting the Verse, either:

- finish the mechanical/substantive typecheck debt and fence Desktop typecheck
  into deploy checks, or
- explicitly document that Verse launch readiness is bounded by deploy checks
  and focused tests while #5556 remains open.

Do not silently call the Desktop fully type-clean while #5556 is open.

### #5510, #5512, #5520, #5521 - money loop

These are not separate from the game world. They are what make sats fly and
Pylons grow for real.

Verse dependency:

- Stripe/card -> credit -> inference spend creates real paid usage events.
- Referral/settlement creates real sats movement events.
- Pylon earning receipts create growth/progression.

Without these receipts, the Verse can show public Pylon liveness and training
proof state, but payment motion and Pylon growth will remain sparse or staged.

### Backlog gap

After #5730-#5740 closed, there is no open issue that directly says:

> Make the Verse the hands-off Autopilot Desktop default, centered on Pylons and
> Tassadar, with coding/cloud/session tools demoted.

Recommended new tracking issue:

`EPIC: Verse default launch - hands-off Pylon/Tassadar world in Autopilot Desktop`

Suggested children:

1. `Desktop: open to Verse, not zero-base shell`
2. `Desktop: remove Codex/Claude target tabs from first paint`
3. `Desktop: direct Tassadar chat path, no coding-session spawn by default`
4. `Desktop: merge Tassadar run core into chat-world scene`
5. `Desktop: bind local Pylon identity to my-base scene state`
6. `Desktop: live SpacetimeDB client and avatar position loop`
7. `Desktop: Verse launch smoke and from-DMG proof`

## What to remove or demote

Do not delete the coding infrastructure. It is valuable. Demote it.

Demote from first paint:

- `shell` as default home.
- Shell target tabs for `Claude Code` and `Codex`.
- "Start coding session" as a primary CTA.
- Repo/worktree picker.
- Spawn/session/swarm as top-level mental model.
- Cloud coding sessions.
- Deploy controls.
- Any language that says the main reason to open Autopilot is to shell out to a
  coding agent.

Keep as advanced:

- Code group in the nav.
- Cmd-K actions.
- Composer and session detail panes.
- Swarm/batch views.
- Cloud coding, when the red promise becomes honestly receipted.

Replace first paint with:

- Verse scene.
- Pylon base status.
- Tassadar run/model status.
- One chat bar.
- Subtle hotbar for Verse, Pylon, Training, Proofs, Settings.

## Required launch path

### P0 - Make the Verse the default surface

Status: landed by #5819 and #5820 on 2026-06-20.

Acceptance:

- Fresh Desktop launch opens to Verse.
- No `Claude Code` or `Codex` target tabs are visible on first paint.
- Chat placeholder and labels say Tassadar/OpenAgents/Verse, not coding session.
- `VITE_CHAT_WORLD_SCENE`, `VITE_CHAT_WORLD_PAYMENTS`, and
  `VITE_AGENT_CHARACTER_CREATION` are enabled in the intended launch build or
  replaced by a single launch flag that enables the whole Verse bundle.
- If live feeds fail, the scene shows honest zero-state, not a blank canvas.

Implementation shape:

- Change default pane from `shell` to `chat` or a dedicated `verse` pane.
- Keep shell as an advanced leaf or remove it from first-run routing.
- Hide coding targets unless an advanced/developer preference is enabled.
- Keep `verseEnabled` as a user toggle, but default on.

### P1 - Put Tassadar in the default scene

Status: landed by #5822 on 2026-06-20.

Acceptance:

- The Verse scene includes a central Tassadar run core.
- The scene shows live run state from `/api/public/tassadar-run-summary` or the
  existing Desktop training projection.
- It distinguishes assigned, submitted, verified, rejected, settled, and
  blocked states.
- It links selected entities to public refs or proof drawers.
- It does not animate training motion without source refs.

Implementation shape:

- Compose the chat-world `trainingRunView` options with the Desktop public
  training projection instead of treating Training Live as the only training
  scene.
- Start with a calm central run core plus Pylon ring and payment layer.
- Move dense training operations into an overlay drawer.

### P2 - Make chat actually talk to Tassadar/OpenAgents

Status: landed by #5821 on 2026-06-20.

Acceptance:

- The default chat submit does not spawn a Codex/Claude coding session.
- The default chat response comes from the configured OpenAgents/Tassadar model
  path or an inference-gateway model with a Tassadar context pack.
- The answer can cite current public run/Pylon state.
- Exact-replay/program-step UI appears only when a turn actually invokes that
  capability.

Implementation shape:

- Split `ClickedChatSubmit` into two paths:
  - default: `TalkToTassadar` or `RespondToVerseInput`;
  - advanced/code: existing `SpawnChatTurn`.
- Build a Tassadar context pack from:
  - public Tassadar summary;
  - Pylon stats;
  - activity timeline;
  - product-promise status for relevant training/Pylon promises.
- Keep semantic routing, but route to conversational/model response first.

### P3 - Bind "my Pylon" as the user's base

Status: landed by #5823 on 2026-06-20.

Acceptance:

- The app knows the user's Pylon ref after onboarding or clearly says what is
  missing.
- My Pylon is visually distinct from other live Pylons.
- Pylon readiness maps to mana/compute.
- Wallet/assignment readiness maps to base status.
- Settled receipts for that Pylon grow the base.

Implementation shape:

- Reuse local Pylon identity detection already present in training command
  helpers.
- Project the local Pylon into the chat-world scene as `myBase`.
- Split fleet growth from my-Pylon growth.

### P4 - Connect SpacetimeDB to Desktop

Status: partially landed by #5825 on 2026-06-20.

Acceptance:

- Desktop subscribes to `openagents-world`.
- Pylon stations, agent avatars, positions, local chat, and attention rows
  update live in the Verse.
- The local avatar can publish position updates within bounds/rate limits
  once a movement UI loop is mounted.
- Payment particles resolve endpoints to actual station/avatar positions when
  possible.
- If SpacetimeDB is down, direct Worker/Pylon/activity feeds keep the Verse
  usable.

Implementation shape:

- Generate or vendor Desktop-safe TypeScript bindings for
  `apps/openagents-world-spacetimedb`.
- Keep the Desktop SpacetimeDB client module reconnect/backoff and public-safe
  auth/token path.
- Feed `projectChatWorldMultiplayer` from live rows, not test rows.
- Use `planChatWorldAvatarPositionWrite` before any browser path calls
  `set_avatar_position`.
- Wire the movement loop after the render path is stable and covered by smoke.

### P5 - Hands-off autopilot loop

Acceptance:

- On first launch, the app automatically loads onboarding, Pylon identity,
  Pylon stats, activity timeline, Tassadar run state, and Verse scene state.
- The user does not need to open Code, Sessions, Spawn, or a terminal.
- If the Forum intro/work-search automation is allowed, the app runs it with
  clear public refs and confirmation state.
- If permission or credentials are missing, the app shows one clean blocker in
  Verse, not a pile of setup panels.

Implementation shape:

- Treat character creation as the front door.
- After character creation, auto-advance into Verse.
- Make "post intro" and "search work" explicit autopilot beats with receipts.
- Keep manual operator actions in drawers.

## Launch smoke checklist

Before calling this hands-off:

- `bun run verify:autopilot-desktop:deploy`
- `bun run check:deploy`
- Focused Desktop tests:
  - Verse default/on/off behavior.
  - no Codex/Claude target tabs on first paint.
  - Pylon stats fetch -> nonblank scene.
  - activity event with refs -> payment particle.
  - activity event without refs -> no motion.
  - Tassadar summary -> central run core.
  - chat submit -> Tassadar/model response, not coding session spawn.
  - SpacetimeDB unavailable -> Worker direct fallback.
  - SpacetimeDB rows -> live station/avatar entities in the Verse.
  - local avatar movement planner rejects out-of-bounds/rate-limited writes.
- Visual smoke:
  - fresh launch;
  - no blank canvas;
  - no overlapping chat/scene text;
  - Training core visible;
  - Pylon count visible;
  - payment inspector clickable when a receipt-backed event exists.
- Typecheck truth:
  - close #5556 before claiming Desktop full typecheck green, or explicitly say
    the launch proof is deploy/focused-test green while #5556 remains open.

## Final recommendation

Make one product decision now: **the Verse is home**.

The codebase already has the ingredients. The next work is not another broad
MMORPG design pass. It is a launch integration pass:

1. Default to Verse.
2. Center Pylons and Tassadar.
3. Talk to Tassadar by default.
4. Show training/benchmark truth in the scene.
5. Demote Codex/Claude/cloud-code to advanced coding tools.
6. Wire SpacetimeDB once the default direct Worker/Pylon/activity path is
   stable.

That is the path to opening the app and immediately seeing a living world that
represents the training state, Pylon supply, payments, and what the system is
doing on autopilot.
