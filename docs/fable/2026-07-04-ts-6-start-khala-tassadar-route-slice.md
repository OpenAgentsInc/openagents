# TS-6 Route Slices On Start

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


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

## Landed Slice 22: Pylon Network Hero (Chrome Ported, 3D Scene Bridged)

The Start staging app now owns `/pylons` — the Pylon network hero page,
ported from `apps/web/src/page/loggedOut/page/pylon.ts` — with one deliberate,
documented exception: the literal WebGL diamond-refraction scene.

**Read the actual scene code before assuming the "biggest lift" reputation.**
Prior slices repeatedly flagged `pylon.ts` as the biggest remaining lift
"because of the three.js scene." Reading the four scene modules it composes
(`scene/pylonElement.ts`, `scene/pylonBezierNetworkElement.ts`,
`scene/pylonStatsElement.ts`, `scene/pylonLaunchGateElement.ts`) found that
three of the four layers have zero dependency weight and are trivially
portable:

- The install CTA (`pylonInstallCta` in `pylon.ts` itself) is static markup —
  a direct Tailwind port, same as every prior slice.
- The bezier network graph (`pylonBezierNetworkElement.ts`) is pure SVG + math
  (a golden-angle ring layout and quadratic bezier paths) polling
  `GET /api/public/pylon-stats` — no 3D dependency at all, "Foldkit custom
  element" was the only thing making it look heavier than it is.
- The stats overlay (`pylonStatsElement.ts`) is four live-polled counters; the
  legacy version uses the `slot-text` package for a digit-roll animation, which
  this port skips (plain text updates) as a decorative simplification, not a
  content change — same values, no separate dependency.
- The launch gate (`pylonLaunchGateElement.ts`) branches on a fixed deadline
  (June 15, 2026, 1 PM America/Chicago) that has already passed and can only
  ever be in the past from here forward — so, same scoping posture as the
  `/onboarding` funding-step branch in Slice 17, only the reachable
  post-launch "Copy Agent Instructions" state is ported; the Effect-driven
  countdown timer is genuinely dead code for any visitor from today onward.

The one layer that is NOT ported: `scene/pylonDiamonds.ts`, the literal
WebGL two-diamond backface/refraction-shader scene. This is a **hard bundle-
budget conflict, not a shortcut.** `pylonDiamonds.ts` imports `three` (the
`three.module.min.js` build alone is ~365 KiB minified, before this app's own
code or the `GLTFLoader` it also needs) plus `@openagentsinc/three-effect/
core`. The Start funnel enforces a 760 KiB total-client-JS budget across every
route (`-funnel-budget.ts`, checked via `bun run budget`) plus a 120 KiB per-
route-chunk cap, and that check sums every emitted `.js` file in the build
output regardless of route-level code-splitting or dynamic `import()` laziness
— so even a lazily-loaded three.js chunk would still blow the total-budget
assertion. At 708.8 KiB going into this slice, there was only ~51 KiB of
headroom; `three` + `GLTFLoader` would blow both budgets by several times
over. That budget is a deliberate, existing performance gate for this exact
bundle (the funnel pages are meant to stay fast; the current 4.1 MB Foldkit
`apps/web` bundle is the explicit low bar being improved on, not matched) —
relaxing it mid-migration for one decorative background scene is a call for
the owner, not something to do unilaterally in this batch.

**What was bridged instead:** a design-consistent ambient glow backdrop
(`data-pylon-scene="ambient-placeholder"`) reusing the same blue-glow visual
language already established for `SceneLayer` on `/code` and `/khala` — not a
fabricated diamond shape, just the same "dark field + soft blue radial glow"
treatment this product already uses elsewhere. The literal WebGL diamond mesh
stays on the legacy Foldkit `/pylons` page (`apps/web`, still served by the
production Worker) until an explicit follow-up decision: either a dedicated
lazy sub-bundle exempted from the total-JS budget check, or a raised budget
specifically for scene-bearing routes.

