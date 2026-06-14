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
- The pane now reads a Bun-local operator readiness projection that exposes only public-safe booleans and refs for admin enablement, admin token presence, lease enablement, Pylon identity, local Pylon home, and local control-token availability.
- The selected public run summary is converted into a `three-effect` snapshot so the scene reflects live run state, lifecycle counts, windows, seal-barrier state, devices, Freivalds refs, closeouts, receipts, verified work, external blockers, pending payouts, and settlement.
- The Three scene now also receives compact product-promise registry signals, so the visual surface shows the live acceptance owner and blocker/evidence pressure instead of leaving that context only in adjacent HTML panels.
- The Three scene also receives compact operator-readiness and operator-command signals for readiness, evidence-packet inspection, plan, activate, lease, bootstrap, closeout, admit, and reconcile, so immediate admin feedback is visible in the primary WebGL surface.
- `three-effect` now carries `examples/training-run/`, a runnable visual smoke for the same dark Training scene with lifecycle, promise, and operator signals.
- Autopilot Desktop now carries `bun run smoke:training-scene`, a Chrome-backed canvas-pixel smoke that builds a Training scene harness from the pinned `@openagentsinc/three-effect`, mounts `oa-training-run`, captures the rendered page, and fails if the WebGL surface is blank or undersized.
- Autopilot Desktop now exposes that smoke through `bun run verify:training` and the root `bun run verify:autopilot-desktop:training` command, alongside the focused Foldkit tests and browser/Bun bundle checks.
- The pane includes a Run Lifecycle panel that maps the selected public run onto the issue 4855 / Pluralis join ramp (`registered`, `qualified`, `state_synced`, `warmup`, `active`, `sync_reentry`) and the Worker window timeline (`planned`, `active`, `sealed`, `reconciled`).
- The pane exposes a launch/readiness feedback button that queues a local Pylon intent through the existing `intent.submit` path.
- The pane also exposes Bun-main-process, env-gated actions for planning an R1 rehearsal run/window, activating a planned window, claiming the active training lease for a local Pylon ref, and reconciling a sealed window. The webview receives only public-safe run/window/lease refs and projections.
- The pane can request a public bootstrap grant for the local Pylon ref against the selected training run, showing whether the joiner is granted the last durable seal, queued by the seal barrier, or refused because no durable seal exists.
- The pane can queue a local Pylon closeout packet task with the selected run/window/lease/bootstrap refs, but it does not synthesize seal metadata or call evidence-admission routes from the webview.
- The pane now inspects the configured local evidence packet through Bun before admission, exposing only counts, loss-budget status, ref-presence booleans, and blocker refs.
- The pane can now build a candidate evidence packet from a local worker-receipts JSON bundle, gated by `OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE=1` and writing only to `OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH`.
- Local Pylon assignment closeouts now update a public-safe `training-worker-receipts.json` bundle in the Pylon home, and Desktop uses that bundle by default when `OPENAGENTS_TRAINING_WORKER_RECEIPTS_PATH` is not set.
- The pane can now admit a local, file-backed CS336 A1 real-gradient evidence packet through a Bun-only admin bridge to `POST /api/training/runs/{runRef}/real-gradient-evidence`. It refuses unless evidence admission is explicitly enabled and `OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH` points at a JSON packet.
- The pane now includes an Operator Readiness panel that shows the pre-click admin/lease/Pylon gates and blocker refs before the operator tries to plan, lease, or bootstrap a run.
- The pane now includes an Operator Feed that consolidates public projection refresh state, dashboard/promise/readiness fetch state, and every operator command result so an admin can see immediate feedback and later projection catch-up in one place.
- The pane now includes a Projection Catch-Up panel that compares Bun-held command results with the latest Worker projection for plan observation time, activation state, lease claim state, admitted evidence receipt counts, and reconcile state.
- The pane now includes a Control Surface panel that maps each Training button to its Foldkit message, Bun RPC, Worker/local route, Foldkit status field, authority boundary, and source file. This is the literal "training code at fingertips" layer for live operation.
- The pane now includes an Authority Boundary panel that spells out the Foldkit webview, Bun main process, OpenAgents Worker, and Pylon responsibilities, including the typed Training messages that can cross the webview boundary.
- The pane now includes an Evidence Ledger panel for the selected run, exposing public authority refs, window refs, Freivalds/gradient closeout refs, receipt refs, and blocker refs without moving raw worker payloads into the webview.
- The pane now includes an Issue 4855 Ledger panel that carries the Pluralis-to-Pylon child issue phases, the closed implementation status, the SPARTA/run/economics caveats, and the registry handoff for live R1/R2 evidence.
- The pane includes a Source Map panel that groups the Worker authority, evidence routes, desktop bridge, `three-effect` scene, and training docs by responsibility.
- Evidence admission, packet reading/writing, settlement, and real worker execution remain outside the webview.

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

