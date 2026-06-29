# Audit + direction: revive the old Autopilot "pro dashboard" as a new `/pro` operator UI

> Status: **AUDIT + direction note**, 2026-06-24. **Not an implementation, not a
> product promise, not public-claim copy.** This doc inventories the historical
> Autopilot chat-UI / "pro dashboard" surfaces (across this repo's history and the
> deprecated sibling repos), audits what `/autopilot` serves *today*, and proposes a
> new operator/power-user surface at `/pro` rebuilt on Foldkit + `@openagentsinc/ui`.
> Everything labeled **FUTURE** is speculative; **OWNER-GATED** needs owner arming.
> Invariants that govern any real build are restated in Part 3.
>
> The motivating user persona: power users / operators who *run, inspect, and
> review* machine work — e.g. the author of `RhysSullivan/executor` and similar
> operators. The hosted-executor experience is the north star.

---

## TL;DR (5 lines)

- The owner's memory is correct: a *previous* Autopilot route **was** a chat UI /
  "pro dashboard that did things with projects." The **best match** is the
  deprecated **React/Vite Autopilot** (`autopilot-deprecated/`) HUD command-canvas,
  and the closest *project-mission cockpit* is **Vortex** (`vortex/`) — a Codex
  mission-control HUD with a live briefing + decision queue + operator actions.
- `/autopilot` **today** is **not** that dashboard: it resolves to the logged-out /
  mid-onboarding **Autopilot onboarding flow** (and `/autopilot/legal` overlay). A
  logged-in *workroom* user is redirected to the existing `Chat` cockpit.
- Recommendation: **add a new `/pro` route** (do **not** repurpose `/autopilot`,
  which owns onboarding). `/pro` is the operator/power-user surface: a chat/operator
  console over projects/workrooms + agent runs, and — the differentiator — a live
  view of the **autonomous-QA / Khala work in flight** (`apps/qa-runner` runs,
  session video, distilled e2e test, Khala sessions). That is "the executor
  experience, hosted."
- Stack: Foldkit + `@openagentsinc/ui`, one `routeRegistry` entry + one startup
  disposition + one `view.ts` case, **operator/admin-gated**
  (`loggedInOperatorAccessAllowed` / `loggedInAdminAccessAllowed`).
- Phased: P0 read-only run/session viewer (reuse the existing `Chat` +
  `artanis-console` docks) → P1 operator actions over runs → P2 hosted-executor
  (watch a Khala/QA run, review the distilled test). Evidence-only where it touches
  Blueprint; no promise widening; single model `openagents/khala`.

---

## Part 1 — Finding the old UI (the candidates, ranked)

I searched both this repo's git history and the deprecated sibling repos under
`/Users/christopherdavid/work/`. Five real prior surfaces exist. None of them is a
"simple chat box": every one was a *command/operator* surface with project/workroom
context and operator actions. Two are the strongest matches for "chat dashboard that
did things with projects."

### Candidate A (BEST chat-canvas match) — `autopilot-deprecated/` (React 19 + Vite)

The renamed deprecated React/Vite Autopilot clone (`autopilot.openagents.com`). This
was a **full-screen voice/text command canvas** with a fixed bottom composer and a
dynamic **HUD** of panes that rendered live operational data, plus **approval-gated
action staging**. Three verticals: CRM, CEO (investor prep), Legal.

What it actually did:
- Chat/voice command input → NLU routing → mounts contextual **HUD panes**.
- **Projects/objects** via the CRM vertical: contact/investor lists, activity
  timelines, enrichment candidates, operator queues.
- **Operator actions**: sensitive-action classification, **dry-run before staging**,
  step-up auth gates, approval-required UX (`ActionReviewPane`), receipt
  expectations, voice decision traces.
- Live assistant audio playout + barge-in; Blueprint backend handoffs.

Representative paths (all under `/Users/christopherdavid/work/autopilot-deprecated/`):
- `src/App.tsx` — main canvas, voice lifecycle, command router, HUD pane mgmt.
- `src/hud/HudRuntime.tsx`, `src/hud/types.ts` — pane lifecycle + descriptors.
- `src/crm/CrmPaneContent.tsx`, `src/crm/crmAdapter.ts` — project/object rendering +
  routing.
