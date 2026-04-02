# Probe Shell Projection

## Scope

This document records how a local Probe session is projected into the existing
Autopilot coding shell.

The goal is not a second UI. The goal is to map Probe runtime truth into the
same app-owned shell state that previously only projected Codex.

## Mapping Rules

The current local mapping is intentionally simple:

- Probe `session_id` is the Autopilot thread id
- the session `cwd` and transcript path map into existing thread workspace
  metadata
- Probe turn-control status now maps into explicit desktop session states:
  `attached`, `attached:running`, `attached:paused`, `queued`, `idle`,
  `completed`, `failed`, `cancelled`, `timed_out`, and `archived`
- runtime progress events update the existing active assistant message flow

This keeps project, transcript, and operator state app-owned even when the
runtime changes.

## Attach And Resume Posture

The desktop lane restores an existing Probe-backed thread by reloading the same
`session_id` instead of fabricating a new local session.

Current local-first behavior:

- startup refresh lists the available Probe sessions
- startup refresh now inspects per-session turn-control state so the thread
  rail can distinguish idle, completed, attached, and archived sessions instead
  of flattening them into one generic active row
- the active workspace reattaches to the matching live session if one is
  already present
- reloading a thread uses the same Probe session id
- session selection restores the thread's saved shell preferences before the
  Probe load command runs
- `new thread` in a Probe-backed workspace now runs an attach-vs-create policy:
  - if exactly one live session already matches the workspace, Autopilot
    attaches to it instead of starting a replacement
  - if the currently attached Probe session already matches the workspace,
    Autopilot reuses that session and reloads it into the shell
  - if multiple live sessions match the workspace and none is already attached,
    Autopilot refuses honestly and tells the operator to choose one from the
    thread rail instead of fabricating a new session

## Transcript And Status Projection

The reducer layer now translates these Probe shapes into app state:

- session snapshot transcript entries
- runtime progress events
- pending approval updates
- queued, interrupted, and cancelled turn control updates

That gives one Probe-backed thread a coherent transcript and runtime status
inside the existing shell.

Attach failures also stay visible at the shell layer. If the desktop cannot
reload the selected Probe session, the error is surfaced directly instead of
falling back to silent session creation.

## Shared Session Layer

Autopilot now keeps an app-owned Forge shared session object above raw Probe
session ids.

Current local-first rules:

- the Forge shared session id is distinct from the Probe `session_id`
- one shared session can point at one or more Probe sessions over time
- the shared session can now also persist delegated child-session cards derived
  from Probe child-session summaries
- the shared session records the local human and the local Probe agent as
  explicit participants
- the current control owner is stored separately from raw Probe runtime status
- explicit handoffs persist summary, provenance, and timestamp in the desktop
  artifact projection
- reducer-level interrupt and resume events also update control-owner posture so
  the shell does not lose lineage when control flips between the operator and
  the background agent
- the shared session now also projects hosted Probe runtime ownership and
  workspace provenance into one app-owned remote-session object:
  - session location and owner kind
  - owner id, display label, and attach target
  - workspace boot mode, baseline id and status, and snapshot ref
  - execution host kind and host identity
  - operator handoff posture derived from the current control owner

The first operator-facing control is the chat command:

- `/handoff human <summary>`
- `/handoff agent <summary>`

When Probe exposes delegated child sessions, Autopilot now projects them as
app-owned cards inside the shared session instead of leaving them buried in the
parent transcript.

Current local-first behavior:

- delegated child cards persist title, cwd, parent-turn linkage, initiator,
  purpose, runtime status, and any terminal closure summary Probe exposes
- the shell renders those child sessions as a first-class section above the
  evidence and delivery layers
- child-session ids are mirrored into the parent evidence bundle and delivery
  receipt so reviewer-facing surfaces can link delegated work back to the same
  shared session story

## Workspace Restore Provenance

The shared session now also carries app-owned workspace restore provenance.

Current local-first behavior:

- `StartSession` marks the shared workspace as a `cold_start`
- `LoadSession` marks the shared workspace as a `warm_start` and records a
  local restore pointer derived from the Probe session id
- hosted `workspace_state` snapshots now override that local seed with Probe's
  real boot mode, baseline, snapshot, and provenance note
- operators can explicitly mark a session as `restored` with:
  - `/restore <restore-pointer>`
  - `/restore <restore-pointer> <snapshot-ref>`
- base repo identity is captured from the local git workspace when available:
  remote origin, current branch, and current head commit
- when Probe cannot supply a snapshot ref, the shell says so directly instead of
  pretending a real snapshot registry already exists

Autopilot now also persists two app-owned objects above that provenance:

- one Forge workspace snapshot linked from the shared session
- one Forge restore manifest linked from the shared session

