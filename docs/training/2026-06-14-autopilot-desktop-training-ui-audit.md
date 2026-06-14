# Autopilot Desktop Training UI Audit

Date: 2026-06-14

Scope: add a desktop Training pane that is primarily a `three-effect` UI, grounded in the Pluralis-to-Pylon roadmap, Tassadar/Psion training docs, and the existing OpenAgents training authority code.

## Summary

The desktop app should be a training operations cockpit, not a new authority layer. The webview can render rich, dark, real-time training visualizations and queue operator intent through the local Pylon bridge, but it should not hold admin credentials, bypass Worker training routes, or invent training state that the run/window authority has not projected.

The first implemented slice follows that boundary:

- `@openagentsinc/three-effect` now provides an `oa-training-run` Foldkit custom element.
- Autopilot Desktop now has a sidebar `Training` pane centered on that Three scene.
- The pane fetches public Worker-authoritative training projections from `/api/training/runs` through the Bun main process.
- The pane also fetches public CS336 dashboard summaries from `/api/training/leaderboards`, `/api/training/device-capabilities/a2`, `/api/training/isoflop/a3`, `/api/training/refinery/a4`, and `/api/training/evals/a5`.
- The pane reads `/api/public/product-promises` and filters the training/Tassadar promises so the remaining registry blockers from issue 4855 are visible beside the live run data.
- The selected public run summary is converted into a `three-effect` snapshot so the scene reflects live run state, windows, devices, Freivalds refs, closeouts, verified work, external blockers, and settlement.
- The pane includes a Run Lifecycle panel that maps the selected public run onto the issue 4855 / Pluralis join ramp (`registered`, `qualified`, `state_synced`, `warmup`, `active`, `sync_reentry`) and the Worker window timeline (`planned`, `active`, `sealed`, `reconciled`).
- The pane exposes a launch/readiness feedback button that queues a local Pylon intent through the existing `intent.submit` path.
- The pane also exposes Bun-main-process, env-gated actions for planning an R1 rehearsal run/window, activating a planned window, claiming the active training lease for a local Pylon ref, and reconciling a sealed window. The webview receives only public-safe run/window/lease refs and projections.
- The pane can request a public bootstrap grant for the local Pylon ref against the selected training run, showing whether the joiner is granted the last durable seal, queued by the seal barrier, or refused because no durable seal exists.
- The pane can queue a local Pylon closeout packet task with the selected run/window/lease/bootstrap refs, but it does not synthesize seal metadata or call evidence-admission routes from the webview.
- The pane now includes an Evidence Ledger panel for the selected run, exposing public authority refs, window refs, Freivalds/gradient closeout refs, receipt refs, and blocker refs without moving raw worker payloads into the webview.
- Evidence admission, settlement, and real worker execution remain outside the webview.

That is the right initial shape. The next high-value step is wiring the plan result to the actual run admission and evidence pipeline while preserving the same authority boundary.

## Sources Inspected

OpenAgents issue and roadmap:

- OpenAgents issue 4855, "Tracking: Pluralis -> Pylon adaptation roadmap (P0-P3)".
- Child issues 4848 through 4854 for lifecycle, seal metadata, bootstrap, join barriers, hardware admission, staleness-priced acceptance, and presence/compute receipt split.
- Psionic issues 1124 through 1128 for derisking ledger entries, shadow-window ramp, collective failure semantics, SPARTA canary, and PowerSGD/Freivalds compatibility.
- `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`.
- `docs/training/2026-06-10-training-program-status.md`.
- `docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md`.
- `docs/tassadar/RESEARCH_PLAN.md`.
- `docs/tassadar/2026-06-14-w3-student-program-report.md`.

OpenAgents authority code:

- `apps/openagents.com/workers/api/src/training-run-window-authority.ts`.
- `apps/openagents.com/workers/api/src/training-run-window-routes.ts`.
- `apps/openagents.com/workers/api/src/training-leaderboards.ts`.
- `apps/openagents.com/workers/api/src/training-real-gradient-evidence.ts`.
- `apps/openagents.com/workers/api/src/training-window-bootstrap.ts`.
- `apps/openagents.com/apps/web/src/page/loggedOut/page/trainingRuns.ts`.

Autopilot Desktop code:

- `apps/autopilot-desktop/src/ui/model.ts`.
- `apps/autopilot-desktop/src/ui/message.ts`.
- `apps/autopilot-desktop/src/ui/update.ts`.
- `apps/autopilot-desktop/src/ui/commands.ts`.
- `apps/autopilot-desktop/src/ui/view.ts`.
- `apps/autopilot-desktop/src/bun/pylon-control.ts`.
- `apps/autopilot-desktop/src/shared/rpc.ts`.