- `src/auth/ActionReviewPane.tsx`, `src/auth/sensitiveAction.ts` — approval staging.
- `src/commands/commandRouter.ts` — confidence/safety routing (legal/crm/ceo/repair).

Stack: React 19, Vite 8, TS 6, Web Audio/MediaDevices, `@ai-sdk/react` + `ai`, WorkOS
magic-link auth, Rust Axum BFF + Blueprint SDK behind it.

> Why it's the best *chat-canvas* match: it is literally "a chat/voice UI that mounts
> panes and does gated things with projects/objects." It is **CRM/CEO/Legal**-shaped,
> not project/repo/run-shaped — so for `/pro` we port the *shell pattern* (composer +
> HUD panes + approval staging) but not the verticals.

### Candidate B (BEST project-mission match) — `vortex/` (Next.js 16 + Convex)

The previous Convex/Next.js `openagents.com` product surface. **Not** a chat UI — a
**Codex power-user command cockpit / Mission HUD**. This is the closest thing to "a
pro dashboard that does things with projects," and the closest to the hosted-executor
idea.

What it actually did:
- Connect GitHub repos + ChatGPT/Codex accounts (fleet), issue a long-running coding
  command/mission, pick runner backend + budget.
- **Live Mission HUD**: status strip, event timeline, **Mission Briefing** (what
  happened / changed / blocked / next / routes / cost / risk), **Decision Queue**.
- **Operator actions**: Continue, Steer with order, Provide context, Rerun tests,
  Retry runner assignment, Stop mission, Create follow-up; **Approve/Deny** gates.
- Durable event/tool-call/artifact/checkpoint telemetry in Convex.

Representative paths (all under `/Users/christopherdavid/work/vortex/`):
- `app/autopilot/page.tsx`, `components/autopilot/autopilot-surfaces.tsx` — dashboard.
- `app/missions/[runId]/mission-hud.tsx` — the ~650-line live operator cockpit.
- `components/autopilot/autopilot-launch-flow.tsx` — mission launch wizard.
- `app/api/autopilot/missions/[runId]/command/route.ts` — continue/steer/retry/cancel.
- `convex/schema.ts` — `codexRuns`, `codexRunEvents`, `codexToolCalls`,
  `codexArtifacts`, `codexRunCheckpoints`, `threads`, `approvals`.

Stack: Next 16 (App Router), Convex, WorkOS AuthKit, `ai`, Tailwind 4 (dark/mono),
CommitMono, XYFlow, streamdown.

> Why it matters most for `/pro`: the **Mission Briefing + Decision Queue + operator
> action buttons over a run** is exactly the shape an operator like Rhys wants. Port
> the *information architecture* (status strip → briefing → timeline → decision queue
> → actions), not the Convex/Next stack.

### Candidate C — `deprecated/openagents.com/` (Laravel 13 + Inertia React)

The Laravel/Inertia site's **admin workspace / chat operator** (the
`docs/admin-workspace-chat-operator-runbook.md` surface). A ChatGPT-like `/chat`
scoped to team/project/repository, plus an `/admin` control room.

What it did: workspace-scoped chat (Basic Chat + **Pylon Codex** local execution with
Reverb streaming + replay), DSPhp "Autopilot picked" inline decision panel, full
team/project/repository CRUD, **operator health actions** (`health.verify`,
`recovery.request`, `funding_invoice.create`, `followup.queue`, `run.stop`,
`run.cancel` — HMAC-signed, audited), **managed-agent approvals** (approve/deny
approvals, custom-tool requests, memory versions), scheduled Pylon Codex runs.

Representative paths (under `/Users/christopherdavid/work/deprecated/openagents.com/`):
- `resources/js/pages/chat/index.tsx` — workspace-scoped chat operator.
- `resources/js/pages/admin/index.tsx` — admin control-room hub.
- `resources/js/pages/admin/health/index.tsx` — operator action catalog/form.
- `resources/js/pages/admin/managed-agents/index.tsx` — approvals surface.
- `app/Http/Controllers/OpenAgentsChatConversationController.php` — chat/Codex/SSE.
- `docs/admin-workspace-chat-operator-runbook.md` — the runbook.