Fifth, operator action feedback. The user's requirement is immediate feedback when an admin triggers a run. The desktop now has a pre-click readiness projection, a pre-admission evidence packet inspection projection, and nine honest feedback paths: a local Pylon readiness intent for gate inspection, an authenticated Bun-main-process Worker plan call, an authenticated Bun-main-process Worker activation call, an explicitly enabled Worker lease-claim call for the local Pylon ref, a public bootstrap grant request for the selected run, a local Pylon closeout-packet intent, a Bun-main-process worker-receipts-to-packet writer, a Bun-main-process real-gradient evidence admission bridge, and an authenticated Bun-main-process reconcile call for sealed windows. The readiness projection is local to Bun and exposes only booleans/refs for the admin env gate, admin token presence, lease env gate, Pylon ref source, Pylon home, local control token, evidence-admission env gate, and evidence-packet path presence. The packet inspection projection is also local to Bun and exposes only the env-label packet source, counts, loss-budget status, ref-presence booleans, and blocker refs; it never exposes the local file path or raw packet. The packet writer is local to Bun, refuses unless `OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE=1`, reads worker receipt refs from `OPENAGENTS_TRAINING_WORKER_RECEIPTS_PATH` or the default Pylon-home `training-worker-receipts.json`, writes only to `OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH`, and returns the same public-safe summary/blocker shape. The admin calls run only when `OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE=1` and an admin token are present. The lease call runs only when `OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE=1` and a public Pylon ref is available from `OPENAGENTS_TRAINING_PYLON_REF`, `PYLON_REF`, or local Pylon identity. The evidence admission call runs only when `OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_ENABLE=1`, an admin token is present, and `OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH` points at a local JSON packet. The plan call returns the planned run ref, planned window ref, and public-safe projections; the activation call advances a planned window to `active` and refreshes the public projection; the lease claim returns a public-safe lease ref/window ref/expiry; bootstrap reports whether the local Pylon can join from the last durable seal; the closeout intent asks local Pylon to gather real worker output refs and blockers; the packet writer turns local worker receipts into an inspectable candidate packet; evidence admission posts the file-backed packet to the Worker and returns public-safe counts plus the recomputed real-gradient status; reconcile advances a Worker-projected sealed window to `reconciled`. These calls and projections do not spend funds, publish an artifact, fabricate seal metadata, run the actual training worker, expose local packet paths, expose raw packet payloads, or expose tokens to the webview.

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
- An Issue 4855 Ledger that shows the roadmap phase, issue ref, closed implementation status, and the promise-registry owner for remaining hardware/settlement receipts.
- Launch feedback.
- An Operator Feed for public projection catch-up plus the latest packet summary, plan, activate, lease, bootstrap, closeout, packet-build, reconcile, and launch-check command states.
- Public and admin API boundary.
- Source-map references for the Worker, desktop, `three-effect`, and training-doc implementation homes.

The operations panel has ten buttons. `Plan R1 window` calls a typed desktop RPC handled only by the Bun main process. The Bun side refuses to call admin routes unless `OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE=1` and either `OPENAGENTS_TRAINING_ADMIN_API_TOKEN` or `OPENAGENTS_ADMIN_API_TOKEN` is available. When enabled, it calls `POST /api/training/runs` and then `POST /api/training/windows/plan`, using public-safe refs derived from the local timestamp and the issue 4855 / Tassadar source docs. The webview stores the returned `TrainingPlanResponse` as an opaque projection and displays only refs/status.