Pluralis reference repos:

- `projects/pluralis/repos/node0/README.md`.
- `projects/pluralis/repos/agora/docs/agora-system/training-architecture.md`.
- `projects/pluralis/repos/agora/docs/agora-system/startup-sequence.md`.
- `projects/pluralis/repos/agora/docs/agora-system/fault-tolerance.md`.
- `projects/pluralis/repos/agora/docs/agora-system/memory-communication.md`.
- `projects/pluralis/repos/AsyncMesh/sparta/sparta.py`.

## Pluralis Dashboard Finding

The local Pluralis reference code did not include the hosted dashboard implementation. The visible dashboard reference is the hosted link in `node0/README.md`: `https://dashboard.pluralis.ai/`. The available local material is still valuable, but it is architecture and runtime documentation rather than a UI source tree.

The useful design inputs are:

- A participant lifecycle with staged admission, synchronization, warmup, active contribution, and sync-mode reentry.
- A dashboard emphasis on training progress, participant contribution, and live cluster status.
- A pipeline architecture where workers hold stages rather than whole models.
- CPU-only trainers coordinating GPU workers.
- SPARTA-style sparse averaging as a communication strategy, but not as a default OpenAgents production optimizer path.
- Failure semantics around chunked collectives, partial-result preservation, and stage-empty fatality.
- Memory gates around BF16 capability, host RAM headroom, activation recomputation, and GPU admission.

The absence of local dashboard code means our desktop pane should not pretend to be a port of Pluralis UI. It is a Pylon-native interpretation of the same operational problems, with OpenAgents-specific verification, receipts, public promises, and authority boundaries.

## Required UI Model

The desktop Training pane should expose five layers.

First, lifecycle state. The issue 4855 adaptation makes lifecycle a first-class operator object: `registered -> qualified -> state_synced -> warmup -> active`, with `lagged -> sync_reentry` when staleness exceeds the configured bound. This belongs in the primary visual surface because it answers the first operator question: which Pylons are merely present, which are syncing, which are contributing, and which are excluded or stale?

Second, run/window authority. The Worker authority already models run state and window state as `planned`, `active`, `sealed`, and `reconciled`. The UI should show the active run, active window, current seal barrier, last durable checkpoint digest, and seal cadence. Joiners must bootstrap from the last durable seal, not from in-flight merge state.

Third, evidence and verification. The public claim boundary requires distinct contributor evidence, Freivalds commitments, gradient closeout refs, merge/eval refs, loss budget status, and verified challenge coverage before any real-gradient claim becomes public. The UI should keep "not yet claimable" visually distinct from "failed"; a missing proof is not the same thing as a bad proof.

Fourth, receipts and settlement. Issue 4854 splits presence receipts from compute receipts. The UI needs to show presence floor, verified compute closeout, class rate, payout state, and settlement blocker reasons separately. Paying for verified compute cannot be conflated with merely showing up in the cluster.

Fifth, operator action feedback. The user's requirement is immediate feedback when an admin triggers a run. The desktop now has seven honest feedback paths: a local Pylon readiness intent for gate inspection, an authenticated Bun-main-process Worker plan call, an authenticated Bun-main-process Worker activation call, an explicitly enabled Worker lease-claim call for the local Pylon ref, a public bootstrap grant request for the selected run, a local Pylon closeout-packet intent, and an authenticated Bun-main-process reconcile call for sealed windows. The admin calls run only when `OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE=1` and an admin token are present. The lease call runs only when `OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE=1` and a public Pylon ref is available from `OPENAGENTS_TRAINING_PYLON_REF`, `PYLON_REF`, or local Pylon identity. The plan call returns the planned run ref, planned window ref, and public-safe projections; the activation call advances a planned window to `active` and refreshes the public projection; the lease claim returns a public-safe lease ref/window ref/expiry; bootstrap reports whether the local Pylon can join from the last durable seal; the closeout intent asks local Pylon to gather real worker output refs and blockers for a later evidence-admission bridge; reconcile advances a Worker-projected sealed window to `reconciled`. These calls do not admit evidence, spend funds, publish an artifact, fabricate seal metadata, or run the actual training worker.

## Current Implementation Slice

This change adds `Training` to Autopilot Desktop's sidebar and renders a dark-mode, mostly white-dot Three scene through `@openagentsinc/three-effect/foldkit`.

The scene encodes:

- The Pluralis-derived lifecycle.
- A center Tassadar/Psion run authority node.
- A staleness ring with `max stale 5`, matching the current default in the training authority.
- Contributor dots moving around the run surface.
- Window, seal, Freivalds, receipt, and settlement gates.
- R1/R2 ladder readiness.
- A simple loss curve and edge pulses for activity.

The Foldkit pane adds operator panels for:

- Run authority status.
- Live Worker training run summaries.
- A selected-run lifecycle timeline for contributor admission, state sync, warmup, active work, staleness reentry, and Worker window state.
- Evidence requirements from the public real-gradient projection.
- A selected-run Evidence Ledger for concrete public refs behind the current gates.
- Issue 4855 gates.
- Launch feedback.
- Public and admin API boundary.

The operations panel has seven buttons. `Plan R1 window` calls a typed desktop RPC handled only by the Bun main process. The Bun side refuses to call admin routes unless `OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE=1` and either `OPENAGENTS_TRAINING_ADMIN_API_TOKEN` or `OPENAGENTS_ADMIN_API_TOKEN` is available. When enabled, it calls `POST /api/training/runs` and then `POST /api/training/windows/plan`, using public-safe refs derived from the local timestamp and the issue 4855 / Tassadar source docs. The webview stores the returned `TrainingPlanResponse` as an opaque projection and displays only refs/status.

`Activate window` uses the same Bun-only admin gate and calls `POST /api/training/windows/{windowRef}/activate` for either the just-planned window or the first planned window in the selected public run summary. It returns a `TrainingWindowActionResponse`, refreshes `/api/training/runs` on success, and still exposes only the public-safe window projection to the webview.

`Claim lease` is separately gated by `OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE=1`. It resolves a public Pylon ref from `OPENAGENTS_TRAINING_PYLON_REF`, `PYLON_REF`, or the local Pylon identity file in the discovered Pylon home, then calls `POST /api/training/leases/claim`. It is disabled in the Foldkit view until the desktop has seen an active window from the public projection or the activation result. The result is stored as a `TrainingWindowLeaseResponse` and displays only lease/window/ref/expiry fields.

`Request bootstrap` uses the same public Pylon ref resolution and calls `POST /api/training/runs/{runRef}/bootstrap-grant` for the selected public run. It is not an admin mutation. The returned outcome is displayed immediately as `granted`, `queued`, or `refused`, preserving the issue 4850/4851 rule that joiners bootstrap only from the last durable seal and queue during an in-flight seal barrier.

`Queue closeout packet` uses the same local Pylon intent bridge as the launch check, but it includes the selected run ref plus any visible window, lease, and bootstrap grant refs. The body asks for a public-safe closeout packet with real worker output refs: checkpoint digest, checkpoint artifact, merge ref, eval ref, Freivalds commitments, gradient closeouts, loss curve, shard receipts, and settlement blockers. It explicitly tells the node not to fabricate seal metadata and not to call admin-only evidence routes from the desktop webview. This gives the operator immediate post-lease workflow feedback while preserving the later Bun-side evidence-admission boundary.

`Reconcile window` uses the admin gate and calls `POST /api/training/windows/{windowRef}/reconcile`, but it is only enabled for a sealed window already visible in the public Worker projection. The desktop deliberately does not synthesize seal metadata. Sealing requires evidence-bearing closeout metadata, so that remains a later Bun-side bridge from actual worker closeout records rather than a button that fabricates a checkpoint digest.

The `Queue launch check` button still queues a local Pylon intent titled `Training run launch check`. Its body asks the node to inspect the issue 4855 gates, `/api/training/runs`, R1 readiness, seal/staleness state, distinct contributors, Freivalds refs, gradient closeout refs, receipts, and settlement blockers. This gives the operator immediate visible feedback when admin planning is disabled or when the local node should inspect readiness before the operator plans another window.

The refresh button and Training pane navigation call typed desktop RPCs that read public Worker endpoints. This is intentionally read-only and produces desktop-local projections with the live run count, per-run state, verified work count, assigned contributor count, device requirement status, Freivalds refs, gradient closeout refs, loss budget, external blocker state, and settled sats.

The Run Lifecycle panel is a compact translation layer for issue 4855. It derives `registered`, `qualified`, `state_synced`, `warmup`, `active`, and `sync_reentry` from assigned contributors, device qualification, durable seal visibility, active/planned windows, verified work, rejected work, and the configured stale bound. A second strip shows the Worker window authority counts for `planned`, `active`, `sealed`, and `reconciled`, plus seal-barrier and closeout readiness. This makes admin-triggered progress visible immediately after projection refreshes without adding a second source of truth.

