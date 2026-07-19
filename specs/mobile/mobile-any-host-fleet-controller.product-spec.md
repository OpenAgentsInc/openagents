---
spec_format_version: "0.1"
title: "OpenAgents Mobile: Any-Host Agent Fleet Controller"
artifact_type: "prd"
spec_revision: 7
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-19T00:00:00.000Z"
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
  openagents_revision_4_note: "Rev 4 makes Cursor mobile and Remote Control breadth a floor: launch and supervise local or background/cloud sessions, voice, notifications/live status, search/history, changes and artifacts, and workstation handback, while retaining the stronger any-host, no-credential, exactly-once command, portable-session, and receipt laws."
  openagents_revision_5_note: "Rev 5 incorporates MemoHarness strictly as safe remote supervision. Mobile may select only released compatible harness bundles/adaptation policies exposed by the host, and projects a run's base/effective bundle digests, adaptation state, frozen bank-snapshot ref, effective execution tuple, candidate/release state, and redacted receipts. It never receives raw experiences, prompts, transcripts, tool output, embeddings, retrieval queries or scores, secrets, credentials, or paths; never runs retrieval/optimization; and cannot mutate the bank, edit modules, promote candidates, or expand run authority."
  openagents_revision_6_note: "Rev 6 adds the mobile projection needed for a Zed-quality Desktop IDE: exact generation-bound project/worktree/file/document/proposal/evidence vocabulary; safe multi-root tree, file/symbol/search, Problems, diffs, test/task/artifact evidence, review/comment and Desktop handoff; and explicit stale/unavailable truth. Mobile remains a controller, not a Monaco/LSP/PTY/Git/native-helper host, and general code editing stays cut. Adds AC-21 through AC-24 and SM-9."
  openagents_revision_7_note: "Rev 7 binds mobile explicitly to IDE-14 of docs/ide/ROADMAP.md and the shared IDE release-rung vocabulary. Mobile renders the same bounded generation-fenced code evidence through shared Effect Schema DTOs and the Tokyo Night semantic review token subset, but it neither hosts a full editor nor owns or toggles Desktop Vim state. Raw type/interface duplicates are forbidden at the projection boundary; lifecycle stays in Effect Native services. Adds AC-25 through AC-27 and SM-10 through SM-11."
  openagents_ide_architecture: "docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md"
  openagents_ide_roadmap: "docs/ide/ROADMAP.md (mobile ownership: IDE-14; projection dependency: IDE-13)"
  openagents_ide_spec_crosswalk: "specs/IDE_ROADMAP_CROSSWALK.md"
  openagents_sibling_specs: "specs/openagents/cursor-capability-parity.product-spec.md, specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/web/openagents-com-trust-surface.product-spec.md"
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
  - Meet the mobile and Remote Control rows of `specs/openagents/cursor-capability-parity.product-spec.md`: no Cursor mobile workflow is omitted merely because OpenAgents supports more hosts or stronger custody.
  - Own the mobile projection and review half of `docs/ide/ROADMAP.md` IDE-14 over IDE-13 portable capabilities. The phone may report the source release rung and remaining gaps, but Files foundation, daily-use basic IDE, agent IDE, portable IDE platform, parity candidate, and full parity remain distinct Desktop/system claims rather than states mobile can promote.
  - Launch new bounded agent work and resume existing work on an explicitly selected owner-local, owner-managed, OpenAgents-managed, or compatible audited-provider target; managed cloud is an option, not the identity or mandatory custody layer.
  - Match Cursor-class mobile continuity with searchable session history, side conversations, voice input/control, privacy-generic push, lock-screen/live run status, questions and approvals, changes/diff/artifact review, rerun, and one-action workstation handback under the same session refs.
  - Operate strictly as a remote control: the phone issues the same typed commands as Desktop and web against the same durable session refs (steer, queue, stop, approve, answer), never executes agent work, holds no desktop token or raw provider credential, and never receives raw filesystem paths.
  - Resolve every command to exactly one outcome across intermittent connectivity: durable admission acknowledgements, worker epochs, and ordered replay guarantee that a command sent from a dead zone lands once or fails visibly, never twice and never silently.
  - Ship an any-host environment directory as the first product layer: discovery, QR and manual pairing that exchanges a bootstrap credential for a scoped session credential, cached offline environment truth, and reachability presented as classed hints rather than proof.
  - Route remote reachability through owned relay infrastructure that is end-to-end encrypted and grants reachability without authorization; the client still presents its normal credential to the environment.
  - Bind every environment-facing grant to DPoP-style scope-limited, revocable capability tokens stored in the device vault; every consequential remote action records a durable outcome and receipt.
  - Make overnight-fleet supervision the anchor journey: see every running workstream across hosts and accounts, catch a run that stalled on an exhausted account or a blocking question, steer or re-dispatch it from the phone, and arrive at a morning review of what completed, what needs attention, and what evidence backs each claim.
  - Surface Full Auto runs as first-class supervised objects: every active run on any connected host listed with its live run state (playing, paused, blocked), current provider/account lane, rotation and failure history, and elapsed budget; Pause, Resume, and Stop issued from the phone as typed durable commands with receipts; and the bounded run report delivered to the attention inbox when a run ends.
  - Project MemoHarness provenance for each Full Auto run through an explicit safe schema: released base and effective `HarnessPolicyBundle` digests, six dimension-policy refs, static/global/adapted classification, frozen experience-bank snapshot ref, adaptation state and receipt ref, compatibility, and the effective provider/model/harness/toolset/evaluator/environment tuple. Private evidence remains on its authoritative host or managed evidence service.
  - Let a mobile launch choose only among released compatible harness bundles and admitted adaptation policies returned by the selected host, behind Advanced disclosure. Mobile submits immutable refs as part of the same typed launch intent; the host performs compatibility, snapshot, retrieval, adaptation, and authority checks and may refuse without minting a run.
  - Render candidate, shadow/dogfood, released, active, rejected, and rolled-back harness states as supervision facts, including owner-action-needed release or privacy gates, without turning the phone into an optimizer, bank editor, or promotion authority.
  - Show fleet capacity as quantities, not presence: connected accounts and Pylons with available, busy, and queued counts, readiness lights lit only from decoded fresh receipts ("no receipt means no light"), and honest provider-condition errors (exhausted, rate-limited) rather than generic failures.
  - Display effective execution identity on mobile exactly as on Desktop: every message's metadata shows the observed effective model, provider, and account, never an inference from the requested brand.
  - Treat the portable session as the stable object: a session moves owner-local to managed cloud and back through quiesce, checkpoint, detach, attach, resume, and failback verbs, with exclusive attachment generations so exactly one host executes, secret-free checkpoints, and source-cleanup receipts.
  - Ship the workbench mode graph, supervision-first: Attention, Recent, Repositories, and Hosts entry points; per-session Thread, Files, Changes, Terminal, Preview, and Artifacts modes; routes and sheets on phone, list-plus-detail-plus-inspector on tablet, from one adaptive app.
  - Project the Zed-quality IDE graph safely: multi-root project/worktree display identity, bounded relative-ref Files tree, quick file and symbol lookup, workspace-search results, Problems, changed files, version-bound proposal diffs, comments, test/task outcomes, artifacts, and bounded syntax-highlighted excerpts all carry exact attachment/document/service/evidence generations and explicit stale, omitted, cached, degraded, revoked, or unavailable state.
  - Decode those project/review projections from the shared Effect Schema sources used by Desktop and web; derive mobile TypeScript types from those schemas, use constrained opaque refs, and reject raw mobile interfaces or handwritten unions as a second contract. Effect Native services own decode, cache, outbox, projection, and teardown lifecycle through scoped layers rather than component-local authority.
  - Render syntax, diff, diagnostic, selection, focus, and status evidence from the safe Tokyo Night semantic token subset supplied by the shared IDE theme contract so the initial review vocabulary is coherent across Desktop/mobile/web. Mobile does not ingest executable theme code, expose a theme marketplace, or claim the deferred Desktop light/high-contrast/system theme corpus has passed.
  - Make every code/evidence object continuable: Open on Desktop carries only opaque session/project/file/range/proposal/evidence refs; Desktop reauthorizes and resolves the current generation, while a missing historical generation opens a snapshot/diff or explicit unavailable state instead of a guessed current line.
  - Keep bounded review and mutation distinct. Mobile can inspect, comment, approve/reject where policy permits, rerun, and issue a small staged edit only through the existing exactly-once command/outcome path with a base generation, explicit preview, conflict refusal, and post-image receipt; it never turns review text or a screen projection into direct filesystem mutation.
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
  - No raw MemoHarness experience or pattern content, prompt/transcript/tool output, embedding, retrieval query or private score, secret, credential, filesystem path, experience-bank mutation, optimization start, module editing, candidate verification/promotion, or cross-tenant retrieval on mobile.
  - No Monaco, language-server/tsserver/DAP host, Git or shell process, PTY, Rust native helper, unsaved-buffer/undo-stack custody, raw terminal stream, or general project editor on mobile; Files, Problems, Terminal, and Debug are bounded host projections and controls only.
  - No mobile ownership or mutation of Desktop `editor.vim.enabled`, Vim mappings, modal state, Monaco key handlers, editor settings, or editor-release-rung admission. A handoff may display a non-authoritative effective-mode hint, but Desktop reauthorizes and resolves its own current setting.
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
- id: AC-14
  criterion: When the Cursor mobile parity corpus runs, a user can launch or resume work, search history, use voice, answer or approve, steer or queue, inspect changes and artifacts, monitor background state, receive privacy-generic attention, and hand the same session back to Desktop without opening Cursor or forking identity.
