# Autopilot MVP Spec

## 1) Product promise

**Autopilot** is your personal agent that lives on your computer, plugs into your real environment (files, GPU, local tools), and can both **work for you** and **pay you**.

The “why” is simple and visceral: **Autopilot turns your machine into a money printer — it prints Bitcoin.** You run the app, you talk to Autopilot to get work done, and when you’re ready you flip **Go Online** and Autopilot becomes a provider on an open network. Other agents can send you work. You execute it locally (on your CPU/GPU and integrations), you get paid in sats into your built-in wallet, and you can withdraw that value instantly over Lightning.

This is not “another chat app.” It is a **two-sided marketplace** collapsed into one product surface:

* **Buy side:** you use Autopilot as a personal agent (a high-leverage wrapper around Codex + your computer) and can submit work requests out to the network when it makes sense.
* **Sell side:** you provide compute now (and later additional provider lanes such as liquidity solving and plugins/skills) to the network and **earn Bitcoin**.

Autopilot Earn is a provider marketplace with multiple potential revenue lanes. The MVP's primary paid loop remains the compute-provider lane; liquidity-solver mode is future scope.

The retained repo now also ships a secondary Data Market MVP slice, but it does not replace the compute-first product promise. Today that slice is:

* a dedicated `Data Seller` conversational pane
* a read-only `Data Market` pane
* a narrow `Data Buyer` targeted-request pane
* full `autopilotctl data-market ...` and headless runtime control
* a targeted NIP-90 data-vending flow layered on top of kernel authority objects

That is a real implementation path for selling permissioned access to packaged local data, but it is not yet the primary onboarding or monetization loop described in this MVP spec.

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

### 5.0 Acquisition and install

The marketing site and install path must do three things almost instantly:

1. Communicate a real problem Autopilot solves right now.
2. Make the product feel credible and safe to try.
3. Make install feel almost frictionless.

The default story is:

- self-hosted agents are too complex for most people to wire up reliably,
- Autopilot makes that simple,
- and once it is working for you, it can also earn sats.

The page should drive toward one obvious install action. Minimize typing, minimize branching, and pair the install CTA with trust signals that make the product feel real.

### 5.1 First-run experience

When the user opens the app for the first time, they should immediately understand three things without reading a doc:

1. This is **your Autopilot**, a personal agent you control on your machine.
2. It is straightforward to try.
3. You can flip **Go Online** and start earning.

The first-run flow must get the user into a “ready to earn” state quickly, but without violating the basic safety requirements of keys and payments. The user needs a wallet identity (Spark), a network identity (for Nostr participation), and an authenticated session for sync/control tokens. The UI should treat this like booting a new character in a game: you’re setting up your “rig,” not filling out a form.

The onboarding copy and structure should be centered around capability, not infrastructure. The user is not “creating a seed phrase,” they are “unlocking custody of your sats.” They are not “configuring relays,” they are “choosing how you connect to the open network.”

By default, the app should come preconfigured to use the OpenAgents-hosted Nexus as its primary relay and authority endpoint, with a curated default public relay set available as additional transport. Users and organizations must still be able to replace this with their own Nexus deployment and their own relay set. A self-hosted Nexus should be public/open by default. Closed/private Nexus modes can exist later, but they are not near-term MVP scope. The OpenAgents-hosted Nexus should remain anon/open for general marketplace participation, while specific buyers can still target preferred or required participants. The starter-job bootstrap is initially only guaranteed on the OpenAgents-hosted Nexus. That does not mean ordinary marketplace intake is Nexus-only: Autopilot should listen for NIP-90 demand across the full configured reachable relay set and deduplicate what it sees. The initial default public relay set should be chosen pragmatically from relays where we observe meaningful recent NIP-90 job activity.

The first “ah ha” moment should happen within 30-60 seconds. For the MVP, it is appropriate for that moment to be earning-shaped rather than task-shaped: the user goes online, receives a real paid job, and sees sats land in the wallet. Task success can reinforce the product later, but it is not required before the earn loop.

