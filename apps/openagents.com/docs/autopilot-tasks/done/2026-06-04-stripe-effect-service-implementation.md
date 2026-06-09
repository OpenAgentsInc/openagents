# Autopilot Task: Stripe Effect Service Implementation

Status: complete; merged, production config-gated

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `Artanis` / `agent_artanis`, or another write-capable private
OpenAgents product surface implementation agent selected by preflight.

Team: OpenAgents core / `team_openagents_core`

Project: OpenAgents product surface billing and Stripe integration / `project_artanis`, resolved by
operator preflight with `agent_artanis`.

Visibility: private/operator-visible. This is billing infrastructure work, not
a public marketing run.

Public route or observer link: none for the implementation run. The product
surface affected by successful implementation is `https://openagents.com/billing`.

## Dispatch Attempt

2026-06-04 operator preflight resolved `project_artanis` and `agent_artanis`,
confirmed GitHub writeback, SHC control, callback configuration, and callback
lag, then attempted to launch this packet from commit `7d369c4e`.

The launch did not start a run because provider gating returned
`provider_reconnect_required`: the only connected ChatGPT/Codex account needs
to be reconnected in Settings -> Connections before Autopilot can launch. No
Stripe secret values, local secret paths, provider tokens, callback tokens, or
OAuth material were included in the launch input or this packet.

After reconnect, operator preflight passed. The first Stripe run
`c05f9f93-6776-4d38-ad01-d0f3c004312f` started, resolved provider and GitHub
write grants, then failed before product work completed. The durable goal
`agent_goal_32dd9ebcb17347e1913d856087b7b162` remains active.

Continuation run `7a288f92-4249-40b2-9534-3d48c5090b52` progressed through
repo inspection, then failed after the runner requested access to an external
Bun package directory under `node_modules/.bun/stripe...`, which the runner
policy correctly auto-rejected. It also failed to emit the required
`result.md` artifact before exit.

The next continuation hit two operator-level issues, both outside the product
implementation itself:

- run `9cefe082-b5d3-4a9c-aafb-a9945e788182` was stopped after the operator
  billing balance reached zero. Operator credit was restored and the same
  durable goal was resumed with a higher token budget.
- run `2a450af7-7434-49f0-b2a4-c58c5f7f6183` failed after a broad glob/path
  probe attempted to read outside the checked-out repo workspace, under the
  runner state directory, and the runner policy correctly auto-rejected it. It
  also exited without the required `result.md` artifact.

Next safe action: continue the active durable goal rather than creating an
unrelated duplicate Stripe goal. The next run must stay inside the checked-out
`OpenAgentsInc/openagents` repository and use only repo-relative tracked
paths unless the runner explicitly grants another path. Do not use broad glob
patterns, absolute paths, `state/*`, `node_modules`, `.bun`, package caches, or
local-only reference repo paths. Treat `projects/repos/stripe-node` and
`projects/repos/effect-cf` as foreground-operator reference material only if
they are absent from the remote checkout; use the packet, tracked audits, local
repo source, installed package types without traversing package caches, and
official Stripe docs instead. The run must emit the required `result.md`
artifact if implementation completes or if it blocks.

Continuation run `dfc6bc78-9af3-40b3-a91f-fb7828650c84` completed and pushed
implementation commit `8490cde0254847eefcba496dbd4f0cd14bc8e090` to PR
`https://github.com/OpenAgentsInc/openagents/pull/54` on branch
`openagents/autopilot-6-4d38-ad01-d0f3c004312f`.

Foreground PR review found that the branch passes:

- `bun run --cwd workers/api typecheck`
- `bun run --cwd apps/web typecheck`
- `bun run check:architecture`

However, after installing the PR worktree dependencies with
`bun install --frozen-lockfile`, the focused API route test fails:

```text
bun run --cwd workers/api test src/billing-routes.test.ts
```

Failure:

```text
billing API handlers > returns unavailable when Stripe config is missing
expected 200 to be 503
```

