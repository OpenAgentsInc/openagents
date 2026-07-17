---
spec_format_version: "0.1"
title: "OpenAgents Desktop: Trust-Complete Coding Workbench"
artifact_type: "prd"
spec_revision: 3
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-17T23:12:48.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-desktop/"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_source_synthesis: "docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md"
  openagents_source_transcripts: "docs/transcripts/200.md through docs/transcripts/255.md plus the episode-256 draft transcript (Full Auto Implementation Audit and Roadmap)"
  openagents_admission_status: "roadmap-reconciled by docs/sol/MASTER_ROADMAP.md revision 119 as surface vision and target intent; implementation dispatch remains limited to live issues and exact accepted plans/work packets, with owner gates and proof rungs intact"
  openagents_revision_3_note: "Rev 3 elevates Full Auto to the roadmap-defining flagship per the episode-256 draft transcript (owner: an AFK-reliable multi-account multi-provider autonomous mode is 'important enough of an unserved need to build the whole short-term roadmap of the company around'). Full Auto becomes a dedicated run mode launched beside New Session (lightning action, one-time objective/workspace/provider-policy setup, full-screen read-only run view, explicit Play/Pause/Stop, no composer while running — 'steering is not Full Auto'); adds automatic model/account/provider failover on limits (the overnight Fable-limit halt is the named incumbent failure), same-thread cross-provider handoff with end-to-end acceptance proof (the live Claude-error -> Codex-continues-same-chat moment), active-run thread retention (the five-thread cache eviction root cause), and the run -> bounded report -> transcript analysis -> replayable fixture iteration loop. Also folds in back-catalog direction from episodes 200-237: the episode-206 Guidance Module lineage (between-turn decision engine with shared state/budget/environment, confidence-gated actions, hard guardrails over soft guidance, budget-bound autonomy); episode-237 clearing-layer doctrine (scoped-in-advance done, rubric grading, dereferenceable receipts, draft/verified/reviewed/bonded confidence tiers); episode-225 account-visibility peeves (at-a-glance identity, rate-limit burn-down, API-key fallback); episode-204 don't-break-userspace; episode-227 one-click full data export; the one-app cockpit/earning thesis deferred to its own contract."
  openagents_sibling_specs: "specs/mobile/mobile-any-host-fleet-controller.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md"
---

## Problem

Developers who supervise coding agents now get supervision breadth from every
competitor — parallel worktree agents, diff review, terminals, remote control
— but no product gives them trust. The full teardown catalog (ChatGPT/Codex
desktop, Claude desktop, Claude Code, Cursor, Factory, Amp, Grok Build,
OpenCode, T3 Code, OpenChamber, Command Code) shows the same holes everywhere:
permission dialogs presented as containment while sandboxes are off by
default; "completed" claimed without delivery states; no record of what
authority a run was granted versus what enforcement actually ran; unsigned or
fail-open release chains; agent topology that is fully retained on disk yet
almost invisible in the UI.

And no product survives the owner walking away. The founder's own AFK test is
the sharpest statement of the unserved need: with a week of delegable work and
a baby due any day, none of the major harnesses can be trusted to keep going
for a day or two unattended. An overnight run with ample total usage halted
after an hour at a single model's limit instead of dropping down a tier
automatically — "why couldn't you do that automatically?" The incumbent loop
modes (`/loop`, `/goal`, cron) are "brittle to the harness: you could run out
of usage credits, you can't connect multiple accounts, you can't connect
multiple providers." A user with one Claude account, three ChatGPT accounts, a
Grok account, and a Cursor account has no system that connects all of them
and routes between them. The first OpenAgents Full Auto attempt failed the
same test for a different reason: the active run's thread was evicted from a
bounded cache and the next continuation addressed a conversation the host
could no longer open. The dogfood episodes record the rest of the defect
classes that make delegation untrustworthy: convincing green summaries
("according to whom?"), a UI chip naming a model that did not run, invented
launch commands, queued messages replaying in loops, composer state leaking
across chats, ambiguous running binaries, hotkeys that reshuffle under muscle
memory, reasoning hidden behind opaque status verbs. "Get the fuck out of the
terminal. Give me an app… Make it clicky."