**First Start route to wire a genuine client-side fetch.** Every prior TS-6
route stayed static/SSR-only rather than being first to call `fetch` from a
mounted component, rendering the model's own Idle first-paint state instead.
This route breaks that posture deliberately: it polls the real, existing
public `GET /api/public/pylon-stats` endpoint (no auth, no spend, no
mutation — the same endpoint the legacy page already uses) to drive the live
stats counters and the bezier network's node/edge rendering. The reasoning:
unlike the text/copy pages that stayed Idle-only (where the Idle empty-state
copy is itself an honest, meaningful rendering), this page's entire purpose is
showing live network state — permanently freezing it at the pre-fetch
placeholder would not be a faithful port of what this page is for. Fail-soft
is preserved exactly like the legacy page: any fetch error (including the
expected 500 when previewing the Start app in isolation without the
`workers/api` Worker attached — see caveat below) renders the same dormant/
loading state, never a fabricated number.

Preserved contract:

- `data-route="pylon"`, the full install-CTA copy and data attributes
  (`data-cta="install-pylon"`, `install-pylon-command`,
  `download-autopilot-link`), the exact `npx @openagentsinc/pylon` command,
  and the `/download` link.
- The four live stat labels verbatim (`pylons online`, `work-ready now`,
  `sats settled · 24h`, `training contributors`) with the same
  `data-stat-value` keys, showing the same `…` loading placeholder before the
  first poll resolves that the legacy page shows.
- The bezier network's exact math (golden-angle ring layout, quadratic bezier
  edges, lit-node opacity driven by the same `computeActivityIntensity`
  formula) — ported to `-pylon-network.ts` since this app cannot import from
  `apps/web` (separate package).
- The "Copy Agent Instructions" control (`data-cta="copy-agent-instructions"`)
  with the exact same states and copy: `Copy Agent Instructions` / `Copying...`
  / `Copied` (`Copied from openagents.com/AGENTS.md`) / `Copy failed`
  (`Open /AGENTS.md`), fetching the real `/AGENTS.md` and writing to the
  clipboard, same as the legacy control.
- `/pylons` added to the Start budget list; total client JS moved to
  716.2 KiB (still well under the 760 KiB budget — confirming the
  three.js-avoidance decision above was the right call), with the new route's
  own chunk at 7 KiB (well under the 120 KiB per-route cap).

Known caveat (documented, not hidden): in a `vite preview` of the Start app
run in isolation (no `workers/api` Worker attached), `/api/public/pylon-stats`
and `/AGENTS.md` both 404/500, since this staging app does not yet proxy or
service-bind to the API Worker. The page degrades exactly as designed (loading
placeholders, "Copy failed" state, zero console/page errors) rather than
crashing — confirmed with a headless-browser pass (see Verification). On the
real production domain (or once `/pylons` is cut over into the same Worker
that serves the API), both calls hit the real endpoints.

Verified with a local `vite preview` + `curl`/headless-browser pass:
`/pylons` renders `data-route="pylon"` with its own title
(`Pylon - OpenAgents`) and every marker above; a Chromium/Playwright pass
confirms the canvas-free scene mounts with zero `pageerror`s and the only
console error is the expected same-origin API 404/500 described above;
`/code`, `/mirrorcode`, `/artanis`, `/stats`, `/download`, `/promises`,
`/training/runs`, `/onboarding`, `/gym`, and `/login` are all unaffected.

Kept the Foldkit `apps/web` counterpart (`pylon.ts` plus its four scene
modules) in place, same as every prior TS-6 slice — the live `openagents.com`
Worker still serves `apps/web/dist`, and the literal WebGL scene has no Start
equivalent yet.

Verification:

```sh
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-pylons.test.tsx
bun run --cwd apps/openagents.com/apps/start test
bun run --cwd apps/openagents.com/apps/start typecheck
bun run --cwd apps/openagents.com/apps/start build
bun run --cwd apps/openagents.com/apps/start budget
bun run --cwd apps/openagents.com check:architecture
bun run --cwd apps/openagents.com check:deploy
bun run test:qa-pre-push-smoke
```