The visible cause is that the test helper still injects the default mocked
Stripe service when `undefined` is passed for the optional `stripe` dependency,
so the missing-config path is not actually exercised. The next Autopilot
continuation should fix the test helper and any real route/service error
mapping issue it reveals. In particular, verify that an environment missing
Stripe config maps to `503 stripe_unconfigured` instead of a success response
or generic billing error, and that this is covered by the focused API test.
While editing PR #54, also re-check that repeated Checkout creation does not
reuse a too-broad outbound Stripe idempotency key for every same-user,
same-package purchase attempt.

Continuation run `e63ced6d-afac-4e53-bda5-6012502ee25d` pushed follow-up
commit `41138f9693ebd4c520c0b77a4928b3561cd03ffc` to PR #54, but foreground
local review still sees the same focused test failure:

```text
bun run --cwd workers/api test src/billing-routes.test.ts
```

Failure remains:

```text
billing API handlers > returns unavailable when Stripe config is missing
expected 200 to be 503
```

The remaining visible issue was the helper signature in
`workers/api/src/billing-routes.test.ts`: the second parameter still had
`= defaultStripe`, so an explicit `undefined` argument was replaced by
`defaultStripe` before the function body ran. The test therefore still injected
the mock.

After the Autopilot follow-up did not land the helper correction, the
foreground operator applied the narrow review fix directly on PR #54:

- commit `3c107bc9`: fixed the missing-config route test helper by adding a
  real `null` sentinel for "omit the injected Stripe dependency";
- commit `c20fc4ab`: hardened the Stripe checkout return path so missing or
  broken Stripe config cannot strand a returning user on an error response;
- merge commit `c7054fd4`: merged PR #54 into `main`.

Merged implementation highlights:

- typed Stripe config, client, customer, checkout, webhook, and billing-credit
  Effect service boundaries;
- Stripe Checkout Session creation for configured credit packages;
- webhook and return-path fulfillment into the D1 billing ledger with
  idempotent paid-session handling;
- migration `0031_stripe_billing.sql`;
- browser billing flow updated to redirect through hosted Stripe Checkout while
  preserving the clean `/billing` URL invariant;
- production remains config-gated until real environment secrets and webhook
  signing are installed and test-mode webhook fulfillment is smoke-verified.

Local verification after merge:

```text
bun run --cwd workers/api test src/billing-routes.test.ts
bun run --cwd workers/api typecheck
bun run --cwd apps/web typecheck
bun run check:architecture
bun run check:deploy
```

All listed checks passed on merged `main`.

## Dispatch Gate

Do not launch this task until the programmatic Autopilot runbook
recommendations are complete enough for reliable delegation:

- operator preflight exists and reports migrations, project/agent presence,
  provider health, SHC health, callback config, and GitHub writeback readiness;
- reconnect-required provider states are caught before dispatch;
- SHC callback payload contracts and retry/backfill paths are covered;
- run continuation attaches to the same durable goal;
- private goal/run observation can show current progress without exposing
  private delivery mechanics.

Source runbook:
`2026-06-04-programmatic-autopilot-operator-runbook.md`

Stripe-specific dispatch constraints:

- Stripe keys for this run are local test-mode operator material only. Do not
  configure staging or production secrets as part of this dispatch.
- The launch plan should prefer a restricted Stripe API key (`rk_`) once the
  exact production permission set is known, but this task may implement against
  local test-mode environment variable names and mocked service tests.
- Stripe test-mode package/Price IDs or server-side package config must be
  available for local verification.
- The webhook endpoint can be registered in Stripe test mode for local/manual
  smoke verification.
- No production card checkout is enabled until webhook fulfillment,
  idempotency, clean URL handling, and ledger tests pass.

This is an Autopilot-owned implementation task. The foreground coding agent
should only administer the goal/run and repair Autopilot infrastructure defects
that block honest execution or reporting.

## Objective

Fully implement the Stripe Effect service audit:

- replace the placeholder credit-card checkout API with Stripe Checkout
  Sessions;