- id: AC-15
  criterion: When a user launches work from mobile, the target picker distinguishes owner-local, owner-managed, OpenAgents-managed, and compatible audited-provider placement and discloses reachability, custody, harness, model, cost, and retained data before admission; unavailable targets fail visibly.
- id: AC-16
  criterion: When voice, push, or lock-screen controls are used, every input is editable or confirmable according to policy, resolves through the same typed command and exactly-once outcome path as touch controls, and neither notification nor transcription state becomes execution authority.
- id: AC-17
  criterion: When a MemoHarness-enabled Full Auto run is inspected, mobile shows its released base and effective bundle digests, dimension-policy refs, static/global/adapted class, adaptation state, frozen bank-snapshot ref, compatibility, and effective provider/model/harness/toolset/evaluator/environment tuple from the host's typed projection; requested and effective harness identity are never merged.
- id: AC-18
  criterion: When a user launches work from mobile with Advanced harness controls, the picker contains only released compatible bundles and admitted adaptation policies supplied by the target host, submits immutable refs in the typed launch command, and leaves snapshot selection, retrieval, adaptation, and fail-closed compatibility enforcement to that host.
- id: AC-19
  criterion: When MemoHarness state crosses the mobile boundary, schema decoding rejects any payload carrying raw experiences, pattern content, prompts, transcripts, tool output, embeddings, retrieval queries or private scores, secrets, credentials, or filesystem paths; the phone exposes no command to mutate a bank, start optimization, edit modules, verify or promote a candidate, or change run authority.