## Hypothesis

If OpenAgents Desktop combines the market-converged supervision shape — one
typed engine seam, a workbench that deepens beyond chat, worktree-parallel
agents, a fully rendered live agent tree — with a legible trust layer that no
competitor ships (named execution profiles compiled to OS enforcement,
per-run authority manifests paired with execution receipts, a delivery
lifecycle distinct from completion, effective-identity truth, evidence-gated
status, a signed end-to-end release chain), presents it as a fast, clicky,
StarCraft-like operator surface, and crowns it with Full Auto — a dedicated
press-play-and-walk-away run mode that connects every account and provider
the user has and keeps working reliably through limits, errors, and restarts
for days — then developers will (a) choose it over broader-but-untrusted
competitors, (b) delegate materially more unattended work through it, and
(c) complete consequential work from ProductSpec intent to evidence-backed
acceptance without falling back to another agent interface, because the
surface makes delegation inspectable instead of faith-based and makes walking
away rational instead of reckless.

## Scope

```productspec-scope
in:
  - Consume one generated, versioned engine protocol with the hierarchy Thread -> Turn -> Item, extended with Work Unit (task identity plus delivery lifecycle), Authority Manifest, Execution Receipt, and Delivery Receipt; the renderer holds projections only and never conversation, tool, Git, or PTY authority.
  - Record every input durably before scheduling (client-chosen idempotent IDs, causal parent, typed delivery intent), and expose steer, queue, and interrupt as three explicit verbs with typed composer admission states; queue never silently becomes steer.
  - Key composer draft, queue, attachments, and stop/steer targeting per thread; every user action resolves unambiguously to exactly one thread, a dispatched queue item leaves the queue, and queued items are visible, editable, and cancellable before promotion.
  - Keep attachment capability invariant across composer states: images and files attach identically on a first message, a follow-up, a steer, and a queued input.
  - Provide three read surfaces per session as contract: bounded current projections, a durable replayable per-aggregate log with a replay-to-live marker, and a volatile live stream documented as lossy; reconnect is repair with honest transient-gap markers, never fabricated completion.
  - Survive restarts honestly: in-flight work resumes or terminates through typed recovery after an app restart, restart attribution is never fabricated, and the app always discloses which build, version, and working-tree source is running.
  - Ship a hardened stock Electron shell: locked fuses, sandboxed renderers, schema-decoded and sender-validated IPC on every channel, partitioned sessions for artifact, preview, terminal, and browser surfaces, deny-by-default permission and navigation handlers, and a locally versioned renderer (never a live site).
  - Work local-first with no account: the local workroom requires no OpenAgents account, uses the user's existing provider logins through named isolated accounts (never default provider homes), and keeps data local until the user explicitly opts into Sync or network capabilities.
  - Ship the StarCraft-like operator surface: clicky, fast, game-feel interaction; stable user-bindable hotkeys that never reshuffle underneath muscle memory (pinned chat command-groups, Harpoon-style); dense data made actionable with game UX; and a noob/pro dual mode — an opinionated out-of-the-box experience for newcomers with full power-user controls underneath.
  - Render reasoning and activity visibly by default: reasoning expanded in the main trace rather than hidden behind status verbs, honest tool-activity descriptions, usage shown inline in the main view, and no raw UUIDs in user-facing UI.
  - Display effective execution identity from observed execution events, never inferred from the selected brand: requested versus effective model are distinct typed facts on every message, no substituted output is ever streamed under the requested model's name, and provider rotation happens only on account or session failure before content.
  - Ship a right-panel workbench surface manager hosting review-summary, diff, files, file, terminal, plan, and preview tabs, each a projection over engine state with no renderer authority.
  - Virtualize the transcript with a turn navigator and a three-level message hierarchy (prose primary, compact work rows, exact evidence on disclosure); checked-in performance baselines gate merges on transcript surfaces.
  - Render the complete agent tree: a canonical persisted agent graph with live per-child lifecycle, causal inline child cards at the exact spawn point showing the child's latest activity, one-gesture navigation into each child's full independent transcript, named roster entries in the right rail, and explicit orphan or gap nodes; the same typed projection renders at every surface density with no capability tier.
  - Make ProductSpec the native intent and execution unit: guided conversational authoring of a valid spec, validation at the relevant section with exact revision, digest, and criterion IDs, a reviewable plan derived from acceptance criteria, durable criterion-addressed work packets that agents execute, and evidence-backed completion — with admission, evidence production, verification, owner acceptance, and release as distinct steps that no executor self-grades.
  - Host the resident Assurance layer: the app speaks both ProductSpec and AssuranceSpec, renders proof-design state honestly (proposed, needs-design, not-run — never a hardcoded second source of truth), and treats the named false-green failure modes (fixture asserts, API mirrors, mocked seams, coverage theater, rounded-up summaries) as detectable defects.
  - Maintain a UX Behavior Contract registry: every owner- or customer-stated UX expectation lands verbatim in a typed registry with oracle tests enforced in the normal sweep, contract deviations found in the wild are strict bugs filed with the contract ID, and in-app violation reporting feeds the fix loop.
  - Ship Full Auto as a dedicated run mode, not a composer preference: launched from its own action beside New Session (a lightning-bolt affordance), configured once with an explicit objective, granted workspace, and provider/account routing policy, then presented as a full-screen read-only run view with explicit Play, Pause, and Stop and no ordinary composer while running — press play and walk away; steering a running Full Auto is a later, separately admitted contract, because steering is not Full Auto.
  - Model Full Auto as a durable run-state machine: play and pause are typed, restart-survivable states owned by the main process; every continuation decision is re-derived from durable state at turn completion and at startup; caps, typed failure policy with bounded backoff, and a working stop are contract, not configuration.
  - Connect every account and provider the user has to one Full Auto routing policy: multiple Claude, ChatGPT/Codex, Grok, and Cursor accounts (and local model lanes) registered as named isolated accounts, with the system smartly routing work across them — usage-maxing the fleet within provider terms, own-capacity-only.
  - Never halt unattended work on a limit the routing policy can route around: when a model tier, account, or provider hits a usage limit or errors mid-run, Full Auto automatically continues on the next admitted model, account, or provider lane, records the rotation in the run receipt, and surfaces the honest provider condition — a run stops only for owner stop, cap, objective completion, or typed policy block, never to wait for a human to acknowledge a limit.
  - Support same-thread cross-provider handoff as a first-class capability: a thread can switch provider lanes mid-conversation, the next provider receives bounded host-owned history, and the capability is proven by end-to-end acceptance tests in which provider A writes a recognizable fact and provider B continues the same visible thread using it — with named, sidebar-visible handoff test runs so the proof is inspectable in the product, not only in CI.
  - Guarantee active-run thread retention: a thread bound to an active Full Auto run is never evicted from host caches or rendered unopenable; cache policy must exempt active runs, and a continuation that addresses an unopenable thread is a typed defect class with an owner-visible reason, never a generic "conversation no longer exists."
  - Run the Full Auto iteration loop as product machinery: every run produces a bounded run report (objective, turns, dispositions, rotations, failures, evidence links); run transcripts are analyzable for improvement; and any failed run is reproducible as a replayable fixture run.
  - Carry the Guidance Module lineage in Full Auto's decision layer: a between-turn decision engine with a shared model of goal, state, budget, and environment across turns, confidence-gated next actions (low confidence waits or consults rather than acting), and hard deterministic guardrails that override soft guidance — budget-bound autonomy as the operating contract.
  - Keep account truth at a glance: the logged-in identity for every connected account visible without digging, per-account rate-limit burn-down rendered inline (the ticking usage counter), and optional API-key fallback lanes when subscription quotas exhaust.
  - Consume FastFollow as a standing work source: owner-configured upstream study, content-addressed study packets, gap assessments, and evidence-only candidate work feeding Full Auto when the issue backlog runs dry — never as a second authority mode; implementation still requires separate admission.
  - Route one-interface consolidation through Khala: the user's own subscriptions and accounts fanned out behind semantic (never keyword) routing, own-capacity-only, with per-request routing disclosure of the effective backend.
  - Pursue local-inference fallback: a local model lane on Apple silicon for cheap operations and offline continuity, so losing the network degrades capability instead of killing the surface.
  - Provide file checkpoints independent of Git with conflict-aware, three-mode (conversation, code, both), staged two-phase rewind that discloses reversible versus irreversible effects before commit.
  - Treat worktrees as durable engine resources with outcome-sensitive lifecycle (auto-remove unchanged, retain changed, refuse dirty or unpushed) and cleanup receipts; render delivery states (changes_produced, reviewed, committed, pushed, merged, accepted) distinct from turn completion, and present confidence tiers (draft, verified, reviewed, bonded) as distinct visible states of a Work Unit.
  - Compile named execution profiles (projection-only, workspace-bounded, networked-build, isolated-guest, owner-local danger mode, managed cloud) to OS enforcement; owner-local danger mode is explicit and visually persistent, never a default; unavailable containment fails closed.
  - Provide a hermetic execution profile that excludes every ambient input unless explicitly admitted and emits a complete admitted-input manifest.
  - Drive palette, keyboard, menus, slash commands, and model-proposed actions from one central command registry of stable command IDs with typed schemas, capability requirements, approval flags, idempotency, and redaction class; there is no code path where prose becomes execution, and the app never surfaces an operational directive (command, script, path) that was not read from the system of record.
  - Ship the Fleet workspace: connected accounts and harnesses in one interface with capacity shown as quantities (available, busy, queued), readiness lights lit only from decoded fresh receipts ("no receipt means no light"), read-only account state on the fleet page, and explicit harness choice on new work.
  - Honor don't-break-userspace as a release law: once a workflow works for users it keeps working; startup predictability, hotkey stability, and existing UX contracts are regression-gated, and no update silently reshuffles a working surface.
  - Provide one-click complete data export: a user can export all local state (threads, specs, receipts, settings) and leave; no lock-in move ever ships.
  - Ship best-of-N and plan-first execution as typed fan-out with per-child receipts and an explicit comparison record, and review fan-out compiled into assurance manifests whose acceptance decision sits outside the reviewed party.
  - Provide a terminal renderer family (full-screen, native-scrollback, headless) over one typed transcript projection, gated by emulator-backed PTY test matrices and frame-time baselines.
  - Package for the full six-target matrix (macOS, Windows, Linux, x64 and arm64) under a signed release-set manifest, a component compatibility ledger across shell, engine, renderer, and extensions, retained-slot rollback, no downgrade flags, and coordinated drain-before-update of live engine work; macOS signed and notarized first.
  - Build in-app community feedback: bug reporting and contribution flows from inside the app, wired to the strict-bug and behavior-contract intake paths.
out:
  - The desktop app does not load a remotely deployed website as its renderer, and web deployment authority never becomes desktop code authority.
  - No browser-runtime fork or proprietary Chromium; capability ships through narrow typed native modules on stock Electron.
  - No terminal TUI as a product surface; the terminal is a workbench tool inside the app, not the app.
  - Steering a running Full Auto session is out of this revision: the first contract is hardcore press-play-and-walk-away; a steerable Full Auto is a later, separately admitted mode.
  - In-app rate-limit reset triggering is deliberately withheld until the surrounding reliability contracts are proven; accidentally consuming a reset at full quota is an unacceptable failure mode.
  - Voice is the next contract, not this one: the one-toggle bidirectional voice stream (talk and watch the right actions happen), with possible free and paid tiers, ships as its own spec after the workroom core holds.
  - Multiplayer capacity contribution and the Pylon provider/earning mode (the Go Online button, wallet, and seed-derived Nostr/Lightning identity of the one-app cockpit thesis) are separate later contracts; contribution is off by default and there is no hidden background compute.
  - No ambient screen-recording or inferred-memory capability in this revision; any future ambient memory is a separate spec bound to private-by-construction custody.
  - No computer-use or OS-automation capability in this revision.
  - No additional provider integrations before one provider is complete under the eleven-predicate closure bar (known, decoded, owned, retained, projected, presented, authorized, recovered, fast, receipted, shipped).
  - No third-party plugin execution inside the trusted engine or shell process; extensions run under declared isolation profiles with signatures and receipts or they do not run.
cut:
  - CUT-DSK-01: Light theme and theme switching are cut; the single dark Khala theme remains the only visual identity.
  - CUT-DSK-02: Simultaneous token-by-token mirroring of every child agent is cut; the roster shows live typed lifecycle and any child transcript opens on demand.
  - CUT-DSK-03: Editor-first IDE ambitions are cut; the workbench deepens supervision and review, not general text editing.
  - CUT-DSK-04: A public plugin marketplace is cut from this revision; signed ingestion of existing open formats (MCP, MCPB, skills) with provenance comes first.
  - CUT-DSK-05: Sub-brands are cut; the desktop app is "OpenAgents," not a separately branded client.
  - CUT-DSK-06: A chat box inside the Full Auto run view is cut; the run view is read-only with Play, Pause, and Stop only.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user sends a follow-up while a turn is running, the composer requires an explicit queue-or-steer choice, and the transcript later shows the input's admission, promotion, execution, and terminal states as distinct facts.
- id: AC-2
  criterion: When a user opens any run's detail view, they see both the authority manifest (what policy admitted) and the execution receipt (what containment actually enforced); requested and effective enforcement are never merged into one indicator.
- id: AC-3
  criterion: When a run is started under a profile whose OS enforcement cannot be represented on the current platform, the run refuses to start with a typed reason instead of degrading silently.
- id: AC-4
  criterion: When a session spawns child agents, the agent tree shows every retained child with live lifecycle state and latest activity, an in-flight spawn is visible before it resolves, any child's full transcript opens within two interactions, and unlinked history renders as an explicit gap node.
- id: AC-5
  criterion: When a user rewinds to a prior turn, the app stages the restore, discloses reversible versus irreversible effects and externally modified files, and only applies on explicit commit, emitting a rewind receipt.
- id: AC-6
  criterion: When agent work in a worktree finishes, unchanged worktrees are auto-removed, changed worktrees are retained, dirty or unpushed worktrees are refused for cleanup, and each outcome emits a cleanup receipt; the Work Unit's delivery state is visible and distinct from turn completion.
- id: AC-7
  criterion: When the packaged app is inspected by release tests, Electron fuse oracles pass (RunAsNode, NODE_OPTIONS, and inspect disabled; ASAR integrity on), and every IPC channel rejects messages that fail schema decoding or sender validation.
- id: AC-8
  criterion: When a transcript holds ten thousand or more items, scrolling and turn navigation stay within the checked-in p95 frame-time baselines, and those baselines gate release.
- id: AC-9
  criterion: When an update is offered, the client verifies the signed release-set manifest against the pinned key before install, refuses version downgrades, retains a rollback slot proven by test, and drains live engine work before relaunch.
- id: AC-10
  criterion: When a run executes under the hermetic profile, the emitted manifest lists every admitted input source, and no ambient instruction, hook, plugin, or memory outside that manifest influenced the run.
- id: AC-11
  criterion: When the same command ID is invoked from the palette, a keyboard shortcut, a menu, and a model-proposed action, all four paths produce identical typed outcome records.
- id: AC-12
  criterion: When a user binds a chat or command to a hotkey, that binding never reshuffles as chats are created, reordered, or closed; the bound target stays stable until the user rebinds it.
- id: AC-13
  criterion: When a user switches between chats mid-composition, draft text, queued inputs, attachments, and stop/steer controls each remain keyed to their own thread; no composer or queue state leaks across chats, and a dispatched queue item never replays.
- id: AC-14
  criterion: When any assistant message renders, its metadata shows the effective model, provider, and account from observed execution events; a turn whose effective model differs from the requested one is displayed as such and is never streamed under the requested model's name.
- id: AC-15
  criterion: When the Fleet workspace shows a readiness or status light, that light derives from a decoded fresh receipt; absent or stale evidence renders as unknown, never as green.
- id: AC-16
  criterion: When an agent turn produces reasoning, the reasoning is visible in the main trace expanded by default, tool activity is described by what actually ran, and current usage is visible inline without leaving the main view.
- id: AC-17
  criterion: When a user authors a ProductSpec in the workroom and accepts its plan, the resulting work packets each retain exact spec revision, criterion ID, and terminal evidence links, and no packet is displayed as completed without its matching terminal outcome and review post-image.
- id: AC-18
  criterion: When the app restarts with work in flight, each affected session either resumes through typed recovery or terminates with an owner-visible typed reason; restart attribution is never invented, and the app discloses the running build, version, and source on demand.
- id: AC-19
  criterion: When a selected account is exhausted or rate-limited, the surfaced error names the real provider condition, and where another connected account is ready, work fails over to it under the recorded execution profile with the rotation visible in the receipt.
- id: AC-20
  criterion: When a user launches Full Auto from its dedicated action, setup asks once for objective, workspace, and provider/account routing policy, then presents a read-only run view with explicit Play, Pause, and Stop and no ordinary composer; pause, resume, and stop are durable typed transitions that survive an app restart.
- id: AC-21
  criterion: When a model tier, account, or provider hits a usage limit or fails during a Full Auto run, the run continues automatically on the next admitted model, account, or provider lane per the routing policy with the rotation recorded in the run receipt; the run never halts to await human acknowledgment of a limit, and it terminates only by owner stop, cap, objective completion, or typed policy block.
- id: AC-22
  criterion: When a thread's provider lane is switched mid-conversation, the next provider receives bounded host-owned history and continues in the same visible thread, and an end-to-end acceptance test proves a recognizable fact written by provider A is used by provider B in that same thread, with named handoff test runs visible in the sidebar.
- id: AC-23
  criterion: When host caches face pressure, a thread bound to an active Full Auto run is never evicted or rendered unopenable; a continuation that addresses an unopenable thread surfaces as a typed defect with an owner-visible reason, never a generic conversation-not-found error.
- id: AC-24
  criterion: When a Full Auto run ends for any reason, it produces a bounded run report covering objective, turns, dispositions, provider/account rotations, failures, and evidence links, and any failed run can be reproduced as a replayable fixture run.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: unattended_turn_share
  target: ">= 40% of desktop turns run in background or Full Auto modes without manual per-turn approval"
  target_status: provisional
  target_owner: "owner"
  window: within 60 days of general availability
- id: SM-2
  metric: median_concurrent_sessions_per_active_user
  target: ">= 3"
  target_status: provisional
  target_owner: "owner"
  window: within 60 days of general availability
- id: SM-3
  metric: receipt_or_manifest_inspections_per_weekly_active_user
  target: ">= 5 per week"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-4
  metric: forced_restart_session_recovery_rate
  target: ">= 99.5% of sessions resume intact with honest gap accounting"
  target_status: committed
  window: rolling 30 days from first packaged release
- id: SM-5
  metric: verified_auto_update_success_rate
  target: ">= 99% of update attempts verify, apply, and relaunch without user intervention"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days per release channel
- id: SM-6
  metric: first_launch_to_first_packet_activation
  target: "opted-in first launches create or open a valid spec, accept a plan, and start its first criterion within 15 minutes"
  target_status: committed
  window: rolling 30 days from workroom availability
- id: SM-7
  metric: no_fallback_task_completion
  target: "qualifying agent tasks reach one reviewed diff and terminal outcome without the user opening another agent interface"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days from workroom availability
- id: SM-8
  metric: second_durable_task_within_seven_days
  target: "activated developers start a second durably admitted task within seven days"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days from workroom availability
- id: SM-9
  metric: false_green_completion_incidents
  target: "zero confirmed incidents where the workroom showed completed without the matching terminal outcome and review post-image"
  target_status: committed
  window: rolling 30 days, continuously
- id: SM-10
  metric: full_auto_typed_termination_rate
  target: ">= 99% of Full Auto runs end by owner stop, cap, objective completion, or typed policy block — never by unhandled error or a limit awaiting human acknowledgment"
  target_status: committed
  window: rolling 30 days from Full Auto run-mode availability
- id: SM-11
  metric: multi_day_afk_dogfood_runs
  target: "at least one owner-AFK Full Auto run of 24-48 hours completes per release cycle with a reviewable run report and accepted work product"
  target_status: provisional
  target_owner: "owner"
  window: per release cycle from Full Auto run-mode availability
```

