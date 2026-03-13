# 2026-03-13 Autopilot Desktop Perf Remediation Audit

## Purpose

This audit is the retrospective companion to
`docs/audits/2026-03-13-autopilot-desktop-perf-harness-audit.md`.

It summarizes the performance work that landed in the recent commit window
before the current clippy cleanup, explains what those commits actually
changed, and recommends the next round of speed / responsiveness work.

The intent is to separate:

- diagnostic work from true fixes,
- startup / background hot-path fixes from pane-rendering fixes,
- and shipped improvements from still-open performance risks.

## Scope

This audit covers the retained MVP desktop app in `apps/autopilot-desktop`
and the app-owned control/harness surface around it.

It does not try to relitigate older repo history. It covers the recent
performance-related commit window that responded directly to observed lag,
beachballs, pane churn stalls, and misleading redraw diagnostics.

## Commit Window Reviewed

Performance-relevant commits reviewed here:

- `e11d702b7` `Wire desktop redraw cadence for Rive surfaces`
- `570ab1af5` `Add frame debugger pane for desktop render diagnostics`
- `0ef40ea82` `Stabilize frame debugger redraw cadence probe`
- `f30f3fa3e` `Stop routine Spark wallet refreshes from rebuilding wallet`
- `982f3bb81` `Reduce desktop startup stall hot paths`
- `53faac3f3` `Throttle redundant Spark wallet refreshes`
- `2e2c7c789` `Audit and reduce desktop perf hot paths`

Non-performance UX commits in the same time window are intentionally omitted.

## Starting Symptoms

These commits were not speculative cleanup. They were a response to concrete
user-visible problems:

- startup produced repeated beachballs with repeated Spark wallet builds/syncs,
- the first `Frame Debugger` reported very low redraw FPS until the pane was
  dragged, which made the tool look untrustworthy,
- repeated `Provider Control` open/close cycles could hang the whole app for
  roughly a second,
- logs were often unhelpful because the dominant failures were event-loop /
  main-thread stalls rather than explicit runtime errors.

The earlier harness audit identified the main culprits:

- repeated Spark refresh / reload churn,
- desktop-control snapshot work on the main thread,
- missing pane lifecycle coverage in the supported harness,
- NIP-90 session-log backfill reparsing on the hot path.

## What Landed

### 1. Rive redraw cadence became explicit instead of accidental

Commit:

- `e11d702b7` `Wire desktop redraw cadence for Rive surfaces`

Key result:

- the desktop loop now treats active Rive surfaces as explicit redraw drivers
  instead of relying on incidental input activity.

Why it mattered:

- once `Rive Preview` and `Presentation` existed, redraw cadence stopped being
  an abstract engine concern and became a user-visible responsiveness issue,
- this commit established the first app-owned redraw policy path for animated
  surfaces.

Limit:

- this only made the redraw policy more deterministic; it did not yet explain
  why the app felt slow when the loop was not scheduling frequent polls.

### 2. A first-class frame debugger pane was added

Commit:

- `570ab1af5` `Add frame debugger pane for desktop render diagnostics`

Key files:

- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/panes/frame_debugger.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`

What shipped:

- a singleton `Frame Debugger` pane,
- rolling redraw FPS / interval reporting,
- per-frame CPU phase timing,
- draw-call / layer pressure reporting,
- current redraw-driver state,
- pane snapshot exposure through the tool/control bridge.

What this fixed:

- before this, the app had no truthful in-product view of redraw cadence or
  background redraw pressure,
- after this, there was at least one operator surface that could show whether
  the desktop loop was idling, thrashing, or being continuously driven.

What it did not fix:

- it measured redraw cadence, not GPU present latency,
- it still had observer effect problems because opening the pane did not itself
  guarantee an appropriate debug sampling cadence.

### 3. The frame debugger probe itself was corrected

Commit:

- `0ef40ea82` `Stabilize frame debugger redraw cadence probe`

What changed:

- the desktop loop now treats the frame debugger as its own high-cadence probe
  while the pane is open,
- the UI labels were corrected to describe redraw cadence rather than implying
  a raw renderer ceiling.

Why it mattered:

- the early debugger could show about `~9 fps` until the user dragged a pane,
  then jump to `~100 fps`,
- that was not primarily renderer performance; it was the idle scheduler
  falling back to a slower poll interval,
- this commit made the tool honest enough to separate scheduler behavior from
  actual rendering throughput.

Tradeoff:

- the pane now intentionally influences redraw cadence while open,
- so it is useful for diagnosis, but it is not yet a zero-observer-effect
  benchmark surface.

### 4. Routine Spark wallet refreshes stopped rebuilding the wallet

Commit:

- `f30f3fa3e` `Stop routine Spark wallet refreshes from rebuilding wallet`

Key files:

- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/hotbar.rs`