- id: AC-20
  criterion: When a MemoHarness-enabled run ends, the attention inbox retrieves a bounded report containing safe base/effective bundle and adaptation receipt refs plus candidate/release follow-up state, while private source evidence remains dereferenceable only through an owner-authorized higher-trust surface.
- id: AC-21
  criterion: When a project has multiple roots, diagnostics, changed files, proposals, tests, tasks, artifacts, and child-agent work, mobile can browse and search the safe bounded projections, open file/symbol/Problem/diff/evidence detail, and see exact attachment/document/service generations plus stale, cached, omitted, degraded, revoked, or unavailable truth without receiving a raw root or unselected repository content.
- id: AC-22
  criterion: When a user taps Open on Desktop from a file range, Problem, proposal hunk, test failure, artifact, or agent backlink, the continuation carries only opaque safe refs, Desktop reauthorizes and resolves the current project generation, and missing historical state opens an exact snapshot/diff or explicit unavailable result rather than a guessed current line or new session.
- id: AC-23
  criterion: When mobile comments, approves, rejects, reruns, or submits an admitted small staged edit, the action uses the same exactly-once outbox and typed command as other controls, binds the exact base generation, previews the effect, refuses conflict, and completes only from a durable outcome and post-image receipt; the app exposes no general editor or direct filesystem mutation path.
- id: AC-24
  criterion: When Files, Problems, Terminal, task/test, or debug evidence is viewed on mobile, schema and capability audits prove the phone hosts no Monaco, LSP/tsserver/DAP, Git/shell process, PTY, Rust helper, raw environment, unsaved-buffer/undo custody, or raw terminal stream; host execution and private state remain on the authoritative target.
- id: AC-25
  criterion: When mobile decodes an IDE tree, excerpt, Problem, change, proposal, test, task, artifact, or continuation, it uses the same identified Effect Schema contract and constrained opaque refs as Desktop/web, derives its TypeScript types from that schema, rejects unknown or forbidden fields at entry, and has no raw interface or handwritten union acting as a parallel projection contract.
- id: AC-26
  criterion: When bounded code evidence renders on phone or tablet, syntax, diff, diagnostic, selection, focus, and status roles come from the allowlisted Tokyo Night semantic review projection, pass the applicable contrast/non-color checks, and contain no executable theme contribution; the UI neither claims broader Desktop theme parity nor stores or mutates Desktop Vim settings or modal state.
- id: AC-27
  criterion: When a mobile release describes IDE support, it identifies its role as IDE-14 bounded supervision/review over an admitted IDE-13 capability and links exact project-generation, acceptance, and assurance state; it never promotes Files foundation, a basic Desktop editor, or an agent-IDE packet into portable/full Cursor parity by inference.
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
- id: SM-7
  metric: cursor_mobile_parity_journeys_completed_without_cursor_or_desktop_fallback
  target: "100% across the maintained mobile parity corpus"
  target_status: committed
  window: every release candidate