The Operator Readiness panel is the pre-click checklist for those buttons. `listTrainingOperatorReadiness` is handled in `apps/autopilot-desktop/src/bun/index.ts`, not in the webview. It reports whether the admin env gate is enabled, whether an admin token exists, whether lease claiming is enabled, whether a Pylon ref came from env or identity, whether a Pylon home exists, whether the local Pylon control token exists, whether evidence admission is enabled, and whether an evidence packet path is configured. The returned blocker refs are operational labels such as `env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE`, `env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH`, or `pylon.control_token`; the projection never includes the underlying token value or local file path.

The Evidence Packet panel is the pre-admission checklist for `Admit evidence packet`. `listTrainingEvidencePacketSummary` is handled by Bun and reads the configured packet path only to compute a public-safe summary. It reports receipt count, evidence-ref count, shard count, distinct Pylon count, loss-curve point count, final/max validation loss, Freivalds commitment count, gradient closeout count, budget/eval/merge ref presence, and blocker refs such as `training.evidence_packet.loss_exceeds_budget` or `training.evidence_packet.requires_two_distinct_pylons`. The webview never receives the raw packet, the raw local path, worker logs, wallet material, or credentials.

`Build evidence packet` is the smallest implemented bridge from local worker output into the file-backed admission path. It reads a worker-receipts JSON bundle from `OPENAGENTS_TRAINING_WORKER_RECEIPTS_PATH` when configured, otherwise from the discovered Pylon home at `training-worker-receipts.json`, and only runs when `OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_WRITE_ENABLE=1`. It converts public-safe receipt/checkpoint/proof/metric/signature refs into a real-gradient evidence packet candidate, writes that candidate to `OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH`, and immediately returns the same summary/blocker projection used by the Evidence Packet panel. If the receipts do not include enough data for a green packet, it still writes an inspectable candidate and reports `packet_blocked`; it does not fabricate missing budget, eval, merge, loss, or distinct-Pylon evidence.

`Activate window` uses the same Bun-only admin gate and calls `POST /api/training/windows/{windowRef}/activate` for either the just-planned window or the first planned window in the selected public run summary. It returns a `TrainingWindowActionResponse`, refreshes `/api/training/runs` on success, and still exposes only the public-safe window projection to the webview.

`Claim lease` is separately gated by `OPENAGENTS_DESKTOP_TRAINING_LEASE_ENABLE=1`. It resolves a public Pylon ref from `OPENAGENTS_TRAINING_PYLON_REF`, `PYLON_REF`, or the local Pylon identity file in the discovered Pylon home, then calls `POST /api/training/leases/claim`. It is disabled in the Foldkit view until the desktop has seen an active window from the public projection or the activation result. The result is stored as a `TrainingWindowLeaseResponse` and displays only lease/window/ref/expiry fields.

`Request bootstrap` uses the same public Pylon ref resolution and calls `POST /api/training/runs/{runRef}/bootstrap-grant` for the selected public run. It is not an admin mutation. The returned outcome is displayed immediately as `granted`, `queued`, or `refused`, preserving the issue 4850/4851 rule that joiners bootstrap only from the last durable seal and queue during an in-flight seal barrier. Any authoritative bootstrap outcome now also triggers the public projection/dashboard/promise refresh commands, so a queued joiner or refused joiner still causes the Training pane and Three scene to catch up after the operator click.

`Queue closeout packet` uses the same local Pylon intent bridge as the launch check, but it includes the selected run ref plus any visible window, lease, and bootstrap grant refs. The body asks for a public-safe closeout packet with real worker output refs: checkpoint digest, checkpoint artifact, merge ref, eval ref, Freivalds commitments, gradient closeouts, loss curve, shard receipts, and settlement blockers. It explicitly tells the node not to fabricate seal metadata and not to call admin-only evidence routes from the desktop webview. Separately, every Pylon assignment closeout now writes or upserts a public-safe worker receipt into the local Pylon-home `training-worker-receipts.json` bundle, giving the Desktop builder a default local source before any operator-provided receipt bundle exists.