What changed:

- routine status probes switched from `SparkWalletCommand::Reload` to
  `SparkWalletCommand::Refresh`,
- full `Reload` was left for true rebuild cases such as identity / environment
  changes.

Why it mattered:

- `Reload` destroys and recreates the Spark wallet object,
- the observed logs showed repeated `Building SparkWallet ...` bursts during
  normal startup / status activity,
- this was expensive I/O disguised as harmless status refresh.

Net effect:

- the desktop app stopped doing unnecessary wallet reconstruction during
  ordinary UI / control-plane refreshes.

### 5. Startup stall hot paths were reduced in three different places

Commit:

- `982f3bb81` `Reduce desktop startup stall hot paths`

Key files:

- `apps/autopilot-desktop/src/state/nip90_payment_facts.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/panes/earnings_jobs.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/app_state.rs`

This commit mattered because it addressed three separate stall classes, not
just one.

#### 5.1 NIP-90 session-log backfill stopped running as an eager hot-path task

What changed:

- session-log backfill was moved behind a delayed background tick instead of
  being effectively tied to redraw-time state sync,
- background backfill now waits `8s` after startup before first refresh,
- refresh cadence is bounded to every `10s`,
- logs newer than `45s` are treated as hot and skipped during background
  import,
- files larger than `4 MiB` are skipped,
- total imported session-log bytes are capped at `16 MiB` per refresh.

Why it mattered:

- the sampled stall showed `serde_json::from_str(...)` dominating the main
  thread while reparsing large JSONL logs,
- the live session log had become a huge retained historical artifact,
- the product model already treated logs as backfill rather than the primary
  truth source, but the runtime behavior had not caught up.

This was one of the most important correctness-of-priority fixes in the whole
perf sequence.

#### 5.2 Startup wallet convergence became less stubborn

What changed:

- startup convergence can now clear early once the wallet is connected enough
  to be useful and the balance is known,
- it no longer insists on extra follow-up refreshes purely to settle cosmetic
  status detail.

Why it mattered:

- the previous startup policy could queue up to three follow-up refreshes at
  `2s` intervals,
- that created real Spark I/O during the same window where the app was also
  bringing up UI, chat lanes, payment facts, and provider state.

#### 5.3 Inactive pane cost was reduced

What changed:

- `Earnings & Jobs` now has a cheap preview path when inactive instead of
  always painting the full, text-heavy dashboard.

Why it mattered:

- post-fix sampling still showed ordinary text shaping as a remaining cost when
  pane focus changed,
- this commit reduced the amount of work paid just because a heavy pane existed
  in the shell.

### 6. Redundant Spark refreshes were throttled

Commit:

- `53faac3f3` `Throttle redundant Spark wallet refreshes`

Key file:

- `apps/autopilot-desktop/src/spark_wallet.rs`

What changed:

- healthy duplicate `Refresh` requests are throttled for `3s`,
- startup convergence retries still bypass the throttle,
- pending-payment confirmation flows still bypass the throttle,
- the worker continues to coalesce refresh-like bursts.

Why it mattered:

- repeated `Provider Control` open/close activity could still indirectly cause
  near-back-to-back Spark syncs even after the `Reload` vs `Refresh` fix,
- this commit turned “refresh spam” into bounded refresh behavior.

Net effect:

- pane churn stopped turning into repeated one-second wallet sync loops.

### 7. The supported harness became capable of reproducing pane churn

Commit:

- `2e2c7c789` `Audit and reduce desktop perf hot paths`

Key files:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `docs/headless-compute.md`

What shipped:

- pane lifecycle actions were surfaced through desktop control,
- `autopilotctl` gained pane list / open / focus / close coverage,
- pane churn could now be replayed through the supported control plane instead
  of manual clicking.

