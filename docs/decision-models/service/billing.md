# Plans, checkout, and entitlements

`gateway`'s billing surface sells the decision API: versioned plans,
checkout, subscriptions, provider webhooks, and the entitlements that
gate a paid door. The domain lives in `tenancy::billing`; this module
mounts it over HTTP. The mode is an explicit operator opt-in — no part
of it runs unless `gateway.json` names a `billing` document, and a
billing document requires the `accounts` and `money` documents because
a subscription binds a workspace and its grants ride the money ledger.
This document covers the configuration, the route surface, the event
contract, and the recovery responsibilities an operator takes on by
enabling it. It does not choose prices or authorize commercial launch;
the terms in [billing-terms.md](billing-terms.md) state what the
published catalog means.

## Configuration

```json
{
  "v": "openagents.gateway.v1",
  "registry": "/srv/registry",
  "require_workspace_membership": true,
  "accounts": {"store": "/srv/protected", "signup_tenant": "acme"},
  "money": {"ledger": "/srv/protected/money.jsonl", "doors": { ... }},
  "billing": {
    "provider": "sandbox",
    "webhook_secret_env": "OPENAGENTS_BILLING_SECRET",
    "checkout_ttl_secs": 86400,
    "webhook_skew_secs": 300,
    "plans": [
      {
        "id": "free",
        "version": "free-2026-02",
        "price": {"amount": 0, "currency": "USD"},
        "period_secs": 2592000,
        "allowance": 500000,
        "signup_credit": 1000000,
        "credit_expiry_secs": 7776000,
        "seats": 1,
        "topups_allowed": false,
        "models": {"listed": ["acme-free-door"]}
      },
      {
        "id": "pro",
        "version": "pro-2026-02",
        "price": {"amount": 9000000, "currency": "USD"},
        "period_secs": 2592000,
        "allowance": 30000000,
        "signup_credit": 0,
        "seats": 5,
        "topups_allowed": true,
        "models": "all"
      }
    ]
  }
}
```

