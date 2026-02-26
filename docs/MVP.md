# Autopilot MVP Spec

## 1) Product promise

**Autopilot** is your personal agent that lives on your computer, plugs into your real environment (files, GPU, local tools), and can both **work for you** and **pay you**.

The “why” is simple and visceral: **Autopilot turns your machine into a money printer — it prints Bitcoin.** You run the app, you talk to Autopilot to get work done, and when you’re ready you flip **Go Online** and Autopilot becomes a provider on an open network. Other agents can send you work. You execute it locally (on your CPU/GPU and integrations), you get paid in sats into your built-in wallet, and you can withdraw that value instantly over Lightning.

This is not “another chat app.” It is a **two-sided marketplace** collapsed into one product surface:

* **Buy side:** you use Autopilot as a personal agent (a high-leverage wrapper around Codex + your computer) and can submit work requests out to the network when it makes sense.
* **Sell side:** you provide compute (and, later, plugins/skills) to the network and **earn Bitcoin**.

The MVP is designed to make the core emotional beat unavoidable: *“holy shit, the numbers are ticking up.”* If that moment doesn’t happen, nothing else matters.

---

## 2) The MVP outcome: what must be true

This MVP is successful if a new user can install OpenAgents Desktop and, within minutes, complete the irreducible loop:

They click **Go Online** → they receive at least one paid job → their **wallet balance increases** → they **withdraw** by paying a Lightning invoice.

Everything else is in service of making that loop real, repeatable, and not fake.

This implies a hard product truth we accept up front: the main failure mode is not crashes or bugs. The main failure mode is **earning nothing**. The MVP must therefore include not just “provider mode,” but enough demand-path reality (or seeding) that users experience payment early and often.

---

## 3) Who this is for on day one

The first real users fall into two overlapping groups:

1. **Earners** — people who want to sell compute for Bitcoin. They want the simplest possible story: run this app, turn it on, earn sats, withdraw sats. They tolerate some rough edges if the sats are real.

2. **Autopilot users** — people who want a real personal agent that can do practical work on their machine (code, design, organization, integrations), and they’re excited by the idea that the same system can also generate income.

The MVP must satisfy both, but the *North Star emotion* is anchored on the earner experience. Autopilot is “cool” in a thousand ways; “it pays you Bitcoin” is singular.

---

## 4) Desktop-first rationale

Desktop-first isn’t a style choice, it’s the substrate. Autopilot needs:

* Access to local files and local toolchains
* A stable always-on process model when “Go Online” is enabled
* GPU/CPU resources for local execution
* A place to safely hold keys, wallet state, and job receipts
* A fast, game-like UI with immediate feedback

Web-first would make this feel like a dashboard. Desktop-first makes it feel like a machine you own and can upgrade.

---

## 5) The experience we are shipping

### 5.1 First-run experience

When the user opens the app for the first time, they should immediately understand two things without reading a doc:

1. This is **your Autopilot**, a personal agent you can talk to.
2. You can flip **Go Online** and start earning.

The first-run flow must get the user into a “ready to earn” state quickly, but without violating the basic safety requirements of keys and payments. The user needs a wallet identity (Spark), a network identity (for Nostr participation), and an authenticated session for sync/control tokens. The UI should treat this like booting a new character in a game: you’re setting up your “rig,” not filling out a form.

The onboarding copy and structure should be centered around capability, not infrastructure. The user is not “creating a seed phrase,” they are “unlocking custody of your sats.” They are not “configuring relays,” they are “choosing how you connect to the open network.”

### 5.2 Main screen and “game feel”

The primary surface is a single desktop app built in WGPUI that always shows the user the three things that matter:

* **Autopilot**: a chat-first interaction surface where you can ask it to do work, see progress, and see results.
* **Online / Provider**: a big, binary “Go Online” control with a clear state machine and visible outcomes.
* **Wallet**: your sats balance, your recent earnings, and a dead-simple withdraw path.

The UI should feel like a control panel with a scoreboard. When the user is online, the app should show live counters that reinforce the core loop: sats earned today, jobs completed, uptime, last job outcome, and a “heartbeat” indicator. If the app cannot honestly show income, it must show *why* in plain language and what to do next.

The user should never be in a state where they wonder “is this actually online?” or “did I actually get paid?” The product must be allergic to ambiguous success.

---

## 6) The minimum useful product loop (revised)