`Admit evidence packet` is the first Bun-side admission bridge for that later boundary. It reads a JSON packet from `OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH` only when `OPENAGENTS_DESKTOP_TRAINING_EVIDENCE_ENABLE=1` and an admin token are present, then calls `POST /api/training/runs/{runRef}/real-gradient-evidence`. The response stored in Foldkit contains only the public run projection, recomputed real-gradient status, evidence/ref counts, receipt count, shard count, distinct Pylon count, reason, and message. It does not expose the packet path, raw packet, admin token, worker logs, wallet material, or raw private output. The separate Evidence Packet panel now lets the operator see whether that same configured packet has the expected counts and blockers before clicking admission, and the packet build button lets the operator create that file from worker receipt refs when the receipt bundle is available.

`Reconcile window` uses the admin gate and calls `POST /api/training/windows/{windowRef}/reconcile`, but it is only enabled for a sealed window already visible in the public Worker projection. The desktop deliberately does not synthesize seal metadata. Sealing requires evidence-bearing closeout metadata, so that remains a later Bun-side bridge from actual worker closeout records rather than a button that fabricates a checkpoint digest.

The `Queue launch check` button still queues a local Pylon intent titled `Training run launch check`. Its body asks the node to inspect the issue 4855 gates, `/api/training/runs`, R1 readiness, seal/staleness state, distinct contributors, Freivalds refs, gradient closeout refs, receipts, and settlement blockers. This gives the operator immediate visible feedback when admin planning is disabled or when the local node should inspect readiness before the operator plans another window.

The refresh button and Training pane navigation call typed desktop RPCs that read public Worker endpoints and one local packet summary. This is intentionally read-only and produces desktop-local projections with the live run count, per-run state, verified work count, assigned contributor count, device requirement status, Freivalds refs, gradient closeout refs, loss budget, external blocker state, settled sats, and configured-packet blocker counts.

The Run Lifecycle panel is a compact translation layer for issue 4855. It derives `registered`, `qualified`, `state_synced`, `warmup`, `active`, and `sync_reentry` from assigned contributors, device qualification, durable seal visibility, active/planned windows, verified work, rejected work, and the configured stale bound. A second strip shows the Worker window authority counts for `planned`, `active`, `sealed`, and `reconciled`, plus seal-barrier and closeout readiness. This makes admin-triggered progress visible immediately after projection refreshes without adding a second source of truth.

The Operator Feed is the immediate-feedback strip for the same contract. It does not create new authority or add a second command path; it reads the Foldkit model fields already populated by typed messages. The first rows show whether `/api/training/runs`, dashboard summaries, promise gates, operator readiness, and evidence packet summary are loaded, refreshing, blocked, or caught up. The remaining rows show the latest local plan, activate, lease, bootstrap, closeout, packet-build, evidence admission, reconcile, and launch-check status with the public run/window/lease/grant/count refs that are safe to display. That gives the admin a single place to look after a button press while the public Worker projection refresh catches up.

The Issue 4855 Ledger panel reflects the current GitHub state rather than the original open tracking state. Issue 4855 is closed because all child implementation issues landed on the canonical branches and the roadmap doc records the outcome. The panel keeps the phase ledger visible in the desktop cockpit while making the important handoff explicit: live R1/R2 device, receipt, settlement, and canary evidence now belongs to the `training.*` product-promise registry, not to a still-open tracking issue.

The Evidence Ledger panel is the bridge between the high-level gates and the actual code-facing refs an operator needs. For the selected run it lists authority refs, the first projected window records, evidence refs including Freivalds and gradient closeouts, receipt refs, and external blocker/requirement refs. It is still a public projection surface: it never displays private worker output, credentials, wallet material, or raw evidence payloads.

The Source Map panel is the "code at fingertips" layer. It does not execute code or carry authority; it points at the owning implementation files and public/admin route families for the Worker authority, evidence gates, desktop Bun/Foldkit bridge, `three-effect` scene, and training docs.

The Control Surface panel is the "what happens when I click this" layer. It lists the concrete dispatch/RPC/route/source chain for the live operations: refresh projections, plan an R1 window, activate a window, claim a lease, request bootstrap, queue a closeout packet, build an evidence packet, admit a real-gradient evidence packet, reconcile a sealed window, and queue a launch check. The refresh row now names `listTrainingEvidencePacketSummary` beside the public Worker reads. Each row also names the Foldkit model status field (`trainingPlanStatus`, `trainingActivationStatus`, `trainingLeaseStatus`, `trainingBootstrapStatus`, `trainingCloseoutStatus`, `trainingEvidencePacketBuildStatus`, `trainingEvidenceAdmissionStatus`, `trainingReconcileStatus`, `trainingLaunchStatus`, `trainingEvidencePacketSummaryStatus`, or the projection/readiness status fields) and shows the current public-safe ref or fallback. That means an operator can see immediate feedback in the Operator Feed and inspect the exact code boundary in the neighboring Control Surface panel without guessing which route or file owns the behavior.

