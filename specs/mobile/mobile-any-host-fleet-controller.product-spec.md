---
spec_format_version: "0.1"
title: "OpenAgents Mobile: Any-Host Agent Fleet Controller"
artifact_type: "prd"
spec_revision: 3
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-17T23:25:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-mobile/"
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
  openagents_source_transcripts: "docs/transcripts/200.md through docs/transcripts/255.md plus the episode-256 draft transcript (mobile remote-control doctrine, fleet supervision, overnight runs, UI-first operations, Full Auto AFK supervision)"
  openagents_revision_3_note: "Rev 3 adds Full Auto run supervision as a first-class mobile object per the episode-256 draft transcript: active runs listed with live run state, remote Play/Pause/Stop as typed durable commands, rotation/failure visibility, and run reports in the attention inbox — the AFK scenario (owner away for a day or two while runs continue) is the anchor journey this surface exists for. Also folds in the back-catalog founding texts: episode 225 ('I want to be able to talk to my Forge, talk to my Probes when I'm at the store... carrying around my laptop like a jackass') and episode 228's untethered North Star ('say what the software you want is... go live your life and then come back'), plus episode 200's portfolio-manager frame (people supervise fleets, set policies, allocate budgets, review outcomes)."
  openagents_admission_status: "roadmap-reconciled by docs/sol/MASTER_ROADMAP.md revision 119 as surface vision and target intent; implementation dispatch remains limited to the T3 mobile accepted packet ledger or another exact admitted issue/plan, with owner gates and proof rungs intact; closed #8980 is bounded first-screen/simulator evidence, not continuing dispatch authority"
  openagents_revision_2_note: "Rev 2 folds in founder-stated direction from transcripts 238-255: the phone-as-remote-control doctrine with exactly-one-outcome command resolution over intermittent connectivity (255); supervision-before-authoring sequencing (253-notes); the overnight-fleet morning-review journey as the anchor use case (246, 250, 255); fleet capacity shown as quantities with evidence-gated readiness inherited from the Desktop Fleet laws (250); per-message effective-identity metadata on mobile (250, 251-notes); UI-first operations — enrollment, visibility, and policy as screens and buttons, never CLI runbooks (255); no desktop token on the phone and no cloud-canonical transcripts (255); counters, earnings, and referral accruals as receipted projections whose public claims follow promise-registry states (243, 244, 245)."
  openagents_sibling_specs: "specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md"
---

## Problem

An owner running parallel coding agents is away from the desk exactly when
the fleet needs them: an approval blocks a turn, a question stalls a thread,
a finished change needs review and push, a Full Auto run rotates onto its
last healthy account. The founding statement of the need is from episode
225: "I want to be able to talk to my Forge, talk to my Probes when I'm at
the store. Right now I'm carrying around my laptop like a jackass letting
Claude Code run" — and the episode-256 stakes make it concrete: an owner
with a baby due any day needs agents to keep going AFK "for a day or two,"
which means the phone must be able to check on, pause, and redirect
unattended runs without a laptop. The transcripts make the stakes
concrete: overnight fleet runs across six connected accounts racked up
billions of tokens — and also produced duplicate PRs, agents stepping on each
other, and runs that "crapped out after 30 minutes because you hit some
limit," discovered only the next morning. Today's mobile options are either
chat companions that cannot control real work, or single-vendor controllers
with structural trust defects: T3 Code's mobile app proves full controller
breadth (multi-environment pairing, diff review, Git controls, native
terminal, offline outbox) but binds threads to single environments, routes
through hosted third-party relay infrastructure, and executes with no
containment or receipts; Cursor's iOS app supervises only Cursor's cloud.
Nobody ships a phone surface that can supervise and steer agents on any of
the owner's hosts — with the phone never executing anything, never holding
raw credentials, and every consequential action leaving a durable receipt.

## Hypothesis

If OpenAgents mobile is the fleet's remote control — a complete controller
client of the same typed engine protocol as Desktop, over the same durable
session refs, where "a command sent from a subway tunnel resolves to exactly
one outcome" — with an any-host environment directory, a durable
per-environment offline outbox, an attention inbox that pins what needs the
owner, full workbench modes, and what no competitor has (portable session
identity across hosts, scoped revocable capability grants, receipts on every
consequential action), then owners will keep materially more unattended work
running: approvals get answered in minutes from anywhere, overnight fleet
runs get caught and steered before they waste the night, and delegating
away-from-desk work becomes rational because supervision stops being
desk-bound.

## Scope