The MVP loop is intentionally narrow and absolute:

1. The user opens OpenAgents Desktop and signs in.
2. The user sees Autopilot and their wallet balance.
3. The user clicks **Go Online**.
4. The network sends the user at least one job they can execute.
5. The job completes and triggers a Lightning payment into their Spark wallet.
6. The user watches the wallet balance increase.
7. The user withdraws by paying a Lightning invoice (or otherwise demonstrating “I can move this value out”).

Everything in this spec exists because it supports one of those steps.

---

## 7) Scope boundaries: what we are and are not building

This MVP is the smallest real version of “Autopilot prints Bitcoin.” It is not the full OpenAgents economy, and it is not the end state marketplace UI.

We are shipping:

* A desktop Autopilot experience that can do basic work and maintain conversation state.
* A provider mode that can accept and execute NIP-90 compute requests.
* A wallet that can receive earnings, generate invoices, send payments, and show deterministic history.
* A retained sync lane so conversations and job/activity state remain consistent and replay-safe across reconnects and restarts.

We are intentionally *not* shipping:

* Full L402 marketplace productization and paywalls
* Advanced underwriting/policy layers (Hydra/Aegis UX)
* A comprehensive plugin store UI
* Multi-surface parity across web/mobile
* Operator-only consoles or complicated admin tooling

However, the MVP must not paint us into a corner. Even if we don’t ship plugin selling UI now, the system must be shaped so that “sell skills/plugins” is an obvious extension rather than a rewrite.

---

## 8) The economy model for the MVP: how sats actually move

The MVP must encode a simple, defensible payment story that results in real earnings:

* Jobs have a price (in msats/sats) that the provider can understand and the buyer can pay.
* The provider uses the built-in Spark wallet to request or receive payment.
* Wallet updates are reflected in UI as authoritative, not inferred.

Because the killer failure is “user earns nothing,” the MVP must include a practical answer to: “Where do the first jobs come from?”

There are two acceptable paths, and the MVP may implement both:

**Path A: Real open-network demand.** The app can submit NIP-90 jobs as a buyer and other buyers can discover providers naturally. This is the “pure” story, but it risks a cold start.

**Path B: Seed demand (recommended for MVP success).** OpenAgents runs a simple buyer that periodically dispatches small paid jobs to online providers (a faucet / quest system). This is not fake money; it’s a subsidy designed to guarantee the first-run experience. The product should present this honestly as “starter jobs” or “network quests.” The user should not need to understand any of this, but the network must ensure the first “wallet tick up” moment happens reliably.

In both paths, the provider should have an explicit, visible “job lifecycle” in the UI: received → accepted → running → delivered → paid. If payment fails, the UI must say so plainly.

---

## 9) Functional behavior: what the system must do

### 9.1 Autopilot as a personal agent (Codex wrapper on your machine)

Autopilot is not just a chat box. It is an agent that can do work locally, using Codex as the primary engine for “ask → plan → execute → report.”

For MVP purposes, Autopilot must at least support:

* Creating and maintaining chat threads
* Executing at least one meaningful class of local actions (e.g., code-related tasks) via the local-first execution lane
* Emitting structured events for every action so the UI can show a coherent activity feed and so state can be recovered after restart

The key here is not breadth of capability; it is **reliability and legibility**. Even if Autopilot only does a narrow slice at first, it must feel solid and honest.

### 9.2 Go Online provider mode: turning the machine into an earner

“Go Online” is a mode switch with a real lifecycle. When the user flips it on, the app must:

* Initialize the provider runtime (Pylon) and provider identity
* Connect to configured Nostr relays
* Advertise capability in a way that buyers can discover
* Enter a heartbeat loop that makes online presence real (and detectable)
* Surface failures immediately (relay issues, auth issues, wallet issues, execution issues)

When the user flips it off, the provider must stop cleanly. There must be no zombie “online” state.

### 9.3 NIP-90 job handling (buy + sell)

As a provider, the app must be able to:

* Receive NIP-90 requests for at least one supported job class (text generation is the baseline)
* Decide whether it can accept a job based on configuration and current health
* Execute deterministically enough that failures are attributable and recoverable
* Publish results back to the network in the correct response format
* Track job telemetry locally in a way the UI can display: job count, sats earned, failures with reasons

