# EN-4 (#8573): web absorption burn-down — route inventory

Under epic EN #8566, lane EN-4 #8573. This is the **first-slice** inventory +
burn-down table for converting every `apps/openagents.com` web route onto the
Effect Native DOM renderer, following the EN-1 `/stage1` pattern
(`docs/fable/2026-07-08-en-1-stage1-effect-native-receipt.md`). It is
maintained across sessions — update the status column and re-order as routes
convert, do not replace this file wholesale.

## Architecture context (read this before converting anything)

There are **two separate apps** under `apps/openagents.com`, at very different
risk levels:

1. **`apps/web`** — a large Foldkit (Elm-style Model/View/Update) single-page
   app. This is **live production** today: it is served by the
   `openagents.com` Cloudflare Worker and by the Cloud Run monolith that wraps
   the same Worker code (`docs/DEPLOYMENT.md` — "openagents.com monolith
   (Cloud Run) ... the SAME app"). Converting or deleting a route's Foldkit
   implementation here **changes what real users see today** and requires a
   serving cutover, not just a new file. Route table / server document
   allowlist source of truth: `apps/openagents.com/apps/web/src/route-table.ts`.
2. **`apps/start`** — a TanStack Start React app that already carries a full
   parallel "interim React" rewrite of most of the site (per
   `apps/openagents.com/CLAUDE.md`'s "existing Foldkit surfaces ... legacy,
   converted and deleted route-by-route" framing and the EN-1 receipt). It is
   **not live** — the root cutover (making `apps/start` the thing DNS/the
   Worker actually serves) is owner-gated on issue #8565. EN-1's `/stage1` and
   this session's `/khala` and `/tassadar` conversions all landed here: safe
   to convert+delete aggressively because no real user traffic depends on it
   yet.

**Practical consequence for sequencing:** EN-4 work should first burn down
`apps/start`'s interim-React routes to Effect Native (zero production risk,
proves the pattern, keeps pace with the eventual root cutover), while
`apps/web` Foldkit → Effect Native conversion is a **separate, larger, higher
-risk program** that needs an explicit per-route serving-cutover plan (most
likely: `apps/start` becomes the live app route-by-route, or `apps/web` pages
are replaced in place with `ssr:false` CSR mounts of the same Effect Native
page — CLAUDE.md documents both directions as "the plan" in different places;
this needs an owner-level decision before large-scale `apps/web` deletion
starts). This burn-down inventories both, but **only `apps/start` rows are
safe to convert-and-delete under today's guardrails**; `apps/web` rows are
listed for visibility and sizing, not for immediate action.

## Part A — `apps/start` (interim React + Effect Native), the safe near-term lane

All page components live in `apps/openagents.com/apps/start/src/routes/` as
`-<name>-page.tsx` (private, not directly routable) wired from a thin
`<name>.tsx` / `<name>/index.tsx` TanStack route file. LOC is the page
component only (the conversion unit), not the thin route file.

| Path | Page file | Impl | LOC | Notes | Status |
| --- | --- | --- | --- | --- | --- |
| `/khala` | `-khala-effect-native-page.tsx` | **Effect Native** | 281 | Public info page, one live counter (`khala-tokens-served`). | **DONE (this session)** |
| `/tassadar` | `-tassadar-effect-native-page.tsx` | **Effect Native** | 326 | Public info page, one client interaction (clipboard copy intent). | **DONE (this session)** |
| `/stage1` | `-stage1-effect-native-page.tsx` | **Effect Native** | 719 | EN-1 pattern-setter; unlinked staging route, not a real nav target. | DONE (EN-1, prior session) |
| `/workspaces/$workspaceId` | `-workspace-invite-page.tsx` | React | 35 | Smallest remaining page file. Auth-adjacent (workspace invite) — check gating before converting. | TODO |
| `/run` | `-run-page.tsx` | React | 37 | Public Terminal-Bench run visualizer entry. | TODO |
| `/training/runs`, `/training/runs/$runId` | `-training-runs-deprecated-page.tsx` | React | 43 | Filename says "deprecated" — confirm with `route-table.ts`/owner whether this should convert or just delete outright. | TODO (confirm scope first) |
| `/adjutant` | `-public-agent-page.tsx` | React | 46 | Shared with `/agents/$agentRef`, `/artanis`. | TODO |
| `/agents/$agentRef`, `/artanis` | `-artanis-console-page.tsx` (via `-public-agent-page.tsx` alias) | React | 506 | See row above — same underlying page, larger console view. | TODO |
| `/login` | `-login-page.tsx` | React | 57 | Auth flow — convert carefully, no auth-gate behavior change. | TODO |
| `/onboarding` | `-onboarding-page.tsx` | React | 68 | Auth-adjacent. | TODO |
| `/artanis/accounts` | `-artanis-accounts-page.tsx` | React | 93 | Public. | TODO |
| `/pylon/codex/assignments/$assignmentRef` | `-pylon-codex-assignment-status-page.tsx` | React | 108 | Public, live data (assignment status). | TODO |
| `preview/landing` | `-landing-preview-page.tsx` | React | 127 | Internal preview route, not linked from nav. | TODO |
| `/activity` | `-activity-page.tsx` | React | 146 | `clientOnly` in the Foldkit table (not server-admitted there either). | TODO |
| `/download` | `-download-page.tsx` | React | 153 | Public, static-ish info page. **Good next candidate** (similar shape to khala/tassadar). | TODO — recommended next |
| `/privacy` | `-privacy-page.tsx` | React | 172 | **Canonical public surface** (named alongside terms/stats/promises in the stage1 footer copy). Static legal prose, but rich inline formatting (bold spans, links inside paragraphs) — Effect Native's `Text` view is flat-content only, so this needs either a "paragraph with inline runs" catalog capability (file against EN-2 if missing) or a structural rework into one `Text` node per run. Convert only after `/download`-style pages prove the pattern further. | TODO |
| `/clients-preview` | `-clients-preview-page.tsx` | React | 202 | Internal/preview. | TODO |
| `/business/kpi/$engagementRef` | `-business-kpi-page.tsx` | React | 214 | Public. | TODO |
| `/artanis/traces` | `-artanis-traces-page.tsx` | React | 215 | Public. | TODO |
| `/gym` | `-gym-page.tsx` | React | 223 | Public Terminal-Bench visualizer. | TODO |
| `/terms` | `-terms-page.tsx` | React | 272 | Same canonical-surface + rich-inline-text caveat as `/privacy`. | TODO |
| `/components`, `/components/$family` | `-components-page.tsx` | React | 274 | Internal component gallery. | TODO |
| `/pylons` | `-pylons-page.tsx` | React | 278 | Public Pylon scene entry. | TODO |
| `/sites/demo-checkout`, `/sites/demo-checkout/$returnAction` | `-site-checkout-demo-page.tsx` | React | 285 | **Payment-adjacent (checkout demo). Explicitly excluded from this session's guardrails — do not convert without an explicit payment-flow review.** | DEFER |
| `/mirrorcode` | `-mirrorcode-page.tsx` | React | 290 | Public. | TODO |
| `/promises` | `-promises-page.tsx` | React | 300 | **Promise registry surface — handle carefully per the issue guardrail. Coordinate with `docs/promises/` conventions before converting; do not change copy or gating semantics.** | DEFER (careful pass) |
| `/code`, `/code/download` | `-code-page.tsx` | React | 376 | Uses shared `SceneLayer` from `-app-shell-routes.tsx` (kept after this session's cleanup). | TODO |
| `/stats` | `-stats-page.tsx` | React | 487 | **Canonical public surface**, live data-heavy (multiple public counters/feeds). | TODO |
| `/share/$shareId` | `-share-page.tsx` | React | 640 | Public, live data. | TODO |
| `/`, `/new` | `index.tsx` (root) + related | React | 47 (root) | **Owner-gated (#8565 root cutover). Do not touch outside that gate** — this is the one route EN-1 explicitly left untouched as the visual baseline. | DEFER (owner-gated) |
| `/autopilot`, `/autopilot/legal`, `/blog`, `/blog/$slug`, `/business`, `/docs`, `/docs/$slug`, `/code/download`, `/khala/chat-sync` | `-funnel-components.tsx` (shared placeholder) | React | 730 (shared file, multiple routes) | Shared marketing/funnel placeholder content across several routes — convert as one batch once the pattern for multi-route shared components is settled. | TODO |

Not listed (internal-preview / non-primary, lower priority): `preview/sales-landing` (`-sales-landing-page.tsx`, 538 LOC — WEB-1 preview baseline, keep untouched alongside `/new`).

## Part B — `apps/web` (Foldkit, live production) — sizing only, do not convert yet

This is a single large Foldkit SPA (Model/View/Update), not N independent
files. Route dispatch lives in `apps/openagents.com/apps/web/src/page/loggedOut/view.ts`
and `.../loggedIn/view.ts`; the path/gating source of truth is
`route-table.ts` (~70 route tags, deduped to ~55 server-document patterns).
LOC below is the primary page-implementation file for each area (excluding
`.test.ts`); several areas have large sub-trees of transition/model files not
counted here.

**Everything in this section is live production.** Do not convert-and-delete
until a serving-cutover plan exists (see Architecture context above).

| Area / route(s) | Primary file(s) | LOC | Gating | Notes |
| --- | --- | --- | --- | --- |
| `/` (root, redirect dispatch) | `page/index.ts` | 3 | public | Thin re-export. |
| `/activity` | `page/activity.ts` | 28 | `clientOnly`, logged-in | Not server-admitted (hard-nav 302s home already). |
| `/workspaces/invite` (Invite) | `loggedOut/page/workspaceInvite.ts` | 63 | logged-in gate | |
| back-button chrome (shared) | `loggedOut/page/backButton.ts` | 112 | n/a | Shared UI, not a route. |
| `/pylons` | `loggedOut/page/pylon.ts` | 111 | public | Public Pylon scene. |
| `/login` | `page/login.ts` | 93 | public | Auth entry. |
| `/stats` (logged-out) | `loggedOut/page/stats.ts` | 154 | public | **Canonical public surface.** |
| `/stats` (logged-in variant) | `loggedIn/page/stats.ts` | 921 | logged-in | Much larger authenticated view. |
| `/run` | `page/run.ts` | 129 | public | |
| forum tipping UI (shared) | `page/forum-tip-ui.ts` | 137 | n/a | Used by `/forum*`. |
| `/pylon/codex/assignments/{ref}` | `page/pylonCodexAssignmentStatus.ts` | 170 | public | |
| `/onboarding` (logged-out) | `loggedOut/page/onboarding.ts` | 315 | special | |
| `/privacy` | `page/privacy.ts` | 221 | public | **Canonical public surface.** |
| business intake chat (shared) | `page/business-intake-chat.ts` + `-controller.ts` | 226 + 329 | public | Feeds `/business`. |
| `/terms` | `page/terms.ts` | 262 | public | **Canonical public surface.** |
| `/artanis/traces` | `page/artanisTraceTree.ts` | 317 | public | |
| `/download` | `page/download.ts` | 320 | `clientOnly` | |
| `/business`, `/business/kpi/{ref}` | `page/business.ts` + `page/businessKpi.ts` | 389 + 331 | public | |
| `/new` (root cutover target) | `page/newLanding.ts` | 336 | public | **Owner-gated (#8565).** |
| `/sites/demo-checkout*` | `page/siteCheckoutDemo.ts` | 336 | public | **Payment-adjacent — defer.** |
| shared logged-out header | `page/publicHeader.ts` | 353 | n/a | |
| `/code/download` | `page/khalaCodeDownload.ts` | 354 | public | |
| `/demo/legal` | `page/demoLegal.ts` | 386 | public/demo | |
| `/artanis/accounts` | `page/artanisAccounts.ts` | 716 | public | |
| `/tassadar`, `/tassadar/replay/{slug}` | `loggedOut/page/tassadar.ts` | 522 | public | |
| `/training/runs`, `/training/runs/{id}` | `loggedOut/page/trainingRuns.ts` | 537 | public | |
| `/gym` (+ `loggedOut/gym/*` flow/progress modules) | `loggedOut/page/gym.ts` + 4 files | 714 + 1,251 | public/demo submodel | |
| `/share/{id}` | `loggedOut/page/share.ts` | 786 | public | |
| `/code` | `page/code.ts` | 542 | public | |
| `/mirrorcode` | `loggedOut/page/mirrorcode.ts` | 1,118 | public/demo | |
| `/promises` | `loggedOut/page/promises.ts` | 1,136 | public | **Promise registry surface — handle with the promise-registry conventions in `docs/promises/`, not a generic conversion.** |
| `/trace/compare/{ids}` | `page/trace-compare.ts` | 655 | public | |
| `/docs`, `/docs/{slug}` | `page/docs.ts` | 804 | public | |
| `/qa/{runRef}` | `page/qa-swarm.ts` | 830 | public | |
| `/forum*` | `page/forum.ts` | 928 | public | |
| `/blog`, `/blog/{slug}` | `page/blog.ts` | 937 | public | |
| `/khala` (Foldkit original) | `loggedOut/page/khalaTokensServed*` + shared scene | — | public | Foldkit counterpart of this session's converted `/khala` in `apps/start`. Still live; unaffected by this session (only the `apps/start` copy changed). |
| `/trace/{uuid}` | `page/trace.ts` | 1,434 | public | Largest logged-out single page; this is the route with the 2026-06-24 302 prod-bug history (`route-table.ts` comments) — extra care warranted. |
| `/agents/{ref}`, `/artanis`, `/adjutant` | `loggedOut/page/publicAgent.ts` | 2,700 | public | |
| root home scene | `loggedOut/page/home.ts` | 2,341 | public | **Owner-gated with `/new` (#8565) — do not touch.** |
| `/dashboard`, `/billing`, `/usage`, `/pro`, admin/gym-oss/mullet/artanis-gym | `loggedIn/page/*.ts` (dashboard 169, billing 152, usage 68, pro 91, artanisGym 253, gymOss 47 + 1,261 controller/runner, mullet 2,641 across model/transitions/view/workbench) | ~4,600 | **admin / logged-in gated** | Operator-only surfaces; lowest external-user risk but highest internal-tooling risk. Defer behind Part A. |
| `/settings*` | `loggedIn/page/settings.ts` | 971 | logged-in | |
| `/files/{id}`, `/teams/*` | `loggedIn/page/files.ts` + team-chat/files transitions | 501 + more | logged-in, workroom gate | |
| `/t/{id}`, chat | `loggedIn/page/chat.ts` + `chatDom.ts`/`chatState.ts` | 605 + more | logged-in, workroom gate | |
| `/onboarding` (logged-in) | `loggedIn/page/onboarding.ts` | 657 | logged-in | |
| `/workspaces/{id}`, `/workrooms/{id}*` | `loggedIn/page/workspace.ts` + `loggedIn/page/workroom.ts` | 432 + 1,348 | logged-in | |
| `/order`, `/orders/{id}` | `loggedIn/page/order.ts` | 2,627 | logged-in | |
| `/admin` | `loggedIn/page/admin.ts` | 2,276 | **admin-only** | |
| `/autopilot/work*` (Autopilot Work order detail + evidence panels) | `loggedIn/page/autopilot-work.ts` + `loggedIn/autopilot-work/*` (~62 files) | 8,282 + ~35,000 | logged-in | **By far the largest single surface in the repo** — dozens of "evidence" panel modules (accessibility, MCP, security-review, telemetry-privacy, etc.). This is its own multi-session conversion program, not a single burn-down row. |

## Recommendation for the next session

1. Stay in `apps/start` (zero production risk) and continue down Part A in
   the listed order: `/download` next (153 LOC, same shape as this session's
   `/khala`/`/tassadar` conversions — static info cards, no auth, no payment,
   no promise-registry citation), then `/pylon/codex/assignments/{ref}`,
   `/artanis/accounts`, `/gym`, `/pylons`, `/artanis/traces`,
   `/business/kpi/{ref}`, `/clients-preview`, `/mirrorcode` roughly in LOC
   order.
2. Before converting `/terms` or `/privacy`, resolve the inline-rich-text gap:
   Effect Native's `Text` view takes flat `content: string` with no inline
   runs (bold-within-paragraph, link-within-paragraph). The legal copy in
   both pages relies heavily on this. File the concrete need against EN-2
   (`#8572`) with the exact shape needed (a `TextRun`/rich-`Text` children
   capability, or a documented "one `Text` node per run + inline `Link`
   sequence" composition pattern) before spending conversion effort there.
3. `/promises` (`apps/start` and, later, the live Foldkit `/promises`) needs
   an explicit pass coordinated with `docs/promises/` conventions — not a
   drive-by conversion. Treat as its own small task.
4. `/sites/demo-checkout*` stays deferred until a payment-flow-aware review
   signs off (checkout demo, still payment-adjacent even as a "demo").
5. `apps/web` (Part B) conversion should not start until there is an explicit
   owner decision on the serving-cutover mechanism (root-cutover-style
   promotion of `apps/start` per route, vs. in-place `ssr:false` CSR mounts
   inside `apps/web` itself — both are referenced in different docs today and
   need reconciling). Until then, keep sizing it (this table) so the eventual
   plan has real numbers, but do not touch production Foldkit page files.
   `loggedIn/autopilot-work` (~43k LOC across ~62 files) is large enough that
   it should get its own dedicated EN-4 sub-lane/issue rather than being one
   row in this table once real work starts there.