Even before the user goes online, the app should not feel empty. It should show live observed market activity from Nexus and the configured relay set so the user immediately sees that real jobs exist. Those rows are preview-only while offline: they create demand visibility and trust, but the provider does not become eligible to accept work until the user explicitly clicks `Go Online`.

### 5.2 Main screen and “game feel”

The primary surface is a single hardware-accelerated desktop app with a game-like HUD feel that always shows the user the three things that matter:

* **Autopilot**: a chat-first interaction surface where you can ask it to do work, see progress, and see results.
* **Online / Provider**: a big, binary “Go Online” control with a clear state machine and visible outcomes.
* **Wallet**: your sats balance, your recent earnings, and a dead-simple withdraw path.

The UI should feel like a control panel with a scoreboard. When the user is online, the app should show live counters that reinforce the core loop: sats earned today, jobs completed, uptime, last job outcome, and a “heartbeat” indicator. If the app cannot honestly show income, it must show *why* in plain language and what to do next.

The user should never be in a state where they wonder “is this actually online?” or “did I actually get paid?” The product must be allergic to ambiguous success.

The first sats should feel ceremonial. Early milestones like `10`, `25`, `50`, and `100` sats should be celebrated in the UI so the user can feel the progression from “it works” to “this thing is paying me.”

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

In multi-lane terms: this MVP ships the **compute-provider lane only**.

We are shipping:

* A desktop Autopilot experience that can do basic work and maintain conversation state.
* A provider mode that can accept and execute NIP-90 compute requests.
* A wallet that can receive earnings, generate invoices, send payments, and show deterministic history.
* A retained sync lane so conversations and job/activity state remain consistent and replay-safe across reconnects and restarts.
* A starter Data Market slice that can register assets, publish grants, accept targeted NIP-90 access requests, issue delivery bundles, revoke access, and expose the same flow through panes, `autopilotctl`, and the no-window headless runtime.

We are intentionally *not* shipping:

* Full L402 marketplace productization and paywalls
* Advanced underwriting/policy layers (Hydra/Aegis UX)
* Liquidity solver mode and Hydra intent-routing UX (future lane, explicit opt-in only)
* A comprehensive plugin store UI
* Broad public Data Market discovery, catalog search, or rich buyer procurement UX beyond the current narrow targeted-request flow
* Multi-surface parity across web/mobile
* Operator-only consoles or complicated admin tooling

However, the MVP must not paint us into a corner. Even if we don’t ship plugin selling UI now, the system must be shaped so that “sell skills/plugins” is an obvious extension rather than a rewrite.

---

## 8) The economy model for the MVP: how sats actually move

The MVP must encode a simple, defensible payment story that results in real earnings:

* Jobs have a price (in msats/sats) that the provider can understand and the buyer can pay.
* The provider uses the built-in Spark wallet to request or receive payment.
* All provider earnings settle into the built-in Spark wallet first. MVP does not support configuring an external receive invoice for provider payouts.
* Wallet updates are reflected in UI as authoritative, not inferred.

Because the killer failure is “user earns nothing,” the MVP must include a practical answer to: “Where do the first jobs come from?”

There are two acceptable paths, and the MVP may implement both:

**Path A: Real open-network demand.** The app can submit NIP-90 jobs as a buyer and other buyers can discover providers naturally. This is the “pure” story, but it risks a cold start.

**Path B: Seed demand (recommended for MVP success).** OpenAgents runs a simple buyer that periodically dispatches small paid jobs to online providers (a faucet / quest system). This is not fake money; it’s a subsidy designed to guarantee the first-run experience. The product should present this honestly as “starter jobs” or “network quests,” but they should still show up inside the normal job flow rather than in a fully separate lane. In the main earn surfaces they should look like ordinary jobs with a visible source marker such as a tag, badge, or star. The user should not need to understand any of this, but the network must ensure the first “wallet tick up” moment happens reliably. Initially, this starter economy is a feature of the OpenAgents-hosted Nexus, not a guaranteed capability of every self-hosted Nexus, and OpenAgents starter jobs target Autopilot users rather than every anonymous participant on the relay. For MVP, those jobs are available only to providers connected to the OpenAgents-hosted Nexus itself, not to providers arriving through a third-party Nexus.