Stack: Laravel 13 + Inertia 3 + React 19, Sanctum + WorkOS, Reverb (WS), LiveKit
voice, Radix/shadcn, Tailwind 4.

> Why it matters: this is the clearest prior art for **operator actions over runs +
> approval workflows + workspace scoping** with real auth gating
> (`approved:admins`). The `/pro` operator-action and approval model should mirror
> this conceptually (run.stop / approve / deny / followup), re-expressed in Effect.

### Candidate D — `autopilot4-deprecated/` (Rust + Maud + htmx)

Server-rendered Autopilot 4. Full **Artanis chat UI** (`/c/{thread}`, `/ws/chat`,
`/ws/voice`), **project pages** (`/p/{project_route_id}`) combining chat + GitHub +
CRM context, and an operator **SHC Dashboard** (`/operator/shc`) with decision strip
+ export. Paths: `src/artanis.rs`, `src/chat_runtime.rs`, `src/project_context.rs`,
`src/shc_dashboard.rs`, `src/routes.rs`. Stack: Rust/Axum/Maud/htmx, WorkOS.

> Relevance: confirms the lineage of the name **Artanis** (the current repo still has
> an `artanis-console` operator dock — see Part 2) and the project-page-as-workspace
> idea. Architecture reference only.

### Candidate E — `backroom/` (archived: Rust/Actix/Maud + WGPUI)

Three archived eras: an Actix/Maud **autopilot-old** metrics dashboard
(`backroom/autopilot-old/src/dashboard.rs` — session metrics, anomaly detection, WS
live updates, JSON/CSV export); a **Maud archive GUI** with a WS chat interface +
session dashboard (`backroom/openagents-maud-archive/crates/autopilot-gui/...`,
`crates/ui/src/chat_pane.rs` with Formatted/JSON/Raw view modes); and a deferred
**WGPUI** native frontend (`backroom/archive/frontend-wgpui/`).

> Relevance: the **metrics/session dashboard** and **multi-view (Formatted/JSON/Raw)
> run inspection** patterns are worth porting into the `/pro` run inspector.

### This repo's own history