## Solution

One long-lived local engine supervisor owns every conversation; Desktop is
its first and deepest client. The engine seam is a generated Effect Schema
protocol; all clients — Desktop renderer, terminal, mobile, web — consume the
same projections. Authority is compiled, not narrated: profiles become OS
enforcement, and every run leaves the manifest/receipt pair. Identity is
observed, not asserted: every message carries the effective model and account
that actually ran. Recovery is local-first: append-before-side-effect logs,
checkpoints, staged rewind, conservative worktree retention, restart-resilient
sessions with build disclosure. The workroom's native unit is the ProductSpec
— authored conversationally, decomposed into criterion-addressed packets,
executed by the fleet, closed only on evidence — with the Assurance layer
resident so green means what it claims.

Full Auto is the flagship built on that substrate: a dedicated run-state
machine (play/pause/stop, durable, restart-survivable) over the user's entire
connected fleet — every Claude, ChatGPT, Grok, and Cursor account plus local
lanes — with a between-turn decision layer in the Guidance Module lineage
(shared goal/state/budget/environment, confidence-gated actions, hard
guardrails over soft guidance), automatic rotation through limits, same-thread
cross-provider handoff with bounded host-owned history, active-run thread
retention, and a run-report/replayable-fixture iteration loop that turns every
failure into a regression. FastFollow keeps its work queue full. Distribution
is a signed transaction verified fail-closed on the client. The synthesis
essay, transcripts 200–255, and the episode-256 Full Auto audit record the
evidence and the owner's stated direction for each element.