- `bun run --cwd apps/openagents.com/apps/start test` ✅ (34 files, 110 tests)
- `bun run --cwd apps/openagents.com/apps/start typecheck` ✅
- `bun run --cwd apps/openagents.com/apps/start build` ✅
- `bun run --cwd apps/openagents.com/apps/start budget` ✅ 716.2 KiB (under
  760 KiB budget), new route chunk 7 KiB (under 120 KiB per-route cap)
- `bun run --cwd apps/openagents.com check:architecture` ✅ zero-debt clean
- `bun run --cwd apps/openagents.com check:deploy` ✅ full sweep clean
- `bun run test:qa-pre-push-smoke` ✅ (7 pass)

`share.ts` (shared-workroom-timeline viewer) remains the one standalone
`loggedOut` page still backlogged for its own dedicated pass — it needs the
workroom timeline/file-panel component set ported to React first. `pylon.ts`
is now migrated for every layer except the literal WebGL scene, which needs
an explicit bundle-budget decision (see above) before it can follow.

## Landed Slice 23: Share (Shared Workroom Timeline Viewer)

The Start staging app now owns `/share/$shareId` — the shared
workroom-timeline viewer, ported from
`apps/web/src/page/loggedOut/page/share.ts`. This was the last fully-
unmigrated standalone public/`loggedOut` page in the TS-6 sweep.

- **The "needs the workroom timeline/file-panel component set ported to
  React first" premise from every prior slice was real, but re-checking it
  (not just re-quoting it) found the actual surface tractable rather than a
  multi-thousand-line project.** `share.ts` itself only reimplements one of
  the two message renderers inline (`shareUserTimelineMessage`, the
  user-message layout) and calls the shared `@openagentsinc/ui` Foldkit-`Html`
  `workroomTimelineMessage` / `workroomTimelinePart` / `workroomFilePanel`
  functions (`packages/ui/src/workroom.ts`) for everything else. Reading those
  functions end to end (not assuming their line count implied irreducible
  complexity) found they compose down to four timeline-part kinds
  (text/tool/diff/file), a metadata key-value list, and a review-file list —
  all straightforward Tailwind ports once isolated from the retired
  `oa-ui-workroom-*` CSS-in-JS classes those functions style with. No
  interactive client JS drives the tool-call "collapsible" chrome within that
  render tree either (it never toggles in the ported source), so this port
  renders tool detail as always-visible rather than reproducing an inert
  expand/collapse affordance — a decorative simplification, not a content
  change, same posture as the digit-roll-animation skip on `/pylons`.
- New `-share-timeline.tsx` is the first React port of that shared component
  set: `ShareTimelinePart` (text/tool/diff/file), `ShareTimelineMessage`
  (dispatches to the user-message layout or the generic assistant/system
  layout), and `shareMessagePreview` for the message-navigation sidebar.
  `-share-fetch.ts` holds the live-fetch logic and a direct port of
  `apps/web/src/display-copy.ts`'s `userFacingCopy` (rewrites the internal
  "Adjutant" codename to "Autopilot" before anything user-facing renders).
  `-share-page.tsx` composes the full page: header (logo, audience/title,
  status pill, review-item count, copy-link button, "Open source run/thread"
  link), the session title block (audience/source/status badges, title,
  subtitle, event/tool/token metrics), the message timeline with its sticky
  message-navigation sidebar (only rendered past one user message, matching
  the legacy threshold), the review side panel (metadata rows + file/artifact/
  approval/receipt list + a "Share" dock action), a `<details>`-based mobile
  review panel, and the 401/403/410/404 failed-state branches
  (`ShareFailedView`, exported separately for direct testing) plus the
  pre-fetch loading state.
