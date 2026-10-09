# Saved customer environments: contract v2 (subscription)

Status: prices decided by the owner 2026-10-09
([#11006](https://github.com/OpenAgentsInc/openagents/issues/11006), ENV-10).
Nothing here can be bought yet. The code path exists and is closed. It opens
only when the plan is published, the Stripe product and price exist, and a
funded qualification is recorded (see [Availability](#availability)).

v1 (2026-10-09, proposed) sold each environment setup on its own, in sats,
with a two-hour wall-time limit and prepaid retention days. The owner replaced
it the same day with a subscription: environments are part of a $20/month
plan with a monthly allowance, and we set no time limit on a run. A change
after this is a new contract version.

The [retail contract v1](retail-contract.md) sells one task on one sandbox and
then deletes the sandbox. This contract is the separate class that the
[environment onboarding contract](example-cursor-cloud-agent-onboarding/environment-onboarding.md)
requires before customers can keep a machine image: a subscriber sets up,
builds, checks, and saves one repository environment, and the saved image is
kept while they stay subscribed.

## The classes

| Identity | Value |
| --- | --- |
| Contract | `openagents.cloud.retail-environment.v2` |
| Computer class | `retail-env-standard-v1` (2 vCPU, 8 GB) |
| Task class | `retail-environment-setup-v1` |
| Plan schema | `openagents.cloud.environment-plan.v1` |
| Terms schema | `openagents.cloud.environment-terms.v1` |
| Billing catalog plan | `pro` (`crates/gateway/fixtures/plans/pro.json`) |

## The plan

The checked-in plan is `crates/retail-cloud/fixtures/environment-plan.json`
(`env-plan-2026-10-09.0`, status `proposed` until the owner publishes it). The
billing catalog carries the same allowance on the `pro` plan, and
`GET /v1/plans` publishes it as `environments`.

| Pro | |
| --- | --- |
| Price | $20 a month (`price.amount` 20,000,000 millionths of USD, 30-day period) |
| Included | 100 machine-hours a month on the standard machine (2 vCPU, 8 GB) |
| At once | 2 machines |
| Saved images | 20 GB, at most 10 saved versions |
| Run length | No limit from us. The person may set their own (`max_seconds`). |
| Extra hours | $0.18 a machine-hour from the account's credits, only when turned on, up to a monthly cap the person sets. Off by default. |
| Rollover | None. Each month starts from its own 100 hours. |
| Models | Never included. The setup agent runs on the person's own Claude or Codex key or subscription (`customer:model`). |

### Market check (2026-10-09)

| Offer | Price | Compute included |
| --- | --- | --- |
| GitHub Codespaces, Pro account | $4/month account | 180 core-hours (90 h on 2-core, 8 GB) and 20 GB-month; then $0.18/h for 2-core and $0.07/GB-month |
| Replit Core | $20/month | $20–25 of usage credits for agent, compute, and deployments |
| Cursor Pro | $20/month | $20 of usage; background agents draw on it |
| Ona (formerly Gitpod) Core | from $20/month | credits shared by environment runtime and agent use; $10 per 40 extra |
| Devin Pro | $20/month | ACU-based; about $2.25 per ACU (about 15 minutes of agent work) |

100 hours on a 2 vCPU / 8 GB machine is slightly more than Codespaces gives a
Pro account, and $0.18 an extra hour is the Codespaces 2-core rate, so the
proposal stood unchanged. Unlike Replit, Cursor, Ona, and Devin, model use is
not in the price; the $20 buys only machines and storage.

## What a setup is

- **Machines.** A setup machine, a clean builder, and an independent verifier
  with its idempotence fork, never more than 2 at once. They are the ENV-03
  to ENV-05 owners' dedicated machines, never a shared pool host and never a
  customer's chat computer. One setup runs per account at a time.
- **Source.** A public HTTPS repository on `github.com` at an exact
  40-character commit, as in retail v1. No private repositories, uploads, or
  submodules.
- **Request.** An objective of at most 8 KiB, a qualification profile name,
  1 to 8 behavior checks of at most 1,024 bytes each, and optionally the
  person's own limit in machine-seconds. These are frozen by digest before any
  build.
- **Saved image.** When the checked version is saved, its image is kept while
  the account is subscribed, within 20 GB and 10 versions. Saving past either
  is refused with a plain message, and the person can delete a version to
  make room.

### Credentials and authority

- **One credential.** The setup agent runs on the person's own model key or
  subscription, delivered when the setup machine starts through the same
  private custody and per-boot delivery as retail v1 (#10712). It is never
  written into a recipe, image, log, or receipt, and the builder's
  sanitization step removes sign-ins before capture. No owner or operator
  credential reaches any machine.
- **Selection.** A saved version belongs to the account that made it. Only
  that account's tasks can select it (`selectable_by`), and only while the
  account is subscribed.
- **No machine access, no publication**, as in v1: no shell, SSH, or terminal,
  and no push, pull request, or issue comment.

## Metering

`crates/retail-cloud/src/environment.rs` keeps the meter in the retail
journal.

- **Months.** Billing records each paid month with `record_period` (account,
  plan version, start, end). A setup counts against the month it was
  confirmed in.
- **Offer and confirm** need a current month and some time left: included
  hours, or extra hours when turned on and under the cap. Nothing is held.
  Otherwise they refuse with a plain sentence (`Refusal::message`):
  - "You've used this month's 100 hours. Turn on extra hours in Settings, or
    wait until November 9."
  - "You've reached the $10 you set for extra hours this month. Raise it in
    Settings, or wait until November 9."
  - "Saved environments come with the Pro plan."
  - "Your 2 machines are busy. Wait for a setup to finish, then try again."
- **Budget.** `budget` returns the machine-seconds left this month (included
  plus what the cap still pays for). The runner stops machines when it
  reaches zero or when the person's own limit is reached. That is the only
  stop; we impose no run length.
- **End** records the ending and the measured machine-seconds once. An
  unknown ending counts nothing until it is known; a known ending never
  changes.
- **Settle** counts the seconds once: first against the month's included
  hours, then as extra hours. Extra hours are charged only when turned on,
  at the terms' rate, and never past the cap; seconds past the allowance with
  extra hours off are not charged.
- **Debits.** Each extra-hours charge is written to an outbox (`env:<purchase>`)
  and handed once to the credits ledger by `post_debits`, through the
  `Credits` adapter, which must be idempotent on the key. A failed post stays
  waiting and is retried.
- **Recover** (the retail worker, every tick) settles known endings once,
  leaves unknown ones waiting, and retires images.
- **Retirement.** When a subscription ends, saved images stay 30 days, then
  are retired (`retire` returns them for the provider owner to delete). A
  person can delete a version any time (`delete`).

| Ending | Counted |
| --- | --- |
| No machine started, or none reachable | Nothing |
| Machines ran; no version saved, or cancelled | Measured machine-seconds |
| Version saved | Measured machine-seconds; the image counts toward storage |
| Not known yet | Nothing until it is known |

The fake-billing checks are `crates/retail-cloud/tests/environment.rs` and
`saved_environments_use_the_retail_transport_and_stay_closed_by_default` in
`crates/retail-service`.

## Transport

Customers use the authenticated retail customer transport (#10956, #10970):
`/v1/retail` with the same principal, grant, and spend checks as tasks.
Environment refusals carry the plain sentence in `message`.

| Operation | Needs |
| --- | --- |
| `environment_offer` | spend, execute, and disclose |
| `environment_confirm` | spend, execute, and disclose |
| `environment` | read; own account only |
| `environment_delete` | spend; own account only |

The native client (`compute-workbench`, feature `client`) has
`environment_offer`, `environment_confirm`, `environment`, and
`environment_delete`.

## Settings

The website's **Settings → Plan** (`crates/openagents-web/src/plan.rs`) shows
the plan, the hours used this month and when they reset, saved storage and
versions, and the extra-hours switch with its monthly cap. It reads and writes
the meter given by `--plan-meter`. Without a meter it says the server doesn't
track hours yet; without `--plan-checkout PLAN` (or without a meter) it says
subscribing isn't open instead of showing a button. The served docs page is
`/docs/pricing`.

With `--plan-checkout pro`, **Subscribe** asks the account service (the
gateway) to open a Stripe Checkout Session for that plan on the person's own
workspace and sends the browser there. Coming back shows "Stripe is confirming
your payment" until Stripe's signed event lands; then Settings shows Pro. A
subscriber gets **Manage subscription**, Stripe's billing page, to change the
card or cancel.

## Billing (Stripe, #11072)

The gateway sells the plan with `billing.provider: "stripe"` and a
`billing.stripe` section (`crates/gateway/src/subscriptions.rs`): the Stripe
price per plan, the names of the environment variables holding the secret key
and webhook signing secrets (never the secrets), the return pages, and the
`environment_meter` (the same journal the website reads with `--plan-meter`
and the retail service keeps). `live: false` is Stripe test mode and refuses
live keys and live events. Stripe's webhook points at `POST /v1/billing/webhook`.

| Stripe event | What happens |
| --- | --- |
| `checkout.session.completed` | The subscription starts in the billing book, with Stripe's subscription and customer. |
| `invoice.paid` | The paid month (the invoice line's period) is recorded with `record_period`, and the workspace that pays extra hours is named. A renewal starts a new month, so hours reset. An invoice that arrives before its checkout is recorded and answered 409, so Stripe sends it again. |
| `invoice.payment_failed` | Past due. No new month is recorded: the paid month runs to its end, then setups refuse ("Saved environments come with the Pro plan.") until Stripe collects, when `invoice.paid` records that month. |
| `customer.subscription.deleted` | The subscription ends; a month cut short ends then (`end_period`). Saved images stay 30 days, then retire. |
| `charge.refunded` | A **full** refund: see Refunds and disputes below. A partial refund is acknowledged and changes nothing. |
| `charge.dispute.created` | The subscription is at risk; new paid months wait. See below. |
| `charge.dispute.closed` | `won` (or `warning_closed`) clears the risk; `lost` is treated like a full refund. Any other status is acknowledged and waits for the final one. |

### Refunds and disputes (#11074)

Every one of these arrives on the same signed `POST /v1/billing/webhook`
(same signature check, same test/live refusal, same event-id idempotency) and
is placed on the invoice it paid: by the charge or payment-intent references
recorded when `invoice.paid` arrived, by the invoice a charge names, or by the
subscription's customer and amount. A dispute names only its charge, so the
gateway reads that one charge back from Stripe (`GET /v1/charges/{id}`, same
key and mode) when the book can't place it; if Stripe can't answer, the
webhook answers 503 and Stripe sends the event again. A charge that is not a
subscription payment is acknowledged and ignored.

- **Full refund.** The invoice closes as refunded and the allowance it granted
  is taken back from the workspace's credits (never more than is unspent, and
  once, keyed `billing:refund:<invoice>`). When that invoice paid the month the
  subscription stands in, the month **ends at the event's time**
  (`end_period`), the subscription is cancelled, setups refuse ("Saved
  environments come with the Pro plan."), and Settings says "Your last Pro
  payment was refunded, so Pro ended." Hours already counted stay counted. A
  refund of an earlier month only takes back its allowance. Paying again later
  starts a fresh month and clears the message.
- **Partial refund.** The month stands; nothing changes.
- **Dispute opened.** The subscription is marked at risk. The month already
  paid runs to its end and nothing is taken back yet, but **no new paid month
  starts while the dispute is open**: an `invoice.paid` for a later month is
  answered 409 `not_yet` without being recorded, so Stripe sends it again (its
  retries last about three days; after that, resend the event from the Stripe
  Dashboard once the dispute has closed). Settings says "Your bank has opened a
  dispute on a Pro payment. Pro stays on for the month you paid for, and a new
  month won't start until the dispute is closed."
- **Dispute won.** The invoice is paid again, the risk is cleared, the message
  goes, and new months start as usual.
- **Dispute lost.** Handled once, like a full refund (the money already went
  back to the bank): the allowance is taken back (`billing:dispute:<invoice>`),
  the paid month ends at the event's time, the subscription is cancelled, and
  Settings says "Your bank reversed a Pro payment, so Pro ended."
- **Once.** The event id is the journal key, so a repeated event is a
  duplicate; a second event for an invoice already closed, or for a dispute
  already opened or closed, is superseded. A closing event that arrives before
  its opening one wins: the late opening is superseded.

The sandbox provider's one-step `charge-disputed` event (claw back at once) is
unchanged; the Stripe dispute steps are `dispute-opened`, `dispute-won` and
`dispute-lost`. Checks: the three refund/dispute tests in
`crates/gateway/tests/billing_stripe.rs` and the Settings messages in
`subscribe_opens_stripe_checkout_and_settings_shows_pro_after_the_event`.

Cancelling happens on Stripe's billing page; the gateway's own cancel route
answers `cancel_in_portal` under Stripe so a cancel can never leave Stripe
charging. Extra-hour charges leave the meter's outbox through
`subscriptions::post_debits` (on every webhook and once a minute): one reserve
and settle on the paying workspace's money ledger, keyed `env:<purchase>`, so
a charge lands once and never past the person's cap. A charge the credits
can't cover yet stays waiting and is offered again.

The checks are `crates/gateway/tests/billing_stripe.rs` (a loopback stand-in
for Stripe's API, test mode only), the ledger test in
`crates/gateway/src/subscriptions.rs`, and
`subscribe_opens_stripe_checkout_and_settings_shows_pro_after_the_event` in
`crates/openagents-web/src/cloud/tests.rs`.

## Availability

`environments` in the retail service configuration holds the owner's launch:
the published plan and its gate. The gate opens only when all of these hold:

1. `contract_reviewed` is true.
2. The plan's status is `published`, and the gate names its exact digest.
3. A funded qualification receipt is recorded: a real subscription paid on
   the deployed service, a real setup, build, check, and save counted against
   its month, and the machines' teardown acknowledged.

Without `environments`, every environment operation answers "not available".
The checked-in plan is `proposed`, so it can never open the gate.

Selling Pro needs the owner's Stripe product and price, the key and webhook
secret in the deployment's environment, and the website started with
`--plan-meter` and `--plan-checkout pro` (see [Billing](#billing-stripe-11072)).

## Not in v1 or v2

GCE placement (the ENV-09 adapter is operator-only), private repositories,
customer shells, publication, continuation on a kept machine, sharing a saved
environment with another account, and model usage paid by OpenAgents.