## Strategic Positioning

Supervision breadth is converging table stakes across Cursor, T3 Code,
Factory, Amp, and OpenCode; "it's pretty easy to get to parity with the good
open source harnesses." The unclaimed lane is the trust half — explicit
authority, effective-containment truth, receipts, portable sessions, signed
provenance — plus the flagship no incumbent lab will build: reliable
AFK-grade Full Auto across all of a user's accounts and providers. The labs
assume a user "sitting there vibing"; their loops are brittle to their own
harness, halt at their own limits, and cannot span competitors' accounts.
OpenAgents routes across all of them — "they're not going to throw them all
into a super harness and route between them, but OpenAgents will" — and the
owner judges this "important enough of an unserved need to build the whole
short-term roadmap of the company around." The trust layer is what makes
walking away rational: run receipts, typed termination, and the clearing-
layer doctrine that the real product "is the receipt that proves the wiring
worked." The audience is deliberate: power users who like video games —
clicky, fast, dense, predictable — with an opinionated simple core for
newcomers. Don't-break-userspace binds every release; one-click export means
no lock-in, ever. Everything ships open source; being copied is accepted.

## Risks

- Trust surfaces could read as friction if receipts and manifests are not
  quiet by default; the design must keep them one gesture away, not in the
  path of every action.