Eligibility for those starter jobs should be enforced from Nexus-side proof, not from a user-supplied `client` tag alone. The practical near-term rule is: if the OpenAgents-hosted Nexus can prove the provider is connected through an authenticated Autopilot session with bound Nostr identity, that is sufficient for MVP. Protocol-visible client tags may still be emitted for observability, but they are optional and should not be the sole trust basis. Stronger anti-spoofing mechanisms such as richer attestation or device-bound proofs are later hardening work, not MVP gates.

In both paths, the provider should have an explicit, visible “job lifecycle” in the UI: received → accepted → running → delivered → paid. If payment fails, the UI must say so plainly.

This is lane A of the broader Earn model:

* **Lane A (MVP):** compute provider via NIP-90 jobs.
* **Lane B (future):** liquidity solver via Hydra intents in an OpenAgents-native solver market, where providers commit capital + execution and earn routing fees/spreads.

Lane B is deliberately out of MVP scope and must never auto-activate for users.

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

* Initialize the embedded Autopilot provider runtime and provider identity
* Connect to the configured Nostr relay set, using Nexus as the default primary relay and a curated default public relay set as additional transport
* Listen for NIP-90 requests across the full configured reachable relay set, not just Nexus-originated demand
* Publish capability/presence across every healthy configured relay by default so buyers on those relays can discover the provider
* Enter a heartbeat loop that makes online presence real (and detectable)
* Surface failures immediately (relay issues, auth issues, wallet issues, execution issues)

For MVP, this online mode advertises compute-provider capability only. Future provider modules (for example liquidity solving) must remain separately gated and explicit. Online mode should not auto-restore on app launch in MVP; even if the user was online previously, each new app session should require a fresh explicit click on `Go Online` before the provider starts accepting work again.

When the user flips it off, the provider must stop cleanly. There must be no zombie “online” state.

### 9.3 NIP-90 job handling (buy + sell)

As a provider, the app must be able to:

* Receive NIP-90 requests for at least one supported launch compute family. The Compute Market launch families are inference and embeddings; the retained MVP implementation is still text-generation-led today, with embeddings as the next standardized compute family to productize. Later Apple adapter-hosting and Apple training operator surfaces may exist in retained operator flows, but they do not replace the inference-led MVP earn loop.
* Surface observed market activity even while offline so the user can browse demand before opting into provider mode
* Auto-accept matching jobs by default based on configuration, capacity, and health; manual per-job approval is not part of the primary earn loop
* Execute deterministically enough that failures are attributable and recoverable
* Publish feedback/results back to the network in the correct response format, fanning out to every healthy configured relay by default
* Track job telemetry locally in a way the UI can display: job count, sats earned, failures with reasons

Relay fanout should be treated as best-effort transport, not as a global blocking quorum. In MVP, the provider should try to publish job feedback/results broadly across healthy configured relays, but one slow or failing relay must not stall execution, settlement, or truthful completion state. Relay publish failures should surface in diagnostics and history rather than silently disappear.

Offline preview must be clearly distinguished from active provider intake. While offline, the app may show recently observed or currently reachable job demand in the same job surfaces, but those rows should be read-only and visibly marked as preview/unclaimable until the user turns provider mode on.

Open-market contention is part of the protocol. If a public NIP-90 request is visible on public relays, Autopilot cannot guarantee it is the only provider that starts work on that request. MVP should not pretend there is a global cross-relay lock. Instead, the provider should keep waste bounded with strict local admission controls: low `max_inflight`, ttl freshness checks, minimum reward thresholds, per-buyer limits, and cheap preflight validation before expensive execution. For MVP, `max_inflight` means the number of active jobs the provider may work at once, and the default should be `1` until the desktop runtime and Codex execution lane are proven to handle safe concurrent job execution.