- **Type reuse, not reinvention.** This route imports the canonical
  `ShareProjectionV1` / `WorkroomTimelineMessage` / `WorkroomTimelinePart` /
  `WorkroomFileItem` types directly from `@openagentsinc/sync-schema` (added
  as a new Start dependency) via `import type` — zero runtime bundle cost,
  and the wire shape stays the single source of truth shared with
  `workers/api` and the legacy Foldkit page, instead of a second hand-rolled
  copy of the same types.
- **Live-fetch exception, same reasoning as `/pylons`.** A share link's whole
  purpose is showing one specific shared conversation, so freezing at the
  pre-fetch idle placeholder (the posture used for `/mirrorcode`, `/promises`,
  `/stats`, `/training/runs`) would not be a faithful port of what this page
  is for. This route fetches the real, existing
  `GET /api/share/{shareId}/v1/data` endpoint once on mount — same request
  shape as the legacy page (`cache: 'no-store'`, `credentials: 'include'`, so
  team/user-audience shares still resolve for a signed-in visitor), no new
  endpoint, no mutation. Fail-soft is preserved exactly: any fetch/parse
  error (including a malformed response) falls through to the same honest
  "Share not found" fallback the legacy `failedBody` renders for an
  unrecognized status, never fabricated transcript content.
- Raw-hex Foldkit styling was swapped for `khala-*` design tokens throughout,
  using the same mapping already established in the Terms/Privacy slice
  (`#ffb400` -> `khala-warning`, `#000`/`#010102` -> `khala-void`/
  `khala-surface`, `#f1efe8` -> `khala-text`) plus the tone-dot mapping already
  used by `/stats`' warn-tone rows extended to tool status
  (`#00c853` -> `khala-success`, `#d32f2f` -> `khala-danger`, the
  `#2979ff`-ish info blue -> `khala-energy`). `Copy` / `ExternalLink` /
  `Terminal` icons come from `lucide-react` (already a Start dependency),
  matching the icon names the legacy `iconView('Copy' | 'ExternalLink' |
  'Terminal', ...)` calls used.
- `share/$shareId.tsx` has no sibling `share.tsx`, so this is a single leaf
  route with no nested-route-layout risk (the footgun fixed across five
  routes in Slice 15).
- Added `/share/123e4567-e89b-42d3-a456-426614174000` (the canonical example
  share UUID already used as a fixture in `apps/web/src/main.test.ts`) to the
  Start budget list, with the honest pre-fetch "Loading share" marker (same
  posture as the deprecated training-runs entries' "temporarily unavailable"
  marker) since an isolated preview has no real backing share to resolve.
  Total client JS moved to 736.6 KiB (still well under the 760 KiB budget),
  with the new route's own chunk at 20 KiB (under the 120 KiB per-route cap).
- New test file `-share.test.tsx` (9 tests) covers: the honest pre-fetch
  loading state; the full loaded view (header, title block, metrics, copy
  link, "Open source run" link); every timeline-part kind rendering correctly
  (text, tool with its shell-output block, diff with `+N -M` counts, file
  write with its excerpt) plus the Adjutant -> Autopilot copy rewrite; the
  side panel and mobile review panel appearing when review items exist; the
  honest "No messages" empty state for a share with no transcript messages;
  and all four failed-state branches (401 sign-in gate with the `returnTo`
  redirect, 403 forbidden, 410 expired vs. revoked, and the generic 404-style
  not-found fallback).
- Verified with a local `vite preview` + `curl` + headless-browser
  (Playwright/Chromium) pass: the route renders `data-route="share"` with its
  own title (`Shared conversation - OpenAgents`); SSR renders the honest
  "Loading share" state (no browser fetch runs during static rendering); the
  client-hydrated pass shows zero `pageerror`s and degrades to the honest
  "Share not found" state on the expected same-origin 500 (no `workers/api`
  Worker attached in this isolated preview, same caveat as `/pylons`); and
  `/pylons`, `/stats`, `/code`, `/mirrorcode`, `/promises`, `/training/runs`,
  and `/artanis` are all confirmed unaffected by the new sibling route files.