`git log --all --oneline -- '**/autopilot*'` in `apps/openagents.com` shows the
**current** `/autopilot` lineage is *onboarding*, not a dashboard — e.g. commits
`492935fcb7` "Assemble the /autopilot onboarding page + flow (#6145)",
`1b2fe1c64a` "Autopilot onboarding UI: markdown, streaming, scroll, sidebar progress
(#6151)", `8a3548d2de` "/autopilot/legal industry overlay". The old Rust desktop
Autopilot (`apps/autopilot-desktop/`, `full_auto.rs`, `codex_control.rs`) was removed
in the big prune commit **`d7f53fccc`** ("chore: prune repo to wgpui + vim and MVP
doc only", 2026-02-25). So the "pro dashboard" the owner remembers does **not** live
in this repo's current tree — it lives in the deprecated siblings above.

### Comparison table

| Candidate | Repo / stack | Chat? | Projects/workrooms? | Operator actions? | Live agent output? | Best for `/pro` |
|---|---|---|---|---|---|---|
| **A** autopilot-deprecated | React 19 / Vite | **Yes** (voice+text, HUD panes) | CRM/CEO/Legal objects (not repos) | **Yes** (dry-run + approval staging, step-up) | Voice traces, panes | **Shell pattern**: composer + HUD panes + approval staging |
| **B** vortex | Next 16 / Convex | No (command-first) | **Yes** (repos + missions) | **Yes** (continue/steer/retry/stop, approve/deny) | **Yes** (briefing + timeline + decisions) | **Information architecture**: mission HUD over a run |
| **C** deprecated/openagents.com | Laravel 13 / Inertia | **Yes** (workspace-scoped) | **Yes** (team/project/repo CRUD) | **Yes** (health actions, approvals, schedules) | Reverb stream + replay | **Operator-action + approval model**, auth gating |
| **D** autopilot4-deprecated | Rust / Maud / htmx | **Yes** (Artanis) | **Yes** (`/p/{id}` project pages) | SHC decision strip | WS chat/voice | Artanis lineage, project-page idea |
| **E** backroom | Rust / Actix / Maud / WGPUI | Yes (WS) | Context browser | metrics-only | WS, anomaly detect | Run inspector (Formatted/JSON/Raw), metrics |

**Synthesis for `/pro`**: take the **shell** from A (composer + HUD panes + gated
action staging), the **information architecture** from B (status strip → briefing →
timeline → decision queue → actions over a *run*), and the **operator-action +
approval model** from C (run.stop / approve / deny / followup, hard auth gating).
Express all of it in the current stack (Foldkit + `@openagentsinc/ui` + Effect).

---

## Part 2 — What `/autopilot` serves today (and why not to touch it)

Verified in `apps/openagents.com/apps/web/src`:

- **Routing** (`route.ts`): `chatRouter = literal('autopilot') -> ChatRoute`, and
  `autopilotRouter = literal('autopilot') -> AutopilotRoute`, plus
  `autopilotVerticalRouter` for `/autopilot/legal` and `autopilotWork*` for
  `/autopilot/work[/:ref]`. The central **`routeRegistry`** (route.ts, the typed
  single source of truth) classifies `Autopilot` / `AutopilotVertical` as
  `requiresAuthBootstrap: true`, `loggedInGate: 'open'`, `render: 'submodel'`.
- **Startup disposition** (`routing/startup.ts`):
  - Logged-out / mid-onboarding: `Autopilot`/`AutopilotVertical` →
    `LoggedOutStartupRoute` (no redirect) — serves the **public onboarding page**
    (lines ~191–199; comment: "Mid-onboarding users have no workspace yet, so the
    autopilot entry serves the public onboarding page").
  - Logged-in, onboarding complete (lines ~264–277): if
    `loggedInWorkroomAllowed(auth)` → **redirect to `ChatRoute()`** (the existing
    cockpit); otherwise serve the public onboarding page. The disposition map records
    `Autopilot: 'autopilot'`, `AutopilotVertical: 'autopilot'`.
- **Render** (`page/loggedIn/view.ts`): the workroom cockpit that `/autopilot`
  *redirects into* is **`Chat`** — rendered via `Ui.workroomChatRoute(Chat.view(model))`
  (the `Chat` page at `page/loggedIn/page/chat.ts` already composes
  `artanisOperatorDock` from `artanis-console/view` and `agentGoalDock` from
  `goals/view`). So the only existing "operator-ish" chat cockpit in-tree today is
  the workroom `Chat` page, gated behind `loggedInWorkroomAllowed`.

**Conclusion**: `/autopilot` today = **the Autopilot onboarding surface** (logged-out
and mid-onboarding), with logged-in workroom users bounced to the `Chat` cockpit. It
is *not* the old pro dashboard, and it is an actively-developed onboarding funnel
(#6129, #6145, #6151, legal overlay #6147).

**Why a new `/pro` instead of changing `/autopilot`:**
1. `/autopilot` is the **onboarding funnel** — the public acquisition surface. A
   power-user operator console has the opposite audience and the opposite auth gate
   (operator/admin, not anonymous). Overloading the route would entangle the funnel
   with a gated cockpit.
2. The route registry treats `Autopilot` as a logged-out-union `submodel`; an
   operator console is a logged-in-union, gated surface (like `Mullet`/`Admin`).
   Different union membership, different gate, different render disposition — a
   cleaner separate `routeRegistry` entry.
3. No copy/onboarding regression risk: adding `/pro` leaves the onboarding flow
   untouched (consistent with the "preserve user-facing copy by default" rule).

---

## Part 3 — Proposed `/pro` (operator / power-user UI)

### Who

Power users / operators who **run, inspect, and review machine work** — the
`RhysSullivan/executor` persona and similar. They want to watch agent/QA runs, read a
deterministic briefing, drive a run (continue/steer/stop), approve/deny gated actions,
and review distilled artifacts — without babysitting a local terminal. `/pro` ties
into the operator surfaces that already exist today:

- `mullet` (chris-only; `loggedInMulletAccessAllowed`) — owner runner workbench.
- `admin` (`loggedInAdminAccessAllowed`) — admin overview.
- `dashboard`, `usage`, `billing` — existing logged-in operator/account surfaces.

`/pro` slots in as the **operator console** alongside these, gated by
`loggedInOperatorAccessAllowed` (Core Team) or `loggedInAdminAccessAllowed` (admin)
— see "Auth gate" below.

### What it should do (synthesized from the old UIs + today's needs)

1. **Run / session inspector (P0).** A list + detail view over agent runs and Khala
   QA sessions, with the **Vortex mission-HUD IA**: status strip → **briefing**
   (what happened / changed / blocked / next / routes / cost / risk) → event timeline
   → artifacts. Port the **Formatted / JSON / Raw** multi-view from `backroom`'s
   `chat_pane.rs` for honest inspection. Read-only first.
2. **Operator actions over a run (P1).** The Vortex/Laravel action set re-expressed
   in Effect: **continue / steer / provide context / rerun / stop**, plus
   **approve / deny** gates (the Candidate-C managed-agent + health-action model).
   Each action is staged with a dry-run/confirmation step (Candidate-A `ActionReview`
   pattern). Wired through existing run/approval seams — **no new authority**.
3. **Hosted-executor: watch + review Khala/QA work (P2, the differentiator).**
   Surface the **autonomous-QA / Khala work in flight**:
   - `apps/qa-runner` runs (computer-use session against a Target in an isolation
     backend) — playable **session video** + Playwright trace + per-step screenshots
     + `result.json` (the runner already emits a dereferenceable, public-safe
     receipt; see `apps/qa-runner/README.md`, epic **#6174**).
   - **Khala sessions** and the **distilled e2e test** — per the distiller spec
     (`docs/khala/2026-06-24-khala-session-distiller-and-program-wiring-spec.md`):
     one capture → distiller → two emitters (a Khala skill candidate + a committed
     e2e scenario). `/pro` is plausibly *where a power user watches a Khala-driven
     run and reviews the distilled test* — the executor experience, hosted.
   - Honest status: today `khalaBrain` and the cloud VM backend are
     **OWNER-GATED / inert** (throw "not armed"); `scriptedBrain` + `localBackend`
     are real. `/pro` P2 must reflect that — show real runs, label un-armed seams as
     owner-gated, never fake green.

### How (stack + wiring)

Rebuilt on **Foldkit + `@openagentsinc/ui`** (the current web stack — same pattern as
the logged-in `Chat`, `mullet`, `admin`, `billing`, `usage` pages). Adding `/pro` is
the now-standard single-route change:

1. **`route.ts`**: add a `ProRoute` to the `AppRoute` union, a `proRouter = pipe(
   literal('pro'), Route.mapTo(ProRoute))`, register it in the `oneOf` list, and add
   the **`routeRegistry.Pro`** entry — `requiresAuthBootstrap: true`,
   `loggedInGate: 'admin'` (or a new `'operator'` gate, see below),
   `inLoggedOutUnion: false`, `inLoggedInUnion: true`, `render: 'loggedInOnly'`.
2. **`routing/startup.ts`**: add `Pro` to the logged-in `routeAllowedForLoggedInAuth`
   branch (the big `M.tag(...)` list ending at `Admin`, `Mullet`, `Billing`, `Usage`)
   and to the disposition map as `'gated'` (matching `Admin`/`Mullet`). For
   logged-out / incomplete-onboarding, it falls through to the onboarding redirect
   like other gated routes.
3. **`product-policy.ts`**: `routeAllowedForLoggedInAuth` already derives from
   `routeRegistry[tag].loggedInGate`, so the gate is automatic once the registry
   entry exists. Add a `Pro` product intent (e.g. `'pro.operator.console'`) to
   `browserRouteProductIntents`.
4. **`page/loggedIn/view.ts`**: add a `Pro: () => proRouter()` URL case, a label
   case, and a render case (e.g. `Ui.workroomChatRoute(Pro.view(model))` or a bespoke
   panel layout), plus a new `page/loggedIn/page/pro.ts` view module (and any
   `pro/` operator-console sub-dir for transitions/model, mirroring `mullet/`).
5. **Model/update**: extend the logged-in submodel `Model`/`Message`/`update` for the
   `/pro` console state (run list, selected run, briefing, action staging). Reuse the
   existing `run-timeline/projection.ts`, `artanis-console`, and `goals` docks rather
   than rebuilding.

> Because the route registry is the typed single source of truth, TypeScript will
> force you to classify `Pro` everywhere (startup union, view render case, policy) —
> the registry comment in `route.ts` documents exactly this safety property.

### Auth gate

Operator/admin-gated. Two clean options:
- **Reuse `'admin'`** (`loggedInAdminAccessAllowed` = `auth.isAdmin &&
  onboardingIsComplete`) — simplest, ships P0/P1 to admins immediately.
- **Add an `'operator'` gate** mapping to `loggedInOperatorAccessAllowed`
  (`authHasCoreTeamAccess`) so Core-Team operators (broader than admins) get access.
  Requires adding `'operator'` to `RouteLoggedInGate` and the
  `routeAllowedForLoggedInAuth` switch.

Recommendation: **start with `'admin'`** (matches `Admin`/`Mullet`/`Usage`/`Billing`
gating posture, no policy-surface expansion), and widen to an `'operator'` gate only
if/when Core-Team-but-not-admin operators need it.

### Migration notes — port vs leave behind

**Port (as patterns, re-expressed in Effect/Foldkit):**
- Vortex **mission-HUD IA**: status strip → briefing → timeline → decision queue →
  actions (the spine of `/pro`).
- Candidate-A **composer + HUD-pane shell** and **dry-run/approval staging**
  (`ActionReviewPane` pattern) for any action that mutates.
- Candidate-C **operator-action + approval vocabulary** (run.stop, approve, deny,
  followup) and hard **auth gating**.
- `backroom` **Formatted/JSON/Raw** run inspection + metrics/session dashboard ideas.

**Leave behind:**
- Convex/Next, Laravel/Inertia, Rust/Maud/htmx stacks — all replaced by
  Foldkit/Effect.
- The CRM/CEO/Legal verticals from Candidate A (different product; not operator-run
  inspection).
- Voice canvas / barge-in audio (Candidate A/D) — out of scope for P0–P2.
- Any direct money-movement / settlement authority — stays INERT/OWNER-GATED.

**Gaps / honest unknowns:**
- The `/pro` run/session data source: today, run inspection in-tree is the workroom
  `Chat` cockpit + `run-timeline/projection`; `apps/qa-runner` emits `result.json` +
  video but there is no hosted run-index API surfaced to the web app yet. P2 depends
  on that plumbing (and on `cloud` wiring the real cloud VM provisioner).
- Khala distiller output (skill candidate + e2e scenario) is a **build spec**, not
  shipped — `/pro`'s "review the distilled test" view trails the distiller landing.

### Invariants (any real build must hold these)

- **Operator/admin-gated**: `/pro` is never anonymous; gate via the registry
  `loggedInGate` (admin first, operator later).
- **Evidence-only Blueprint**: where `/pro` touches Blueprint program runs, it reads
  evidence; it does not widen authority or write claims.
- **No promise widening / no public-claim copy**: `/pro` is an operator surface; any
  user/agent-facing claim still routes through `docs/promises/`. This doc is not a
  promise.
- **No fake green**: surface real runs; label un-armed seams (khalaBrain, cloud VM
  backend, settlement) as **OWNER-GATED**; failed runs render as failures.
- **Single model**: `openagents/khala` (no `-code`/`-mini`/`-pro` variants; the
  served alias `openagents/khala-oss-20b` is separate).
- **No keyword/string routing for intent** — use the existing typed route/command
  registry and semantic selection paths.

### Phased proposal

- **P0 — Read-only operator console (admin-gated).** New `/pro` route (registry +
  startup + view), a run/session **inspector**: list → briefing → timeline →
  artifacts, with Formatted/JSON/Raw views. Reuse `run-timeline/projection`,
  `artanis-console`, `goals`. No mutations. Ships behind `loggedInAdminAccessAllowed`.
- **P1 — Operator actions over runs.** Add continue / steer / provide-context / rerun
  / stop + approve / deny, each with dry-run/confirm staging. Wire through existing
  run/approval seams only. Audit every action (mirror Candidate-C's audited
  health-action model).
- **P2 — Hosted executor (the differentiator).** Surface `apps/qa-runner` runs
  (video + trace + screenshots + `result.json`) and Khala sessions + the distilled
  e2e test. Watch a Khala/QA run and review the distilled artifact in `/pro`.
  Depends on the run-index plumbing + distiller landing + (for cloud runs) the
  `cloud` provisioner; everything un-armed stays OWNER-GATED.
- **P3 (FUTURE) — widen to Core-Team operators** via an `'operator'` gate, and
  launch flow (create a run/mission from `/pro`) once the run API supports it.

---

## Appendix — key file citations (this repo, verified 2026-06-24)

- `apps/openagents.com/apps/web/src/route.ts` — `routeRegistry` (typed source of
  truth), `chatRouter`/`autopilotRouter`/`autopilotVerticalRouter`/`autopilotWork*`,
  `Autopilot` registry entry (~819), `RouteLoggedInGate`/`RouteRenderDisposition`.
- `apps/openagents.com/apps/web/src/routing/startup.ts` — `/autopilot` dispositions
  (logged-out onboarding ~191; logged-in → `ChatRoute()` ~264), disposition map (~417).
- `apps/openagents.com/apps/web/src/product-policy.ts` — `routeAllowedForLoggedInAuth`
  (~85), `loggedInOperatorAccessAllowed`/`loggedInWorkroomAllowed`/
  `loggedInAdminAccessAllowed`/`loggedInMulletAccessAllowed` (~63–74),
  `browserRouteProductIntents` (~135).
- `apps/openagents.com/apps/web/src/page/loggedIn/view.ts` — URL/label/render cases
  for `Chat`/`Mullet`/`Admin`/`Billing`/`Usage`; `Ui.workroomChatRoute(Chat.view(...))`.
- `apps/openagents.com/apps/web/src/page/loggedIn/page/chat.ts` — current workroom
  cockpit; composes `artanisOperatorDock` + `agentGoalDock`.
- `apps/openagents.com/apps/web/src/page/loggedIn/artanis-console/view.ts` — operator
  dock (Artanis lineage from Candidate D).
- `apps/qa-runner/README.md` — Khala autonomous-QA runner (epic #6174); real vs
  owner-gated table; one-model statement.
- `docs/khala/2026-06-24-khala-session-distiller-and-program-wiring-spec.md` — capture
  → distiller → {skill candidate, e2e scenario}.
- `docs/feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md` — executor
  prior art (epic #6174).
- Prune commit `d7f53fccc` — removed the old Rust `apps/autopilot-desktop/`.

### Sibling-repo citations (read-only reference)

- `autopilot-deprecated/src/App.tsx`, `.../hud/HudRuntime.tsx`, `.../crm/CrmPaneContent.tsx`,
  `.../auth/ActionReviewPane.tsx`, `.../commands/commandRouter.ts`.
- `vortex/app/missions/[runId]/mission-hud.tsx`,
  `vortex/components/autopilot/autopilot-launch-flow.tsx`,
  `vortex/app/api/autopilot/missions/[runId]/command/route.ts`, `vortex/convex/schema.ts`.
- `deprecated/openagents.com/resources/js/pages/chat/index.tsx`,
  `.../pages/admin/health/index.tsx`, `.../pages/admin/managed-agents/index.tsx`,
  `.../docs/admin-workspace-chat-operator-runbook.md`.
- `autopilot4-deprecated/src/artanis.rs`, `.../src/project_context.rs`,
  `.../src/shc_dashboard.rs`.
- `backroom/autopilot-old/src/dashboard.rs`,
  `backroom/openagents-maud-archive/crates/ui/src/chat_pane.rs`.
