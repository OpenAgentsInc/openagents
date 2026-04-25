[Home](../README.md) · [Investor Path](README.md) · **03. Autopilot — The Wedge**

# 3. Autopilot — The Wedge

> _"Autopilot turns your machine into a money printer — it prints Bitcoin."_
>
> — [`docs/MVP.md`, OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md)

<figure>
  <img src="../assets/graphics/slide-what-we-shipped.jpg" alt="Demo Day slide — What We Shipped" />
  <figcaption>Already-shipped infrastructure that the Autopilot wedge is built on: decentralized Bitcoin-paid compute, the first agent marketplace, Percepta reproduction, and Psionic edge inference.</figcaption>
</figure>

**You will learn:**

- The MVP spec and the "irreducible loop"
- Why the wedge is desktop-first
- The pane inventory and what each pane is allowed to do

## The product, in one paragraph

Autopilot is the desktop wedge. It's the one surface where a user can install OpenAgents, flip a single switch, and earn real Bitcoin for compute work they're supplying to the network. From [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"Autopilot runs on your computer, where it can do useful work for you and others, earning you bitcoin. Soon you can control Autopilot from our mobile app or openagents.com. Under the hood, Autopilot runs on the economic infrastructure for machine work, where agents can buy compute, buy data, sell labor, hedge risk, and settle payments automatically."_

## The irreducible loop

The MVP is intentionally narrow. Everything in the product exists to make one loop real, repeatable, and not fake. From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"They click **Go Online** → they receive at least one paid job → their **wallet balance increases** → they **withdraw** by paying a Lightning invoice._
>
> _Everything else is in service of making that loop real, repeatable, and not fake."_

That is the product. Everything in the rest of this chapter — panes, modes, runtimes, relays, wallets — exists because it supports one step in that loop.

## Why desktop-first

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"Desktop-first isn't a style choice, it's the substrate. Autopilot needs:_
>
> - _access to local files and local toolchains_
> - _a stable always-on process model when 'Go Online' is enabled_
> - _GPU/CPU resources for local execution_
> - _a place to safely hold keys, wallet state, and job receipts_
> - _a fast, game-like UI with immediate feedback_
>
> _Web-first would make this feel like a dashboard. Desktop-first makes it feel like a machine you own and can upgrade."_

That last sentence is the whole wedge strategy. Autopilot is not SaaS. It's a rig.

## The first "holy shit" moment

The [MVP doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md) is explicit about the emotional design target:

> _"The MVP is designed to make the core emotional beat unavoidable: 'holy shit, the numbers are ticking up.' If that moment doesn't happen, nothing else matters."_
>
> _"The first 'ah ha' moment should happen within 30-60 seconds."_

On-stage at Demo Day, Chris points to the same moment from the supply side: the [earnings scoreboard](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md) celebrates `10`, `25`, `50`, `100` sats as ceremonial milestones. The user should feel the progression from _"it works"_ to _"this thing is paying me"_ in under a minute.

## The pane + command surface

Every user-facing feature in Autopilot is reachable by exactly one command-palette command opening exactly one pane. From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

| Pane                     | Command ID                  | What it covers                                                 |
| ------------------------ | --------------------------- | -------------------------------------------------------------- |
| Autopilot Chat           | `pane.autopilot_chat`       | Personal agent thread + local execution                         |
| Go Online                | `pane.go_online`            | Provider mode toggle and lifecycle                              |
| Provider Status          | `pane.provider_status`      | Uptime, heartbeat, degraded/error visibility                    |
| Job Inbox                | `pane.job_inbox`            | Incoming NIP-90 intake                                          |
| Active Job               | `pane.active_job`           | `received → running → delivered → paid`                         |
| Earnings Scoreboard      | `pane.earnings_scoreboard`  | Sats/day, lifetime sats, jobs/day, last result                  |
| Spark Lightning Wallet   | `pane.wallet`               | Balance, receive, send, history                                 |
| Pay Lightning Invoice    | `pane.pay_invoice`          | Withdraw / prove custody                                        |
| Relay Connections        | `pane.relay_connections`    | Nostr relay connectivity + failure diagnosis                    |
| Sync Health              | `pane.sync_health`          | Spacetime subscription and cursor state                         |
| Starter Jobs             | `pane.starter_jobs`         | Seed-demand visibility for first earnings                       |

The full inventory — 19 panes covering every MVP lane — is canonical in the MVP doc. Every one of them exists to resolve an ambiguity the user would otherwise have to trust on faith.

## Two-sided marketplace, collapsed into one app

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"This is not 'another chat app.' It is a **two-sided marketplace** collapsed into one product surface:_
>
> - _**Buy side:** you use Autopilot as a personal agent through an app-owned local coding shell, currently backed by Codex and designed to host our own Probe runtime later, and can submit work requests out to the network when it makes sense._
> - _**Sell side:** you provide compute now (and later additional provider lanes such as liquidity solving and plugins/skills) to the network and **earn Bitcoin**."_

The MVP ships the sell-side lane — compute provider via NIP-90 jobs — first. Buy-side is there to validate the sell-side loop, not to be a general-purpose buyer client yet. The [`v0.1` release cut](https://github.com/OpenAgentsInc/openagents/blob/main/docs/v01.md) is explicit:

> _"v0.1 is not intended to be a real buyer product for serious work. The buyer-facing surfaces that exist in the app are there to bootstrap and validate the seller network."_

## Where the demand comes from

The killer failure mode of a two-sided marketplace at launch is cold start — the seller earns nothing. From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"The main failure mode is not crashes or bugs. The main failure mode is **earning nothing**."_

The answer is seeded demand. Production Nexus automatically dispatches a bounded CS336 A1 homework/training run to online eligible Pylons on a `~10-minute` cadence, paying `25 sats` per accepted contribution with a `6,400-sat` cap per automatic cycle, supporting up to `256 contributors`. That loop is how the first-earn scoreboard tick is guaranteed. Full detail in [Chapter 4 — The Earn Loop](04-earn-loop.md) and [Chapter 5 — Pylon Provider](05-pylon-provider.md).

## Data Market: already the second lane shipped

Autopilot doesn't stop at compute. The secondary Data Market MVP slice is already in-repo:

- `Data Seller`, `Data Market`, `Data Buyer` panes
- `autopilotctl data-market ...` full shell-first control
- `autopilot_headless_data_market` no-window runtime
- NIP-90 kinds `5960` (request) / `6960` (result) / `31990` (handler)
- Live verification on `wss://relay.damus.io` and `wss://relay.primal.net`

See [Chapter 6 — Data Market MVP](06-data-market-mvp.md).

## The investor read-through

Autopilot is the wedge for one reason: if the desktop-install → go-online → first-sats loop works reliably on a fresh machine, the rest of the five-market marketplace has a _user_ to hand off to. Without the wedge, the kernel is an economy with no entry door.

The kernel, the five markets, and the substrate already exist (Chapters 2, 7, 8). Autopilot is the product that makes them reachable. It's why the repo defines its final product mantra:

> _"If a proposed feature does not make it easier for a user to:_
>
> 1. _go online,_
> 2. _earn sats,_
> 3. _trust the earnings are real,_
> 4. _withdraw instantly,_
>
> _…it is not MVP."_

— [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md)

---

**← Previous:** [02. The Five Markets](02-five-markets.md) · **Next:** [04. The Earn Loop](04-earn-loop.md) **→**