**This closes out the standalone `loggedOut` public-page backlog.** Every
fully-unmigrated standalone public page named across this document
(`share.ts`, `pylon.ts`, `stats.ts`, and the rest of the Slice-1-through-22
list) now has a Start route. The two genuinely open follow-ups are: (1) an
explicit owner decision on the `pylon.ts` WebGL diamond-scene bundle-budget
exemption (see Slice 22), and (2) the large authenticated `loggedIn/`
app-shell tree, which needs real Start session/auth before it can start.

Verification:

```sh
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-share.test.tsx
bun run --cwd apps/openagents.com/apps/start test
bun run --cwd apps/openagents.com/apps/start typecheck
bun run --cwd apps/openagents.com/apps/start build
bun run --cwd apps/openagents.com/apps/start budget
bun run --cwd apps/openagents.com check:architecture
bun run --cwd apps/openagents.com check:deploy
bun run test:qa-pre-push-smoke
```

- `bun run --cwd apps/openagents.com/apps/start test` ✅ (35 files, 119 tests)
- `bun run --cwd apps/openagents.com/apps/start typecheck` ✅
- `bun run --cwd apps/openagents.com/apps/start build` ✅
- `bun run --cwd apps/openagents.com/apps/start budget` ✅ 736.6 KiB (under
  760 KiB budget), new route chunk 20 KiB (under 120 KiB per-route cap)
- `bun run --cwd apps/openagents.com check:architecture` ✅ zero-debt clean
- `bun run --cwd apps/openagents.com check:deploy` ✅ full sweep clean
- `bun run test:qa-pre-push-smoke` ✅ (7 pass)

## Boundary

This is not the final TS-6 closure. The live `openagents.com` Worker still
serves `apps/web/dist`, and the Foldkit counterparts remain in `apps/web`
because these Start routes are not production-cut-over on the real domain yet.
Delete-as-you-go starts only when a route is actually cut over from the live
Worker to Start.

Remaining TS-6 work:

- migrate the remaining standalone public/`loggedOut` pages that are not yet
  in Start: `share.ts` (a full shared-workroom-timeline viewer — bigger lift,
  needs the workroom timeline/file-panel component set ported to React
  first) is the one fully-unmigrated page left.
  `apps/web/src/page/loggedOut/page/pylon.ts` was migrated in Slice 22 for
  every layer except the literal WebGL diamond-refraction scene
  (`scene/pylonDiamonds.ts`): that one piece is deliberately bridged, not
  ported, because it needs `three` + `@openagentsinc/three-effect` +
  `GLTFLoader`, and those blow the Start funnel's 760 KiB total-JS budget by
  several times over on their own. Follow-up needed before it can land: an
  explicit owner decision on either a dedicated lazy sub-bundle exempted from
  the total-JS budget check, or a raised budget for scene-bearing routes.
  `stats.ts` (public/anonymous variant) was migrated in Slice 21 above,
  rendering the honest Idle first-paint state for all nine shared panels; the
  distinct authenticated `loggedIn/page/stats.ts` view stays out of scope
  until Start has real session auth, same as the rest of the `loggedIn/` tree
  below. `/training/runs/$runId` was migrated in Slice 20, but the whole
  training-runs/gym feature (both Start routes and their nav entry points) is
  now **deprecated-for-now** per the owner decision recorded above — do not
  pick this back up (live-data wiring, nav restoration, or otherwise) until
  the owner reinstates it;