OpenAgents starter jobs should avoid that waste entirely. The OpenAgents-hosted Nexus should assign each starter job to one eligible provider at a time using a short-lived assignment lease derived from provider liveness and current load. The desktop auto-accepts the starter job only when it holds the active lease. That lease should be aggressive about confirming the provider actually started, on the order of roughly `10-15s` to emit a start/progress signal, while still allowing a separate, more forgiving execution window once work is underway. If the provider drops offline, fails to acknowledge/start within that short confirm window, or later stops heartbeating through execution, Nexus may reassign the job. That assignment authority should live in Nexus backend services. Spacetime may later mirror live lease state for projection or coordination, but it is not required as the authoritative gate for MVP.

Buyer resolution mode should also be explicit. For MVP, OpenAgents-posted public jobs should default to a `race` model for tiny deterministic commodity work: the first valid result wins and later duplicate results are unpaid. That is acceptable for small, fast, easy-to-verify jobs because it keeps the marketplace simple. It should not be treated as the only long-term mode. A later `windowed` mode should allow a buyer to declare a submission window (for example `5 minutes`), collect valid results until the deadline, and then evaluate them after the window closes using an explicit scoring or acceptance policy. That later mode is the right answer when "fastest wins" would create bad incentives or degrade quality.

For `race` jobs, silent no-pay is not the target behavior. When the OpenAgents buyer can correlate a slower or late duplicate result, it should send explicit terminal feedback indicating that the result lost the race and is unpaid. In the current NIP-90 model that can be represented with a terminal `kind 7000` feedback event using a standard status plus explanatory `status_extra` reason text such as `lost-race` or `late-result-unpaid`. If correlation is incomplete, we should still record the unpaid reason in operator/audit telemetry rather than leave the outcome unexplained.

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

Provider earnings should always appear here first. If the user wants funds in another wallet, the supported path is to withdraw from Spark by paying a Lightning invoice.

Withdrawal should remain available while the provider is online. MVP should not require the user to go offline before paying out from the built-in Spark wallet.

The withdraw experience must be one of the most polished moments in the MVP, because it’s the user’s proof that the sats are real.

### 9.5 Sync: Spacetime-backed continuity that never lies

Chat threads, job history, activity feeds, and key state must be robust against:

* App restart
* Network disconnect
* Cursor staleness
* Partial failures

Spacetime is the retained live sync lane. Rollout semantics are phase-scoped and must stay explicit:

* **Current (Phase 1, mirror/proxy semantics):**
  * Desktop enforces canonical sync bootstrap/token contract (`POST /api/sync/token`, retained subscribe target form).
  * Replay-safe local apply/checkpoint discipline is active for deterministic restart/reconnect behavior.
  * Presence/projection panes use Spacetime-shaped local state (`spacetime.presence`, projection stream ids) while preserving authority boundaries.
* **Target (Phase 2, live remote semantics):**
  * Presence/checkpoints/projections run against live Spacetime subscriptions/reducers for ADR-approved domains.
  * Release parity/chaos gates and handshake smoke checks are required before promotion.

Canonical rollout/runbook index:

* `docs/SPACETIME_ROLLOUT_INDEX.md`

---

## 10) Authority model and non-negotiable invariants (in plain language)

This MVP is a money-moving, network-participating desktop app. That means the system’s guarantees matter. There are a few invariants we will not violate because they protect determinism, safety, and our ability to evolve without breaking users:

Terminology in this spec:

- `OpenAgents Runtime` is the execution environment on the user/provider node where jobs run, local state advances, and provenance is produced.
- `OpenAgents Kernel` is the authority layer that verifies outcomes, settles value, and emits canonical receipts.
- For this MVP, the desktop embeds the runtime in `apps/autopilot-desktop`. A thin backend kernel authority slice already exists in `apps/nexus-control` and `openagents-kernel-core`, while richer market coverage and broader package surfaces remain in flight.

* The retained implementation is Rust-only. We do not ship a split-brain authority system.
* Cross-boundary contracts are proto-first. The desktop app and services talk in typed, versioned contracts.
* Spacetime is the retained live sync transport. Desktop sync must not depend on legacy websocket/Phoenix frames.
* Sync/replay remains non-authoritative by default, with one explicit exception class: ADR-approved app-db domains (presence/checkpoints/projections) may be Spacetime-authoritative.
* Commands are explicit, authenticated, and receipt-able. If something changes state (especially money state), we know exactly why and we can replay or prove it.