- Fail-closed containment on platforms with weak sandbox primitives may block
  workflows competitors allow; the owner-local danger mode must remain an
  explicit, visible escape hatch without becoming a default.
- Full Auto is only trustworthy after the bug-bash defect classes (queue
  replay, composer leaks, restart ambiguity, active-thread eviction) are
  closed by contract; shipping it earlier converts every defect into an
  unattended failure discovered a day later.
- Cross-provider handoff is currently plumbing-present, experience-unverified;
  claiming it before the end-to-end acceptance tests exist would be exactly
  the false-green failure the product exists to eliminate.
- Multi-account failover must stay within provider terms; own-capacity-only
  and no-resale-of-subscription-inference are standing constraints.
- The six-target matrix and owned-runner release chain are operationally
  expensive; sequencing must not let breadth starve depth.
- Rendering the full agent tree live at fleet scale has real performance
  risk; the perf-baseline gates exist to keep it honest.

## Open Questions

- Which two or three receipt kinds are surfaced most prominently in the
  default UI before users opt into the rest?
- Where exactly does the Full Auto launch affordance live — beside New
  Session in the sidebar, in the command palette, or both — and what does
  the run view show while paused?
- When steering of a running Full Auto eventually ships, is it a distinct
  "supervised run" mode rather than a composer inside the run view?
