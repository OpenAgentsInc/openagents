# Autopilot Task: Customer Software Ordering Flywheel

Status: complete; moved to `docs/autopilot-tasks/done`

Completion evidence:

- `software_orders` D1 migration and `CustomerOrderStore` Effect service landed.
- `/api/customer-orders/active` creates/reads the active persisted order.
- authenticated non-core startup routes land on customer order status instead
  of the operator workroom.
- `/autopilot` owns the operator workroom route; `/` and `/order` show order
  flow/status.
- customer order UI includes the 24-hour email promise and hides runner
  mechanics.
- focused route/startup/update/customer-order API/UI tests and deploy checks
  passed in foreground sessions.

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: local foreground coding agent. This task is intentionally not
delegated to Autopilot because the launch funnel is needed immediately and
must avoid runner/provider bugs.

Team: OpenAgents core / `team_openagents_core`

Project: OpenAgents product surface customer ordering and launch funnel. Resolve the concrete
project ID through operator preflight before dispatch; do not invent one in the
runner prompt.

Visibility: product work is public by default. During the beta intake,
customers are told that the request, generated work, and learning data may be
public. Private delivery is a separate paid option for later.

Public route or observer link: proposed customer routes are `/order` and
`/orders/:orderId`, with onboarding still reachable through the existing
logged-in onboarding route. Operator routes remain the existing workroom,
thread, team/project chat, and operator API surfaces.

## Implementation Gate

This is not an Autopilot dispatch packet anymore. Do not launch it through the
programmatic Autopilot runbook. Implement it directly in this repo on `main`.

The direct implementation must preserve these gates:

- non-core GitHub-authenticated users can complete repository and request
  onboarding;
- non-core users land on a customer order status page, not the chat/workroom;
- OpenAgents core members keep the operator chat, task streaming, thread,
  files, diagnostics, billing, settings, and API surfaces;
- onboarding tells users that beta work is public, OpenAgents pays compute,
  OpenAgents uses the data, and customers do not need to connect ChatGPT;
- all customer order work is public for now, with private delivery reserved for
  a separate paid option later;
- no raw runner events, provider refs, callback tokens, private repo contents,
  or customer session cookies appear in customer-visible order pages.

## Objective

Turn OpenAgents product surface's current logged-in onboarding and operator workroom model into the
launch wedge described in the product transcript:

- GitHub-authenticated users pick a repository and describe what they want
  built;
- OpenAgents scopes the request asynchronously instead of dropping the user
  into the full chat/workroom UI;
- customers see a status page where they can tell when the agent has been
  queued or has started working;
- small first slices can be offered for free, while larger scopes become
  credit-priced orders with payment follow-up;
- customer code-session data consent is explicit and can support future
  revenue-share accounting for reusable Autopilot learnings;
- all work is public during the beta intake;
- OpenAgents pays for compute during this beta intake;
- customers do not connect a ChatGPT account;
- OpenAgents core operators keep the full chat, task streaming, run diagnostics,
  files, and thread navigation surfaces;
- normal ordering users see only intake, order status, quote/free-slice
  decisions, payment state, and notification state.

The result should support the launch story:

- external hook: "Your agent dealer";
- external compute hook: "Sell us your compute for Bitcoin";
- new code hook: give us useful repo tasks and learning rights, get free work
  now and potential future credit or Bitcoin revenue share when your learning
  contributes to paid workflows.

Do not implement a complex new dashboard for customers. The customer promise is
"say what you want, let OpenAgents pay for the public beta compute, and check
status." The operator promise is "OpenAgents core can inspect and drive the
actual Autopilot work in the existing workroom surfaces."

## Transcript Source Input

The user supplied a launch transcript on 2026-06-04. The durable product input
from that transcript is:

- OpenAgents already has traction by paying people for resources the system
  needs, especially compute.
- The next required resource is good AI-generated code and the repo/task data
  that teaches the system how to produce it.
- Users should connect GitHub, choose a repo, say what they want done, and get
  a lightweight "$0 today, check back" result while OpenAgents scopes the work.
