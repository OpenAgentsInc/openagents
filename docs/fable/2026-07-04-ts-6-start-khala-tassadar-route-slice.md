# TS-6 Route Slices On Start

Date: 2026-07-04
Issue: [#8348](https://github.com/OpenAgentsInc/openagents/issues/8348)
Epic: [#8339](https://github.com/OpenAgentsInc/openagents/issues/8339)

## Landed Slice 1: Khala And Tassadar

The Start staging app now owns the public `/khala` and `/tassadar` routes that
the Start landing page already linked to.

- `/khala` preserves the visible behavior contract from the Foldkit route:
  `data-route="khala"`, the Khala API basics (`openagents/khala`,
  `https://openagents.com/api/v1`, `POST /api/keys/free`), the
  `data-counter="khala-tokens-served"` DOM anchor, and the back-home affordance.
- `/tassadar` preserves the public training-run route contract:
  `data-route="tassadar"`, `data-pose="tassadar"`, the Copy Agent Instructions
  control, the agent instruction block, and the back-home affordance.
- `src/routeTree.gen.ts` was regenerated so `/khala/chat-sync` is now a child
  route under `/khala` instead of an unrelated root route.

## Landed Slice 2: Gym

The Start staging app now owns a public `/gym` route that preserves the
Foldkit route's no-spend, live-data-only contract.

- `/gym` preserves the stable route markers: `data-route="gym"`,
  `data-gym-page`, `data-gym-no-spend-banner`, `data-gym-terminal-bench-panel`,
  `data-gym-run-progress-panel`, and the accessible
  `data-gym-run-progress-accessible-mirror`.
- The Terminal-Bench and live run-progress surfaces keep the honest empty
  states: no decision-grade benchmark reports, no active run, no fixture
  result, and no fabricated pass-rate numbers.
- The public controls keep the old vocabulary that users and tests expect:
  `Provider fan-out`, `Program signature modules`, and a no-spend economics
  panel. They are rendered as disabled Start controls in this slice; live
  dispatch remains backend/owner-gated, not a public page action.
- `src/routeTree.gen.ts` and the Start route-budget list include `/gym`.

## Landed Slice 3: Components

The Start staging app now owns `/components` and `/components/$family` as a
static design-system workbench slice.

- `/components` preserves the public route markers and workbench orientation:
  `data-route="components"`, `Component library`, and
  `Internal - design-system workbench`.
- The family inventory keeps the old gallery's representative headings:
  Primitives, Shared, Forms, Layout, Navigation, Data display, Feedback,
  Workroom, Public, Public theme, Business landing, Page examples, V4, AI
  Elements, and Live samples.
- Selected family routes preserve the behavior-test anchors for
  `data-display`, `training`, `business`, and `public-theme`, including
  `tableList`, `Contract`, `oa-training-grammar-replay-pair`,
  `businessIntakeForm`, and `data-public-landing-shell`.
- This slice does not claim the old Foldkit live registry has been deleted; it
  gives Start the route and route-test contract ahead of production cutover.

## Landed Slice 4: Clients Preview

The Start staging app now owns `/clients-preview` as a static Autopilot client
protocol fixture page.

- `/clients-preview` preserves the old route's public contract markers:
  `data-route="clients-preview"`, `data-autopilot-session-list`,
  `data-autopilot-session-ref`, `data-autopilot-decision-id`, and
  `data-autopilot-decision-action`.
- The two shared protocol session refs remain visible:
  `session.pylon.codex_composer.fixture0001` and
  `session.pylon.claude_composer.fixture0002`.
- The pending decision fixture remains visible as
  `decision.fixture.req01` / `action.fixture.approve_pr`.
- This slice intentionally does not import the old Foldkit Autopilot UI package
  into the Start scaffold. It keeps the server-rendered public-safe fixture
  contract in React while the real client workflow remains on the existing app
  until production route cutover.

## Landed Slice 5: Activity

The Start staging app now owns `/activity` as the public activity timeline
shell.

- `/activity` preserves the top-level route and custom-element anchors:
  `data-route="activity"`, `oa-public-activity-timeline`, and
  `data-start-activity-timeline`.
- The server-rendered light DOM keeps the old public activity surface's stable
  pane markers visible before the live client controller hydrates:
  `data-activity-pane="fleet-map"`, `active-tasks`, `fleet`, `money`, `forum`,
  `timeline`, and the `data-proof-drawer` proof panel.
- The source-lag and filter anchors remain visible:
  `data-activity-source-lag`, `data-activity-filter="settle"`, and
  `data-activity-filter="forum"`.
- This slice keeps the old Foldkit custom-element implementation in place
  until production cutover. The Start route provides the SSR shell and parity
  anchors; moving the live timeline controller is still part of the remaining
  TS-6 route-by-route work.

## Landed Slice 6: Landing Preview

The Start staging app now owns `/preview/landing` as the review-only candidate
landing page.

- `/preview/landing` preserves the review-only marker and copy contract:
  `data-route="landing-preview"`, `data-landing-preview`, and
  `preview - proposed landing page, not the live homepage`.
- The two-door product fork remains visible: `Build it myself` links to
  `/khala`, and `Build it for me` links to `/business`.
- The public-claim guardrails remain visible through the footer links:
  `source: github.com/OpenAgentsInc/openagents`,
  `every claim: /docs/product-promises`, and `live usage: /stats`.
- This slice does not replace the live homepage; it only ports the review
  candidate route to the Start scaffold ahead of any cutover decision.

## Landed Slice 7: Run

The Start staging app now owns `/run` as the retired Tassadar web-scene pointer.

- `/run` preserves the old route contract: `data-route="tassadar"` and
  `data-tassadar-scene="retired"`.
- The page keeps the same public copy and links: `Tassadar lives in the Verse`,
  `Autopilot Desktop Verse`, `/api/public/tassadar-run-summary`, and
  `/tassadar/replay/first-real-settlement`.
- The route test keeps the retired scene separate from the old persistent
  scene overlay and landing-squares element.

## Landed Slice 8: Login

The Start staging app now owns `/login` as the public sign-in entry route.

- `/login` preserves the old overlay/route markers:
  `data-persistent-scene-overlay="login"` and `data-route="login"`.
- The sign-in assertion target remains `Log in to OpenAgents`.
- The email one-time-code form keeps `action="/login/email"` and
  `method="get"` with the `you@example.com` placeholder.
- The GitHub OAuth entry remains a plain link to `/login/github`.
- This slice ports only the login page shell. Auth callback, OAuth/email
  handlers, downstream entitlement checks, and account gating remain owned by
  the existing Worker/API surfaces.

## Landed Slice 9: Business KPI

The Start staging app now owns `/business/kpi/$engagementRef` as the public
business scorekeeper route.

- `/business/kpi/engagement.public.vertical_pipeline_1` preserves
  `data-business-kpi-dashboard="engagement.public.vertical_pipeline_1"` and
  `data-route="business-kpi"`.
- The live metric anchors remain visible: `lead_volume`, `conversion`, `aov`,
  `revenue`, and `consult_attach`.
- Evidence and privacy-boundary copy remains visible, including
  `/api/public/business/funnel-dashboard`, `table:business_funnel_events`,
  `issue:8105`, `roadmap:BF-7.1`, and the excluded private fields.
- The route test keeps customer emails and phone-like private values absent.

## Landed Slice 10: Download

The Start staging app now owns `/download` as the Autopilot Desktop download
page, distinct from `/code/download` for Khala Code.

- `/download` preserves `data-route="download"` and the signed DMG CTA
  `data-cta="download-autopilot"`.
- The current signed asset remains
  `AutopilotDesktop-1.0.0-rc.3-macos-arm64.dmg`, with the one-click
  auto-onboarding gate still false.
- Platform honesty remains visible for macOS Apple Silicon, macOS Intel,
  Windows, and Linux.
- The Pylon CLI alternative remains visible as `npx @openagentsinc/pylon`.

## Landed Slice 11: Site Checkout Demo

The Start staging app now owns `/sites/demo-checkout` and
`/sites/demo-checkout/$returnAction` as the public Omega Site checkout demo.

- The route preserves `data-route="site-checkout-demo"` and
  `data-site-checkout-demo`.
- The visible shell keeps the old contract copy: `Demo checkout`, `Start
  checkout`, and `Start a demo checkout for an Omega Site product and inspect
  the clean return status.`
- The browser script continues to use Site commerce APIs only:
  `/api/sites/{siteId}/commerce/discovery` and
  `/commerce/checkout-returns/{intentRef}/{action}`.
- The checkout intent request keeps the idempotency header and public-safe
  customer-data references:
  `'Idempotency-Key': idempotencyKey()` and
  `customerDataRefs: item.customerDataRequirementRefs || []`.
- The route test keeps unsafe checkout query/material words absent from the
  rendered shell and script contract.

## Landed Slice 12: Pylon Codex Assignment Status

The Start staging app now owns
`/pylon/codex/assignments/$assignmentRef` as the public, owner-scoped status
surface for one Khala coding delegation.

- The route preserves `data-route="pylon-codex-assignment-status"`.
- The assignment ref remains visible in the page shell.
- The closeout commands remain owner-scoped CLI instructions:
  `pylon khala status --assignment-ref ... --json` and
  `pylon khala proof ... --json`.
- The green-evidence guard remains explicit:
  `proofChecklist.blockerRefs` must be empty and
  `pylon-codex-own-capacity` token rows must be exact.
- The route test keeps raw trace chunks, safe-metadata JSON, and bearer-token
  language out of the rendered shell.

## Landed Slice 13: Artanis Trace Tree

The Start staging app now owns `/artanis/traces` as the public ref-only RLM
trace tree for Artanis.

- The route preserves `data-route="artanis-traces"` and the
  `Artanis execution tree` heading.
- The FRLM conductor structure remains visible:
  `FrlmConductor`, `SubQuery.Submit`, `SubQuery.Return`, and `Run.Done`.
- Blueprint governance refs remain visible:
  `program_signature.frlm_conductor.v1` and
  `program_signature.rlm_leaf_executor.v1`.
- The source/projection/privacy anchors remain public-safe:
  `/api/operator/rlm/traces`, `openagents.operator.rlm_traces.v1`, and
  `operator_refs_only`.
- The authority boundary keeps `No direct execution authority`, and the route
  test keeps raw prompts/traces, trajectory JSON, bearer tokens, and API keys
  absent.

## Landed Slice 14: Terms And Privacy

The Start staging app now owns `/terms` and `/privacy` as standalone legal
content pages, ported from the Foldkit `apps/web/src/page/terms.ts` and
`apps/web/src/page/privacy.ts` routes.

- Both routes preserve the copy verbatim, including the `PENDING OWNER /
  LEGAL REVIEW` notice banner, the `Last updated: 2026-06-23` date, and every
  numbered section's exact legal text (this is a legal document, so the
  original Foldkit language could not be paraphrased).
- The raw-hex Foldkit styling (`#000`, `#f1efe8`, `#ffb400`, `#7da3d9`) was
  swapped for the equivalent `khala-*` design tokens (`khala-void`,
  `khala-text`, `khala-warning`, `khala-energy-soft`/`khala-energy-cyan`) per
  the epic's StarCraft-token requirement.
- `/terms` and `/privacy` add `data-route="terms"` / `data-route="privacy"`
  anchors; the Foldkit originals had no `data-route` DOM contract for these
  two pages, so this is additive, not a behavior change.
- A shared `-legal-components.tsx` holds the section/paragraph/bullet
  helpers so the two pages do not duplicate ~40 lines of layout/typography
  boilerplate.
- Both routes were added to the Start budget list and `routeTree.gen.ts` was
  regenerated; total client JS stayed at 628.1 KiB (well under the 760 KiB
  budget), with each new route chunk under 8 KiB.
- This slice keeps the old Foldkit `terms.ts` / `privacy.ts` pages in place
  in `apps/web`, same as every prior TS-6 slice: the live `openagents.com`
  Worker still serves `apps/web/dist`, so deleting the Foldkit counterpart
  now would remove the live page from production before any cutover.

## Landed Slice 15: Khala Code Landing

The Start staging app now owns `/code` — the Khala Code marketing/demo
landing, the sibling public page next to the already-migrated
`/code/download` install-paths route.

- `/code` preserves `data-route="code"`, the `data-pose="khala"` scene layer
  (reusing the shared `SceneLayer` from the Khala/Tassadar slice instead of
  duplicating the glow background), the `Khala Code` eyebrow, the `Code, on
  your own capacity` headline, the `model: openagents/khala` badge, and the
  intro paragraph copy verbatim.
- The representative simulated coding-agent conversation is preserved turn by
  turn: both user turns, both assistant turns' reasoning/response text, the
  `Plan` task checklist, the `read_file` and `cargo test` tool calls, both
  diffs, and the `src/greet.ts` code block with its `bun test · 6 passed`
  result — using plain Tailwind markup plus the shared `Badge` component
  rather than a new AiElements port (the retired Foldkit `AiElements` kit in
  `packages/ui` is still Foldkit-`Html`-based, not React, so this slice does
  not depend on it).
- The composer keeps its `data-chat-composer="khala-code"` anchor and
  `Ask Khala to change your code…` placeholder; it stays decorative (no wired
  submit), same posture as the retired page.
- **Routing-nesting bug found and fixed across the whole app in the same
  change.** Adding a plain `code.tsx` alongside the existing
  `code/download.tsx` would make `code.tsx` an implicit TanStack Router
  layout for everything under `/code/*` — and since none of this app's page
  components render `<Outlet />`, that silently breaks the child route (the
  parent's full page renders in the body instead of the child's, though the
  child's `head()` meta still applies, so the `<title>` looks right while the
  content is wrong). Verified concretely with a local `vite preview` + `curl`
  before/after.
  Checking for the same shape (`X.tsx` with a sibling `X/` folder) found four
  more already-shipped instances of this exact bug, all from earlier TS-6/TS-2
  slices: **`/blog/$slug` served the blog index**, **`/docs/$slug` served the
  docs index**, **`/khala/chat-sync` served the Khala info page**, and
  **`/business/kpi/$engagementRef` served the business funnel page** — every
  case silently discarding the deep-linked content while the tab title still
  matched. (`/components/$family` was not affected: it deliberately reuses the
  same `ComponentsPage` component with a `selectedFamily` prop rather than a
  distinct child page, so its shared `data-route="components"` marker across
  both `/components` and `/components/$family` is by design, not a bug.)
  Fixed all five (`blog`, `business`, `components` for consistency, `docs`,
  `khala`, plus the new `code`) the same way, following the existing
  `autopilot/index.tsx` + `autopilot/legal.tsx` convention already in this
  codebase: move `X.tsx` to `X/index.tsx` and change
  `createFileRoute('/X')` to `createFileRoute('/X/')`, so each parent and
  child are independent sibling leaf routes with no shared layout dependency.
  Re-verified all previously-broken paths render their own content after the
  fix (`/blog/introducing-khala-code` → `data-route="blog-post"`,
  `/docs/api` → `data-route="docs-page"`, `/khala/chat-sync` →
  `data-route="khala-chat-sync"`, `/business/kpi/...` →
  `data-route="business-kpi"`), and the full Start test/typecheck/build/budget
  sweep stayed green.

## Landed Slice 16: Artanis Accounts And Workspace Invite

The Start staging app now owns two more standalone public routes ported from
`apps/web/src/page/artanisAccounts.ts` and
`apps/web/src/page/loggedOut/page/workspaceInvite.ts`.

- `/artanis/accounts` is the owner-only operator account-observability
  surface (Codex/Claude coding-account cooldowns and usage windows). It lives
  under the same `artanis/` directory as the already-migrated
  `artanis/traces.tsx`, so there is no bare-`artanis.tsx` sibling and no risk
  of the nested-route layout bug from Slice 15.
  - Preserves `data-route="artanis-accounts"` (additive; the Foldkit original
    had no `data-route` DOM contract for this page), the `Artanis / accounts`
    eyebrow, `Operator account observability` heading, and the exact
    owner-only description and closing-boundary copy (`This surface is
    operator evidence and control only. It does not grant dispatch, spend,
    settlement, provider-account ownership transfer, or cross-owner routing
    authority.`).
  - The Foldkit original does a live client-side `fetch('/api/operator/
    accounts/status')` with 401/403 handling, a refresh button, per-account
    manual-reset buttons, and a live countdown timer — the first "genuinely
    live, owner-gated" page in this migration wave. Every prior TS-6 Start
    route has been static/SSR-only (no route in this app calls `fetch` or
    `useEffect` yet), so rather than being the first to introduce live
    client-fetch wiring, this slice keeps that same static posture: it
    honestly renders the **Unauthorized** state a real anonymous visitor gets
    today (no owner session exists in Start yet), keeps the column headers
    and an explicit "No operator account rows are available" empty state
    (no fabricated account rows, cooldowns, or usage numbers), and keeps the
    `/api/operator/accounts/status` and `/api/operator/accounts/reset`
    endpoint refs visible. Live refresh/reset/countdown behavior stays on the
    existing Foldkit operator page until this route carries real
    owner-session auth.
- `/workspaces/$workspaceId` is the sign-in gate a logged-out visitor sees on
  a shared project-workspace invite link.
  - Preserves `data-route="workspace-invite"` (additive, same reasoning as
    above), the `Workspace invite` eyebrow, `Open your project workspace`
    heading, the `Your project setup is waiting. Sign in to review the seeded
    notes and starter workflows.` body copy, the visible `workspaceId`, and
    the `Log in with GitHub` link to `/login/github`.
  - The raw-hex Foldkit styling (`#f1efe8`, `#222`, `#010102`) was swapped for
    the equivalent `khala-*` design tokens, same as the Terms/Privacy slice.
  - This is a single leaf route with no sibling `workspaces.tsx`, so there is
    no nested-route layout risk here either.
- Both routes were added to the Start budget list (`/artanis/accounts`,
  `/workspaces/workspace.public.invite_example`) and `routeTree.gen.ts` was
  regenerated; total client JS moved to 653.1 KiB (still well under the
  760 KiB budget).
- Re-verified with a local `vite preview` + `curl` pass: `/artanis/accounts`
  renders `data-route="artanis-accounts"` with its own title, `/workspaces/
  workspace.public.invite_example` renders `data-route="workspace-invite"`
  with its own title, and the existing `/artanis/traces` route still renders
  its own unrelated content (no cross-route regression from adding the new
  `artanis/accounts.tsx` sibling file).
- Kept both Foldkit `apps/web` counterparts in place, same as every prior
  TS-6 slice — the live `openagents.com` Worker still serves `apps/web/dist`,
  so deleting them now would remove the live pages before any production
  cutover.

## Landed Slice 17: Onboarding And Training Runs

The Start staging app now owns `/onboarding` and `/training/runs`, ported from
`apps/web/src/page/loggedOut/page/onboarding.ts` and `trainingRuns.ts`.

- `/onboarding` preserves `data-route="onboarding"`, the `OpenAgents
  Autopilot` header/eyebrow, the `Stop Babysitting Your AI` headline, the
  `Launch coding agents. Close your laptop. Stay in the loop from anywhere.`
  body copy, the `Start work. Walk away.` / `Your agents keep going.` block,
  and both `Log in with GitHub` links to `/login/github` (header + hero CTA).
  - **Scoped-down by design, not by shortcut.** The Foldkit view also defines
    a `funding` step with an interactive credit-amount slider and coupon
    toggle, but `initOnboardingModel()` always starts at `step: 'github'`,
    and nothing in this standalone `loggedOut` view ever dispatches
    `ClickedOnboardingStep({ step: 'funding' })` — that message is only fired
    from a *different*, already-authenticated `loggedIn/page/onboarding.ts`
    flow and from unit tests. So a real anonymous visitor at `/onboarding`
    today only ever sees the GitHub-login landing; this port covers exactly
    that reachable state and does not fabricate a funding-demo entry point
    that does not exist in production. The funding-demo branch stays
    back-logged with the rest of the unmigrated `loggedOut` pages.
- `/training/runs` preserves `data-route="training-runs"`, the `Training Runs`
  heading, and the `Public CS336 run state, verification, and settlement
  projection.` subtitle.
  - The Foldkit original drives a `PublicTrainingRunsModel` union (`Idle` /
    `Loading` / `Loaded` / `Failed`) fed by a client fetch against
    `/api/training/runs`. Following the same posture as `/artanis/accounts`
    in Slice 16, this port renders the model's own `PublicTrainingRunsIdle`
    state honestly — the exact `No Worker-authoritative training runs are
    recorded yet.` / `No run projection is available for this route.` empty
    copy the Foldkit view already shows before its fetch resolves — rather
    than fabricating run rows or wiring a first live fetch on a standalone
    page. The `/api/training/runs` and `/api/training/leaderboards` endpoint
    refs stay visible.
  - Lives at `training/runs/index.tsx` (`createFileRoute('/training/runs/')`)
    rather than a bare `training/runs.tsx`, because the Foldkit route also
    has a `/training/runs/$runId` detail child
    (`publicTrainingRunRouter`) — using the folder+index convention from
    Slice 15 avoids ever creating the `X.tsx` next to `X/` nested-layout
    footgun when that detail route is migrated later. The `$runId` detail
    route (per-run metrics, Real Gradient status, windows, receipts,
    leaderboard links) is out of scope for this slice and stays unmigrated.
- Both routes were added to the Start budget list (`/onboarding`,
  `/training/runs`) and `routeTree.gen.ts` was regenerated; total client JS
  moved to 659.1 KiB (still well under the 760 KiB budget).
- Re-verified with a local `vite preview` + `curl` pass: `/onboarding` renders
  `data-route="onboarding"` with its own title and copy, `/training/runs`
  renders `data-route="training-runs"` with its own title and the idle empty
  state, and the existing `/artanis/traces`, `/artanis/accounts`,
  `/workspaces/$workspaceId`, and `/code` routes are all unaffected.
- Kept both Foldkit `apps/web` counterparts in place, same as every prior
  TS-6 slice.

## Landed Slice 18: MirrorCode And Product Promises

The Start staging app now owns `/mirrorcode` and `/promises`, ported from
`apps/web/src/page/loggedOut/page/mirrorcode.ts` and `promises.ts`.

- Both Foldkit pages are entirely client-fetch-driven (`MirrorCodeRunsModel`
  and `PublicProductPromisesModel` / `PublicPromiseTransitionsModel` unions,
  each fed by a `Idle` / `Loading` / `Loaded` / `Failed` cold read on route
  entry). Every prior TS-6 Start route has stayed static/SSR-only, so both
  ports keep that posture rather than being first to wire a live fetch on a
  standalone page.
- `/mirrorcode` preserves `data-route="mirrorcode"`, the `data-mirrorcode-page`
  contract markers, the `Live data only / public tasks only` banner, the
  `MirrorCode, powered by Khala` headline and intro paragraph verbatim, and
  the entire always-static playground contract (the `MirrorCode-as-a-Service
  playground` section, its three numbered steps, the owner-gated-launch
  notice, and both `Launch intent` / `Status read` code blocks) — none of that
  content is behind the fetch in the Foldkit original, so it is ported in
  full rather than summarized. The three genuinely data-dependent panels
  (Live run, Execution visualizer, Leaderboard) render the model's own
  `MirrorCodeRunsIdle` empty-state copy honestly — `No runs yet — machinery
  shipped, awaiting first Phase-0 run` and `No execution rows to visualize
  yet`, both verbatim from `mirrorcode.test.ts` — rather than fabricating run
  rows, and the `/api/gym/mirrorcode/runs` endpoint stays visible.
- `/promises` preserves `data-route="promises"`, the top nav (`OpenAgents`
  home link, `Docs` / `JSON` / `Forum` links to `/docs/product-promises`,
  `/api/public/product-promises`, `/forum/f/product-promises`), the
  `Human-readable promise ledger` eyebrow, the `Product promises` headline,
  and the intro paragraph verbatim. Every panel (Registry Status, State Map,
  Product Areas, Current Caveats, Blockers And Evidence, the
  `proof.claim_upgrade_receipts.v1` Claim-upgrade audit panel with its
  `A passing receipt is evidence for a flip, never the flip itself.` rule
  sentence, and Promise records) renders the exact honest "nothing fetched
  yet" copy the Foldkit view already shows when both models are null —
  `Waiting for live registry.`, `None listed.`, `Waiting for
  /api/public/product-promises.`, and so on — rather than fabricating
  registry rows or receipts. The receipt-feed and audit-projection endpoint
  refs (`/api/public/product-promises/transitions`,
  `/api/public/product-promises/audit`) stay visible.
- Both routes were added to the Start budget list (`/mirrorcode`,
  `/promises`) and `routeTree.gen.ts` was regenerated; total client JS moved
  to 679.2 KiB (still under the 760 KiB budget).
- Re-verified with a local `vite preview` + `curl` pass: `/mirrorcode` renders
  `data-route="mirrorcode"` with its own title and copy, `/promises` renders
  `data-route="promises"` with its own title and copy, and the existing
  `/training/runs`, `/artanis/accounts`, `/artanis/traces`, `/code`,
  `/onboarding`, and `/gym` routes are all unaffected.
- Kept both Foldkit `apps/web` counterparts in place, same as every prior
  TS-6 slice.

## Landed Slice 19: Public Agent Console (Artanis, Adjutant, And Arbitrary Refs)

The Start staging app now owns `/artanis`, `/agents/$agentRef`, and
`/adjutant`, ported from the ~2,700-line
`apps/web/src/page/loggedOut/page/publicAgent.ts` that the prior slice
deliberately backlogged for its own dedicated pass.

- The Foldkit `view(model, agentRef)` function has three distinct branches:
  the special-cased `artanis` branch (always renders the full recruitment
  console regardless of load state), the `Loaded`/`Failed` branch for any
  `agentRef` whose `model.publicAgent` has resolved, and a minimal fallback
  shell (eyebrow "Public agent", the agent's display name, and "Loading
  public goal.") for every other case — including the true first-paint
  `PublicAgentIdle` state. No prior TS-6 Start route has wired a live client
  fetch on a standalone page, so this port renders exactly that first-paint
  state for every route: the full static console for `artanis` (matching the
  Foldkit special case), and the minimal fallback shell for `adjutant` and
  any other `agentRef` (matching what a real anonymous visitor sees before
  the Foldkit page's fetch resolves).
- `/artanis` (`artanis/index.tsx`, using the established `X/index.tsx` +
  `createFileRoute('/X/')` folder convention since `artanis/traces.tsx` and
  `artanis/accounts.tsx` already exist as siblings — a bare `artanis.tsx`
  would have made it an implicit parent layout for those two routes, the
  same nested-route footgun fixed in Slice 15) renders the full console:
  - The masthead (`ARTANIS console`, the `LIVE` badge, `Artanis` h1, `No
    public goal` / `Active slots loading` / `no active public run` status
    strip, and a 0% daily-token-pace bar) — the exact copy the Foldkit
    `artanisConsoleHeader` produces for a null goal and unresolved Pylon
    stats.
  - The Pulse panel renders `Token pace unavailable.`, the exact fallback
    `artanisPulseView` produces once `PublicKhalaTokensServedHistoryModel`
    is anything but `Loaded` (including the `Idle` first-paint tag).
  - The fleet map + task board (`data-component="artanis-fleet-map-task-board"`)
    renders all 8 fleet-map slots as genuinely empty
    (`data-fleet-map-slot="empty"`, "no public heartbeat") and all four task
    board lanes (Ready / Claimed / Verifying / Resolved) with "No public rows
    in this lane." — this is not fabricated: `fleetMapSlots(null)` and
    `activeTaskBoardEvents` on a non-`Loaded` model produce exactly these
    outputs in the Foldkit original.
  - The virtual merge queue (`data-component="artanis-virtual-merge-queue"`)
    and the fleet-onboarding panel ("Have Codex or Claude? Join the fleet.",
    the `khala fleet connect` command block) are ported in full fidelity —
    neither Foldkit view function takes a model argument, so nothing here is
    an idle placeholder.
  - The 3-column grid (campaign objective / fleet shipping / Pylon stats +
    onboarding) renders the campaign-objective goal panel verbatim, the
    fleet-shipping feed's own `Loading live fleet activity.` fallback text,
    and the Pylon-stats panel's own `-` metrics / `Feed loading` / `Loading
    recent Pylon presence.` fallback copy.
- `/agents/$agentRef` (`agents/$agentRef.tsx`, no nested-layout risk since
  there is no sibling `agents.tsx` or `agents/index.tsx`) reproduces the same
  `agentRef === 'artanis'` special case for URL parity with `/artanis`
  (matching `urlToAppRoute` mapping both paths to the same route value), and
  falls back to the generic shell for every other ref.
- `/adjutant` (top-level, no sibling `adjutant/` folder so no nested-layout
  risk) renders the same generic fallback shell as any other non-artanis
  `agentRef` — the Foldkit `isAdjutant`-specific Autopilot-activity content
  (deployed Sites, milestones) only renders once `model.publicAgent` reaches
  `Loaded`/`Failed`, which requires the live fetch this slice intentionally
  does not wire.
- Added `/artanis`, `/agents/artanis`, and `/adjutant` to the Start budget
  list; total client JS moved to 696.7 KiB (still under the 760 KiB budget).
- Re-verified with a local `vite preview` + `curl` pass: `/artanis` and
  `/agents/artanis` both render `data-route="artanis"` with the identical
  `Artanis - OpenAgents` title and console markup; `/adjutant` and
  `/agents/some-other-ref` both render `data-route="public-agent"` with
  their own display name and `Loading public goal.`; and
  `/artanis/traces`, `/artanis/accounts`, `/code`, `/mirrorcode`,
  `/training/runs`, `/promises`, `/onboarding`, `/gym`, and `/login` are all
  unaffected by the new `artanis/index.tsx` and `agents/$agentRef.tsx`
  siblings.

Not migrated in this slice (deliberately out of scope, same posture as every
prior live-data page): the rich `Loaded`-state content for `artanis` (real
token-burn pace, real Pylon/fleet-map rows, real task-board rows, real
fleet-shipping events, real campaign-goal data) and for `adjutant` (real
Autopilot deployed-Sites/milestone rows) — all of it requires a live client
fetch this app does not wire yet, same as `/mirrorcode`, `/promises`, and
`/training/runs`.

## Landed Slice 20: Training Run Detail

The Start staging app now owns `/training/runs/$runId`, the per-run detail
alias of the already-migrated `/training/runs` list route, ported from the
same `apps/web/src/page/loggedOut/page/trainingRuns.ts` this slice's
predecessor used.

- The Foldkit `publicTrainingRunRouter` (`apps/web/src/route.ts`) parses
  `training/runs/{runId}` into `PublicTrainingRunRoute({ runId })`, and
  `loggedOut/view.ts` renders both this route and the list route with the
  exact same `TrainingRuns.view(model.publicTrainingRuns, runId)` function —
  the list route passes `runId: null`, this route passes the requested
  `runId`. In the Idle first-paint state (the only state any prior TS-6 route
  has rendered, since none wire a live fetch yet), the Foldkit
  `selectedSummary(model, runId)` always returns `null` regardless of `runId`
  — there are no loaded summaries to search yet — so the two routes produce
  byte-identical panel markup honestly: the same "No Worker-authoritative
  training runs are recorded yet." / "No run projection is available for this
  route." empty copy, rather than fabricating a runId-specific message the
  Foldkit view does not produce.
- `training/runs/$runId.tsx` lives as a sibling of the existing
  `training/runs/index.tsx` inside the `training/runs/` folder (no bare
  `runs.tsx` file exists, so there is no nested-layout-footgun risk — the same
  pattern as `artanis/traces.tsx` and `artanis/accounts.tsx` sitting alongside
  `artanis/index.tsx`). `TrainingRunsPage` (the shared component in
  `-training-runs-page.tsx`) now takes an optional `runId` prop; when present
  it renders a `data-run-id` attribute (mirroring the `data-agent` attribute
  on the `/agents/$agentRef` port from the prior slice) so the two routes stay
  distinguishable in tests and markup without fabricating divergent content.
  The detail route also gets its own document title,
  `Training run {runId} - OpenAgents` — the Foldkit `title()` switch in
  `view.ts` has no case for either `PublicTrainingRun` or
  `PublicTrainingRuns`, so both fall back to the generic `OpenAgents` title
  there; this is a reasonable improvement over that fallback rather than a
  fidelity regression.
- Added `/training/runs/run.cs336.a1.demo` (the real example run ref named in
  this repo's own `route-table.ts`) to the Start budget list; total client JS
  moved to 697.6 KiB (still under the 760 KiB budget), with the new route's
  own chunk at well under 1 KiB since it reuses the existing
  `-training-runs-page` chunk.
- Re-verified with a local `vite preview` + `curl` pass: `/training/runs`
  renders `data-route="training-runs"` with no `data-run-id` attribute;
  `/training/runs/run.cs336.a1.demo` renders the same `data-route` plus
  `data-run-id="run.cs336.a1.demo"`, the title
  `Training run run.cs336.a1.demo - OpenAgents`, and the identical honest
  empty-state copy; and `/artanis`, `/artanis/traces`, `/artanis/accounts`,
  `/agents/artanis`, `/adjutant`, `/code`, `/mirrorcode`, `/promises`, and
  `/onboarding` are all unaffected.

While scoping this slice, confirmed (rather than assumed) that the `stats.ts`
public/anonymous variant remains genuinely out of reach for a quick pass: none
of the ~9 shared panel functions it composes from `home.ts`
(`khalaTokensServedHeaderCounter`, `khalaTokensServedHistoryChart` (a real
chart, ~200 lines), `khalaTokensServedModelMixPanel`,
`khalaTokensServedChannelMixPanel`, `pylonStatsPanel`, `forumStatsPanel`,
`accountingPanel`, `copyBoundaryPanel`, `endpointManifestPanel`,
`nostrRelayPanel`) exist yet as React ports anywhere in the Start app or in
`@openagentsinc/ui`. It stays backlogged for its own dedicated pass, same
posture as `pylon.ts` and `share.ts`.

## Deprecation Note: Training Runs / Gym (2026-07-05)

**The training-runs/gym feature is intentionally deprecated-for-now** (owner
decision, 2026-07-05), immediately after Slice 20 landed above. This is a
scope decision, not a migration gap — do not treat it as unmigrated or
incomplete in future TS-6 passes, and do not spend further effort porting or
polishing training-runs/gym UI until the owner reinstates it.

What changed:

- Both Start routes (`training/runs/index.tsx` and `training/runs/$runId.tsx`)
  now render a shared `TrainingRunsDeprecatedPage`
  (`-training-runs-deprecated-page.tsx`) — an honest "This page is temporarily
  unavailable" notice with `data-route="training-runs-deprecated"` — instead
  of the real idle-state UI landed in Slice 20. The real `TrainingRunsPage`
  component (`-training-runs-page.tsx`, with the `runId` prop added in
  Slice 20) and its test coverage (`-training-runs.test.tsx`) are left in
  place, dormant and unreferenced by any route, for restoration later.
- The Start funnel budget list (`-funnel-budget.ts`) markers for both
  `/training/runs` paths were updated to `'temporarily unavailable'` to match
  the new notice copy; total client JS dropped slightly to 695.7 KiB.
- Removed the two live nav entry points that surfaced this feature on the
  legacy Foldkit pages still served by the production Worker: the "Training
  runs" sidebar link on the `/demo` "Training Live" sidebar
  (`apps/web/src/page/demo/view.ts`) and the "Training runs" footer link on
  `/tassadar` (`apps/web/src/page/loggedOut/page/tassadar.ts`). Both removals
  are commented in place pointing back to this note.
- **Not touched, per the owner's explicit "restorable later" instruction:**
  the legacy Foldkit `apps/web/src/page/loggedOut/page/trainingRuns.ts` page
  itself, its Foldkit route registration
  (`publicTrainingRunsRouter`/`publicTrainingRunRouter` in
  `apps/web/src/route.ts`), and all backend training-run data/API surfaces
  (`workers/api/src/training-*.ts`, `training_run_window*` D1 tables/
  migrations, `/api/training/runs`, `/api/training/leaderboards`). The
  feature is hidden from discovery, not deleted or data-scrubbed — a direct
  URL hit on the legacy Foldkit page still works exactly as before.
- Companion sync-engine issue [#8415](https://github.com/OpenAgentsInc/openagents/issues/8415)
  (KS-6.5 gym run-progress public-projection cutover) was closed as not
  applicable given this deprecation — see the issue comment for detail.

Re-verified after this change: `bun run --cwd apps/openagents.com/apps/start test` (32 files, 98 tests), `typecheck`, `build`, and `budget` all green; `bun run --cwd apps/openagents.com/apps/web test` (155 files, 1795 tests) green after the two nav-link removals; a local `vite preview` + `curl` pass confirms both `/training/runs` and `/training/runs/run.cs336.a1.demo` render the deprecated notice and every other route checked in Slice 20 is still unaffected.

## Landed Slice 21: Stats (Public/Anonymous Variant)

The Start staging app now owns `/stats` — the public/anonymous variant of the
`StatsRoute` — ported from
`apps/web/src/page/loggedOut/page/stats.ts`. The same URL also has a distinct
authenticated `loggedIn/page/stats.ts` view (real account dashboards, private
settlement detail); that view stays out of scope, same "public-safe default
until Start has real session auth" treatment as `/artanis/accounts`.

- **Re-checked, not just re-assumed, the "needs 9 unbuilt panel components"
  premise from Slice 20.** Searched `@openagentsinc/ui` and the whole Start
  app for existing React ports of the nine shared panel functions
  `stats.ts` composes from `home.ts`
  (`khalaTokensServedHeaderCounter`, `khalaTokensServedHistoryChart`,
  `khalaTokensServedModelMixPanel`, `khalaTokensServedChannelMixPanel`,
  `pylonStatsPanel`, `forumStatsPanel`, `accountingPanel`,
  `copyBoundaryPanel`, `endpointManifestPanel`, `nostrRelayPanel`) — none
  exist yet, confirming the prior finding. But every one of those Foldkit
  functions is itself model-driven (`Idle` / `Loading` / `Loaded` / `Failed`
  unions fed by client fetches this app does not wire), and no prior TS-6
  Start route has wired a live client fetch on a standalone page. Following
  that established posture, the real unblocking move was not "port 9 live
  React data panels" but "render the exact Idle first-paint markup each
  panel already produces before its fetch resolves" — the same technique
  already used for `/promises`, `/mirrorcode`, and `/training/runs`. That
  made this batch tractable as a single static page rather than a
  multi-panel component-library project, without fabricating a single
  number.
- `-stats-page.tsx` (a new `StatsPage` component, following the established
  per-page local `PanelHeader`/`MetricRow` convention already used by
  `-promises-page.tsx`, `-mirrorcode-page.tsx`, and
  `-training-runs-page.tsx` rather than a shared cross-page primitive) ports:
  - The hero row: `data-route="stats"`, the `Home` link, the `Network Stats`
    heading and its exact subtitle copy, and the tokens-served hero counter
    (`data-counter="khala-tokens-served"`, `data-counter-display`, and
    `data-value="—"` — the exact Foldkit `khalaTokensServedHeaderCounter`
    idle-state placeholder and caption).
  - The history chart + mix-panel row: `data-chart="khala-tokens-served-history"`
    on all three chart shells (matching the Foldkit `historyChartShell`
    helper, which sets that same data attribute on every panel that uses
    it), the `Tokens Served / Day` / `Model Family Mix` / `Channel Mix`
    titles, the Daily/Cumulative metric toggle (rendered because `stats.ts`
    calls the history chart with `showMetricToggle: true`), and the exact
    `Waiting for data…` / `Waiting for model mix…` / `Waiting for channel
    mix…` idle bodies plus their captions.
  - The two-column evidence grid: `Pylon Stats`, `Forum Stats`, and
    `Accounting Strip` on the left (every metric row rendering the exact
    `Unavailable` value and idle-state detail/tone the Foldkit functions
    produce for null models — including the one literal-typo copy string
    `Active \ orange check badges bought by registered agents...` preserved
    byte-for-byte from `home.ts`, and the `warn`-tone rows for `Tip gate` and
    `Accepted-work gate` kept visually distinct from the `muted` rows); and
    the always-static `Claim Boundary` and `Endpoint Manifest` panels plus
    `Nostr Relay Configuration` on the right (the first two are fully static
    in the Foldkit original — no fetch dependency — so they're ported in
    full fidelity; the relay panel renders its own idle-state `Unavailable`
    rows and the `No relay endpoint list is public in the current response.`
    fallback line).
  - A closing "Live surface" panel naming the four backing endpoints
    (`/api/public/pylon-stats`, `/api/forum/tip-leaderboards`,
    `/api/forum/launch-status`, `/api/public/product-promises`), same
    pattern as the closing panel on `/promises` and `/mirrorcode`.
- Raw-hex Foldkit styling was swapped for `khala-*` design tokens throughout
  (no exact-hex preservation needed since the project's `khala-*` tokens are
  the equivalent brand colors, same as every prior slice's styling swap).
- `/stats` was added to the Start budget list; total client JS moved to
  708.8 KiB (still under the 760 KiB budget), with the new route's own chunk
  at 12.7 KiB.
- Re-verified with a local `vite preview` + `curl` pass: `/stats` renders
  `data-route="stats"` with its own title (`Network Stats - OpenAgents`) and
  every panel listed above; `/promises`, `/mirrorcode`, `/training/runs`,
  `/artanis`, and `/code` are all unaffected by the new sibling `stats.tsx`
  route file (no nested-route-layout risk either — `stats.tsx` has no
  sibling `stats/` folder).
- Kept the Foldkit `apps/web` counterpart (`stats.ts`, plus the untouched
  authenticated `loggedIn/page/stats.ts`) in place, same as every prior TS-6
  slice — the live `openagents.com` Worker still serves `apps/web/dist`.

Verification:

```sh
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-stats.test.tsx
bun run --cwd apps/openagents.com/apps/start test
bun run --cwd apps/openagents.com/apps/start typecheck
bun run --cwd apps/openagents.com/apps/start build
bun run --cwd apps/openagents.com/apps/start budget
bun run --cwd apps/openagents.com check:architecture
bun run --cwd apps/openagents.com check:deploy
bun run test:qa-pre-push-smoke
```

- `bun run --cwd apps/openagents.com/apps/start test` ✅ (33 files, 105 tests)
- `bun run --cwd apps/openagents.com/apps/start typecheck` ✅
- `bun run --cwd apps/openagents.com/apps/start build` ✅
- `bun run --cwd apps/openagents.com/apps/start budget` ✅ 708.8 KiB (under
  760 KiB budget)
- `bun run --cwd apps/openagents.com check:architecture` ✅ zero-debt clean
- `bun run --cwd apps/openagents.com check:deploy` ✅ full sweep clean
- `bun run test:qa-pre-push-smoke` ✅ (7 pass)

`pylon.ts` (three.js/scene custom elements) and `share.ts` (shared-workroom-
timeline viewer) remain the two standalone `loggedOut` pages still backlogged
for their own dedicated passes — both are still genuinely bigger lifts than a
static idle-state port, unlike `stats.ts`.

## Boundary

This is not the final TS-6 closure. The live `openagents.com` Worker still
serves `apps/web/dist`, and the Foldkit counterparts remain in `apps/web`
because these Start routes are not production-cut-over on the real domain yet.
Delete-as-you-go starts only when a route is actually cut over from the live
Worker to Start.

Remaining TS-6 work:

- migrate the remaining standalone public/`loggedOut` pages that are not yet
  in Start: `apps/web/src/page/loggedOut/page/pylon.ts` (depends on the
  three.js/scene custom elements — bigger lift than a plain static port) and
  `share.ts` (a full shared-workroom-timeline viewer — bigger lift, needs the
  workroom timeline/file-panel component set ported to React first).
  `stats.ts` (public/anonymous variant) was migrated in Slice 21 above,
  rendering the honest Idle first-paint state for all nine shared panels; the
  distinct authenticated `loggedIn/page/stats.ts` view stays out of scope
  until Start has real session auth, same as the rest of the `loggedIn/` tree
  below. `/training/runs/$runId` was migrated in Slice 20, but the whole
  training-runs/gym feature (both Start routes and their nav entry points) is
  now **deprecated-for-now** per the owner decision recorded above — do not
  pick this back up (live-data wiring, nav restoration, or otherwise) until
  the owner reinstates it;
- migrate logged-in app-shell panels route-by-route (this is the large
  authenticated `loggedIn/` tree behind `Ui.workroomShell` — dozens of
  interconnected panels including chat, dashboard, billing, settings, admin,
  and workroom — and genuinely needs a real Start session/auth mechanism
  before any panel beyond the standalone `Ui.pageShell` surfaces like
  Onboarding/Pro/Order can be ported honestly; still fully unstarted);
- migrate or explicitly retire the Forum web shell from `apps/web`;
- cut production routes over from the Start Worker;
- delete each Foldkit counterpart after its production route cutover;
- repoint the `ASSETS` binding and remove `apps/web` from the build.

## Verification

```sh
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-code.test.tsx src/routes/-app-shell.test.tsx src/routes/-components.test.tsx src/routes/-gym.test.tsx src/routes/-index.test.tsx src/routes/-artanis-accounts.test.tsx src/routes/-workspace-invite.test.tsx src/routes/-mirrorcode.test.tsx src/routes/-promises.test.tsx src/routes/-artanis-console.test.tsx src/routes/-public-agent.test.tsx src/routes/-training-runs.test.tsx src/routes/-training-runs-deprecated.test.tsx
bun run --cwd apps/openagents.com/apps/start test
bun run --cwd apps/openagents.com/apps/start typecheck
bun run --cwd apps/openagents.com/apps/start build
bun run --cwd apps/openagents.com/apps/start budget
bun run --cwd apps/openagents.com check:architecture
bun run --cwd apps/openagents.com check:deploy
bun run test:qa-pre-push-smoke
```

The route tests are the parity guard for this slice; the full final TS-6
closure still needs the per-route visual smokes and delete evidence named in
#8348.
