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

## Boundary

This is not the final TS-6 closure. The live `openagents.com` Worker still
serves `apps/web/dist`, and the Foldkit counterparts remain in `apps/web`
because these Start routes are not production-cut-over on the real domain yet.
Delete-as-you-go starts only when a route is actually cut over from the live
Worker to Start.

Remaining TS-6 work:

- migrate logged-in app-shell panels route-by-route;
- migrate or explicitly retire the Forum web shell from `apps/web`;
- cut production routes over from the Start Worker;
- delete each Foldkit counterpart after its production route cutover;
- repoint the `ASSETS` binding and remove `apps/web` from the build.

## Verification

```sh
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-app-shell.test.tsx src/routes/-components.test.tsx src/routes/-gym.test.tsx src/routes/-index.test.tsx
bun run --cwd apps/openagents.com/apps/start typecheck
bun run --cwd apps/openagents.com/apps/start build
```

The route tests are the parity guard for this slice; the full final TS-6
closure still needs the per-route visual smokes and delete evidence named in
#8348.