Why it mattered:

- the earlier perf problem was specifically “open/close `Provider Control`
  repeatedly and the app sometimes hangs,”
- without pane actions in the harness, that scenario could not be measured in a
  supported, scriptable way,
- after this change, pane lifecycle performance became reproducible.

This is the highest-value measurement change in the whole sequence because it
turned a hand-reported stall into a replayable workload.

### 8. Desktop-control snapshot work was cut down

Commit:

- `2e2c7c789` `Audit and reduce desktop perf hot paths`

Key files:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/autopilot_peer_roster.rs`

What changed:

- background snapshot sync now gates work before building a full snapshot,
- buy-mode target selection and peer-roster construction were de-duplicated,
- active-message history handling stopped building larger vectors only to trim
  them down immediately,
- snapshot signature work was reduced on the hottest idle path.

Why it mattered:

- the earlier OS sample showed desktop-control snapshot work consuming main
  thread time inside `about_to_wait`,
- that meant the measurement/control plane was itself part of the responsiveness
  problem.

Net effect:

- the control plane became much cheaper and therefore much safer to leave on
  while profiling normal app behavior.

## Measured Outcome Of The Shipped Fixes

The earlier harness audit captured the post-fix rerun. The important numbers
were:

- startup Spark activity fell to one `Building SparkWallet` and one full
  `sync_wallet_internal` during initial launch,
- desktop-control snapshot latency dropped to about:
  - `1.31 ms` average
  - `12.44 ms` max in a 12-sample burst
  - roughly `20.4 KB` payload size
- `Provider Control` replay through desktop control reached:
  - `80` close/open cycles
  - `0` errors
  - open average `7.99 ms`, max `10.05 ms`
  - close average `8.82 ms`, max `46.55 ms`

The most important qualitative change was not one number. It was this:

- the main thread was no longer dominated by NIP-90 log-backfill reparsing
  during pane churn,
- and repeated wallet rebuild/sync bursts were no longer the default startup
  behavior.

## What The Shipped Work Actually Solved

The recent perf work solved four real problems:

1. It turned redraw cadence into something observable.
2. It stopped routine wallet status updates from doing heavyweight rebuild work.
3. It demoted historical JSONL backfill from a hot-path behavior to a bounded
   background behavior.
4. It made pane lifecycle stalls reproducible through the supported harness.

That is meaningful progress. The app is better instrumented and less likely to
freeze from obviously avoidable background work.

## What It Did Not Solve

The shipped work was necessary, but it was not a full responsiveness program.
Several important risks remain.

### 1. The app still relies heavily on main-thread composition work

Even after the backfill and wallet fixes, text shaping and pane painting remain
visible costs during pane transitions.

Likely remaining hotspots:

- pane-local text layout,
- scene assembly for inactive-but-mounted panes,
- repeated layout work for dashboards with many labeled rows.

This is not primarily a Spark problem anymore. It is a UI composition problem.

### 2. The frame debugger still has observer effect

The debugger is useful, but opening it changes redraw cadence so the pane can
observe frames accurately.

That means:

- it is a diagnostic pane,
- but it is not yet a neutral benchmark instrument.

The app still needs an always-on background frame ring buffer that can be read
without opening a live high-cadence probe pane.

### 3. Desktop-control snapshots are cheaper, not free

The snapshot/control path is much better than it was, but it still lives in
the app process and still performs app-owned state assembly work.

The current design is reduced hot-path work, not true push-driven incremental
snapshot caching.

### 4. Startup still fans out several systems at once

The worst redundant wallet behavior was fixed, but startup still brings up
multiple concerns in a tight window:

- chat lane,
- Spark wallet,
- payment-fact ledger,
- provider/runtime surfaces,
- animated/rendered shell surfaces.

The app is still more eager than it needs to be.

### 5. There is still no perf regression gate for desktop UX

The harness exists and is more capable, but there is not yet a stable pass/fail
budget for:

- startup responsiveness,
- pane open/close latency,
- fullscreen transitions,
- resize/drag responsiveness,
- idle main-thread budget.

Without a gate, regressions will recur.

## Recommendations Next

### Priority 1: Add a no-observer-effect frame telemetry ring buffer

Owner:

- `apps/autopilot-desktop`

Do next:

- capture rolling frame samples continuously in app state,
- record event-loop cadence, render CPU time, and redraw reasons even when the
  `Frame Debugger` pane is closed,
- let `autopilotctl` fetch that ring buffer directly.

Why:

- the current debugger is good for live diagnosis,
- but the next step is a measurement source that does not perturb scheduling.

### Priority 2: Turn `autopilotctl` into a real perf suite

Owner:

- `apps/autopilot-desktop`

Do next:

- add repeatable scenarios for:
  - cold startup settle time,
  - pane open/focus/close loops,
  - pane fullscreen enter/exit,
  - window resize / drag pressure,
  - `Go Online` transition,
  - wallet refresh while UI is idle,
  - `Presentation` / Rive continuous animation load,
  - `Provider Control` while background chat / wallet updates continue.
- emit JSON summaries with average, p95, p99, and max latencies.

Why:

- the plumbing now exists,
- but the repo still lacks a first-class “measure the entire app” scriptable
  workload suite.

### Priority 3: Stage startup after first paint

Owner:

- `apps/autopilot-desktop`

Do next:

- distinguish first-paint-critical initialization from deferred startup work,
- delay nonessential background projections, backfills, and refreshes until the
  first frame is presented and the shell is interactive,
- make startup sequencing visible in diagnostics.

Why:

- the biggest subjective complaint has been startup beachballs,
- and “everything initializes at once” is still directionally true even after
  the wallet/backfill fixes.

### Priority 4: Cache or cheapen inactive pane rendering

Owner:

- `apps/autopilot-desktop`
  and, if reusable caching primitives are needed, `crates/wgpui` /
  `crates/wgpui-render`

Do next:

- identify panes with the highest text/layout cost,
- avoid full text shaping for inactive panes,
- reuse shaped text / preview scenes where possible,
- consider app-owned snapshot cards for inactive heavy panes instead of
  repainting their full dashboard bodies.

Why:

- the post-fix samples already point toward ordinary pane paint cost as the
  next bottleneck.

### Priority 5: Make desktop-control snapshots push-driven

Owner:

- `apps/autopilot-desktop`

Do next:

- maintain a cached snapshot plus dirty-domain flags,
- rebuild only the affected snapshot domains when state changes,
- serve read requests from the cached snapshot instead of rebuilding during idle
  polling.

Why:

- the current design is much improved but still recomputes app state on demand,
- and a control plane should not meaningfully compete with UI responsiveness.

### Priority 6: Tighten Spark refresh provenance and backpressure

Owner:

- `apps/autopilot-desktop`

Do next:

- track which surface requested each refresh,
- expose queue depth / last refresh source / refresh suppression counts in
  diagnostics,
- keep coalescing and throttling aggressive for healthy steady-state refreshes.

Why:

- Spark was a major source of early beachballs,
- and future regressions will be easier to catch if refresh demand is explicit.

### Priority 7: Split or rotate session logs more aggressively

Owner:

- `apps/autopilot-desktop`

Do next:

- keep active logs small enough that backfill never sees giant hot files,
- consider domain-specific logs or archival rotation for historical JSONL,
- continue treating session logs as historical evidence, not live state.

Why:

- the `636 MB` session-log sample was a clear product smell,
- even with import caps, giant live logs are a bad substrate for a desktop app.

### Priority 8: Add packaged-app perf verification

Owner:

- `apps/autopilot-desktop`

Do next:

- pair `autopilotctl` roundtrips with responsiveness thresholds in a packaged
  app run,
- capture a small set of headline numbers per release candidate.

Why:

- dev-mode runs are useful,
- but release regressions matter more than local debug behavior.

## Bottom Line

The recent pre-clippy performance work was real and valuable. It was not random
micro-optimization.

It shipped:

- better redraw instrumentation,
- less harmful Spark refresh behavior,
- bounded historical backfill behavior,
- and a harness that can finally reproduce pane lifecycle stalls.

The next performance phase should stop focusing on obvious background I/O
mistakes and start focusing on:

- first-paint startup staging,
- always-on no-observer-effect telemetry,
- pane rendering cost,
- and release-grade automated perf thresholds.

That is the path from “we fixed several beachball causes” to “the whole app is
continuously measured and hard to make slow by accident.”
