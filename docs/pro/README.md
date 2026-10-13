# OpenAgents Pro: plan spec (proposal)

Status: **proposal, 2026-10-13.** Prices and allowances marked *proposed* need
the owner's sign-off. Stripe product and price details get filled in once the
Stripe side is set up (see [Stripe](#stripe)).

Related: Nostr in the CLI and what's free, Pro or usage: [nostr.md](nostr.md).
Paying for Pro in Bitcoin over Lightning: [bitcoin.md](bitcoin.md).

## The idea in one paragraph

A really good free plan for people and for agents: Coder everywhere, every
harness, your own subscriptions and keys, projects, sync, sharing, and a
free daily model allowance. When someone wants the heavier things that cost
us real money (cloud machines, cloud agents running while their laptop is
closed, image and file uploads, advanced project settings, more model
allowance), they move to **Pro: $20 a month, starting with a 3-day free
trial, plus usage** past what's included.

## Plans

| | Free | Pro |
| --- | --- | --- |
| Price | $0 | **$20 a month** (Stripe), 3-day free trial |
| Who | People and agents | People and agents (an agent's Pro is paid by the account that owns it) |
| Usage past what's included | Credits only (card or Lightning) | Credits, card or Lightning, under a monthly cap you set |

### Free: everything you need to use Coder for real

- **Coder on every device:** terminal, desktop, web and phone, one
  conversation everywhere.
- **Every harness:** Claude Code, Codex, OpenCode, Grok Build, Devin and
  Cursor, on Auto or picked with Shift+Tab, on your own subscription or key.
  We never charge for a model you bring.
- **System 1 in front:** routing, briefs, trimming, approvals and checks.
  These are the savings, so they stay free.
- **Free daily model allowance:** the signed-in daily allowance on our router
  models (#11264), plus the public free tier for keys. Same amounts as
  today.
- **Projects, basic:**
  - unlimited projects and chats;
  - chats filed into projects by repo;
  - moving chats between projects;
  - a project name, its repo, and a short project instruction (text only).
- **Sync and history:** full sync across devices.
- **Sharing:** public share links for a chat.
- **Plugins:** install and use any plugin, and publish your own.
- **Local agents:** delegation between harnesses on your own machine,
  background agents, parallel parts, /rewind and Second look.
- **Agents:** the same free plan.
  - An `oa_agent_` identity gets its own free daily allowance and can use
    every free feature through the API and CLI.
  - Refusals tell the agent exactly what's needed, in a form a program can
    read (see [Entitlements](#entitlements)).

### Pro: the heavy and advanced parts

Everything in Free, plus:

1. **6,000 minutes of cloud time a month (100 machine-hours).**
   - The existing environments plan (`docs/cloud/retail-environment-contract.md`):
     100 machine-hours on the standard machine (2 vCPU, 8 GB), 2 machines at
     once, 20 GB of saved images (up to 10 versions).
   - Unused hours don't roll over.
   - Past 100 hours, extra hours are $0.18 each from credits. That's only if
     you turned extra hours on, and never past your cap.
2. **Cloud agents.**
   - Run Coder on our machines with your laptop closed: issue runs, long jobs
     and scheduled runs.
   - These draw on the included hours.
3. **Advanced project settings.**
   - **Image and file uploads:** in chats and as project files (screenshots,
     designs, PDFs, logs). *Proposed:* 5 GB of project storage.
   - **Project knowledge:** files and docs the agent reads every time, beyond
     the short free instruction.
   - **Saved environment setup** per project: setup commands, packages and a
     saved image to start from.
   - **Project secrets** for cloud runs, held server-side, never shown back.
   - **Multiple repos** per project.
4. **More model allowance.**
   - *Proposed:* $5 of model usage included each month on our gateway, on top
     of the free daily allowance.
   - This is spent before credits are touched, and it doesn't roll over.
5. **Private sharing.** Share a chat with specific people instead of a public
   link. *Proposed;* it can come after launch.
6. **Faster System 1 lane.** Pro requests go first on our own Clef hardware
   (TPU/GPU) when it's busy. *Proposed;* it only matters at load.

What is **not** gated, ever:
- **Your own models and subscriptions.**
- **Safety:** approvals, gates and holds.
- **Your data:** export, sync and deleting your account.
- **Plugin payouts.** Authors are paid the same whoever calls the plugin.

## Usage, on top of Pro

- **Same rule as now: we never stop work at a limit we chose.** The only
  caps are ones the person sets (`docs/cloud/retail-environment-contract.md`).
- **When included amounts run out,** work continues from credits, but only if
  the person turned that on. Otherwise it stops with one clear line and a
  button:
  > You've used this month's 100 hours. Turn on extra hours in Settings, or wait until Nov 9.
- **Credits** are bought by card (Stripe Checkout, existing card funding) or
  over Lightning.
  - The plan itself is billed by Stripe as the owner asked.
  - Usage stays payable in Bitcoin over Lightning, like everything else.
- **Prices for usage** stay in `docs/cloud/retail-prices.md` and the gateway
  rate card. This spec doesn't change them.

## The 3-day free trial

- **Starting it.** Subscribe on Settings → Plan, or from any Pro button. This
  opens Stripe Checkout with `subscription_data.trial_period_days = 3` and
  collects a card up front.
- **During the trial:** every Pro feature is on.
  - Cloud compute is limited to *proposed* 10 machine-hours and $1 of the
    included model usage, so a trial can't be farmed for compute.
  - The rest of the month's allowance starts when the first $20 is paid.
- **Reminder.** One day before the trial ends, we email and show a line in
  the app ("Your Pro trial ends tomorrow. Cancel anytime in Settings."),
  driven by Stripe's `customer.subscription.trial_will_end` event.
- **End of trial.** Stripe charges $20 and the first full month starts.
  - Cancelling during the trial costs nothing.
  - If the charge fails, the account goes back to Free.
- **One trial per person.** Checked on the account and on the card's Stripe
  fingerprint. A second attempt goes straight to paid checkout, no trial.

## Downgrade and cancel

- **Cancel anytime** in Settings (Stripe's billing page). Pro runs to the end
  of the paid month.
- **Back on Free, nothing is deleted right away:**
  - Uploaded files and project knowledge stay readable and downloadable, but
    no new uploads.
  - Saved environment images are kept 30 days, then removed after an email
    warning.
  - Project secrets are kept but not used.
  - Cloud agents stop starting new runs. A run in progress finishes.
- **Coming back to Pro** brings it all back as it was, within those 30 days.

## Entitlements

One typed answer to "what may this account do", used by every surface. Gates
are enforced on the server; the apps only reflect them.

- **`tenancy::billing` decides it.** The billing book stays the authority on
  the plan. A new `Entitlements` value is derived from the subscription state
  (`trialing`, `active`, `past_due`, `canceled`) and holds the plan, the trial
  end, and a fixed set of capability flags:
  - `cloud_compute`
  - `cloud_agents`
  - `uploads`
  - `project_knowledge`
  - `project_secrets`
  - `multi_repo`
  - `env_setup`
  - `private_share`
  - `priority_system1`
  - `included_model_usd_micros`
- **`GET /v1/me/plan`** answers the plan, trial end, capability flags and this
  month's included amounts and use. It's the same for people (session) and
  agents (`oa_agent_` token).
- **Refusals.** A gated request answers `402` with a machine-readable body:
  `{"error":"pro_required","capability":"uploads","upgrade_url":"https://openagents.com/settings/plan"}`.
  - The person sees one plain line with an Upgrade button, e.g. "Image
    uploads are part of Pro. Try it free for 3 days."
  - No dead controls: a Pro control on Free either works (by opening the
    upgrade) or isn't shown.
- **Past due.** A missed payment gets 3 days of grace, on card and on
  Bitcoin ([bitcoin.md](bitcoin.md)). During that
  time Pro stays on, with a line asking to update the card. After that the
  account goes to Free, as in [Downgrade and cancel](#downgrade-and-cancel).

## Stripe

What exists (#11072, `crates/gateway/src/subscriptions.rs`):
- Checkout in subscription mode.
- The billing page.
- Signed webhooks: `checkout.session.completed`, `invoice.paid`,
  `invoice.payment_failed`, `customer.subscription.deleted`, plus refunds and
  disputes.
- Paid months written into the environment meter.

What this spec adds:
- **Product and price.** One Product "OpenAgents Pro" and one recurring Price
  of $20/month (USD). The plan id is `pro`. The existing billing catalog entry
  carries the Stripe price id and its plan terms. *To fill in from Stripe:*
  - product and price ids for test and live;
  - tax behaviour (Stripe Tax on or off);
  - currencies.
- **Trial.**
  - Checkout sets `trial_period_days: 3` and
    `payment_method_collection: always`, plus a trial-end behaviour of cancel
    if there's no payment method.
  - The trial is left off when the account or card fingerprint already had
    one.
- **New events.**
  - `customer.subscription.created` and `customer.subscription.updated`: track
    `trialing → active`, plan changes, and `cancel_at_period_end`.
  - `customer.subscription.trial_will_end`: the reminder.
  - `invoice.paid` keeps recording paid months. The trial's $0 invoice starts
    only the trial allowance.
- **Customer Portal.** Cancel, card change and invoices. Plan switching is
  off until there's more than one paid plan.
- **Test mode first.** The existing `live: false` guard applies. Live keys are
  the owner's step.

## Agents on Pro

- **Who can pay.** An agent identity belongs to an account. That account's
  Pro covers its agents, who share the account's included hours and model
  usage.
- **Agents with their own Lightning credits** can use metered compute without
  Pro, at usage prices but without the included amounts. This keeps the "pay
  per use, no subscription" path open for agents. *Proposed:* only cloud
  compute and model usage, not the project features.
- **Machine-readable.** Every agent can read its plan from `GET /v1/me/plan`
  and the upgrade path from a refusal body. Nothing needs a browser except
  the card checkout itself.

## Where it shows

- **Settings → Plan** (web, desktop, phone).
  - Free or Pro, the trial's end date, and this month's hours and model usage.
  - Extra hours with their cap, Subscribe / Manage subscription, and cancel.
- **Pro features in context.** The upload button, cloud agent start, project
  knowledge and secrets show a small "Pro" mark on Free, and open the upgrade
  when used.
- **Terminal.**
  - `openagents plan` shows the plan and use.
  - `openagents plan upgrade` prints the checkout link and opens it.
- **Pricing page** on openagents.com: see [Pricing page](#pricing-page).

## Pricing page

`/pricing` on openagents.com (`crates/openagents-web/src/pages/pricing.rs`,
cards from `openagents_ui::content::PlanCards`). Linked from the site's footer
links and from Settings → Plan ("Compare plans").

- **Three cards:** Free, Pro ($20/mo., 3-day free trial) and Teams &
  companies ("Talk to us", a mail link; no team price).
- **Pro leads with minutes:** "6,000 minutes of cloud time a month", with
  "2 vCPU, 8 GB machines · 2 at once" under it. The numbers come from the
  checked-in plan terms, the same ones Settings → Plan shows.
- **Buttons always go somewhere:** Pro's goes to Settings → Plan (signed
  out, through sign-up first), where Subscribe opens checkout or says
  subscribing isn't open on that server. Free's says "Current plan" when
  signed in. Teams' opens an email to us.
- **Nothing marked proposed** shows on the page: an unsettled number (such
  as the included model usage) is described without one.
- **Questions under the cards:** the trial, going past what's included,
  paying (card or Bitcoin over Lightning), models you bring, and agents.

## Build order (issues to file)

1. **Entitlements** in `tenancy::billing`, `GET /v1/me/plan`, and the `402
   pro_required` refusal. Server-side, with tests per capability.
2. **Stripe trial:** Checkout `trial_period_days`, the new subscription
   events, one trial per account/card, the trial allowance, and the reminder.
3. **Gate the existing Pro features** behind entitlements: cloud environments
   and cloud agents, which use the meter we already have.
4. **Uploads:**
   - images and files in chats and projects;
   - storage (our GCS bucket);
   - size limits;
   - web, desktop and phone pickers.
5. **Project settings:** knowledge, secrets, multi-repo and saved environment
   setup.
6. **Included model usage** on the gateway: spent before credits, reset on
   renewal.
7. **Settings → Plan, the pricing page, and `openagents plan`.**
8. **Downgrade handling:** read-only uploads, the 30-day image keep, warning
   emails.
9. *Later:* private sharing and the priority System 1 lane.

## Open questions for the owner

1. **The proposed numbers:**
   - $5 of included model usage a month;
   - 5 GB of uploads;
   - a trial limited to 10 hours and $1 of models;
   - 30 days of keep after downgrade.
2. **Agent access without Pro.** Should agents with only Lightning credits get
   metered cloud compute without Pro (proposed yes), or must every cloud run
   be on a Pro account?
3. **Paying for Pro in Bitcoin.** Decided: Pro can be paid by card or over
   Lightning; the trial is card-only ([bitcoin.md](bitcoin.md)).
4. **Teams.** A team plan (per-seat Pro with shared hours) now or later?
5. **Annual price.** E.g. $200 a year, now or later?