Current local-first rules:

- both objects persist in the desktop artifact projection and survive restart
- the workspace snapshot records shared-session id, Probe session ids,
  workspace root, base repo identity, startup kind, restore pointer, optional
  snapshot ref, and links back to the current evidence bundle or delivery
  receipt when those exist
- the restore manifest records the same restore inputs plus the linked
  workspace snapshot id so later hosted or cross-device flows have a typed
  restore object to consume
- the shell renders both objects separately so the operator can see exactly
  what restore state exists and whether the snapshot ref is still missing from
  Probe

## Evidence Bundle Layer

Autopilot now also keeps one app-owned evidence bundle above the shared session.

Current local-first behavior:

- the evidence bundle is linked from the Forge shared session and persists in
  the same desktop artifact projection
- latest diff and latest review truth are pulled into the bundle automatically
  from app-owned shell artifacts instead of asking the reviewer to spelunk the
  raw transcript
- operators can extend the bundle with:
  - `/evidence verify <label> <passed|failed|running> [reference]`
  - `/evidence log <label> <reference>`
  - `/evidence preview <label> <reference>`
  - `/evidence screenshot <label> <reference>`
- verification and log entries capture the current terminal tail when available
  so a reviewer gets durable evidence even though the live terminal buffer is
  not the persistence contract
- reviewer-facing evidence state is rendered honestly as missing, partial,
  complete, or failed

## Delivery Receipt Layer

Autopilot now also keeps one app-owned delivery receipt above the shared
session and evidence bundle.

Current local-first behavior:

- one delivery receipt is linked from the Forge shared session
- the receipt points back at the evidence bundle that justified the delivery
- `/deliver pr [base-branch] [pr-url]` records branch, commit, compare URL,
  optional GitHub PR URL, and suggested title/body state
- Probe session snapshots and detached workspace-state events now project the
  live local branch head and delivery posture into that same receipt
- the shell records app-owned branch and compare watch state above the receipt,
  including current head, upstream, ahead or behind counts, dirty-worktree
  posture, and Probe's delivery classification (`needs_commit`, `needs_push`,
  `synced`, `diverged`, `local_only`)
- `/deliver status` renders the current local branch, compare, PR, and CI watch
  summary directly in the shell
- `/deliver refresh` performs one bounded refresh pass:
  - local branch and compare watch is recomputed from the current git workspace
    using the same rules Probe uses for its delivery posture
  - GitHub PR and CI watch is refreshed explicitly through `gh` when a PR URL
    or head branch can be resolved
- `/deliver review <commented|approved|changes_requested> <reviewer-label> [summary]`
  records reviewer outcome explicitly
- `/deliver merge <reviewer-label> [summary]` records merge closure explicitly
- authorship mapping is stored as an explicit product object with separate local
  human and local Probe agent roles instead of leaving that inference to raw
  transcript history
- GitHub-specific PR and CI details stay app-owned in the delivery receipt
  instead of leaking browser-provider semantics into Probe runtime truth

## Hosted Audit Bundle Layer

Autopilot now also keeps app-owned hosted audit bundles above the shared
session, evidence bundle, delivery receipt, hosted Probe runtime projection,
and the bookkeeping objects linked to the same hosted run.

Current local-first behavior:

- one hosted coding audit bundle and one hosted bookkeeping rehearsal bundle
  can be linked from the Forge shared session
- the bundle snapshots:
  - the latest hosted preflight report and exported preflight artifact path
  - environment summary
  - session location
  - Probe session ids
  - workspace root and base repo identity
  - routed and mounted pack ids plus unsupported-route reasons
  - hosted Probe receipts for auth, checkout, worker ownership, cost, and
    cleanup when Probe reports them
  - linked evidence bundle and delivery receipt state
- operators can extend the bundle through:
  - `/hosted preflight [path]`
  - `/hosted coding <environment-summary>`
  - `/hosted bookkeeping <environment-summary>`
  - `/hosted note coding <summary>`
  - `/hosted recovery coding <summary>`
  - `/hosted defect coding <summary>`
  - `/hosted note bookkeeping <summary>`
  - `/hosted recovery bookkeeping <summary>`
  - `/hosted defect bookkeeping <summary>`
  - `/hosted export <coding|bookkeeping> [path]`
  - `/hosted status`
- the shell renders the audit as a first-class card so the operator can review
  hosted closeout truth without spelunking the raw transcript or detached Probe
  session JSON
- the operator can export the active hosted coding or bookkeeping bundle into a
  deterministic Markdown or JSON artifact with the concrete shared-session,
  mounted-pack, hosted-receipt, preflight, evidence, delivery, campaign,
  promotion, bounty, claim, and settlement ids and statuses for review or
  check-in