```productspec-scope
in:
  - Operate strictly as a remote control: the phone issues the same typed commands as Desktop and web against the same durable session refs (steer, queue, stop, approve, answer), never executes agent work, holds no desktop token or raw provider credential, and never receives raw filesystem paths.
  - Resolve every command to exactly one outcome across intermittent connectivity: durable admission acknowledgements, worker epochs, and ordered replay guarantee that a command sent from a dead zone lands once or fails visibly, never twice and never silently.
  - Ship an any-host environment directory as the first product layer: discovery, QR and manual pairing that exchanges a bootstrap credential for a scoped session credential, cached offline environment truth, and reachability presented as classed hints rather than proof.
  - Route remote reachability through owned relay infrastructure that is end-to-end encrypted and grants reachability without authorization; the client still presents its normal credential to the environment.
  - Bind every environment-facing grant to DPoP-style scope-limited, revocable capability tokens stored in the device vault; every consequential remote action records a durable outcome and receipt.
  - Make overnight-fleet supervision the anchor journey: see every running workstream across hosts and accounts, catch a run that stalled on an exhausted account or a blocking question, steer or re-dispatch it from the phone, and arrive at a morning review of what completed, what needs attention, and what evidence backs each claim.
  - Surface Full Auto runs as first-class supervised objects: every active run on any connected host listed with its live run state (playing, paused, blocked), current provider/account lane, rotation and failure history, and elapsed budget; Pause, Resume, and Stop issued from the phone as typed durable commands with receipts; and the bounded run report delivered to the attention inbox when a run ends.
  - Show fleet capacity as quantities, not presence: connected accounts and Pylons with available, busy, and queued counts, readiness lights lit only from decoded fresh receipts ("no receipt means no light"), and honest provider-condition errors (exhausted, rate-limited) rather than generic failures.
  - Display effective execution identity on mobile exactly as on Desktop: every message's metadata shows the observed effective model, provider, and account, never an inference from the requested brand.
  - Treat the portable session as the stable object: a session moves owner-local to managed cloud and back through quiesce, checkpoint, detach, attach, resume, and failback verbs, with exclusive attachment generations so exactly one host executes, secret-free checkpoints, and source-cleanup receipts.
  - Ship the workbench mode graph, supervision-first: Attention, Recent, Repositories, and Hosts entry points; per-session Thread, Files, Changes, Terminal, Preview, and Artifacts modes; routes and sheets on phone, list-plus-detail-plus-inspector on tablet, from one adaptive app.
  - Provide a durable per-environment offline outbox built on durable admission: commands queued with client-chosen idempotent IDs, admission acknowledged before the UI shows accepted, explicit steer-versus-queue choice surfaced, and the queue visible, editable, and cancellable.
  - Ship attention as a product: an inbox where actionable items (approvals, questions, blockers) are pinned and never collapse; privacy-generic push payloads that revalidate at open and deep-link to the exact session; lock-screen presence for running work; share targets and quick actions as controller citizens; notification state is never completion authority.
  - Render the complete agent-graph projection with the same typed density rules as Desktop: full roster, live child lifecycle, drill-down into child transcripts, explicit gap accounting.
  - Make Changes writeback safe by construction: no force push, exact post-image receipts on every mutation.
  - Make every operation UI-first: enrollment, environment pairing, visibility modes, policy, and grant revocation are screens and buttons — never CLI runbooks — while remaining available programmatically for agents.
  - Project counters and earnings as receipted facts: tokens served, work verified, sats earned, and referral accruals render as projections reconcilable to exact receipted rows, with any pays-you economics copy strictly following the promise registry's recorded states.
  - Add voice as a session-neutral control channel over a sequenced, acknowledged dictation transport, layered after the controller core is complete.
  - Verify against disposable real servers with seeded deterministic state across device geometries, plus fault injection (network loss, token revocation, host restart), keeping fixture, deployed, and physical-device evidence as separate claims.
out:
  - No on-device agent execution or local model serving in this spec.
  - No desktop tokens, raw provider credentials, raw filesystem paths, or hidden danger modes on the phone.
  - No third-party hosted relay, tunnel, identity, or build/update dependencies; infrastructure is owned.
  - No web-wrapper shell and no second UI tree; one typed component contract renders both phone and tablet.
  - Notification state is never completion authority; only durable outcomes and receipts complete an action in the UI.
  - Mobile authoring of ProductSpecs and full workroom authoring flows are deferred; supervision precedes authoring on this surface, per the multiplayer contract's explicit exclusion.
cut:
  - CUT-MOB-01: Pixel-streaming remote desktop is cut; the phone renders typed projections, not screen mirrors.
  - CUT-MOB-02: General on-phone code editing is cut to bounded review comments and small staged edits; full editing remains a desktop concern.
  - CUT-MOB-03: A separate tablet app is cut; one adaptive application serves both geometries.
  - CUT-MOB-04: Voice-first ambient assistant framing is cut from this revision; voice ships as a control channel for existing sessions.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user pairs a clean install to a new host by QR, the bootstrap credential is exchanged for a scoped session credential, the vault stores only scoped revocable tokens, and no raw long-lived credential ever persists on the device.
- id: AC-2
  criterion: When a user approves a tool call, steers a turn, or pushes a change from the phone, the action records a durable outcome with a receipt, and the UI completes only from that outcome, never from a notification alone.
- id: AC-3
  criterion: When the device is offline or on intermittent connectivity, commands queue in a visible, editable, cancellable per-environment outbox and resolve to exactly one outcome on reconnect via idempotent IDs, worker epochs, and ordered replay, with admitted and pending states rendered distinctly.
- id: AC-4
  criterion: When a session is moved from an owner-local host to a managed host and back, exactly one attachment generation is executing at every moment, the checkpoint contains no secrets, and the source host's cleanup is receipted.
- id: AC-5
  criterion: When actionable items exist (approvals, questions, blockers), the attention inbox pins them uncollapsed, and opening a push notification deep-links to the exact session state revalidated from the server.
- id: AC-6
  criterion: When a user reviews changes on the phone, the writeback path refuses force-push and displays the exact post-image receipt after any accepted mutation.
- id: AC-7
  criterion: When a session has child agents, the phone renders the complete roster with live lifecycle, supports drill-down into any child transcript, and renders missing history as explicit gap nodes.
- id: AC-8
  criterion: When the fault-injection suite severs the network, revokes a token, or restarts a host mid-action, the app shows honest degraded states (transient gap, unreachable, revoked) and never fabricates success.
- id: AC-9
  criterion: When the release verification harness runs, screenshot matrices against disposable real servers pass across phone and tablet geometries, and physical-device evidence is recorded separately from simulator evidence.
- id: AC-10
  criterion: When the fleet view renders connected accounts and Pylons, capacity appears as quantities (available, busy, queued) backed by decoded fresh receipts; absent or stale evidence renders as unknown, never as ready, and provider exhaustion surfaces as the named provider condition.
- id: AC-11
  criterion: When any assistant message is inspected on mobile, its metadata shows the observed effective model, provider, and account for that turn, matching the Desktop projection of the same session.
- id: AC-12
  criterion: When a user enrolls a device, changes a visibility mode, revokes a grant, or adjusts policy, the complete flow is achievable through screens and buttons with no terminal command required.
- id: AC-13
  criterion: When a Full Auto run is active on any connected host, the phone lists it with live run state, current provider/account lane, and rotation history; Pause, Resume, and Stop from the phone are typed durable commands whose outcomes are receipted; and when the run ends its bounded run report is retrievable from the attention inbox.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: mobile_weekly_active_supervisors_as_share_of_desktop_actives
  target: ">= 50%"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-2
  metric: median_push_to_decision_latency_for_approvals
  target: "<= 2 minutes"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days after push attention ships
- id: SM-3
  metric: share_of_fleet_approvals_handled_on_mobile
  target: ">= 40%"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of general availability
- id: SM-4
  metric: outbox_replay_exactly_once_success_rate
  target: ">= 99.9%"
  target_status: committed
  window: rolling 30 days from outbox availability
- id: SM-5
  metric: portable_session_moves_initiated_from_mobile_per_week
  target: "baseline established, then >= 1 per weekly active supervisor"
  target_status: provisional
  target_owner: "owner"
  window: within 120 days of portable movement shipping
- id: SM-6
  metric: overnight_run_interventions_from_mobile
  target: "baseline established: share of overnight fleet runs receiving a corrective mobile action (steer, re-dispatch, approval) before morning"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of fleet supervision shipping
```