- build the Start auth prerequisite, then migrate `loggedIn/` app-shell panels
  route-by-route. Slice 24 (scoping only, see above) inventoried the real
  size: ~105,700 lines across 220 files (~73,100 non-test), ~19 page routes
  behind one shared `Model`/`Message`/`update` triad (`model.ts` alone is
  7,731 lines) plus ~22 feature subsystems, with the single
  `autopilot-work.ts` page + its `autopilot-work/` subsystem accounting for
  ~34,900 lines on its own. Confirmed (not assumed) that Start has zero
  session/auth infrastructure today (`workers/api/src/auth/session.ts` is the
  only session-verification code, and it lives in a different Worker), that
  every `loggedIn/` page — even the smallest `Ui.pageShell` ones
  (Onboarding/Pro/Order) — reads real session data with no honest anonymous
  branch, and that at least one page (`gymOss.ts`, an hourly-billed live
  inference playground) has no gate of its own and would become a public,
  unbilled-trigger safety exposure if ported without auth. Still fully
  unstarted; the auth prerequisite is real infrastructure work deserving its
  own slice, not a corner cut inside a scoping pass;
- migrate or explicitly retire the Forum web shell from `apps/web`;
- cut production routes over from the Start Worker;
- delete each Foldkit counterpart after its production route cutover;
- repoint the `ASSETS` binding and remove `apps/web` from the build.

## Verification