The product framing of this is simple: **the app must never “feel like it paid you” unless it actually did.** The architecture exists to enforce that honesty.

Domain-scoped authority matrix:

| Domain | Authority Owner | Spacetime reducer authority |
| --- | --- | --- |
| Money/settlement/wallet truth | authenticated command lanes | no |
| Trust/policy/security verdicts | authenticated command lanes | no |
| Provider/device online presence | Spacetime presence reducers | yes |
| Replay checkpoints/cursor continuity | Spacetime checkpoint reducers | yes |
| Non-monetary projections/counters | Spacetime projection reducers/queries | yes |

Canonical decision record: `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`

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

If the user is offline, the UI should still show current or recently observed market activity so first launch immediately feels alive. The key distinction is not “visible vs invisible,” it is “previewing demand” vs “available to accept work.”

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
* Desktop sync bootstrap rejects legacy token endpoint forms and enforces retained Spacetime token/claim contracts.
* Phase-appropriate sync gates are green per `docs/SPACETIME_ROLLOUT_INDEX.md`.

If any one of these fails in a first-run flow, we treat it as a product failure, not a “later improvement.”

---

## 14) MVP pane + command surface (required)

For MVP, every user-facing feature must be reachable by one command-palette command that opens exactly one pane. Pane names and command labels below are canonical user-facing strings.

| Pane name (user sees) | Command label (user sees) | Command ID | MVP feature covered |
| --- | --- | --- | --- |
| Autopilot Chat | Autopilot Chat | `pane.autopilot_chat` | Personal agent chat thread + local execution UX |
| Go Online | Go Online | `pane.go_online` | Provider mode toggle and lifecycle state |
| Provider Status | Provider Status | `pane.provider_status` | Uptime, heartbeat, degraded/error visibility |
| Tailnet Status | Tailnet Status | `pane.tailnet_status` | Live Tailnet device roster and link diagnostics |
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

* The pane inventory above is retained as canonical surface naming for MVP.
* Truth-source semantics for current rollout phase are tracked in `docs/PANES.md` and `docs/SPACETIME_ROLLOUT_INDEX.md`.

---

## 15) Implementation mapping (retained structure, aligned to the new story)

The implementation remains grounded in the same lanes as the original draft, but the emphasis shifts: we are building a “money printer” experience, not a pile of subsystems.

The primary app surface is `apps/autopilot-desktop`. It owns the moment-to-moment experience: Go Online, job lifecycle visibility, Autopilot chat, wallet tick up.

The default OpenAgents-hosted server-authority stack is the Nexus role: an open-source, opinionated, self-hostable surface for desktop-facing auth/session flows, sync token issuance (`POST /api/sync/token`), public stats, and the primary Nostr relay/index path the desktop uses by default. Autopilot should connect to the OpenAgents Nexus by default while still allowing users and organizations to point at their own Nexus deployment, including using that Nexus as the primary relay with other relays as backup. A self-hosted Nexus is expected to operate as a public/open relay by default. Private/team-scoped Nexus modes belong on the roadmap, not in the near-term MVP. The starter-job path, however, is initially an OpenAgents-hosted service rather than part of the minimum Nexus contract. The current in-repo backend authority slice is hosted by `apps/nexus-control`, which exposes the retained mutation and projection entry points the desktop can call.

`apps/autopilot-desktop` currently contains the embedded OpenAgents Runtime for MVP execution boundaries, provider lifecycle, and any sync-facing projection publishing the desktop needs.

Core crates remain as previously enumerated (wgpui, autopilot_ui/app/core, client-core, codex control, spacetime client, provider runtime, spark wallet integration, nostr client/core, proto/protocol). The key requirement is that these crates expose outcomes in a way that the UI can render as legible state transitions, not just logs.

---

## 16) Post-MVP (kept explicit, but not allowed to pollute MVP)

After the MVP loop is stable and users can reliably earn and withdraw, the next expansions are obvious:

* richer provider capability (more job types, better scheduling, GPU specialization)
* liquidity solver lane (Hydra intent fills, route policy controls, explicit capital opt-in)
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