## Solution

The phone is the fleet's remote control, not its runtime. One Effect Native
application model renders phone and tablet; all state arrives as typed
projections of the same engine protocol Desktop consumes, over Sync and the
owned relay. The same session refs resolve everywhere: steer, queue, and stop
from the phone are the same typed intents Desktop dispatches, with durable
admission and replay making each one land exactly once. The environment
directory and pairing come first; portable session movement is the substrate
that makes "which host" a detail rather than an identity; the workbench
modes, outbox, and attention inbox make supervision complete; receipts make
it trustworthy; and the overnight-fleet morning review is the journey the
whole surface is tuned for. T3 Code's mobile app is the breadth bar; the
trust layer is the difference.

## Strategic Positioning

Competitors prove demand for mobile supervision (T3's full controller,
Cursor's remote control, Amp's mobile thread control) but every one couples
it to a single vendor cloud or an environment-local thread model. The
founder's stated payoff — "when I've gotten mobile working, it's been
amazing — controlling this kind of stuff from a mobile app" — depends on the
one-interface consolidation Desktop provides: because the engine holds all
accounts and capacity, the phone can be a thin, complete controller over all
of it. Any-host control plus portable session identity plus receipts is the
position no one else can copy without rebuilding their custody model.

## Risks

- Controller breadth without the desktop-grade engine work lands as a hollow
  shell; sequencing must keep mobile behind the protocol and portability
  substrates it projects.