The Evidence Ledger panel is the bridge between the high-level gates and the actual code-facing refs an operator needs. For the selected run it lists authority refs, the first projected window records, evidence refs including Freivalds and gradient closeouts, receipt refs, and external blocker/requirement refs. It is still a public projection surface: it never displays private worker output, credentials, wallet material, or raw evidence payloads.

The desktop also summarizes the public CS336 dashboard surfaces in a compact panel. It counts ranked leaderboard lanes from `/api/training/leaderboards`, A2 observed/verified device measurements from `/api/training/device-capabilities/a2`, A3 verified ISOFLOP cells from `/api/training/isoflop/a3`, A4 verified data-refinery stages from `/api/training/refinery/a4`, A5 verified eval suites from `/api/training/evals/a5`, and public blocker refs across those projections. This gives the operator the same public readiness context available to the web dashboards without moving raw evidence, private worker payloads, or admin authority into the webview.

The Promise Gates panel closes a different gap: issue 4855 itself is closed, but its final comment delegates remaining live R1/R2 receipt evidence to the public promise registry. The desktop now pulls that registry through Bun, filters the training and Tassadar-related promise records, and shows state counts plus blocker/evidence counts for the relevant promises. This keeps "the code says the tracking issue is done" separate from "the registry says the broad training claim is green."

The Three scene receives the same selected summary through a compact snapshot mapper in `three-effect`. That mapper updates node labels/statuses, contributor dots, stale bound, receipt/settlement gates, and loss curve inputs without importing OpenAgents Worker internals into the rendering package.

## Authority Boundary

The desktop webview must remain a Foldkit UI over typed projections and commands. It should not store:

- Worker admin tokens.
- CRM/operator tokens.
- Training evidence mutation credentials.
- Private payout or wallet secrets.

The right boundary is:

- Webview: render projections, collect user intent, display command results.
- Bun main process: hold local control token, talk to local Pylon, and proxy the env-gated training plan, activate, lease-claim, and reconcile commands.
- Pylon/local node: decide how to schedule or escalate operator intent.
- OpenAgents Worker: own run/window authority, public projections, admin planning, evidence admission, and claim boundaries.

This matches the existing desktop pattern: `commands.ts` calls typed RPC requests, Bun forwards to local Pylon `/command`, and node state returns through projections.

## Fit With Foldkit And three-effect

Foldkit is a good fit for the desktop shell because the UI is already an Effect TEA program with a pure model, typed messages, typed commands, and a single view. It is also the right place to keep training controls honest: the view can only render state and dispatch declared messages.

`three-effect` is a better fit for the animated training surface than hand-managed DOM or React. It lets us keep renderer lifecycle, resize handling, and disposal inside an Effect-scoped scene, then expose a Foldkit custom element. That creates a clean split:

- Foldkit owns application state and command routing.
- `three-effect` owns WebGL resources and animation.
- The custom element is the bridge.

The same pattern can be reused in the web app and mobile app if Foldkit remains the shared UI substrate. The desktop app can use `oa-training-run` today; the openagents.com web app can later mount the same element beside public run summaries; mobile can use a lower-detail version or a static projection if WebGL pressure is too high.

## Risks

The current pane is now partly live: it reads public run, dashboard, and promise-registry projections and can plan, activate, claim a lease, and reconcile sealed windows through Bun when explicitly enabled. It is still not a full worker-execution dashboard. The most likely failure mode is visual polish outrunning authority integration. That would create a good-looking control room that does not answer the operator's actual question after a lease is claimed: which worker process accepted the lease, which evidence landed, which seal metadata was verified, and which receipts settled?

The second risk is authority leakage. The implemented planning bridge keeps admin tokens in Bun, but future evidence admission and launch controls must follow the same pattern. Do not add admin tokens, private evidence, wallet material, or raw payout details to the Foldkit webview model.

The third risk is claim confusion. Public real-gradient claims have explicit blockers. The UI should avoid wording like "training live" unless the authority has actually accepted the required evidence. Until then, the correct language is "planned", "queued", "warmup", "evidence missing", or "not public-claimable".

## Next Steps

1. Add evidence-admission and worker-closeout bridges that remain Bun-side and only expose public-safe results to Foldkit.
2. Add command result state for admission queue count, lease claim state, evidence refs, and first observed projection timestamp after planning.
3. Add tests proving the webview cannot access admin credentials and only dispatches typed training messages.
4. Add a lower-detail responsive scene mode before sharing the same visualization with mobile.