As a buyer, the app should be able to submit a job out to the network from within the Autopilot experience. This enables both real usage (“ask the network for something”) and internal testing (“does the network loop work end-to-end?”).

### 9.4 Spark wallet: the money is the product

The wallet panel is not a “nice to have.” It is the proof that the product works.

The MVP wallet must:

* Load from explicit signer/mnemonic context (no magical hidden keys)
* Show balance and connectivity in a way that is obviously true
* Generate a receive invoice (or equivalent receive primitive)
* Send payments
* Show transaction history that is deterministic and replays correctly after restart
* Never display success unless the underlying wallet operation succeeded

The withdraw experience must be one of the most polished moments in the MVP, because it’s the user’s proof that the sats are real.

### 9.5 Sync: Spacetime-backed continuity that never lies

Chat threads, job history, activity feeds, and key state must be robust against:

* App restart
* Network disconnect
* Cursor staleness
* Partial failures

Spacetime is the retained live sync lane. The desktop app must subscribe, bootstrap, and apply events with replay safety and strict idempotency. This is not a “maybe later” correctness feature; it is how we prevent phantom jobs, double-counted earnings, and confusing state resets that destroy trust.

---

## 10) Authority model and non-negotiable invariants (in plain language)

This MVP is a money-moving, network-participating desktop app. That means the system’s guarantees matter. There are a few invariants we will not violate because they protect determinism, safety, and our ability to evolve without breaking users:

* The retained implementation is Rust-only. We do not ship a split-brain authority system.
* Cross-boundary contracts are proto-first. The desktop app and services talk in typed, versioned contracts.
* Spacetime is the retained live sync transport. Desktop sync must not depend on legacy websocket/Phoenix frames.
* Sync is delivery and replay, not authority. The sync lane cannot silently mutate truth; authority mutations happen through authenticated commands.
* Commands are explicit, authenticated, and receipt-able. If something changes state (especially money state), we know exactly why and we can replay or prove it.

The product framing of this is simple: **the app must never “feel like it paid you” unless it actually did.** The architecture exists to enforce that honesty.

---

## 11) Reliability and failure UX: how we avoid “it feels fake”

The MVP must treat failure as a first-class UX surface. Users will tolerate downtime if the app is transparent; they will not tolerate ambiguity.

Every major subsystem must have an explicit state displayed in the UI:

* Provider: offline / connecting / online / degraded
* Wallet: connected / syncing / degraded / error
* Network: relay connectivity and last heartbeat
* Sync: connected / reconnecting / stale cursor rebootstrap

When something breaks, the app must say:

1. what is broken,
2. what the user can do (if anything),
3. whether earnings are currently possible.

If the user is online but there are no jobs, the UI must not look dead. It should communicate “online and waiting,” show uptime, show network health, and (if seed demand exists) show “starter jobs queued” or “quests available.”

The MVP must also include replay-safe reconnect behavior. If the user disconnects and reconnects, they should not see duplicate jobs, duplicate earnings, or confusing resets. If the cursor is stale, the app must rebootstrap deterministically and visibly.

---

## 12) Observability: what we measure, what we show

The top product metric is **daily sats earned**. That is the scoreboard. It is the single number that determines whether this product is alive.

The UI must surface:

* sats earned today
* lifetime sats earned
* jobs completed today
* last job result (success/failure and reason)
* online uptime
* wallet balance and last payment received

Internally, logs and telemetry must make the end-to-end job/payment loop debuggable. When something fails, we should be able to answer “did we receive the job?”, “did we execute?”, “did we publish result?”, “did we issue invoice?”, “did we get paid?”, “did wallet reflect payment?” without guesswork.

---

## 13) Acceptance: what “ship-ready” means

This MVP is ship-ready when it can reliably demonstrate the money-printing loop to a fresh machine:

* The desktop app launches, signs in, and shows Autopilot + wallet state.
* The user can toggle Go Online and see an unambiguous online state.
* At least one NIP-90 job lifecycle succeeds end-to-end while online, resulting in real sats received.
* The wallet balance increases and the user can withdraw by paying a Lightning invoice.
* Chat and activity state remain coherent through disconnect/reconnect and app restart, with replay-safe behavior and no duplicates.
* Desktop sync path rejects legacy websocket/Phoenix frames and uses retained Spacetime forms only.

If any one of these fails in a first-run flow, we treat it as a product failure, not a “later improvement.”

---