- Owned relay operations (E2EE courier, device revocation) are new
  infrastructure surface; the reachability-not-authorization boundary must
  hold under audit.
- Push-attention latency targets depend on platform notification behavior
  the app does not control; measure honestly before committing targets.
- Earnings and counter projections must never outrun the promise registry;
  pays-you copy on a store-distributed app is a public claim with review
  consequences.
- Voice adds transcription privacy surface; it stays behind the controller
  core and its own custody review.

## Open Questions

- What is the minimum credential-recovery story when a phone is lost —
  revoke-all from any other authenticated surface, plus what re-pairing
  proof?
- Which two workbench modes ship first after Thread (Changes and Files, or
  Changes and Terminal)?
- Do Live Activities ship at initial GA or after push attention proves
  reliable?
- When supervision is solid, what is the first authoring affordance worth
  adding — spec review sign-off, or full conversational spec authoring?

## Related Artifacts

- Roadmap reconciliation and AC-by-AC gap crosswalk:
  `docs/sol/MASTER_ROADMAP.md` revision 119 and
  `docs/fable/2026-07-17-surface-vision-gap-analysis-and-roadmap.md`
- Active bounded implementation authority:
  `docs/sol/2026-07-17-t3-code-mobile-full-parity-accepted-plan.md`. Closed
  #8980 and children #8981/#8982 are bounded first-screen/simulator evidence,
  not full-spec closure or continuing dispatch authority.
- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Controller-parity evidence: `docs/teardowns/2026-07-17-t3-code-mobile-app-teardown.md`,
  `docs/teardowns/2026-07-17-t3-code-openagents-mobile-controller-gap-analysis.md`
- Transcript sources: `docs/transcripts/225.md` (founding mobile text) and
  `docs/transcripts/228.md` (untethered North Star), `docs/transcripts/200.md`
  (fleet portfolio-manager frame), `docs/transcripts/244.md` (mobile-control
  payoff, one-interface consolidation), `docs/transcripts/246.md` +
  `docs/transcripts/250.md` (overnight fleet runs, evidence-gated fleet
  truth), `docs/transcripts/253-notes.md` (supervision-before-authoring),
  `docs/transcripts/255.md` (remote-control doctrine, exactly-one-outcome,
  UI-first operations), and the episode-256 draft transcript (Full Auto AFK
  supervision)
- Sibling surface specs: `specs/desktop/desktop-trust-complete-workbench.product-spec.md`,
  `specs/web/openagents-com-trust-surface.product-spec.md`
- Portable-session intent: `specs/openagents/portable-coding-sessions.product-spec.md`

## Owner Gates

- Approval of the owned relay design and its operational cost before any
  hosted reachability ships.
- App Store distribution decisions (TestFlight cohorts, store listing,
  release cadence) remain owner actions.
- Push-notification entitlement and APNs key management.
- Sign-off on the lost-device revocation flow before scoped tokens ship
  broadly.
- Any earnings, payout, or referral-accrual display requires the matching
  promise-registry state and settlement evidence before it renders as more
  than a receipted projection.

## Receipts

Planned receipt kinds this surface renders or triggers: remote-action
outcome receipts, writeback post-image receipts, session-movement and
source-cleanup receipts, grant issuance and revocation receipts, outbox
replay records, account-rotation records surfaced from the engine, and
earnings/counter projections reconcilable to exact rows. This section plans
kinds; evidence lives in the receipt systems, not in this spec.

## Promise Links

None yet. Public claims derived from this spec (any-host control, phone
never executes, exactly-once replay, earnings displays) must land in the
promise registry with verification gates before they appear in copy; the
pays-you economics remain `planned`-state promises until settlement evidence
exists.
