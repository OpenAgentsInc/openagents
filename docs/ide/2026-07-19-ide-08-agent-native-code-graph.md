# IDE-08 agent-native code graph

Date: 2026-07-19
Issue: [#9036](https://github.com/OpenAgentsInc/openagents/issues/9036)
Status: implemented for the exact evidence-bound macOS arm64 candidate named
in the checked IDE-08 acceptance receipt; Desktop AC-17/AC-43 and Cursor
CP-AC-20 AssuranceSpecs remain `proposed` and owner disposition remains
`unreviewed`.

## Outcome and claim boundary

IDE-08 makes a coding agent a participant in the existing Desktop project,
document, language, and review graph. A turn can now:

1. attach to one exact project/root/worktree/session/grant generation;
2. disclose an exact context manifest and effective runtime;
3. submit a hash-checked, version-bound create/edit/rename/delete proposal;
4. enter the existing Pierre Changes plane for explicit review;
5. accept, reject, partially accept, refuse/rebase, apply, or undo through the
   canonical workspace authority;
6. resolve conversation-to-code and code-to-conversation backlinks; and
7. distinguish host-observed diagnostics, formatting, tests, Git, delivery,
   verification, and owner acceptance from runtime completion.

This packet does **not** create a public release rung. “OpenAgents agent IDE”
still requires IDE-08 through IDE-12 as a group. It does not claim inline
completion/next edit, terminal/tasks/tests, debug, SCM delivery, remote
placement, mobile/web projection, extensions, browser/computer use, agent
platform breadth, migration/custody closure, or Cursor parity.

## One Schema graph

`apps/openagents-desktop/src/ide/agent-code-contract.ts` is the sole boundary
schema for this feature. All boundary TypeScript types derive from identified
Effect Schemas. The graph contains:

- branded attachment, manifest, context-item, operation, decision, review,
  apply, undo, backlink, turn, packet, and ProductSpec-revision refs;
- exact project/root/worktree/session/grant, attachment generation, and
  placement generation;
- context sources for file, range, diagnostic, symbol, Git change, rule,
  skill, recent edit, lexical retrieval, optional semantic retrieval, runtime
  policy, and typed unavailability;
- per-item selection origin, source generation, disposition, destination,
  freshness, sensitivity, retention, byte/token estimate, truncation, label,
  and bounded excerpt;
- effective harness, model, provider, account, placement, tool policy,
  permission mode, sandbox, memory, instruction policy, and semantic-retrieval
  posture;
- proposal bases carrying the admitted content, digest, disk/document/Git/
  checkpoint generations, encoding, EOL, and mode;
- create, edit, rename, and delete operations with explicit symlink policy;
- pending, reviewing, partially accepted, accepted, applying, applied,
  rejected, rebase-required, superseded, cancelled, failed, and undone states;
- preimages, apply/undo receipts, current/historical/unavailable backlinks,
  and independently typed evidence; and
- a structurally public-safe receipt that has counts and stable refs but no
  root, prompt, provider payload, file content, retrieval query, credential,
  or private evidence payload.

The older `project-contract.ts` proposal-shaped row is now explicitly named
`IdeProposalIndexEntry`: it is only an identity/navigation catalog entry. It
is not a second mutation protocol. Proposal mechanics have one authority.

## Effect ownership and adapter boundaries

The runtime is `IdeAgentCodeService`, a `Context.Service` built by
`Layer.effect`. Its non-trivial operations are named `Effect.fn`s, its expected
failures are `Schema.TaggedErrorClass` values, and persisted, renderer, IPC,
and runtime input decodes at entry.

The ownership map is deliberately narrow:

| Layer | Owns | Must not own |
| --- | --- | --- |
| Effect agent-code service | identities, generations, lifecycle, admission, decisions, checkpoints, receipts, backlinks, evidence linkage, retention | host filesystem bytes or UI mechanics |
| main workspace authority | re-read of current files, secret/private/binary/size checks, exact canonical mutation, rollback, Git/language observation | proposal authorship or owner decision |
| renderer | context disclosure, selection intent, accessible controls, navigation requests | filesystem, evidence assertion, apply, rollback, grant, or root authority |
| Pierre | unified/split diff projection and review interaction | proposal state, file mutation, checkpoint, Git, or bridge authority |
| Monaco | document projection and user editing mechanics | proposal apply, runtime context, persistence, or evidence authority |
| harness/runtime | proposal-only decoded output | direct filesystem/Monaco mutation, evidence, delivery, verification, or acceptance |
| Rust/native | nothing in IDE-08 | project, proposal, policy, storage, credential, approval, or receipt authority |

`agent-code-workspace-authority.ts` is the only adapter from the abstract
document authority into the existing main-owned `DesktopWorkspaceService`.
Create/edit/rename/delete still pass through its grant, root containment,
revision compare-and-swap, ignored-path, symlink, encoding, and size rules.
No new Rust code or native helper was admitted.

## Context disclosure

The composer and Editor share `AgentContextTray`. “Add context” creates a
manifest only after a bounded editor file has been explicitly attached. The
production active-file inventory always names eleven source slots:

1. explicit file;
2. active selection or typed absence;
3. current diagnostics or typed absence;
4. current symbols or typed absence;
5. matching Git/co-change fact or typed absence;
6. project rule or typed absence;
7. invoked skill or typed absence;
8. recent-edit metadata or typed absence;
9. lexical path match;
10. optional semantic retrieval, currently omitted as `retrieval_disabled`;
11. effective runtime/tool policy.

Each row shows inclusion/omission reason, selector, generation, destination,
handling class, freshness, retention, bytes/tokens, and truncation. The
manifest has fixed 200,000-byte and 50,000-token ceilings. Excerpts are UTF-8
and 64,000-character bounded; later included items that cannot fit become
explicit `over_budget` omissions rather than expanding the budget. This keeps
the product useful with semantic retrieval disabled: explicit context,
lexical path facts, language/symbol evidence, and Git facts remain available.

Source generation and project document generation are intentionally distinct.
Monaco's first model incarnation is source generation zero; the project and
workspace authorities are one-based. Manifest assembly preserves the exact
Monaco source generation and performs the single explicit `source + 1`
translation at the renderer-to-authority boundary. It never passes an
unbranded zero into the project document-generation contract.

No embeddings are generated, stored, or uploaded by IDE-08. The contract can
describe future local or managed semantic placement, but enabling one requires
an independently disclosed, exportable, rebuildable, retention-bound, and
deletable source. Public receipts never include query text, scores, vectors,
or file excerpts.

## Proposal admission and exact retry

Untrusted proposal output is decoded before admission. Admission checks:

- exact attachment, manifest, creating turn/conversation, and generation;
- unique proposal, operation, and target-path identities;
- exact replay reconciliation and conflicting-ref rejection;
- a complete missing base for create and complete existing base for edit,
  rename, and delete;
- SHA-256 equality between claimed and actual create/edit bytes;
- declared EOL agreement with the proposed bytes;
- exact document identity agreement with the base;
- explicit create encoding/EOL/mode and `symlink: refuse`; and
- bounded content and operation counts.

Manifest, proposal, review, and decision retries are idempotent when their
bytes are equal. Reusing a stable ref for different content, identity, or
disposition fails closed. Attachment replacement cancels every unsettled
proposal and clears manifests; equal relative paths in two worktrees remain in
separate service scopes.

## Review, partial decisions, apply, and rebase

Single-file and aggregate proposals compile into the IDE-05 `AgentProposal`
review source and render through the same owned Pierre adapter as repository
changes. Pierre receives serializable versioned source data only.

The owner selects exact operations, not screen positions. A full decision
moves the proposal to `Accepted` or `Rejected`. A partial decision freezes the
parent as `PartiallyAccepted` and creates a new exact child containing the
admitted operation set. No hunk is spliced against stale line numbers.

Before apply, main re-reads every source and rename target. A dirty document,
missing/created file, changed disk or document generation, symlink, secret,
private/binary/oversize content, or unsupported policy refuses. Base movement
persists `RebaseRequired` with the exact current path/state, disk revision,
document generation, and content digest where available; the original base
remains inspectable. A replacement rebase must be a new pending child on the
same attachment, manifest, and turn. There is no fuzzy apply.

Apply creates a retained checkpoint, transitions to `Applying`, executes
operations sequentially through the canonical authority, and records exact
post-image revision, digest, encoding, EOL, and mode. Any mid-transaction
failure restores completed operations in reverse order. Incomplete rollback
is an explicit non-recoverable failure, never success.

Undo requires the exact proposal/apply/checkpoint tuple and exact current
post-image. It restores preimages in reverse order, emits an undo receipt,
changes the lifecycle to `Undone`, converts code links to historical, and
marks post-image evidence stale. Private preimages expire after the configured
checkpoint window; recovery purges them and changes affected historical links
to `retention_expired` rather than retaining content indefinitely.

## Backlinks and evidence

Every applied current document emits a backlink bound to proposal, operation,
session, creating turn, optional creating conversation, attachment generation,
file/document generation, and path. The review surface can open the creating
conversation when that local thread remains available. The opposite action
opens the exact current file through ordinary Files/editor authority.
After undo it opens the retained historical proposal/checkpoint state; after
generation or retention loss it says unavailable.

Evidence is not a renderer or harness command. After apply, main observes:

- diagnostics and formatting through the current language service;
- Git status and per-path Git diff through the current workspace service;
- tests as `Unavailable` unless an exact test command was admitted;
- commit/push/PR/delivery as `Unavailable` in IDE-08 rather than inferred from
  changed files or runtime prose;
- independent verification as `Unavailable` without its reviewer; and
- owner acceptance as `Unavailable` until the owner acts.

Each fact binds the exact apply ref and post-image generation and is one of
requested, running, passed, failed, unavailable, or stale. ProductSpec
spec-revision/digest, criterion, packet, terminal-outcome, and review-post-
image lineage must exactly equal the proposal lineage. A process saying
“tests passed” or “pushed” cannot create any of these facts.

## Persistence and local data

Agent-code recovery is an owner-private JSON snapshot under Electron
`userData/ide-agent-code/<sha256-of-private-root>.json`. The filename does not
contain the root. The directory is mode 0700 and replacement file is mode
0600. It contains the private state required for restart—bounded context
excerpts, proposal base/target content, decisions, unexpired checkpoint
preimages, backlinks, and evidence. It is never Sync, public receipt, or
provider memory.

Corrupt persisted state cannot partially hydrate: the host starts empty,
returns `corrupt_persistence`, and requires an explicit fresh attachment.
Stopped scopes clear checkpoint material and manifest excerpts. Retention
recovery drops expired preimages. IDE-18 still owns the complete per-project
inventory UI, export, selective deletion, backup/tombstone convergence, and
whole-profile deletion proof; IDE-08 does not overclaim those surfaces.

## Deterministic, packaged, and performance evidence

The checked corpus covers schema rejection, exact accounting and budgets,
proposal hash mismatch, full and partial decisions, mixed create/edit/rename/
delete, dirty/symlink/secret/private/binary/oversize and external-revision
refusal, exact rebase replacement, attachment fencing, two-worktree isolation,
exact retry, ProductSpec lineage, applied/undoable restart, corrupt recovery,
retention expiry, host-only evidence, and accessible context/review controls.

The packaged macOS arm64 journey launches the real `.app` through
LaunchServices on a disposable Git repository. It observes diagnostics,
attaches/discloses the production context manifest, admits an exact proposal,
renders Pierre, applies with keyboard input, reads host evidence, follows a
backlink to the editor, returns, undoes with keyboard input, verifies disk
preimage restoration, takes a screenshot, and proves root withholding.

The benchmark receipt contains p50/p95/p99 rows for production manifest decode
and projection, proposal decode, single and 25-file aggregate diff, stale-base
detection, single and 25-file aggregate apply, post-evidence insertion,
1,000-link navigation, attachment cancellation, restart recovery, and scoped
teardown. Every row records corpus, repetitions, warmup, percentile method,
noise, threshold, and pass state. Resource evidence requires zero active-
handle, listener, proposal-stream, and temporary-preimage growth and zero
remote requests or embeddings.

The exact candidate SHA, `.app` tree SHA-256, measurements, screenshot, trace,
reviewer disposition, rollback target, and final `main` SHA live in:

- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-agent-code.json`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.json`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.png`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code-trace.json`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-acceptance.json`; and
- issue #9036's `CLAIM-RELEASE` comment.

## Assurance and remaining gaps

The evaluator is a non-overridable deterministic repository oracle. It may
say that the exact implementation/evidence bundle passed; it cannot admit the
Desktop or Cursor AssuranceSpec, impersonate an independent human reviewer,
or set owner acceptance. Desktop AC-17/AC-43 and Cursor CP-AC-20 therefore
remain `proposed`/`unreviewed` even after #9036 closes.

IDE-09 is next: completion, next-edit prediction, inline ask/edit/generate,
selection transforms, and quality/latency/effective-model disclosure must use
this exact proposal/apply graph. IDE-10 through IDE-19 remain visible in the
canonical roadmap and cannot borrow IDE-08 evidence for their terminal, debug,
SCM, placement, mobile/web, extension, browser, agent-platform, custody, or
parity outcomes.