- keep OpenAgents product surface's D1 billing ledger authoritative for Autopilot credits;
- add typed Effect services and layers for Stripe config, client construction,
  customer management, Checkout Session creation/retrieval, webhook
  verification, and credit fulfillment;
- append exactly one positive ledger entry for each paid Stripe credit
  purchase;
- keep `/billing` a clean product URL;
- keep all Stripe secrets, webhook payloads, and provider details out of
  browser code, public projection, logs, and docs.

The implementation should make the current billing system look like it was
designed around Effect services from the start, not add a raw Stripe SDK call
inside the existing Promise-shaped route handlers.

## Current OpenAgents product surface Starting Point

The foreground agent already committed the planning audit:

- commit: `eddd99b8`
- file: `../2026-06-04-stripe-effect-service-audit.md`

OpenAgents product surface currently has:

- D1-backed `billing_accounts`, `billing_ledger_entries`,
  `billing_usage_cursors`, `billing_coupon_redemptions`, and
  `billing_credit_notifications`;
- live launch grants, coupon credits, operator manual credits, SHC container
  debits, Codex token debits, and out-of-credits suspension/email logic;
- `POST /api/billing/checkout` returning a placeholder response;
- `GET /api/billing/summary` returning derived balance, rates, recent entries,
  and active runs;
- `/billing` in the logged-in product surface;
- a clean URL invariant forbidding checkout/payment state in product route
  query parameters or fragments.

Relevant OpenAgents product surface files:

- `../2026-06-03-autopilot-billing-credits.md`
- `../2026-06-04-stripe-effect-service-audit.md`
- `../2026-06-04-openagents-broader-effect-refactor-audit.md`
- `../2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- `../../workers/api/src/billing.ts`
- `../../workers/api/src/billing-routes.ts`
- `../../workers/api/src/operator-billing-routes.ts`
- `../../workers/api/src/config.ts`
- `../../workers/api/src/runtime.ts`
- `../../workers/api/src/index.ts`
- `../../workers/api/src/http/responses.ts`
- `../../workers/api/src/json-boundary.ts`
- `../../workers/api/migrations/0016_billing_credits.sql`
- `../../workers/api/migrations/0018_billing_out_of_credits.sql`
- `../../apps/web/src/page/loggedIn/billing/commands.ts`
- `../../apps/web/src/page/loggedIn/billing/transitions.ts`
- `../../apps/web/src/page/loggedIn/page/billing.ts`
- `../../apps/web/src/ui/page-examples.ts`

Local reference repos and docs:

- `../../../projects/repos/stripe-node`
- `../../../projects/repos/effect-cf`
- Stripe Checkout Sessions:
  `https://docs.stripe.com/api/checkout/sessions/create`
- Stripe Checkout fulfillment:
  `https://docs.stripe.com/checkout/fulfillment`
- Stripe webhook signatures:
  `https://docs.stripe.com/webhooks/signature`
- Stripe customer creation:
  `https://docs.stripe.com/api/customers/create`
- Stripe customer balance:
  `https://docs.stripe.com/invoicing/customer/balance`
- Stripe customer balance transactions:
  `https://docs.stripe.com/api/customer_balance_transactions`
- Stripe idempotent requests:
  `https://docs.stripe.com/api/idempotent_requests`
- Stripe restricted keys:
  `https://docs.stripe.com/keys`

Production/private links that are safe to show:

- `https://openagents.com/billing`
- `https://openagents.com/api/billing/summary`

Do not include Stripe Dashboard URLs containing account-specific state, API
keys, webhook secrets, event payload bodies, session cookies, provider tokens,
or local `.secrets` paths in run output.

## Commit Input For Dispatch

Before dispatch, commit and push this task spec. The Autopilot launch input
must reference the commit that contains this file.

Suggested commit message for this delegation packet:

```text
docs: add Stripe Effect service task packet
```

Launch input fields:

```json
{
  "repository": "OpenAgentsInc/openagents",
  "baseRef": "main",
  "taskSpecPath": "docs/autopilot-tasks/2026-06-04-stripe-effect-service-implementation.md",
  "agentId": "agent_artanis",
  "teamId": "team_openagents_core",
  "projectId": "project_artanis",
  "visibility": "private",
  "goal": "Implement the Stripe Effect service audit fully: typed Stripe config/client/customer/checkout/webhook services, webhook-backed credit fulfillment, D1 ledger authority, clean billing URLs, and complete tests.",
  "delivery": "commit_or_pull_request_with_tests_and_deployment_notes"
}
```

Do not include Stripe API keys, webhook signing secrets, provider tokens,
callback tokens, OAuth material, local secret paths, private runner prompts, or
raw runner payloads in the launch payload.

## Autopilot Work Plan

1. Read all referenced audits and billing files. Confirm the final
   architecture before editing: Stripe is an external payment provider; D1
   remains the product credit ledger.
2. Add Schema models and tagged errors for Stripe IDs, credit packages,
   Checkout snapshots, webhook results, credit fulfillment, and Stripe-safe
   error summaries. Use branded IDs for `cus_*`, `cs_*`, `evt_*`, `pi_*`, and
   `price_*`.
3. Add D1 migrations for Stripe customer mappings, Checkout Session attempts,
   webhook event receipts, and a new `stripe_checkout` billing ledger source.
   Preserve historical `credit_card_placeholder` rows, but stop writing new
   placeholder checkout credits after live Stripe checkout is enabled.
4. Add `StripeConfig` as the only owner of Stripe API key, webhook signing
   secret, API version `2026-05-27.dahlia`, package config, success URL, and
   cancel URL. Use redacted Effect config values.
5. Add `StripeClient` as the only owner of `stripe-node` construction. Build
   it lazily inside the layer with `Stripe.createFetchHttpClient()`, bounded
   retry/timeout settings, and no module-load secret reads.
6. Add `StripeCustomerService` for customer mapping, creation, retrieval, and
   email/metadata synchronization. D1 mapping is the authority; Stripe metadata
   is only a recovery aid.
7. Add `StripeCheckoutService` for creating credit Checkout Sessions,
   retrieving Sessions, and idempotently fulfilling paid Sessions. Omit
   `payment_method_types` in all non-Terminal calls.
8. Add `StripeWebhookService` for raw-body signature verification and event
   processing. Use `constructEventAsync` and the Worker-compatible crypto
   provider from `stripe-node`.
9. Split the current Promise-shaped billing helpers into an Effect-owned
   `BillingCreditService` or equivalent service/repository boundary. Add
   `applyStripeCheckoutCredit` so a paid Checkout Session writes exactly one
   positive D1 ledger row keyed by `billing:stripe-checkout:<sessionId>`.
10. Replace `POST /api/billing/checkout` placeholder behavior with Checkout
    Session creation returning `checkoutUrl` and the current billing summary.
11. Add `POST /api/billing/stripe/webhook` as the authoritative fulfillment
    path. It must not require a browser session; Stripe signature verification
    is the authentication boundary.
12. Add `GET /api/billing/stripe/checkout-return` as a callback that consumes
    `session_id`, calls the same idempotent fulfillment service, and redirects
    to clean `/billing` without checkout result query parameters.
13. Update the logged-in billing command flow to navigate to the returned
    Checkout URL and continue showing billing summary state through the
    existing Foldkit model/update pattern.
14. Add reconciliation support for locally-created but unfulfilled Checkout
    Sessions: retrieve Stripe state, fulfill paid Sessions, and mark unpaid or
    expired attempts.
15. Update docs after implementation:
    - `../2026-06-03-autopilot-billing-credits.md`
    - `../2026-06-04-stripe-effect-service-audit.md` only if implementation
      decisions materially change the audit target.
16. Tighten architecture guardrail budgets if the implementation deletes
    Promise route adapters, raw Stripe/billing helpers, raw Env reads, or other
    zero-tech-debt migration exceptions.