- `plans` is the operator-declared catalog. Each plan carries a stable
  `id`, a `version` a subscription pins at checkout, a `price` in
  millionths of an uppercase three-letter currency, a `period_secs`
  renewal length, the `allowance` a paid period grants, a once-per-
  workspace `signup_credit`, an optional `credit_expiry_secs` bound on
  grant credit, the `seats` the plan covers (`null` leaves the
  workspace's seats untouched), whether `topups_allowed`, and `models`
  — `"all"` or `{"listed": [...]}` — naming the doors the plan
  entitles. A listed door must name a configured backend; the config
  check refuses a plan that sells a door the deployment cannot serve.
- `provider` names whose events the webhook accepts. `sandbox` is the
  built-in provider: an operator-driven journal of provider-side
  events in `billing-provider.jsonl` beside the registry, emitted by
  the `billing-sandbox` binary. Any other value is refused at load.
- `webhook_secret_env` names the environment variable holding the
  webhook HMAC secret — a name, never the secret itself. The secret
  never enters the repository, a log line, or a response.
- `checkout_ttl_secs` bounds how long a pending checkout stands —
  a payment link is not durable. Default one day.
- `webhook_skew_secs` bounds the accepted age of a signed event's
  timestamp. Default five minutes.

A `billing` document without `accounts` or `money` refuses at load:
billing cannot bind a subscription without workspaces, and it cannot
grant an allowance without the ledger.

## The catalog and checkout

`GET /v1/plans` is unauthenticated — the published catalog is what a
buyer reads before it has a credential. It answers each plan's id,
version, price, period, allowance, seats, and door list.

An owner — `Permission::ManageBilling` is owner-only — drives a
workspace's billing through `/v1/workspaces/{id}/billing/*`:

- `GET /v1/workspaces/{id}/billing` answers the workspace's standing:
  subscription and state, pending plan change, open and recent
  checkouts, invoices, and the ledger balance.
- `POST .../billing/subscribe` with `{"plan": "free"}` activates a
  zero-price plan immediately — no checkout stands between a free
  plan and its allowance.
- `POST .../billing/checkout` with `{"plan": "pro"}` opens a checkout
  for a paid plan and answers the session's id and provider reference.
  `{"top_up": {"amount": N}}` on an active subscription opens a
  top-up checkout in the plan's currency; a plan with
  `topups_allowed: false` refuses as `topups_closed`.
- `POST .../billing/portal` answers the billing view's URL — the
  self-serve portal surface.
- `POST .../billing/plan` with `{"plan": "free"}` schedules a
  downgrade at the next paid renewal. A change that would shrink
  seats below the workspace's active membership refuses as
  `seats_below_members` at schedule time, not at the renewal.
- `POST .../billing/cancel` keeps the paid period's entitlement and
  ends renewal; the subscription expires when the period does.
- `POST .../billing/reconcile` is the owner-driven recovery sweep —
  see [Reconciliation](#reconciliation).

`GET /v1/billing/sessions/{checkout}` is the browser's return target
after a hosted payment page. It is unauthenticated by design and
answers the session's standing — `pending`, `complete`, `expired` —
and moves nothing. A return URL carrying `success` means nothing;
only a signed webhook event completes a checkout.

## Provider events

`POST /v1/billing/webhook` accepts one signed event. The
`x-openagents-billing-signature` header carries `t=<unix>,v1=<hex>`
where `v1` is HMAC-SHA256 over `"<t>.<raw body>"` under the
deployment's secret; the timestamp inside the signature is checked
against `webhook_skew_secs`, so a replayed signature is refused as
stale. A missing, malformed, or wrong signature answers
`bad_signature`.

Six event kinds move state:

| Kind | Effect |
| --- | --- |
| `checkout-completed` | Completes a pending checkout: activates the subscription on the pinned plan version, records the invoice, issues the period's allowance grant and — once, ever — the plan's sign-up credit, applies the plan's seats. |
| `invoice-paid` | Renews: closes the prior period, grants the new period's allowance once, lands a scheduled plan change, marks a past-due subscription recovered. |
| `invoice-failed` | Moves the subscription to `past-due`; the entitlement stands through the paid period, and recovery is a later `invoice-paid`. |
| `subscription-cancelled` | Provider-side cancellation — the paid period runs out, then the entitlement ends. |
| `charge-refunded` | Marks the invoice refunded and claws the granted credit back. |
| `charge-disputed` | The same clawback under a dispute marker. |

Every event carries the references its effect needs — subscription,
checkout, invoice, period — so a duplicate, a replay, or an
out-of-order delivery decides against committed state rather than
arrival order. An event that arrives before the state it amends —
an `invoice-paid` ahead of its `checkout-completed` — is journaled
unapplied rather than dropped; reconciliation replays the journal's
unapplied events once the earlier state commits. Event ids are
deduplicated: the same id delivered twice applies once, and a
delivered-again answer is `duplicate`.

## Grants, clawbacks, and the ledger

Applying an event returns effects — the cross-store mutations the
adapter owes `tenancy::money` and the account store. Each effect is
idempotent on its `source` key:

- `billing:signup:<workspace>` — the sign-up credit, once per
  workspace.
- `billing:allowance:<subscription>:<period>` — a paid period's
  allowance, once per period.
- `billing:topup:<checkout>` — a completed top-up.
- `billing:refund:<invoice>`, `billing:dispute:<invoice>`,
  `expired:<grant-source>` — clawbacks.

A grant records the exact `audit` string it posted to the ledger, and
a clawback journals its debit before the billing state seals, so a
crash between the ledger append and the store seal replays the
identical mutation — the ledger's `(workspace, source)` idempotency
makes the replay a no-op, and a conflicting replay is a conflict, not
a second charge.

A clawback debits the lesser of the named amount and the available
balance: a refund or an expired grant can never remove credit already
committed to work, and it can never drive the account negative.

## Entitlements

Under a billing config, every `POST /v1/systemone` call names a
subscribed workspace whose plan covers the door — before registry
authorization and before any quota or money reservation:

- No subscription, or an expired one — `402 no_subscription` /
  `subscription_expired`.
- A subscription whose plan does not list the door — `403` with the
  plan's refusal code.
- An absent `billing` config — the check is a no-op; deployment
  without billing behaves exactly as before.

Entitlement binds the workspace, not the key: a rotated credential
keeps the subscription, and a member's key reaches only the doors the
workspace's plan covers.

## Reconciliation

`POST /v1/workspaces/{id}/billing/reconcile` is the owner-driven sweep
that converges billing state and ledger state after a crash or a lost
delivery:

1. Scans the provider journal for events emitted but never delivered,
   applies each through the same code path the webhook runs.
2. Replays received-but-unapplied events against committed state.
3. Re-issues every recorded grant to the ledger — idempotent, so an
   already-standing grant costs nothing.
4. Applies expiry debits to grants past `credit_expiry_secs`, then
   closes them.
5. Re-issues every journaled clawback — the same idempotent
   discipline as grants.

The answer reports `delivered`, `replayed`, `expired`, and `effects`
so an operator sees what the sweep moved. Reconciliation is safe to
run repeatedly: every step is idempotent on stable source keys.

## Recovery

`billing.json` is the store, opened under `billing.lock` with the same
discipline as the sessions store: bounded event and access histories,
bounded grants, a revision digest per write, and atomic replacement
with the prior revision archived under `billing-history/`. A store
that cannot open answers `billing_unavailable`, never a guessed
standing.

## Refusals

| Code | Status | Cause |
| --- | --- | --- |
| `accounts_unavailable` | 503 | The account store cannot open for a seat update. |
| `already_subscribed` | 409 | The workspace already holds a subscription. |
| `bad_signature` | 401 | Missing, malformed, tampered, or stale webhook signature. |
| `billing_unavailable` | 503 | The billing store cannot open or cannot write. |
| `checkout_pending` | 409 | A checkout for the workspace is already open. |
| `checkout_required` | 409 | A paid plan moves through checkout, not direct subscribe. |
| `currency_mismatch` | 400 | A checkout body's currency disagrees with the plan's. |
| `duplicate_event` | 409 | An event id arrived with a different body. |
| `forbidden` | 403 | The caller is not the workspace's owner. |
| `free_plan` | 409 | A free plan subscribes directly, not through checkout. |
| `ledger_unavailable` | 503 | The money ledger cannot open for a grant or clawback. |
| `malformed` | 400 | A body fails to decode. |
| `no_subscription` | 402 | The route or the door needs an active subscription. |
| `plan_excludes_model` | 403 | The subscription's plan does not cover the door. |
| `seats_below_members` | 409 | A plan's seats fall below active membership. |
| `subscription_closed` | 409 | The subscription can no longer take the action. |
| `subscription_expired` | 402 | The subscription's paid period has ended. |
| `topups_closed` | 403 | The plan does not sell top-ups. |
| `unknown_checkout` | 404 | No checkout by that id. |
| `unknown_event` | 409 | An event names a record the store does not hold. |
| `unknown_grant` | 409 | A clawback names a grant the store does not hold. |
| `unknown_plan` | 404 | No configured plan by that id; 503 when a subscription's pinned plan leaves the catalog. |
| `unknown_provider` | 400 | The event names a provider this deployment does not run. |
| `webhook_unconfigured` | 503 | The secret's environment variable is unset. |

## Limits

- `sandbox` is the only provider. It exists to prove the checkout,
  renewal, failure, cancellation, refund, dispute, and recovery paths
  end to end; it moves no real money and carries no commercial
  meaning. Wiring a live provider is a new adapter against the same
  event contract, plus the commercial decisions tracked in
  [#9498](https://github.com/OpenAgentsInc/openagents/issues/9498).
- Plans are operator configuration. Editing the catalog changes what
  new checkouts sell; a subscription pins the version it bought, so
  an in-flight customer is never repriced mid-period.
- The browser return is display-only by construction. Any deployment
  that grants credit on a return parameter is misconfigured, not
  billed.