- If the scope is large, OpenAgents should split the job into a free first
  slice and a paid broader order, for example "$50 of work free" or "$100 first
  feature free" before asking for a card for the rest.
- Pricing language should be customer-facing credits with margin included, not
  raw provider cost credits.
- The product should compete against Upwork, Replit, Lovable, and general
  developer hiring by removing cloud/database/dev-tool complexity for business
  users.
- Customers should be able to ask operational questions through agents or
  OpenAgents client reps, but they should not have to sit in the full workroom
  stream to use the product.
- Users can opt in to coding-session data use. Future paid workflows that use
  reusable learnings from their sessions should create a revenue-share basis:
  credits when revenue is credit-denominated and Bitcoin when revenue is
  Bitcoin-denominated.
- This revenue-share promise needs product and legal language later; the first
  implementation should model consent and accounting hooks without pretending
  payouts are complete.

Avoid putting the transcript's illegal-market analogy into product UI. The
approved launch hook may say "Your agent dealer" externally, but in-app copy
should stay direct, commercial, and trust-building.

## Current OpenAgents product surface Starting Point

OpenAgents product surface already has the raw ingredients for the order funnel, but the current
access model is still operator-first:

- `apps/web/src/page/loggedIn/page/onboarding.ts` renders a three-step
  onboarding rail: `Repository`, `Goal`, and `Billing`.
- The repository step can search GitHub repositories, select a repo, manually
  enter owner/repo, continue, or skip.
- The goal step already collects free-form task text.
- The billing step currently shows disabled packages (`$25 Starter`,
  `$100 Builder`, `$500 Team`) and a `Skip for now` action.
- `apps/web/src/page/loggedIn/onboarding/transitions.ts` posts repository,
  goal, and billing-skip commands to `/api/onboarding/*`.
- `workers/api/src/onboarding/routes.ts` currently calls
  `requireAuthorizedSession`, which then requires OpenAgents core-team access.
  That means non-core GitHub users cannot complete the customer intake flow
  today.
- `workers/api/src/onboarding/repository.ts` persists onboarding state in D1:
  selected repository, skipped repository, submitted goal, billing skipped,
  current onboarding step, and completion.
- `apps/web/src/domain/session.ts` models onboarding steps, teams, billing
  summary, and `isAdmin`.
- `apps/web/src/routing/startup.ts` currently routes logged-in users through
  `loggedInPermissionGate`; users without core-team access hit the invite page
  instead of customer intake.
- `apps/web/src/product-policy.ts` currently makes
  `loggedInPermissionGate(auth)` require `authHasCoreTeamAccess(auth)`, and
  `loggedInWorkroomAllowed(auth)` requires core-team access plus completed
  onboarding.
- `apps/web/src/page/loggedIn/page/invite.ts` is the current non-core fallback.
- `apps/web/src/route.ts` now includes demo routes under `/demo`, but those are
  recordable fixture/demo surfaces. They are not the public customer ordering
  surface.

OpenAgents product surface also has a mature operator workroom path:

- `apps/web/src/page/loggedIn/page/chat.ts` renders the real chat/workroom
  experience.
- `apps/web/src/page/loggedIn/view.ts` renders personal chat, team rooms,
  project rooms, thread routes, team files, and file detail pages.
- `apps/web/src/page/loggedIn/team-chat/transitions.ts` can launch Autopilot
  from team/project chat.
- `apps/web/src/page/loggedIn/runs/transitions.ts` handles personal launch,
  run fetch, thread entry, run metadata, and sidebar mission projection.
- `apps/web/src/page/loggedIn/sync/transitions.ts` applies sync snapshots and
  patches for team messages, agent runs, agent goals, files, and sidebars.
- `workers/api/src/omni-routes.ts` and `workers/api/src/omni-handlers.ts`
  expose browser run APIs plus operator preflight, checklist, continuation,
  callback retry, team-chat, fleet, and billing-credit APIs.

The implementation must split these worlds cleanly:

- customer intake/order routes for normal authenticated users;
- operator workroom routes and streaming APIs for OpenAgents core members.