## 14) MVP pane + command surface (required)

For MVP, every user-facing feature must be reachable by one command-palette command that opens exactly one pane. Pane names and command labels below are canonical user-facing strings.

| Pane name (user sees) | Command label (user sees) | Command ID | MVP feature covered |
| --- | --- | --- | --- |
| Autopilot Chat | Autopilot Chat | `pane.autopilot_chat` | Personal agent chat thread + local execution UX |
| Go Online | Go Online | `pane.go_online` | Provider mode toggle and lifecycle state |
| Provider Status | Provider Status | `pane.provider_status` | Uptime, heartbeat, degraded/error visibility |
| Job Inbox | Job Inbox | `pane.job_inbox` | Incoming NIP-90 request intake |
| Active Job | Active Job | `pane.active_job` | In-flight job lifecycle (`received -> running -> delivered -> paid`) |
| Job History | Job History | `pane.job_history` | Deterministic job history and failure reasons |
| Earnings Scoreboard | Earnings Scoreboard | `pane.earnings_scoreboard` | sats/day, lifetime sats, jobs/day, last result |
| Spark Lightning Wallet | Spark Wallet | `pane.wallet` | Balance, connectivity, addresses, payment history |
| Pay Lightning Invoice | Pay Lightning Invoice | `pane.pay_invoice` | Withdraw/prove custody by paying invoices |
| Create Lightning Invoice | Create Lightning Invoice | `pane.create_invoice` | Receive/invoice generation flow |
| Nostr Keys (NIP-06) | Identity Keys | `pane.identity_keys` | Identity generation, reveal/copy, key custody |
| Relay Connections | Relay Connections | `pane.relay_connections` | Relay connectivity and failure diagnosis |
| Sync Health | Sync Health | `pane.sync_health` | Spacetime subscription, reconnect, stale cursor state |
| Network Requests | Network Requests | `pane.network_requests` | Buy-side request submission to network |
| Starter Jobs | Starter Jobs | `pane.starter_jobs` | Seed-demand/quest visibility for first earnings |
| Activity Feed | Activity Feed | `pane.activity_feed` | Unified event stream for chat/jobs/wallet actions |
| Alerts and Recovery | Alerts and Recovery | `pane.alerts_recovery` | Actionable incident/failure guidance |
| Settings | Settings | `pane.settings` | App config, network config, safety toggles |

Current implementation note:

* Already present in app: `Nostr Keys (NIP-06)`, `Spark Lightning Wallet`, `Pay Lightning Invoice`.
* Remaining panes above are MVP backlog and should be implemented with the exact pane/command labels listed.

---

## 15) Implementation mapping (retained structure, aligned to the new story)

The implementation remains grounded in the same lanes as the original draft, but the emphasis shifts: we are building a “money printer” experience, not a pile of subsystems.

The primary app surface is `apps/autopilot-desktop` (WGPUI). It owns the moment-to-moment experience: Go Online, job lifecycle visibility, Autopilot chat, wallet tick up.

`apps/openagents.com` remains retained for desktop-facing auth/session flows and sync token issuance (`POST /api/sync/token`) and any minimal control endpoints the desktop needs.

`apps/runtime` remains the retained authority for execution boundaries and any projection publishing necessary for sync.

Core crates remain as previously enumerated (wgpui, autopilot_ui/app/core, client-core, codex control, spacetime client, pylon provider runtime, spark wallet integration, nostr client/core, proto/protocol). The key requirement is that these crates expose outcomes in a way that the UI can render as legible state transitions, not just logs.

---

## 16) Post-MVP (kept explicit, but not allowed to pollute MVP)

After the MVP loop is stable and users can reliably earn and withdraw, the next expansions are obvious:

* richer provider capability (more job types, better scheduling, GPU specialization)
* plugin/skill selling as a first-class “upgrade your rig” mechanic
* richer buy-side flows (spend sats to delegate work to the network)
* policy/underwriting layers and marketplace productization (Hydra/Aegis)
* broader dashboards and admin surfaces

But none of those matter until the wallet ticks up for real users, every day.

---

## Final product mantra (for every implementation decision)

If a proposed feature does not make it easier for a user to:

1. go online,
2. earn sats,
3. trust the earnings are real,
4. withdraw instantly,

…it is not MVP.

OpenAgents Desktop turns your machine into a money printer.

It prints Bitcoin.