- Does the hermetic profile ship as a developer feature first or as the
  substrate for reproducible support bundles?
- Where do the cost-efficiency dials (speed versus token burn across
  connected accounts) live — per-run, per-spec, or global?
- When does the local-model lane graduate from fallback to a routable
  first-class capacity source?

## Related Artifacts

- Roadmap reconciliation and AC-by-AC gap crosswalk:
  `docs/sol/MASTER_ROADMAP.md` revision 119 and
  `docs/fable/2026-07-17-surface-vision-gap-analysis-and-roadmap.md`
- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Full teardown catalog and evidence conventions: `docs/teardowns/README.md`
- Full Auto implementation authority: `specs/desktop/full-auto.product-spec.md`
  (the implementation-level Full Auto spec; this surface spec states the
  product vision above it) and
  `docs/fable/2026-07-17-full-auto-implementation-audit.md` (the corrected
  post-incident audit from the episode-256 session)
- Transcript sources: `docs/transcripts/200.md`–`209.md` (Guidance Module
  lineage in `206.md`, identity/wallet in `207.md`, don't-break-userspace in
  `204.md`), `docs/transcripts/214.md` + `225.md` (account visibility,
  hybrid routing, GUI-over-TUI), `docs/transcripts/228.md` (walk-away
  contract), `docs/transcripts/237.md` (clearing layer, confidence tiers),
  `docs/transcripts/244.md`–`255.md` (one-interface consolidation, UX
  contracts, workroom MVP, Assurance, bug bash, Full Auto/FastFollow), and
  the episode-256 draft transcript (Full Auto audit and roadmap)