- bookkeeping rehearsal bundles also snapshot the linked campaign,
  promotion-ledger, bounty-contract, bounty-claim, and settlement-receipt ids
  and statuses so the operator can see which bookkeeping steps were actually
  tied to the hosted run versus which steps still required manual intervention

This keeps Probe as the source of hosted runtime truth while keeping the closeout
story, reviewer-facing notes, bookkeeping linkage, and operator defect
accounting app-owned.

## Campaign Layer

Autopilot now also keeps one app-owned Forge campaign above the shared session,
evidence bundle, and delivery receipt.

Current local-first behavior:

- one active campaign is linked from the Forge shared session with a stable
  `forge-campaign-*` id
- the campaign keeps operator intent explicit instead of burying retained-case
  selection in transcript prose:
  - title
  - goal summary
  - scope summary
  - candidate refs
  - retained case selections
  - verification refs
- campaign refs are typed instead of freeform notes:
  - `probe_summary`
  - `accepted_patch_summary`
  - `evidence_bundle`
  - `delivery_receipt`
  - `psionic_retained_eval_bundle`
  - `psionic_comparison_manifest`
- local-first convenience refs like `active`, `latest`, and `current` resolve
  against the current shared session for accepted patches, evidence bundles,
  and delivery receipts instead of forcing the operator to remember internal
  ids
- operators can manage the current local campaign with:
  - `/campaign open <title>`
  - `/campaign goal <summary>`
  - `/campaign scope <summary>`
  - `/campaign candidate <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign case <case-id> <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign verify <evidence_bundle|delivery_receipt|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign status`
- the shell renders the active campaign as a first-class card between the
  shared session and promotion layers so retained-case and eval-selection
  posture is visible before bounty or settlement closure

## Promotion Ledger Layer

Autopilot now also keeps one app-owned promotion ledger above the campaign so
admitted improvements move through explicit shadow, promoted, and rolled-back
states instead of transcript-only notes.

Current local-first behavior:

- one active promotion ledger is linked from the active campaign with a stable
  `forge-promotion-*` id
- each candidate admission becomes a typed revision with explicit source kind,
  source reference, admitting actor, and provenance
- promotion and rollback stay explicit lifecycle mutations instead of mutating
  the campaign object in place
- rollback records capture authority, reason, provenance, and an explicit
  fallback promoted revision when one exists
- operators can manage the current local promotion ledger with:
  - `/promote shadow <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> <actor-label> [summary]`
  - `/promote promote <actor-label> [summary]`
  - `/promote rollback <actor-label> <reason>`
  - `/promote status`
- the shell renders the active promotion ledger as a first-class card between
  the campaign and bounty layers so rollout posture is visible before payout or
  settlement logic

## Knowledge-Pack Catalog Layer

Autopilot now also keeps an app-owned Forge knowledge-pack catalog above shared
sessions and project/workspace identity.

Current local-first behavior:

- knowledge packs persist in the same desktop artifact projection file as the
  rest of the Forge shell state
- each pack gets a stable `forge-pack-*` id, explicit kind, explicit scope, and
  explicit source references instead of hidden transcript text
- the first supported pack kinds are:
  - repo docs
  - repo runbooks
  - retained session summaries
  - accepted patch summaries
  - benchmark references
  - judge references
- pack scope is explicit:
  - `project`
  - `workspace`
- source references are explicit and typed instead of being inferred later:
  - repo file
  - Probe retained session summary artifact
  - Probe accepted patch summary artifact
  - Psionic benchmark manifest
  - Psionic judge manifest
- the first operator-facing pack authoring loop is command-driven in the shell:
  - `/pack docs <title> <path> [path ...]`
  - `/pack runbook <title> <path> [path ...]`
  - `/pack retained [title]`
  - `/pack patch [title]`
  - `/pack status`
  - `/pack route status`
  - `/pack route auto <pack-id> [pack-id ...]`
  - `/pack route off <pack-id> [pack-id ...]`
- repo docs and runbook packs are authored from repo-scoped file paths inside
  the active workspace root instead of manual JSON edits
- retained-summary and accepted-patch packs are imported as typed pointers to
  the current Probe artifact ids so later routing can mount real runtime-owned
  context instead of transcript prose
- each pack now also carries an app-owned session-start routing policy:
  - `auto`
  - `excluded`
- starting a new Probe session converts the routed pack set into typed Probe
  `mounted_refs` instead of keeping routing hidden in prompt text
- the shared-session shell now persists and renders the difference between:
  - the pack ids OpenAgents routed
  - the pack ids Probe reported as mounted
  - explicit unsupported route cases
- the shared-session shell now renders the active scoped pack catalog as a
  first-class card so later routing can use a real app-owned object set rather
  than one-off notes