- id: SM-8
  metric: memo_harness_runs_with_complete_safe_mobile_provenance
  target: "100%"
  target_status: committed
  window: every release candidate and rolling 30-day dogfood
- id: SM-9
  metric: mobile_project_review_and_desktop_continuation_journeys_resolving_exact_safe_generations
  target: "100% across the maintained Zed-quality mobile projection corpus"
  target_status: committed
  window: every release candidate
- id: SM-10
  metric: mobile_ide_projection_contracts_decoded_from_shared_effect_schemas_with_derived_types
  target: "100%; zero mobile-local parallel boundary contracts"
  target_status: committed
  window: continuously and every release candidate
- id: SM-11
  metric: mobile_code_review_roles_using_the_allowlisted_tokyo_night_semantic_projection
  target: "100% of initially supported code-evidence surfaces"
  target_status: committed
  window: every release candidate
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

MemoHarness follows the same projection-only law. The Effect Native client
decodes shared Effect Schema DTOs for base/effective harness identity,
adaptation, compatibility, and release state, then sends only ordinary typed
launch or supervision commands. The authoritative Desktop or managed Effect
service freezes the bank snapshot, retrieves, adapts, compiles terminal
experiences, optimizes, stores private evidence, and resolves Blueprint
release state. The phone neither reproduces that control plane nor receives
its private working set.

The IDE projection follows the same rule. Shared Effect Schema DTOs expose
only safe project/root/file/document/proposal/evidence refs and bounded
tree/search/symbol/Problem/diff/test/task/artifact content. The authoritative
host owns Monaco, document recovery, language/Git/task/debug services,
external runtimes, and any Rust PTY/containment helper. Mobile keeps no shadow
IDE database: it renders typed snapshots, sends exactly-once commands, and
hands opaque refs back to Desktop for fresh authorization and resolution.
The Effect Native projection service decodes the same identified schemas and
derives its TypeScript types rather than restating DTO interfaces. Scoped
layers own cache, stream, outbox, and teardown lifetimes. An allowlisted Tokyo
Night semantic subset colors code evidence consistently; it conveys no theme
code or Desktop Vim/editor authority.

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
- A convenient harness inspector could accidentally become a private-memory
  exfiltration or remote self-promotion surface; the shared safe-projection
  schema and absence of bank/optimizer/promotion commands are hard boundaries.
- A rich Files/Changes projection can drift into a fragile phone IDE or leak
  source by convenience. The safe-field schema, content bounds, explicit
  omission/staleness, no-native-runtime rule, and Desktop handoff remain
  release gates.

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
- Which harness/adaptation facts merit lock-screen visibility versus requiring
  an authenticated run-detail view?

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
- MemoHarness and Blueprint integration authority:
  `docs/research/2026-07-18-memoharness-blueprint-integration-analysis.md`
- Zed-quality IDE projection and Effect/Rust boundary:
  `docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md`
- Canonical IDE roadmap and mobile IDE-14 boundary: `docs/ide/ROADMAP.md`
- Roadmap-to-spec and assurance traceability:
  `specs/IDE_ROADMAP_CROSSWALK.md`

## Owner Gates

- Approval of the owned relay design and its operational cost before any
  hosted reachability ships.
- App Store distribution decisions (TestFlight cohorts, store listing,
  release cadence) remain owner actions.
- Push-notification entitlement and APNs key management.
- Sign-off on the lost-device revocation flow before scoped tokens ship
  broadly.
- Sign-off on the exact MemoHarness mobile safe-projection fields and on any
  mobile surface that can request a released adaptation policy; candidate
  promotion and private bank access remain separate higher-trust,
  independently admitted owner workflows, not mobile capabilities.
- Any earnings, payout, or referral-accrual display requires the matching
  promise-registry state and settlement evidence before it renders as more
  than a receipted projection.

## Receipts

Planned receipt kinds this surface renders or triggers: remote-action
outcome receipts, writeback post-image receipts, session-movement and
source-cleanup receipts, grant issuance and revocation receipts, outbox
replay records, account-rotation records surfaced from the engine, safe
HarnessAdaptationReceipt refs, harness release/rollback state, and earnings/
counter projections reconcilable to exact rows. This section plans kinds;
evidence lives in the receipt systems, not in this spec.

## Promise Links

None yet. Public claims derived from this spec (any-host control, phone
never executes, exactly-once replay, earnings displays) must land in the
promise registry with verification gates before they appear in copy; the
pays-you economics remain `planned`-state promises until settlement evidence
exists.