## Safety Rules

- Product balance remains derived from `billing_ledger_entries`.
- Stripe Customer Balance must not be presented as the Autopilot prepaid
  credit balance unless a later explicit invoice-backed product design changes
  that contract.
- No `stripe` import in route modules or browser code.
- No Stripe secret, restricted key, webhook secret, provider token, session
  cookie, or callback token in browser code, docs, logs, D1 public projection,
  commit messages, or run summaries.
- No raw Stripe event payload bodies in durable logs or public artifacts.
- No `payment_method_types` in Stripe calls, except for future Terminal
  in-person payments with `card_present`, which is out of scope here.
- Do not put checkout state in product URLs such as `/billing?checkout=...` or
  `/billing#session=...`.
- Webhook verification must use the raw request body exactly as Stripe sent it.
- Expected failures must be tagged errors and mapped at route boundaries, not
  `message.includes(...)` string classifiers.
- Use outbound Stripe idempotency keys and local D1 idempotency for every
  credit-applying path.
- Do not implement subscriptions, Connect, custom card forms, or customer
  portal work in this task unless a direct dependency appears and is recorded
  as a scoped follow-up.

## Acceptance Criteria

Implementation:

- `POST /api/billing/checkout` creates a Stripe Checkout Session in `payment`
  mode and returns a Checkout URL.
- Checkout Session creation associates the authenticated OpenAgents product surface user with a
  Stripe Customer through a D1 mapping.
- A paid Checkout Session creates exactly one positive
  `billing_ledger_entries` row with source `stripe_checkout`.
- Duplicate webhooks, duplicate return-page calls, and webhook/return races do
  not duplicate credits.
- Positive Stripe credit fulfillment reactivates suspended billing accounts,
  matching coupon and operator credit behavior.
- Product billing summary still reads from the derived D1 ledger.
- `/billing` remains clean after success and cancellation.
- `credit_card_placeholder` is no longer used for new live checkout writes.

Effect architecture:

- Stripe config, client, customer, checkout, webhook, and credit fulfillment
  operations are behind `Context.Service` contracts and `Layer`
  implementations.
- Service methods expose typed `Effect<Success, TaggedError, Requirements>`
  contracts and do not return `Response`.
- Route modules own HTTP method/session checks and domain error mapping only.
- No new `Effect.promise(() => dependencies.*)` route dependency adapters are
  added.
- Stripe SDK errors are converted to redacted, serializable tagged errors.

Tests and checks:

- Unit/service tests cover customer creation/reuse, checkout creation,
  checkout package validation, webhook signature verification, paid
  fulfillment, duplicate fulfillment, unpaid/mismatched sessions, and deleted
  customer handling.
- Route tests cover unauthorized checkout, checkout URL response, webhook
  signature failure, webhook duplicate success, and checkout-return clean
  redirect.
- Billing tests prove ledger-derived balances and account reactivation after
  Stripe credit.
- Browser/Foldkit tests cover billing command navigation behavior without
  exposing Stripe secrets or result query parameters.
- Architecture checks pass, including any zero-tech-debt budgets changed by
  the implementation.
- Run `bun run check:deploy` or document the exact blocker if full deploy check
  cannot run in the Autopilot environment.

Delivery:

- Produce a commit or pull request with implementation, tests, migrations, and
  docs.
- Include migration/deployment notes for future Worker secrets, Stripe webhook
  registration, package/Price configuration, and local test-mode verification.
- Do not deploy live card checkout until test-mode Checkout and webhook
  fulfillment have been verified.

## Suggested Private Run Summary

```text
Implemented Stripe-backed Autopilot credit purchases through typed Effect
services. Checkout now creates Stripe Sessions, webhooks and return callbacks
fulfill paid sessions idempotently into the D1 credit ledger, `/billing` stays
clean, and tests cover customer mapping, Checkout creation, webhook signature
verification, duplicate fulfillment, and billing balance projection.
```
