# Autopilot Desktop Training UI Audit

Date: 2026-06-14

Scope: add a desktop Training pane that is primarily a `three-effect` UI, grounded in the Pluralis-to-Pylon roadmap, Tassadar/Psion training docs, and the existing OpenAgents training authority code.

## Summary

The desktop app should be a training operations cockpit, not a new authority layer. The webview can render rich, dark, real-time training visualizations and queue operator intent through the local Pylon bridge, but it should not hold admin credentials, bypass Worker training routes, or invent training state that the run/window authority has not projected.

The first implemented slice follows that boundary:

- `@openagentsinc/three-effect` now provides an `oa-training-run` Foldkit custom element.
- Autopilot Desktop now has a sidebar `Training` pane centered on that Three scene.
- The pane exposes a launch/readiness feedback button that queues a local Pylon intent through the existing `intent.submit` path.
- Admin planning, evidence admission, and run/window transitions remain outside the webview.

That is the right initial shape. The next high-value step is not more static dashboard decoration; it is a Bun-main-process training bridge that can read public run projections and, for approved operators, call admin training routes without leaking credentials into the Foldkit webview.

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

Fifth, operator action feedback. The user's requirement is immediate feedback when an admin triggers a run. The correct initial desktop feedback is "the request was queued through Pylon" because the current desktop bridge exposes `intent.submit`, not an authenticated training admin command. The future feedback should be stronger: request accepted by admin Worker, planned run ref, planned window ref, admission queue state, and first public projection update.

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
- Issue 4855 gates.
- Launch feedback.
- Public and admin API boundary.

The launch button queues a local Pylon intent titled `Training run launch check`. Its body asks the node to inspect the issue 4855 gates, `/api/training/runs`, R1 readiness, seal/staleness state, distinct contributors, Freivalds refs, gradient closeout refs, receipts, and settlement blockers. This gives the operator immediate visible feedback without turning the webview into an admin client.

## Authority Boundary

The desktop webview must remain a Foldkit UI over typed projections and commands. It should not store:

- Worker admin tokens.
- CRM/operator tokens.
- Training evidence mutation credentials.
- Private payout or wallet secrets.

The right boundary is:

- Webview: render projections, collect user intent, display command results.
- Bun main process: hold local control token, talk to local Pylon, and later proxy approved training admin commands.
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

The current pane is mostly a static operational map. It is useful as an orientation surface, but it is not yet a live run dashboard. The most likely failure mode is visual polish outrunning authority integration. That would create a good-looking control room that does not answer the operator's actual question: what happened to my run request?

The second risk is authority leakage. Adding "admin trigger" directly from the webview would be fast, but it would violate the desktop security model. The next implementation should resist that shortcut and add a typed Bun-side request instead.

The third risk is claim confusion. Public real-gradient claims have explicit blockers. The UI should avoid wording like "training live" unless the authority has actually accepted the required evidence. Until then, the correct language is "planned", "queued", "warmup", "evidence missing", or "not public-claimable".

## Next Steps

1. Add a typed desktop RPC for `listTrainingRuns` that reads `/api/training/runs` from the Worker/public surface and stores the latest summaries in the model.
2. Feed live run/window status, staleness summary, contributor counts, and real-gradient claim blockers into `three-effect` scene options.
3. Add a Bun-main-process admin bridge for approved operators to plan runs and windows. Keep tokens out of the webview.
4. Add command result state for `plannedRunRef`, `plannedWindowRef`, admission queue count, and first observed projection timestamp.
5. Add tests proving the webview cannot access admin credentials and only dispatches typed training messages.
6. Add a lower-detail responsive scene mode before sharing the same visualization with mobile.