## Bounty Contract Layer

Autopilot now also keeps one app-owned bounty contract and optional active claim
above the shared session, evidence bundle, and delivery receipt.

Current local-first behavior:

- one bounty contract is linked from the Forge shared session with a stable
  `forge-bounty-*` id
- the active claim is tracked separately with its own stable `forge-claim-*` id
- objective kind is explicit instead of freeform:
  - `accepted_merge`
  - `admitted_metric_win`
- the shell persists claim lifecycle state directly:
  - `draft`
  - `claimed`
  - `admitted`
  - `completed`
  - `canceled`
  - `disputed`
- participant credit envelopes are stored separately from claim ownership so
  operator credit, implementation credit, reviewer credit, and later evaluator
  credit do not get collapsed into one winner-take-all note
- bounty contracts link back to the current evidence bundle, delivery receipt,
  and reserved knowledge-pack or eval-pack refs so later payout or dispute
  logic can reason from typed objects instead of transcript fragments
- operators can manage the current local contract with:
  - `/bounty open <merge|metric> <title>`
  - `/bounty credit <participant-label> <basis-points>`
  - `/bounty claim <claimant-label> [summary]`
  - `/bounty advance <admitted|completed|canceled|disputed> [summary]`
  - `/bounty status`
- the shell renders the active bounty contract and active claim as first-class
  cards above the workspace snapshot layer so settlement posture is visible
  before funds movement exists

## Settlement Receipt Layer

Autopilot now also keeps one app-owned settlement receipt above the shared
session, bounty contract, evidence bundle, and delivery receipt.

Current local-first behavior:

- one settlement receipt is linked from the Forge shared session with a stable
  `forge-settlement-*` id
- the receipt keeps closure posture explicit instead of inferring payout
  readiness from transcript fragments:
  - `accepted_merge`
  - `admitted_metric_win`
- the receipt also keeps operator-visible settlement state explicit:
  - `recorded`
  - `disputed`
  - `canceled`
- merge-backed settlement reuses the current merged delivery receipt and latest
  reviewer outcome instead of asking the shell to reconstruct acceptance later
- metric-backed settlement stores an explicit evaluator label plus reference so
  retained eval or benchmark closure can stay typed
- operators can manage the current local settlement with:
  - `/settle merge <reviewer-label> [summary]`
  - `/settle metric <evaluator-label> <reference> [summary]`
  - `/settle dispute <actor-label> [summary]`
  - `/settle cancel <reason>`
  - `/settle status`
- dispute-window timing, cancel reason, reviewer closure, evaluator closure,
  and payout-facing evidence or delivery links all live on that typed receipt
- the shell renders the active settlement receipt as a first-class card so the
  current closeout posture is visible without reading raw command history

## Artifact Ownership

Plan, diff, review, and compaction presentation stays app-owned.

The first Probe projection slice does not pretend Probe already emits every
desktop-native artifact shape that the Codex lane has today. Where a Probe path
is not wired yet, the shell now refuses honestly instead of routing the action
through Codex against the wrong session type.

Examples in the current slice:

- thread reload is Probe-aware
- rename plus archive or unarchive now stay app-owned above the Forge shared
  session and persist as shell overlays instead of pretending Probe already owns
  those product semantics
- review now produces an app-owned review snapshot from the current shared
  session, evidence bundle, and delivery receipt state instead of refusing
- compaction now records an app-owned shell checkpoint artifact instead of
  pretending Probe already exposes a runtime-native compaction primitive
- rollback still stays one explicit product-level refusal because the current
  seam does not yet mutate a Probe workspace back to an earlier snapshot or
  restore pointer
- desktop mention and image attachments now go through an explicit app-owned
  forwarding manifest for Probe-backed turns
- the forwarded manifest is visible both in shell activity state and in the
  Probe transcript because the same rendered manifest is what the runtime
  receives
- shell-selected skill attachments still stay app-owned for Probe turns until a
  real tool-attachment contract exists

## What Still Follows

This projection layer is enough to make one local Probe-backed coding thread
real inside Autopilot.

The local operator loop is now shipped for the current Probe sidecar slice:

- queued follow-ups, approval roundtrips, queue cancel, and interrupt stay
  inside the app-owned shell above Probe runtime truth
- shared sessions now carry workspace snapshot and restore provenance, delegated
  child-session cards, reviewer evidence, delivery receipts, attachment
  forwarding, delivery watch state, and active campaign state

The next honest follow-ons are outside this hosted-projection slice:

- a real hosted control-plane transport and multi-worker routing story above
  the same shared-session objects
- push-driven remote session activity instead of desktop polling and bounded
  refresh passes
- a real tool-attachment contract for skill-like attachments instead of the
  current app-owned forwarding manifest