Frontend route hiding is not enough. Backend authorization must enforce the
same split.

## Relevant Repo Files

Planning and runbooks:

- `2026-06-04-programmatic-autopilot-operator-runbook.md`
- `2026-06-04-stripe-effect-service-implementation.md`
- `2026-06-04-effect-driven-chat-demo-page.md`
- `../2026-06-04-effect-foldkit-codebase-audit.md`
- `../2026-06-04-openagents-broader-effect-refactor-audit.md`
- `../2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- `../2026-06-03-autopilot-billing-credits.md`
- `../../DESIGN.md`

Customer onboarding and routing:

- `../../apps/web/src/route.ts`
- `../../apps/web/src/product-policy.ts`
- `../../apps/web/src/routing/startup.ts`
- `../../apps/web/src/domain/session.ts`
- `../../apps/web/src/page/loggedIn/page/onboarding.ts`
- `../../apps/web/src/page/loggedIn/onboarding/transitions.ts`
- `../../apps/web/src/page/loggedIn/onboarding/commands.ts`
- `../../apps/web/src/page/loggedIn/page/invite.ts`
- `../../apps/web/src/page/loggedIn/billing/transitions.ts`
- `../../apps/web/src/page/loggedIn/page/billing.ts`

Backend onboarding, billing, and operator APIs:

- `../../workers/api/src/onboarding/routes.ts`
- `../../workers/api/src/onboarding/repository.ts`
- `../../workers/api/src/onboarding/schema.ts`
- `../../workers/api/src/billing.ts`
- `../../workers/api/src/billing-routes.ts`
- `../../workers/api/src/operator-billing-routes.ts`
- `../../workers/api/src/omni-routes.ts`
- `../../workers/api/src/omni-handlers.ts`
- `../../workers/api/src/team-chat.ts`
- `../../workers/api/src/team-chat-routes.ts`
- `../../workers/api/src/thread-file-routes.ts`
- `../../workers/api/migrations/`

Operator workroom and sync paths:

- `../../apps/web/src/page/loggedIn/model.ts`
- `../../apps/web/src/page/loggedIn/message.ts`
- `../../apps/web/src/page/loggedIn/update.ts`
- `../../apps/web/src/page/loggedIn/view.ts`
- `../../apps/web/src/page/loggedIn/page/chat.ts`
- `../../apps/web/src/page/loggedIn/page/files.ts`
- `../../apps/web/src/page/loggedIn/runs/transitions.ts`
- `../../apps/web/src/page/loggedIn/team-chat/transitions.ts`
- `../../apps/web/src/page/loggedIn/sync/transitions.ts`
- `../../apps/web/src/page/loggedIn/sync/projection.ts`
- `../../apps/web/src/ui/workroom.ts`
- `../../packages/sync-schema/src/index.ts`
- `../../packages/sync-client/src/index.ts`

Local references:

- `../../../projects/repos/effect-cf/`
- `../../../projects/repos/effect-solutions/`
- `../../../projects/repos/stripe-node/`

## Proposed Customer Journey

Implement one simple, production-shaped path:

1. A visitor signs in with GitHub from `openagents.com`.
2. A non-core authenticated user is allowed into customer onboarding, not the
   operator workroom.
3. The user picks a GitHub repository or manually enters owner/repo.
4. The user writes the desired outcome in plain language.
5. The user reviews a concise consent block:
   - OpenAgents may use coding-session data and derived operational learnings
     to improve Autopilot.
   - Future revenue-share accounting may be attached to reusable learnings.
   - Payment may be in OpenAgents credits or Bitcoin depending on future paid
     workflow rails.
6. The user submits.
7. The user lands on an order status page:
   - `Submitted`
   - `Scoping`
   - `Free slice available`
   - `Quote ready`
   - `Building`
   - `Delivered`
   - `Needs customer input`
   - `Declined` or `Unavailable`
8. The first screen after submit should make it clear that the user owes `$0`
   today and can check back or wait for email.
9. If the request is too large for the free slice, the quote state should show
   the free slice and the paid broader order in customer-facing credits.
10. If Stripe checkout is available from the Stripe Effect Service task, the
    quote state can link into checkout. If not, keep the quote/payment state
    disabled or "coming next" without fake charging.
11. The order is projected into an operator queue or team/project workroom so
    OpenAgents core can scope, ask questions, and launch real Autopilot runs.
12. Only core-team operators can open chat streaming, thread timelines, files,
    diagnostics, provider state, SHC state, callback retry, continuation, and
    run metadata.

The customer should never need to understand Cloudflare, D1, Stripe webhooks,
provider accounts, runner hosts, callback tokens, or workroom internals.

## Commit Input

Suggested commit message for this foreground implementation:

```text
feat: add public customer software order intake
```

There is no Autopilot launch payload for this task. Do not dispatch it.

Historical launch input fields are intentionally withdrawn:

```json
{
  "dispatch": "disabled",
  "reason": "foreground implementation required immediately"
}
```

Do not include provider tokens, callback tokens, OAuth material, local secret
paths, private runner prompts, raw runner payloads, or customer private repo
content in the launch payload.

## Foreground Work Plan

1. Read the referenced onboarding, routing, product-policy, billing, and
   operator workroom files before editing. Confirm the final route map in the
   implementation summary.
2. Introduce explicit policy helpers for:
   - authenticated customer intake access;
   - completed customer onboarding access;
   - operator workroom access;
   - operator API access;
   - public/demo route access.
3. Change startup routing so a signed-in non-core user can enter onboarding
   and order status instead of being sent directly to the invite page.
4. Keep existing core-team users on the current operator path. Do not regress
   OpenAgents core access to `/`, team rooms, project rooms, `/t/:threadId`,
   billing, files, settings, or operator APIs.
5. Split onboarding backend authorization:
   - repository list/select/update/skip and goal submit should be available to
     authenticated GitHub users where the product permits customer intake;
   - operator-only onboarding/admin mutations must stay core-team gated;
   - existing session and CSRF/auth boundaries must stay intact.
6. Add a Schema-backed customer order domain:
   - `SoftwareOrderId`;
   - order status;
   - repo owner/name/ref;
   - customer goal/request text;
   - consent snapshot;
   - quote estimate;
   - free-slice estimate;
   - payment state;
   - operator projection state;
   - timestamps and actor IDs.
7. Add tagged errors and Effect services/layers for customer order storage,
   order creation, order status lookup, quote update, and operator projection.
   Follow the OpenAgents product surface Effect guidance: `Context.Service`, layers, Schema data,
   tagged errors, and no `Effect.runPromise` below boundaries.
8. Add D1 migrations for customer orders, order events, consent snapshots, and
   future revenue-share learning references. Keep the initial revenue-share
   records as accounting hooks, not payout execution.
9. Connect onboarding completion to order creation:
   - repository + goal should create or update the active customer order;
   - billing skip should not mean "done forever" for customer orders;
   - the post-submit route should be the order status surface, not the
     operator chat.
10. Build the customer order status UI using existing Foldkit patterns and the
    existing logged-in shell where appropriate. Keep it sparse, durable, and
    mobile-safe:
    - selected repo;
    - request summary;
    - status;
    - free-slice/quote card when ready;
    - payment/credits call to action only when real backend support exists;
    - email/check-back messaging;
    - no run stream.
11. Add a minimal customer order API:
    - create/update active order from onboarding goal;
    - get active order;
    - get order by ID for the owning customer;
    - acknowledge quote/free-slice decision if available.
12. Add an operator projection:
    - either a dedicated operator order queue route/API, or a projection into
      the existing OpenAgents core team/project workroom;
    - core operators can see repo, request, customer identity, consent state,
      status, quote, and safe links into workroom/run creation;
    - non-core users cannot read this projection.
13. Add a manual scoping path first. The task may add a typed placeholder
    `ScopingService`, but it must not rely on ad hoc string matching for
    product intent, cost estimation, routing, or tool selection.
14. If using LLM-assisted scoping, model it as a typed service with Schema
    inputs/outputs and conservative failure states. Do not block the customer
    path on provider success.
15. Integrate Stripe only if the Stripe Effect Service task has landed. If it
    has not landed, leave payment states as quote-ready but not chargeable, and
    point follow-up work to
    `2026-06-04-stripe-effect-service-implementation.md`.
16. Add email notification calls only through an existing approved email
    boundary. If that boundary is missing, store notification-needed events and
    document the follow-up.
17. Add tests for route policy, startup routing, onboarding API authorization,
    order API ownership, order state transitions, operator-only projections,
    and customer UI update behavior.
18. Run the repo's focused checks plus the deploy check suite required by the
    current AGENTS/runbook guidance. Record exact commands and results in the
    implementation summary.

## Safety Rules

- Non-core customers must not receive raw chat streams, task streaming,
  provider state, SHC state, callback payloads, shell logs, private run
  diagnostics, or other operator-only delivery mechanics.
- Backend route authorization must enforce the operator/customer split. UI
  hiding alone is not acceptable.
- Do not promise legally binding revenue share in product copy until terms,
  payout rails, tax/compliance, and ledger authority are explicit.
- Do not execute Bitcoin payouts in this task. Model consent and future
  accounting hooks only.
- Do not take live card payments unless the Stripe Effect service,
  webhook-backed fulfillment, idempotency, clean URL handling, and tests are
  already implemented and verified.
- Do not present raw provider cost as customer credit price. Customer-facing
  credit quotes must include OpenAgents pricing/margin policy.
- Do not use ad hoc keyword/string matching for scoping, routing, retrieval,
  or tool selection. Use typed services, Schema outputs, semantic selectors, or
  explicitly modeled parsers.
- Do not store secrets, OAuth tokens, provider tokens, session cookies, Stripe
  secrets, callback tokens, or private prompts in order rows, docs, fixtures,
  logs, or public projection fields.
- Do not copy large chunks from reference repos into OpenAgents product surface. Study references,
  then implement OpenAgents product surface-native code.
- Keep UI copy concise. The customer surface should feel like ordering
  software, not learning an internal dashboard.

## Acceptance Criteria

- A signed-in GitHub user without OpenAgents core-team membership can complete
  customer onboarding: select or enter a repo, submit a goal, consent to the
  learning/revenue-share terms snapshot, and see a customer order status page.
- The same non-core user cannot access `/`, team/project workrooms,
  `/t/:threadId`, team files, operator APIs, run diagnostics, callback retry,
  continuation, or private sync streams.
- An OpenAgents core-team user retains access to the current operator workroom
  flow and can see customer orders through a core-only projection.
- Customer order status is persisted in D1 and survives reload.
- Order APIs enforce owner access for customers and core-team access for
  operator projections.
- The implementation supports quote/free-slice states even when payment is not
  live.
- If Stripe checkout is enabled, payment uses the typed Stripe Effect service
  and D1 billing ledger. No placeholder checkout credit behavior remains on the
  live path.
- Revenue-share consent and learning references are stored as future-accounting
  hooks, not live payout promises.
- The customer UI contains no raw runner logs or internal delivery mechanics.
- Tests cover startup routing, product-policy helpers, onboarding API auth,
  order API auth, order state transitions, quote/free-slice state rendering,
  and operator-only projection.
- The final implementation commit or PR summary lists:
  - routes added or changed;
  - migrations added;
  - Effect services/layers added;
  - tests run;
  - known follow-ups for Stripe, email, revenue share, or Bitcoin payout rails.

## Suggested Run Summary

Implemented the OpenAgents customer software ordering wedge. GitHub-authenticated
customers can choose a repo, describe desired work, consent to learning and
future revenue-share hooks, and land on an asynchronous order status surface.
OpenAgents core keeps the full operator workroom, task stream, thread, files,
and diagnostics experience. Non-core customers cannot access operator-only run
streams or APIs. Payment, email, and revenue-share payout behavior are gated by
the relevant typed backend services and follow-up packets.