The desktop also summarizes the public CS336 dashboard surfaces in a compact panel. It counts ranked leaderboard lanes from `/api/training/leaderboards`, A2 observed/verified device measurements from `/api/training/device-capabilities/a2`, A3 verified ISOFLOP cells from `/api/training/isoflop/a3`, A4 verified data-refinery stages from `/api/training/refinery/a4`, A5 verified eval suites from `/api/training/evals/a5`, and public blocker refs across those projections. This gives the operator the same public readiness context available to the web dashboards without moving raw evidence, private worker payloads, or admin authority into the webview.

The Promise Gates panel closes a different gap: issue 4855 itself is closed, but its final comment delegates remaining live R1/R2 receipt evidence to the public promise registry. The desktop now pulls that registry through Bun, filters the training and Tassadar-related promise records, and shows state counts plus blocker/evidence counts for the relevant promises. This keeps "the code says the tracking issue is done" separate from "the registry says the broad training claim is green."

The Three scene receives the same selected summary through a compact snapshot mapper in `three-effect`. That mapper updates node labels/statuses, lifecycle contributor dots, stale bound, seal-barrier state, receipt/settlement gates, blocker counts, pending-payout state, loss curve inputs, promise-registry signal dots, and operator-command signal dots without importing OpenAgents Worker internals into the rendering package. The standalone visual smoke is `OpenAgentsInc/three-effect:examples/training-run/`; build it with `bun run build:demo:training` in the `three-effect` repo and open `examples/training-run/index.html`.

## Authority Boundary

The desktop webview must remain a Foldkit UI over typed projections and commands. It should not store:

- Worker admin tokens.
- CRM/operator tokens.
- Training evidence mutation credentials.
- Private payout or wallet secrets.

The right boundary is:

- Webview: render projections, collect user intent, display command results.
- Bun main process: hold local control token, talk to local Pylon, report public-safe readiness booleans/refs and evidence-packet counts/blockers, write candidate evidence packets from configured worker receipt refs, and proxy the env-gated training plan, activate, lease-claim, evidence-admission, and reconcile commands.
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

The current pane is now partly live: it reads public run, dashboard, promise-registry, readiness, and packet-summary projections and can plan, activate, claim a lease, build an evidence packet candidate from worker receipt refs, admit a file-backed A1 real-gradient evidence packet, and reconcile sealed windows through Bun when explicitly enabled. It is still not a full worker-execution dashboard. The most likely failure mode is visual polish outrunning authority integration. That would create a good-looking control room that does not answer the operator's actual question after a lease is claimed: which worker process accepted the lease, which evidence landed, which seal metadata was verified, and which receipts settled?

The second risk is authority leakage. The implemented planning and A1 evidence-admission bridges keep admin tokens in Bun, but future evidence families, worker-closeout packet generation, and launch controls must follow the same pattern. Do not add admin tokens, private evidence, wallet material, raw packet paths, or raw payout details to the Foldkit webview model.

The third risk is claim confusion. Public real-gradient claims have explicit blockers. The UI should avoid wording like "training live" unless the authority has actually accepted the required evidence. Until then, the correct language is "planned", "queued", "warmup", "evidence missing", or "not public-claimable".

## Next Steps

1. Add richer closeout metadata to the Pylon-emitted worker receipt bundle: budget refs, eval refs, merge refs, loss curves, and distinct multi-Pylon aggregation once real R1/R2 devices submit them.
2. Add Worker-exposed post-admission queue depth and first projection-lag deltas once the training authority emits those counters directly.
3. Add a GitHub Actions workflow, when this repo adopts workflows, that runs `bun run verify:autopilot-desktop:training` on a runner with Chrome available; add an Electrobun-packaged shell variant once the build artifact path is stable.
4. Add a lower-detail responsive scene mode before sharing the same visualization with mobile.