```sh
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-code.test.tsx src/routes/-app-shell.test.tsx src/routes/-components.test.tsx src/routes/-gym.test.tsx src/routes/-index.test.tsx src/routes/-artanis-accounts.test.tsx src/routes/-workspace-invite.test.tsx src/routes/-mirrorcode.test.tsx src/routes/-promises.test.tsx src/routes/-artanis-console.test.tsx src/routes/-public-agent.test.tsx src/routes/-training-runs.test.tsx src/routes/-training-runs-deprecated.test.tsx src/routes/-stats.test.tsx src/routes/-pylons.test.tsx
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

## Slice 24 (Scoping Only): `loggedIn/` App-Shell Tree Inventory

Every standalone public `loggedOut` page is now migrated (Slices 1-23). This
pass reads the whole `apps/web/src/page/loggedIn/` tree end to end (rather than
re-quoting the prior "dozens of interconnected panels" summary) to produce a
real, file-level scope breakdown, checks whether Start has any session/auth
mechanism yet, and scopes the smallest tractable first slice. Conclusion:
**no `loggedIn/` code was migrated in this batch.** Every page in the tree
depends on real authenticated-session data with no honest anonymous/idle
branch, and Start has zero auth infrastructure today. Forcing a port would mean
either fabricating session data (against every established TS-6 no-fabrication
rule) or exposing an owner-gated/billed surface with no gate — both worse than
shipping nothing. The auth prerequisite is the real next slice; the rest of
this section is the scoping deliverable.

### Real size

`apps/web/src/page/loggedIn/` is **~105,700 lines across 220 files** (~73,100
non-test lines across 140 files, ~32,600 test lines across 80 files) — an
order of magnitude larger than any single standalone page tackled in
Slices 1-23 (the largest of which, `publicAgent.ts`, was ~2,700 lines).

Breakdown:

- **Core app-shell** (`model.ts`, `view.ts`, `message.ts`, `update.ts` +
  `update-dispatch.ts`, `chatState.ts`, `initial-commands.ts`,
  `forge-automations.ts`, `thread-route.ts`, `transition.ts`, `index.ts`,
  `site-code-context.ts`, `site-element-context.ts`,
  `site-preview-bridge.ts`): ~11,600 non-test lines across ~15 files. This is
  one giant Elm-style (Foldkit `Submodel`) triad — a single `Model` union
  (`model.ts` alone is **7,731 lines**), a single `Message` union
  (`message.ts`, 1,565 lines), and a single `update`/`view` dispatch
  (`view.ts` 756 lines, `update-dispatch.ts` 376 lines) that every one of the
  ~19 pages below plugs into. Unlike the standalone `loggedOut` pages (each
  independently portable), this tree has no natural per-page seam: every page
  view function takes the *same* shared `Model` and is switched on by the
  *same* `model.route._tag` union in `view.ts`.
- **~19 page routes** (`page/*.ts`, ~20,200 non-test lines): `admin.ts`
  (2,276), `artanisGym.ts` (253), `billing.ts` (152), `chat.ts` (605),
  `dashboard.ts` (169), `decisions.ts` (372), `files.ts` (501), `gymOss.ts`
  (47), `images.ts` (366), `invite.ts` (52), `onboarding.ts` (657),
  `order.ts` (2,627), `pro.ts` (91), `settings.ts` (971), `stats.ts` (921),
  `usage.ts` (68), `workroom.ts` (1,348), `workspace.ts` (432), and
  **`autopilot-work.ts` alone at 8,282 lines** — nearly as large as the
  entire `loggedOut` backlog this doc just closed out.
- **`autopilot-work/` feature subsystem**: **26,587 non-test lines across 62
  files** (`accessibility-non-interactive-evidence.ts`,
  `mcp-capability-catalog.ts`, `terminal-ui-shell.ts`,
  `security-review-evidence.ts`, and 58 more like it) — this is the
  Autopilot-coder product-promise evidence-gate content backing the single
  `autopilot-work.ts` page above. Combined, the Autopilot Work surface
  (page + subsystem) is **~34,900 lines**, roughly a third of the whole
  `loggedIn/` tree by itself.
- **~22 other feature subsystems** backing the remaining pages (`admin`,
  `artanis-console`, `artanis-dashboard`, `billing`, `customer-order`,
  `decisions`, `goals`, `gymOss`, `images`, `mullet`, `notifications`,
  `onboarding`, `pro`, `providers`, `run-timeline`, `runs`, `session`,
  `stats`, `sync`, `team-chat`, `thread-files`, `workroom`, `workspace`):
  ~14,600 non-test lines across ~40 files, ranging from `session/transitions.ts`
  (55 lines) up to `mullet/` (2,538 lines across 4 files) and
  `run-timeline/projection.ts` (1,594 lines).

All of this sits behind one shared-sidebar app shell
(`Ui.workroomShell`/`Ui.workroomRail`, `packages/ui/src/workroom.ts`, 2,038
lines) that renders a 280px nav rail + main content CSS grid, populated by
live session/notification/nav-item data carried on the shared `Model`. Three
pages (`Onboarding`, `Pro`, `Order`/`OrderDetail`) bypass that shared shell and
render through the simpler standalone `Ui.pageShell` (a bare full-height div,
`packages/ui/src/layout.ts`) instead — these looked like the most promising
"smallest first slice" candidates going in, per the closing note on Slice 23.

### Auth-prerequisite finding (this is the real blocker, confirmed not assumed)

**Start (`apps/start`) has no session or auth mechanism at all today.**
Confirmed by direct search: the only session-verification code in the whole
`apps/openagents.com` app is `workers/api/src/auth/session.ts`
(`makeBrowserSessionBoundary`/`requireBrowserSession`, built on
`@openauthjs/openauth`), which lives in the separate `workers/api` Worker, not
`apps/start`. Grepping `apps/start/src` for `auth`/`session` finds only the
unrelated `-khala-sync-session.ts` (Khala Sync's own concept, not user auth).
`apps/start/src/server.ts` wires exactly two request-routing concerns today
(shared public agent-surface routes and the Khala Sync WebSocket proxy) —
nothing reads or verifies a session cookie. The already-migrated `/login`
route (Slice 8) explicitly documented this: "Auth callback, OAuth/email
handlers, downstream entitlement checks, and account gating remain owned by
the existing Worker/API surfaces" — confirmed still true.

**Every `loggedIn/` page depends on real session data with no honest
anonymous state, unlike every `loggedOut` page ported so far.** Checked the
smallest candidate first: `page/pro.ts` (91 lines, the smallest of the three
`Ui.pageShell` pages) reads `model.session.email` directly in its top-strip
view with no branch for a missing/anonymous session — unlike `loggedOut`
pages (which either need no session or have an explicit `Idle` union case
this migration wave has been rendering honestly for `/artanis/accounts`,
`/mirrorcode`, `/promises`, `/stats`, `/training/runs`). There is no
"idle/pre-fetch" analog to fall back to here: the whole `Model` this tree
renders from is already-resolved, already-authenticated state, not a
client-fetched projection with a natural pre-load placeholder.

**At least one page is actively dangerous to port without a gate, not just
lower-fidelity.** `page/gymOss.ts` (47 lines) is an "owner-gated... hourly
billed" live-inference latency playground with **no idle/unauthorized render
branch of its own** — its `view()` unconditionally renders the full
interactive playground regardless of model state (unlike `artanisAccounts.ts`,
which explicitly has an `Unauthorized` case this migration rendered honestly
in Slice 16). Porting it verbatim to a Start route with no auth would make a
billed, owner-only compute-trigger surface publicly reachable — a real
safety/billing exposure, not merely a content gap.

**Routing itself is auth-gated, not just page content.** Read
`apps/web/src/routing/startup.ts` end to end:
`startupRouteForLoggedIn`/`startupRouteForCompleteOnboarding` decide which
route a visitor even lands on based on a resolved `AuthBootstrap` (permission
gate, onboarding-complete flag, invite-required flag) — e.g. a signed-in user
without full permission gets redirected to `/invite` instead of their
requested route. This dispatch logic has no meaning without a real resolved
auth state to switch on.

### What this rules in/out for "smallest tractable slice"

- The three `Ui.pageShell` pages (`Onboarding`/`Pro`/`Order`) looked like the
  natural first candidates precisely because they skip the big shared
  `workroomShell`/sidebar — but skipping the shell does not skip the
  session dependency: all three still read real session/order/dashboard state
  with no anonymous branch (`Order` is 2,627 lines of real order/billing
  detail, not a simple form).
- `page/invite.ts` (52 lines) is the one page in the tree that reads *no*
  session-derived data — its body is entirely static copy ("Open beta is
  live... Continue to order"). It is not, however, reachable by a real
  anonymous visitor in production (it only exists behind the "signed in but
  not yet invited" gate branch in `startup.ts`), so porting it now would be
  speculative prep for a routing state the real Worker still owns, not an
  honest rendering of what an anonymous visitor at that URL sees today —
  a different situation from `/workspaces/$workspaceId` or `/artanis/accounts`
  (both of which *are* real, currently-reachable anonymous states). Left
  unmigrated this batch rather than shipped as decorative-only prep.
- The `Ui.workroomShell`/`Ui.workroomRail` grid layout itself is genuinely
  trivial (a CSS grid + a few Tailwind classes, no session dependency) and
  could be ported as a Start UI primitive at any time — but a shell with no
  route behind it is not a testable route slice, so it is left as documented
  prep rather than manufactured busywork this batch.

### Recommended next slice

Build the auth prerequisite first, as its own dedicated slice/issue (not
improvised inside a scoping batch): wire real session verification into
`apps/start` — mirroring `workers/api/src/auth/session.ts`'s
`@openauthjs/openauth`-based `requireBrowserSession`/`AuthBootstrap`
resolution, reachable from the Start Worker's `server.ts` fetch handler, plus
the `/login/github` and `/login/email` callback handling the existing
`/login` page shell already points at. This is a routing/authority-boundary
change (`Read INVARIANTS.md before changing authority, routing, payment,
projection, or public-claim surfaces` per this repo's `CLAUDE.md`), so it
deserves its own focused review rather than a corner cut inside this pass.
Once a real `AuthBootstrap` can be resolved server-side in Start, the
honest-anonymous-state technique already proven on `/artanis/accounts` and
`/workspaces/$workspaceId` (render the real "not signed in" gate rather than
fabricated dashboard content) becomes available for the `loggedIn/` tree too,
and the smallest genuinely tractable slice becomes porting one bounded page
(`page/gymOss.ts`, `page/invite.ts` in its real routing position, or
`page/pro.ts`) with a real signed-out gate plus its real signed-in content.