- Sibling surface specs: `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`,
  `specs/web/openagents-com-trust-surface.product-spec.md`
- Desktop implementation-state authorities remain
  `apps/openagents-desktop/GUARANTEES.md` and `docs/sol/MASTER_ROADMAP.md`.

## Owner Gates

- Sign-off on the named execution-profile set and the visual treatment of
  owner-local danger mode.
- Sign-off on the Full Auto dedicated-mode UX (launch placement, read-only
  run view, play/pause/stop semantics) as the replacement for the composer
  toggle.
- The Full Auto release gate: owner confirmation that the queue, isolation,
  restart, thread-retention, and failover contracts hold — including at
  least one multi-day AFK dogfood run — before unattended operation is
  promoted in copy.
- Platform signing identities and the owned six-target runner fleet for the
  release matrix.
- Release channel policy (stable/RC identity split, rollback windows).
- In-app rate-limit reset triggering stays withheld until the owner
  explicitly admits it.
- Any future ambient-memory capability requires a separate owner-admitted
  spec; voice, multiplayer contribution, and the Pylon provider/earning mode
  ship as their own owner-admitted contracts.

## Receipts

Planned receipt kinds this surface emits or renders: execution receipts,
authority manifests, delivery receipts, worktree cleanup receipts, rewind
receipts, update/rollback receipts, hermetic admitted-input manifests,
fan-out comparison records, account/model/provider rotation records, Full
Auto run reports, handoff acceptance-test records, packet evidence links,
and assurance receipts rendered from the resident proof layer. This section
plans kinds; the evidence ledger lives in the assurance and receipt systems,
not in this spec.

## Promise Links

None yet. Public claims derived from this spec (trust layer, signed updates,
recovery guarantees, Full Auto AFK reliability, cross-provider handoff,
zero-false-green) must land in the promise registry with verification gates
before they appear in copy; UX Behavior Contracts carry the micro-scale
equivalents in the same discipline.
