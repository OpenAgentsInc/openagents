# What OpenAgents Should Take from Zed for the Agent IDE

Date: 2026-07-18

Status: source-grounded adaptation analysis; proposed architecture and packet
deltas only. This document does not admit implementation work, reorder the
accepted roadmap, change a ProductSpec, or authorize copying upstream code.

## Executive decision

Zed should become OpenAgents' primary public architecture and behavior
reference for the **integrated agent IDE**. That is narrower and more useful
than making Zed the component stack:

- Zed is the reference for how files, buffers, language intelligence, Git,
  terminals, worktrees, remote placement, agents, and persistence share one
  project model.
- Monaco `0.55.1` remains the code-editing component.
- `@pierre/trees@1.0.0-beta.5` and `@pierre/diffs@1.2.12` remain the tree and
  diff presentation components behind an owned adapter.
- Effect Native, Effect services, the generated engine protocol, and the
  Electron main process remain the OpenAgents control plane.
- Cursor remains a product-breadth floor, and VS Code remains a useful
  behavior/protocol comparison, but neither supplies Zed's unusually coherent
  answer to how an agent and an IDE should share a project.

The most important Zed idea is not GPUI, Rust, a rope, or an agent panel. It is
that the editor and agent operate over the same typed project graph: the same
worktrees, project paths, open buffers, document versions, diagnostics, Git
state, tasks, terminals, skills, and service lifecycles. The agent does not
merely inherit an ambient current directory, and the IDE does not merely show
whatever files the agent happened to touch.

OpenAgents should adapt that coherence while keeping a stronger authority
boundary. Sharing identity and evidence must not imply sharing privilege. A
Monaco model, a Claude Code or Codex harness, a Full Auto run, the mobile
controller, and the web supervision client may all refer to the same project,
file generation, diagnostic, patch, test, and Git receipt while retaining
different execution, approval, placement, retention, and disclosure scopes.

The resulting product thesis is:

> One project and evidence graph, many editors and agent runtimes, one
> canonical authority path.

## Evidence boundary and pins

This analysis combines the requested OpenAgents corpus with the existing Zed
source audit and targeted source rereads. No upstream binary, build, test,
extension, agent, installer, or network service was executed.

### Zed source pin

| Field | Value |
| --- | --- |
| Repository | `zed-industries/zed` |
| Local checkout | `projects/repos/zed` |
| Commit | `f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4` |
| Tree | `bc5e231b224529baeb1a3cc2c8ea54eff8ac21ad` |
| Commit time | `2026-07-18T22:40:55Z` |
| Application version | `1.13.0` |
| Default license | GPL-3.0-or-later, with separately marked components |

The checkout was clean and current from the immediately preceding Zed audit.
The exact source findings are cataloged in
`docs/teardowns/2026-07-18-zed-teardown.md`.

### OpenAgents target pin at authoring

| Field | Value |
| --- | --- |
| Repository | `OpenAgentsInc/openagents` |
| Branch | `main` |
| Commit | `aec9e1b5b7af9d14953adce8eeb683f09cb3048b` |
| Tree | `260a019bf3121761b9101ab5639044260d82497f` |

### Requested target corpus read in full

- `docs/transcripts/257.md`
- every file under `docs/ide/`
- every file under `specs/desktop/`
- every file under `specs/mobile/`
- every file under `specs/web/`

### Fast Follow and corpus lock

This is the `gap_analysis` lane for FastFollowSpec
`openagents.fast_follow`, revision 3, lifecycle `admitted`, at
`FASTFOLLOW.md` SHA-256
`25296b0dfd75c5d57bff5c44062d7d267f1fe86df25ef61a6eaa5a2d2ef1fb34`.
The selected source is `zed_industries.zed`; the matching directive is
`persistent_workroom_attention`; and the relevant lesson IDs are
`typed_project_capability_graph`, `multi_root_versioned_file_identity`,
`excerpt_projection_plane`, `local_remote_capability_symmetry`,
`project_bound_agent_context`, `explicit_local_ide_state_inventory`,
`wasm_guest_vs_host_effect`, `integrated_ide_verification`, and the rejection
`gpui_editor_and_scm_wholesale`. The directive's target scopes are Desktop,
mobile, and shared UI. Web appears here only because the owner explicitly
requested the web specs and their projection/no-editor boundary constrains the
shared protocol.

The following hashes bind the target corpus at the target commit, before this
analysis and README link were added:

| Target artifact | SHA-256 |
| --- | --- |
| `AGENTS.md` | `d41739b946b18b3f917fc996f97a773b2d8cd06362ab28bd133e18ced253881f` |
| `INVARIANTS.md` | `157e947c0b94d5dbb036088179a6961e5199df6a8993b4d141c5466b71244a3f` |
| `docs/transcripts/257.md` | `c9caa425edee3ffaf1485f6bcdcc75899e8113873d3dd78815524ec48902708b` |
| prior `docs/ide/README.md` | `983d67fdea594ed98b9690f1d367fe7c5e4039c74d8b0c2c85120525f0e21927` |
| `docs/ide/2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md` | `cfe4bcb8be3d8611597a34c11d0840de9cb961f51d7938a9b24acad31c134876` |
| `specs/desktop/desktop-trust-complete-workbench.product-spec.md` | `0ca775ac397e0717a6c602a2cf586ddc6ca2648c9b12bc1deff1f593ed01ca79` |
| `specs/desktop/full-auto.product-spec.md` | `1ec816bd58dce62b71060381188e2a82307d4e50baa3ba86ee2d0f8a827857ef` |
| `specs/desktop/full-auto.assurance-spec.md` | `03bfbfa52d1308789e2ec54b1e3451c8848f6e32cc4a17a599aea6d58233ac6a` |
| `specs/mobile/mobile-any-host-fleet-controller.product-spec.md` | `9129ab7cca23783b48fd595e79010ce89352831721cfe6ebaff6824124b31b7a` |
| `specs/web/openagents-com-sales-landing.product-spec.md` | `54ffaa824c4b0f7ca4e25105b1f179b8e1a4fb2ebb79dd811758baddcc542b1e` |
| `specs/web/openagents-com-trust-surface.product-spec.md` | `5e1c0b82642c0e83a430d7c4b8db6417ea532f8fa7a80173bd42b93c85097c8a` |
| `docs/teardowns/2026-07-18-zed-teardown.md` | `0190a82a7fdd27b6cea1fb7cf6e4484a14d5e6817babedc10b1c7eba1e8c405d` |

The target commit/tree above supplies the revision lock for these files. The
Zed commit/tree supplies the upstream content lock. Source visibility is
public; access was local source-only; confidence is high for encoded structure
and deliberately limited for runtime behavior; and GPL-default provenance is
why this document recommends adaptation rather than source transplantation.

Evidence labels in this document are deliberate:

- **[Zed source]** means encoded at the pinned Zed tree.
- **[OpenAgents current]** means encoded in the requested target corpus or
  current checked source described by that corpus.
- **[Inference]** explains why the source is structured that way.
- **[Proposal]** is a target design for OpenAgents and is not current behavior.
- **[Constraint]** is an existing OpenAgents invariant or accepted scope
  boundary that the proposal must preserve.

## 1. The non-negotiable product law from episode 257

The transcript supplies a simpler and more important requirement than any
architecture diagram:

```text
open a file
→ the file is visibly open in a real editor
→ in the main workspace region
→ immediately
```

Cursor failed the recorded interaction. The OpenAI app opened an unusably
small right-side pane. OpenAgents' Command-E Files flow and Finder Open With
path succeeded because they produced the expected main-workspace file view.
The existing IDE plan correctly converts this into an editor-first cold-open
law.

**[Proposal]** Keep the following as a release-blocking journey, not a visual
preference:

1. Finder Open With, Command-E, recent-file restore, tree click, search result,
   diagnostic, Git hunk, and agent backlink all resolve through one file-open
   command.
2. That command attaches or resolves an exact `WorkContext` and
   `ProjectFileRef`.
3. Desktop creates or restores a Monaco document model and makes it the
   primary main-region item.
4. Chat history, provider account hydration, agent startup, indexing, LSP,
   Git, and remote-service readiness may continue asynchronously and must not
   block the first editable frame.
5. Unsupported encoding, large-file policy, missing grant, stale attachment,
   or read failure produces a typed editor error in that same main region. It
   never silently falls back to a tiny panel or no-op.

This is the base of the agent IDE. If opening a file is unreliable, deeper
agent integration only makes the product more confusing.

## 2. Why Zed is the right main example

Zed is a legitimate IDE first and an agent product second. That ordering is
visible in its source graph:

```text
worktree snapshots and ProjectPath identity
        ↓
Project service graph
        ├─ BufferStore / document state
        ├─ LspStore / language capability
        ├─ GitStore / diff state
        ├─ search / tasks / terminal / debug
        ├─ settings / trust / context servers
        └─ native agent / ACP / terminal threads
        ↓
Editor / Project Panel / agent UI / workspace persistence
```

**[Zed source]** `Project` aggregates worktrees, buffers, language servers,
Git, tasks, terminals, debug, context servers, agents, search, collaboration,
and settings. Local and remote variants sit behind project/store contracts.
The native agent maintains per-project context, groups sessions by project,
loads project rules and trusted project skills, and invokes the project's
real file, language, Git, and terminal services.

**[Inference]** This is why Zed's agent integration feels like part of the IDE.
The agent is another participant in the project's state graph, not a chat
widget bolted beside an editor and handed a path string.

Zed is nevertheless the wrong direct stack for OpenAgents:

- GPUI combines UI, application state, action dispatch, focus, invalidation,
  and async lifetime. Importing a view means importing architectural gravity.
- Zed's rope, SumTree coordinate substrate, collaborative text, MultiBuffer,
  and Editor are mutually reinforcing, not small replaceable widgets.
- Zed's default license is not an invitation to transplant its integrated
  source into an owned product without a separate legal decision.
- OpenAgents already has an accepted Effect Native direction, an Electron
  main authority boundary, typed engine protocol, Monaco/Pierre choices, and
  stronger receipt and custody requirements.

The right move is therefore a semantic port: reproduce the boundaries,
identities, lifecycles, and interactions in the OpenAgents stack.

## 3. Component and concept mapping

| Zed concept | What it accomplishes | OpenAgents adaptation |
| --- | --- | --- |
| GPUI app/entities/contexts | one scoped state, action, focus, and async-lifetime vocabulary | Effect services, scopes, fibers, stores, and Effect Native projection |
| `Worktree` snapshot | filesystem scan, ignore rules, Git discovery, stable entries, local/remote state | Electron-main worktree/index service bound to `WorkContext` |
| `ProjectPath` | multi-root path identity | `ProjectFileRef` with project, root, relative path, and attachment generation |
| `Project` and stores | aggregate IDE services behind one project lifecycle | `IdeProjectService` composed from typed capability services |
| text/language `Buffer` | text version, save base, encoding, parse, diagnostics, conflict | main-owned revisioned document service plus Monaco model adapter |
| `MultiBuffer` excerpts | composite editor views over exact source ranges | typed `ExcerptSet` projection, initially read-only |
| `Editor` | edits plus language, Git, navigation, predictions, agent actions | Monaco plus OpenAgents document/language/review/agent adapters |
| Project Panel | virtualized state machine over project snapshots | Pierre tree projection plus canonical commands |
| `LspStore` | capability lifecycle and versioned language results | main-owned `LanguageCapabilityService` |
| `GitStore` / `BufferDiff` | HEAD/index/worktree truth and diff transforms | `GitEvidenceService`, Pierre diff, later separately admitted mutation |
| native Agent | project-aware owned agent runtime | owned OpenAgents agent runtime or HarnessAgent adapter |
| ACP threads | external runtime preserves native semantics | real external harness adapter with provider-private event plane |
| terminal threads | terminal-native agent surface | PTY projection with explicit runtime identity, not fake native parity |
| related excerpts / BM25 / Git context | multiple provenance-rich prediction signals | typed context candidates selected under disclosure and budget policy |
| remote Project stores | same UI intent, different placement | placement adapter behind capability contracts |
| SQLite + thread DB | workspace/editor/thread persistence | explicit local store inventory with retention/export/delete controls |
| theme registry | typed theme projected across IDE parts | one validated Effect Native editor theme projected to Monaco/Pierre/terminal |
| WASM extensions | narrower guest ABI with host effects | future component ABI behind deny-by-default canonical tool brokers |

This division keeps the practical dependencies and discards the idea that
integrated behavior requires Zed's implementation language.

## 4. The missing center: one IDE project graph

The existing basic IDE plan has the correct components and process boundary.
Zed adds the missing organizing abstraction: the unit around which editor and
agent services attach should be a typed project graph, not a set of unrelated
IPC endpoints.

### 4.1 Proposed canonical identities

The names below are proposals; exact schema names remain an implementation
packet decision.

```ts
type IdeProjectRef = {
  projectId: IdeProjectId
  workContextId: WorkContextId
  attachmentGeneration: number
}

type ProjectRootRef = {
  project: IdeProjectRef
  rootId: ProjectRootId
  displayName: string
  order: number
}

type ProjectFileRef = {
  project: IdeProjectRef
  rootId: ProjectRootId
  relativePath: NormalizedRelativePath
}

type DocumentSnapshotRef = {
  documentId: DocumentId
  file: ProjectFileRef
  documentGeneration: number
  diskRevision: DiskRevision
}

type ServiceResultRef = {
  document: DocumentSnapshotRef
  serviceId: ProjectCapabilityId
  serviceGeneration: number
}
```

**[Constraint]** Absolute roots remain in privileged host state. A renderer,
portable agent event, mobile projection, web projection, or public receipt
must never use a raw root as canonical identity.

**[Proposal]** All of these operations consume the same `ProjectFileRef`:

- tree open, reveal, select, rename, move, create, and delete;
- Monaco open, save, reload, and navigation;
- workspace search and symbol results;
- diagnostics, definitions, references, rename, formatting, and code actions;
- Git status, hunk, compare, and later staging intents;
- terminal “open here” and task working-directory selection;
- agent context mentions, tool calls, edit proposals, and code backlinks;
- mobile and web safe projections.

This eliminates a large class of bugs where each surface constructs its own
path string and silently refers to a different root or attachment.

### 4.2 Proposed project capability lifecycle

Zed models stores and language servers as capabilities with lifecycle. The
OpenAgents graph should make that visible and general:

```ts
type ProjectCapabilityState =
  | { _tag: 'unconfigured' }
  | { _tag: 'starting'; since: Timestamp }
  | { _tag: 'ready'; generation: number; placement: PlacementRef }
  | { _tag: 'degraded'; generation: number; reason: TypedLoss }
  | { _tag: 'stopped'; reason: TypedReason }
  | { _tag: 'failed'; reason: TypedFailure; retry: RetryPolicy }
```

Capabilities include filesystem/index, documents, language, Git, search,
terminal, tasks, debug, agent context, and persistence. The renderer asks for
an intent such as “find references” or “run task”; the project graph resolves
whether the capability is local, remote, starting, degraded, unsupported, or
failed. No surface infers readiness from a missing spinner.

### 4.3 Effect service target

OpenAgents should not reproduce Zed's Rust object graph literally. Effect
services provide a better target decomposition:

```text
IdeProjectService
  ├─ ProjectIdentityService
  ├─ WorktreeSnapshotService
  ├─ DocumentService
  ├─ LanguageCapabilityService
  ├─ GitEvidenceService
  ├─ SearchService
  ├─ TerminalTaskService
  ├─ AgentContextService
  ├─ ProjectPersistenceService
  └─ ProjectPlacementService
```

Each service is scoped to an exact `IdeProjectRef`; attachment revocation or
generation change invalidates the whole composed layer. This gives the user
Zed-like coherence while preserving OpenAgents' typed process and authority
boundaries.

## 5. One document truth plane for humans and agents

Zed's language buffer is the strongest practical warning against treating a
file as `{ path, text }`. It carries text version, disk save base, modification
time, language, parse state, diagnostics, encoding/BOM, conflict, and
capability.

### 5.1 State ownership

**[Proposal]** Divide document state this way:

| State | Canonical owner |
| --- | --- |
| file identity, encoding, EOL, disk revision, save base, conflict, recovery | Electron-main document service |
| text editing, undo stack, cursor, selection, folds, scroll, decorations | Monaco model while attached |
| durable dirty recovery and restart restoration | main-owned persistence via explicit document snapshots |
| language results | language service, bound to document and service generations |
| Git base and hunk identity | Git evidence service, bound to exact repository/index/worktree revisions |
| agent proposal | agent-edit service, bound to document snapshots |

The main document service need not reimplement Monaco's text engine. It does
need a typed incremental change stream, expected-generation checks, atomic
save, external-change detection, and restart recovery. Monaco is not allowed
to become filesystem authority because it happens to contain the current
text.

### 5.2 Coordinates are versioned facts

Zed's SumTree-based layers explicitly translate among bytes, points, display
rows, excerpts, and diff positions. Monaco and Pierre have different internal
coordinate systems, so OpenAgents must make those translations explicit:

```text
ProjectFileRef + documentGeneration + UTF-16 range
  ↔ Monaco model URI/version/range
  ↔ LSP URI/version/range
  ↔ excerpt source/projection range
  ↔ Pierre file/hunk identity
  ↔ Git base/index/worktree revision
```

Every range-bearing command or event must carry its document generation.
Applying an agent edit or code action against an older generation must produce
a typed stale result, offer a separately specified rebase, or regenerate the
proposal. Silent best-effort line-number application is not admitted.

### 5.3 Stable model identity

The current plan's proposed URI remains sound:

```text
oa-workspace://<workspace-session-ref>/<document-ref>
```

The URI should map through `ProjectFileRef` and never encode an absolute root.
Renames should preserve a causal link between the previous and next file
reference so open tabs, diagnostics, proposals, and review state can be
rebound or invalidated explicitly.

## 6. The exact agent-to-code relationship to build

This is the main delta from the existing basic editor plan.

### 6.1 An agent session is project-bound, not CWD-bound

**[Zed source]** The native agent keeps per-project state, builds project
context from visible worktrees, refreshes it when worktrees or trusted skills
change, and associates sessions with a project ID. Its thread store persists
parent-session identity and project paths for grouping.

**[Proposal]** Every coding session attaches to:

```ts
type AgentProjectAttachment = {
  sessionRef: SessionRef
  threadRef: ThreadRef
  runRef?: FullAutoRunRef
  project: IdeProjectRef
  worktreeRefs: ReadonlyArray<ProjectRootRef>
  executionProfileRef: ExecutionProfileRef
  placementRef: PlacementRef
  attachmentGeneration: number
}
```

The attachment does not grant tools by itself. It says what project evidence
the session can name. Actual reads, writes, processes, Git mutations, network,
and external effects still pass through ordinary scoped tools, approvals, and
containment.

### 6.2 The context tray is an explicit manifest

Zed's file/directory mentions, related excerpts, diagnostics, rules, skills,
and project context show what good integration feels like. OpenAgents should
make the resulting disclosure more explicit.

```ts
type AgentContextItem = {
  contextItemId: ContextItemId
  source:
    | 'explicit_file'
    | 'explicit_selection'
    | 'open_document'
    | 'diagnostic'
    | 'definition'
    | 'search_result'
    | 'git_change'
    | 'recent_edit'
    | 'project_rule'
    | 'skill'
    | 'retrieval_candidate'
  file?: ProjectFileRef
  documentGeneration?: number
  range?: Utf16Range
  reason: TypedContextReason
  sensitivity: DataClassification
  audience: ContextAudience
  byteCost: number
  tokenEstimate?: number
  truncated: boolean
}
```

The composer and inline edit surface should show a context tray with:

- explicit files, directories, symbols, selections, diagnostics, and changes;
- why an automatic item was proposed;
- whether it stays local or is eligible to reach the selected provider;
- the provider/account/placement destination for the next turn;
- truncation and omitted-item counts;
- remove, pin, and inspect controls.

The context manifest is persisted by reference and linked from the turn/run
receipt. It must not put raw content, paths, secrets, embeddings, or provider-
private events into public-safe projections.

### 6.3 Agents propose version-bound changes

An agent must not mutate Monaco directly. It uses canonical file tools, or it
returns a proposal that the same host authority can apply.

```ts
type AgentEditProposal = {
  proposalId: AgentEditProposalId
  sessionRef: SessionRef
  threadRef: ThreadRef
  runRef?: FullAutoRunRef
  project: IdeProjectRef
  runtimeIdentity: EffectiveRuntimeIdentity
  contextManifestRef: ContextManifestRef
  operations: ReadonlyArray<
    | CreateFileProposal
    | UpdateDocumentProposal
    | MoveFileProposal
    | DeleteFileProposal
  >
  baseDocuments: ReadonlyArray<DocumentSnapshotRef>
  validation: ProposalValidationState
  status:
    | 'streaming'
    | 'ready'
    | 'stale'
    | 'partially_applied'
    | 'applied'
    | 'rejected'
    | 'undone'
}
```

`UpdateDocumentProposal` uses typed operations or an exact patch against a
base document generation. `Create`, `move`, and `delete` remain independently
permissioned. Application records exact preimages/postimages or content
digests according to data policy, the approving actor/policy, conflicts,
skipped operations, and the resulting document generations.

### 6.4 Review is a first-class code state

Zed's agent diff and MultiBuffer model point toward a better review loop than
chat messages containing patches:

1. Proposed files appear in Changes immediately with `streaming`, `ready`,
   `stale`, or `applied` status.
2. Selecting a change opens a Pierre diff in the main editor area, with the
   corresponding agent turn and rationale available as an inspector—not as a
   replacement for the code.
3. Accept/reject works per proposal, file, or independently addressable hunk
   only where hunk identity is stable.
4. Apply checks every base document generation and filesystem precondition.
5. Undo is independent of Git and remains available until its retention bound
   expires.
6. Tests, diagnostics, formatter results, and Git state after apply attach to
   the proposal as evidence rather than becoming prose claims.

This is how the human and agent share code without sharing an unsafe mutable
editor object.

### 6.5 Code and conversation backlink each other

**[Proposal]** Add stable cross-navigation:

- agent tool calls and change summaries deep-link to exact files/ranges and
  document generations;
- the editor gutter or change marker can reveal the creating proposal/turn;
- diagnostics and test failures deep-link back to the turn or run that caused
  them where causality is known;
- selecting a historical link whose generation is gone opens a snapshot/diff
  view or an explicit unavailable state, never a misleading current line;
- child-agent work is shown under the parent session/run with the exact
  project/worktree attachment.

## 7. Agent UI interactions worth adapting

The agent IDE should feel editor-first while making agent state spatially
legible.

### 7.1 Primary layout

```text
primary rail       project navigation        main workspace          inspector
────────────       ──────────────────        ──────────────          ─────────
Thread             Pierre file tree          Monaco editor           agent turn
Editor             Search / Problems         Pierre diff             context
Changes            Symbols / Outline         excerpt/review view      evidence
Terminal           Changed files             terminal/task item       approvals
Preview
Artifacts
```

The inspector is optional and resizable. It may explain agent context,
evidence, or a proposal, but it never owns the only file view. Opening a file
always targets the main workspace.

### 7.2 Agent context in the editor

Useful, bounded indicators include:

- files and exact ranges explicitly attached to the next turn;
- active proposal ranges, with non-color status cues;
- changed-by session/run attribution where evidence exists;
- pending question or approval associated with a range/tool action;
- diagnostics and tests produced after an agent apply;
- current agent focus only when reported by a typed runtime event.

Do not create a noisy real-time cursor theater from token streams. Presence or
“agent is looking here” is displayed only from an observed event with a
document generation and expiry. Inference from tool text is not presence.

### 7.3 Contextual actions through one command registry

Editor, tree, diff, Problems, search, terminal, palette, menus, shortcuts,
mobile, and model-proposed actions must resolve the same commands. Initial
agent-oriented commands should include:

- Ask about Selection
- Explain Symbol
- Propose Edit for Selection
- Fix Diagnostic
- Add File/Selection/Diagnostic/Change to Context
- Run Relevant Tests
- Review Current Changes
- Show Creating Turn
- Open in New Worktree
- Continue on Desktop

Whether a command is shown or executable comes from project capability,
selection, runtime, and authority schemas—not scattered UI conditionals.

### 7.4 Thread and worktree organization

**[Zed source]** Threads are grouped by project paths and may carry parent
session IDs. Zed also has careful worktree archival logic: it checkpoints Git
state, tracks Zed-created worktrees, refuses destructive cleanup when
provenance or identity no longer matches, and accounts for a worktree loaded
in multiple projects.

**[Proposal]** OpenAgents should:

- group coding threads by canonical project and worktree, then by parent/child
  topology;
- display runtime type separately: owned native, Claude Code, Codex, Pi,
  external protocol, in-process emulation, or terminal projection;
- retain `initialProjectAttachmentRef` and current attachment separately;
- mark agent-created worktrees with provenance and exact creation receipts;
- checkpoint before archival and refuse cleanup if the worktree was replaced,
  moved, externally recreated, dirty beyond policy, or still attached;
- keep archive, delete, and detach as different commands.

This should influence the existing worktree and delivery lanes; it does not
authorize a new destructive flow in the basic editor milestone.

## 8. Context and edit prediction: use a portfolio, not magic embeddings

Zed's current source is valuable because its edit prediction does not reduce
“understands the codebase” to a persistent vector database.

**[Zed source]** At the pinned tree it uses:

- nearby identifiers and LSP definitions through `RelatedExcerptStore`;
- in-memory BM25 over tracked files, with 40-line chunks, 10-line overlap, up
  to 12 selected chunks, no more than three per file, and a 1,000,000-byte
  individual-file cap;
- recent edits, open/view history, cursor context, diagnostics, uncommitted
  changes, and editable ranges;
- a file co-change graph derived from up to 5,000 Git commits.

Those numbers describe Zed, not OpenAgents targets. A defined
`embeddings_dir` has no active repository call site at the pin, so it is not
evidence that Zed currently persists a repository embedding index.

**[Inference]** Zed's likely reason for this portfolio is latency and
provenance. Open buffers, definitions, diagnostics, recent edits, and Git
co-change are cheap, local, explainable signals. BM25 broadens recall without
requiring an always-on embedding pipeline. Each signal also ages and
invalidates differently, so combining candidates late is safer than forcing
everything into one opaque index.

**[Proposal]** OpenAgents should define a context-candidate protocol before
choosing retrieval implementations:

```ts
type ContextCandidate = {
  item: AgentContextItem
  sourceGeneration: number
  freshness: FreshnessClass
  selectionReason: TypedContextReason
  score?: number
  scoreKind?: 'semantic' | 'lexical' | 'recency' | 'structural' | 'explicit'
  providerEligible: boolean
  invalidationRefs: ReadonlyArray<GenerationRef>
}
```

The central typed semantic selector or structured query planner chooses among
candidates. Lexical parsing is acceptable after the route is selected; ad hoc
keyword routing for user intent, retrieval, or tool choice remains forbidden.

Indexing must be unbundled and explicit:

- `off`, bounded local lexical, local semantic, and disclosed remote semantic
  are distinguishable configurations;
- every mode states what it reads, derives, persists, uploads, retains, and
  deletes;
- ignored/secret files and workspace policy apply before candidate creation;
- generated chunks and embeddings inherit source deletion/tombstone state;
- provider disclosure previews what leaves the host;
- “no index” still supports explicit files, open buffers, LSP, diagnostics,
  recent edits, and Git context.

## 9. Excerpts are the bridge among IDE and agent views

Zed's MultiBuffer is one of its most important long-term ideas. Search results,
references, diffs, diagnostics, and agent context can become editor-like
views composed from exact source excerpts without pretending to be a physical
file.

**[Proposal]** Reserve this contract now:

```ts
type ExcerptRef = {
  excerptId: ExcerptId
  source: DocumentSnapshotRef
  sourceRange: Utf16Range
  projectionRange: Utf16Range
  writable: boolean
}

type ExcerptSet = {
  excerptSetId: ExcerptSetId
  purpose:
    | 'workspace_search'
    | 'references'
    | 'problems'
    | 'git_review'
    | 'agent_context'
    | 'agent_proposal'
  excerpts: ReadonlyArray<ExcerptRef>
  generation: number
}
```

The first version should be read-only. Writable multi-file excerpt editing is
deferred until coordinate mapping, stale-generation behavior, undo, and
authority have dedicated proof. Reserving the identity now prevents search,
Problems, review, and agent context from inventing incompatible pseudo-file
formats.

## 10. Language intelligence should be shared evidence

Zed treats parsing and LSP as project capabilities rather than editor
ornament. OpenAgents should do the same.

### Required first language capability contract

- lifecycle: unconfigured, starting, ready, degraded, stopped, failed;
- placement: local, owner-managed remote, OpenAgents-managed, or compatible
  audited provider;
- document and service generations on every result;
- cancellation, supersession, timeout, and partial-result semantics;
- typed support/unsupported outcomes for diagnostics, definitions,
  references, symbols, hover, rename, formatting, code actions, semantic
  tokens, inlay hints, and folding;
- main-owned URI translation with no raw-root renderer leak;
- status and failure visible in the editor and to an agent using the service.

### Agent-specific use

- “Fix Diagnostic” sends the exact diagnostic identity, source service, file,
  range, and generations—not copied error prose alone.
- “Explain Symbol” resolves the symbol and relevant definitions before
  provider context is assembled.
- agent tool results for diagnostics/references become the same typed objects
  the Problems and references views render.
- post-edit verification compares diagnostics from known before/after service
  generations and never claims “fixed” because the agent said so.
- local parsing may provide immediate outline/symbol context while a remote LSP
  is starting or unavailable; the UI states the evidence tier.

## 11. Git, worktrees, and delivery

Zed's recent Git history and source show that exact mutable index behavior is
hard even in a mature IDE. Its model distinguishes HEAD, index, and worktree,
tracks staged/unstaged/partial state, and uses optimistic index operations.
The prior audit found a recent ambiguous-hunk corruption fix.

**[Proposal]** Preserve the existing safe sequencing:

1. First ship read-only status, diff, base identity, and changes navigation.
2. Bind every Pierre diff/hunk to exact repository, HEAD, index, worktree, and
   document generations.
3. Let agent proposals exist independently of Git; applying or undoing an
   agent change must not require a commit.
4. Add staging, partial staging, discard, commit, and push only through a
   separately admitted Git mutation packet with expected versions and exact
   post-state receipts.
5. Keep delivery status separate from agent completion: proposed, applied,
   saved, tested, committed, pushed, and accepted are distinct facts.

Worktrees should be first-class isolation objects for agent runs. A session or
Full Auto run may request a dedicated worktree, but creation, attachment,
archive, and removal remain host-owned operations with provenance and
receipts. A worktree path is not a session identity, and a session may not
delete a worktree merely because it created files inside it.

## 12. Local and remote IDE symmetry

Zed's remote architecture keeps UI/model concerns local while remote stores
own files, LSP, tasks, and terminals behind the same project interface. The
exact split is not mandatory for OpenAgents, but the interface symmetry is.

**[Proposal]** A project-capability request should not change shape because
placement changes:

```text
open file / save / search / diagnostics / run task / open terminal
                        ↓
              project capability interface
                        ↓
       local | owner-remote | managed | unsupported
```

Every response and receipt carries effective placement. Attachments have
exclusive generations where execution authority requires exclusivity.
Reconnect must distinguish:

- temporarily unreachable;
- remote component missing;
- component version incompatible;
- credential/grant revoked;
- project moved or attachment generation stale;
- service degraded but cached evidence still viewable.

OpenAgents should not silently download and execute a remote helper, relocate
the session, upload a project, or substitute a managed service. Those are
explicit placement and custody decisions.

## 13. Trust, skills, context servers, and extensions

**[Zed source]** Project-local skills are loaded only from worktrees treated as
trusted, and project context refreshes when trust or skill state changes.
Zed's extension guest is WASM, but host capabilities include broad process,
download, and package-manager effects under some defaults. Its native agents
use platform containment and explicit grants, but source study does not turn
those into OpenAgents assurance.

**[Proposal]** Separate four decisions:

1. **Project attachment:** this session refers to this project.
2. **Instruction trust:** these project rules/skills may influence this
   runtime.
3. **Context disclosure:** these exact code/evidence items may reach this
   provider/account/placement.
4. **Tool authority:** this runtime may perform these filesystem, process, Git,
   network, or external effects.

Trusting a project must not automatically grant all four. A project skill may
be readable but disabled; a context server may be configured but unavailable
to the chosen runtime; an external harness may expose native tools that remain
blocked by the canonical host broker.

Future extensions should use a versioned component contract but remain deny by
default. WASM memory isolation does not justify wildcard process or network
effects. Extension capabilities, provenance, version, placement, data access,
and active grants belong in the same user-visible inventory as harness tools.

## 14. Local state OpenAgents must declare

Zed persists much more than preferences: workspace/pane layout, project and
remote connections, open and unsaved editors, selections/folds/scroll,
breakpoints, trust, searches, commands, terminal state, Git views, agent
thread metadata, and compressed thread content in a separate database. It also
installs or caches extensions, languages, formatters, external agents, remote
servers, themes, snippets, prompts, and update assets.

**[Proposal]** Before each OpenAgents IDE store lands, add it to a user-visible
local data inventory with:

| Required declaration | Question answered |
| --- | --- |
| purpose and schema owner | why does this store exist and which service owns it? |
| data classes | content, path, metadata, transcript, terminal, diagnostic, embedding, secret? |
| physical location | where on the machine is it stored? |
| path sensitivity | are absolute roots or filenames present? |
| encryption | at rest, vault-bound, or plaintext? |
| retention and quota | how long and how large can it grow? |
| export | what user-facing export includes it? |
| deletion/tombstone | what removes it and derived artifacts? |
| Sync/backup | may it leave the device, and to which audience? |
| runtime access | may external harnesses or extensions read it? |
| crash behavior | can a crash leave partial or recoverable content? |

At minimum, treat these as separate stores/policies:

- workspace roots and recent projects;
- tree/index metadata;
- document snapshots and unsaved recovery;
- search history and search indexes;
- language caches and diagnostics;
- Git evidence and graph caches;
- terminals, task output, and debug state;
- agent threads and provider-private events;
- context manifests and generated excerpts;
- agent proposals, checkpoints, and undo records;
- skills, rules, context-server configuration, and trust grants;
- optional lexical/semantic indexes and embeddings;
- telemetry, crash, and update queues.

No database name or directory is evidence of active indexing. Runtime call
sites and observed behavior must back every product disclosure.

## 15. Desktop, mobile, and web share vocabulary—not authority

The requested surface specs intentionally assign different roles.

| Capability | Desktop | Mobile | Web |
| --- | --- | --- | --- |
| full Monaco editing | yes, primary Editor mode | no; bounded review comments/small staged edits only | no; explicitly cut |
| file tree/search | authoritative projection over attached project | safe relative-ref projection | safe supervision projection where synced |
| Problems/symbols | full project capability | inspect and deep-link | inspect synced evidence |
| agent context manifest | create, inspect, modify, disclose | inspect safe summary; bounded selection for commands | inspect safe summary; bounded selection for remote launch |
| agent proposal/diff | full review/apply/undo under policy | review, comment, approve bounded action | review/approve/steer; no browser execution authority |
| terminal/task | local or remote authoritative projection | remote-control projection | remote-control/log projection |
| raw paths/content | main-owned; renderer receives bounded refs/content | prohibited raw filesystem paths | prohibited raw filesystem paths/private evidence |
| placement | execute locally or attach remote | select/supervise target | select/supervise target |
| canonical transcript/execution | authoritative host/runtime | never | never by implication |

### Mobile adaptation

Mobile's Files and Changes modes should render `ProjectFileRef`, status,
diagnostics, proposals, test outcomes, and receipts. It can deep-link to the
same file/proposal on Desktop and may issue typed durable review commands
through the exactly-once outbox. It must not receive raw roots, become a pixel-
streamed desktop, or quietly grow a general code editor.

### Web adaptation

Web may launch, inspect, approve, answer, steer, queue, pause, stop, rerun, and
hand off the same session. It may show diffs, artifacts, logs, Problems, and
proposal evidence as safe typed projections. `CUT-WEB-02` remains binding: an
in-browser IDE/editor is not part of this direction, and the browser never
gains workspace execution authority.

### Cross-surface continuation

A mobile or web “Open on Desktop” link carries stable session, project,
proposal/file, and safe snapshot references. Desktop revalidates the current
attachment and authorization before resolving them. It must not accept a raw
path or inherit the controller's weaker projection as local authority.

## 16. Full Auto and the IDE project graph

Full Auto is not an editor mode and its run view has no ordinary composer.
It can still benefit from the same project evidence model.

**[Proposal]** A Full Auto run record may bind an `IdeProjectRef` and dedicated
worktree attachment in addition to its already distinct `runRef`, `threadRef`,
objective, done condition, execution profile, provider routing policy,
guardrails, and budgets.

The run view can safely project:

- current project/worktree display identity and attachment generation;
- changed files and proposal/application states;
- diagnostics before/after known generations;
- tests/tasks with exact invocation and outcome refs;
- Git/delivery state as separate facts;
- child-agent project attachments and gaps;
- context/adaptation provenance through existing safe schemas;
- the final bounded run report with code-evidence refs.

It must not:

- widen workspace grants because the IDE has the project open;
- infer completion from a clean editor or agent prose;
- mutate a proposal from the read-only run view;
- merge `runRef`, `threadRef`, project, worktree, provider session, or harness
  session identity;
- expose private prompts, transcripts, embeddings, paths, tool output, or
  retrieval scores through public-safe projections;
- change a frozen HarnessPolicyBundle or adaptation snapshot mid-run.

This creates a strong morning-review experience: the owner can see exactly
what changed, what evidence exists, what remains unverified, and which Desktop
project view will open it.

## 17. Concrete deltas to the existing IDE packet sequence

The accepted basic IDE plan defines IDE-00 through IDE-07. Zed should refine
those packets, not replace or silently reorder them.

### IDE-00 — contracts and invariants

Add or reserve:

- `IdeProjectRef`, `ProjectRootRef`, `ProjectFileRef`;
- separate attachment, document, disk, service, and Git generations;
- `ProjectCapabilityState` and effective placement;
- read-only `ExcerptRef`/`ExcerptSet` identity;
- `AgentProjectAttachment`, `AgentContextItem`, and proposal identity;
- the rule that project attachment, context disclosure, and tool authority
  are independent.

### IDE-01 — packaged adapter spike

In addition to proving Monaco/Pierre packaging, prove:

- one shared theme maps to Monaco, Pierre tree/diff, terminal, and chrome;
- one command opens a file from tree, Command-E, and a synthetic agent
  backlink into the same Monaco model;
- cancellation and teardown dispose models/listeners without leaving project
  services or content reachable.

### IDE-02 — path index and Pierre tree

Add Zed-derived behavior targets:

- multi-root identity and reorder;
- virtualized flattened projection, not 500-row rendering;
- folded single-child directories and sticky ancestor context;
- explicit incomplete/truncated/error scan states;
- Git, diagnostic, conflict, hidden, ignored, symlink, and remote badges with
  non-color cues;
- stable selection/scroll/focus across incremental updates;
- all mutations and terminal/reveal/compare actions through commands.

### IDE-03 — Monaco lifecycle

Add:

- document generation separate from disk revision;
- stable `ProjectFileRef` to model mapping;
- incremental typed change events with gap/resync behavior;
- stale language, diff, code-action, and agent-proposal rejection;
- editor-first cold open independent of chat/provider/LSP readiness.

### IDE-04 — navigation and file operations

Add:

- one navigation history for file, search, symbol, diagnostic, hunk, and agent
  backlinks;
- rename/move rebinding or explicit invalidation of open documents, context
  items, excerpts, and proposals;
- project/worktree/thread grouping and safe worktree provenance fields.

### IDE-05 — Pierre diff and review

Add:

- exact HEAD/index/worktree/document generation identity;
- agent proposal as a diff source distinct from Git;
- per-file and safe per-hunk review;
- apply/undo receipts and post-apply diagnostics/test attachments;
- no mutable Git claim in this packet.

### IDE-06 — language, LSP, and Problems

Add:

- visible capability lifecycle and placement;
- versioned diagnostics/symbols/navigation/code actions;
- typed unsupported/degraded outcomes;
- read-only excerpt sets for Problems, references, and context;
- agent commands consume the same diagnostic/symbol identities.

### IDE-07 — themes, accessibility, performance, release

Add packaged journeys and budgets for:

- editor-first cold open;
- large multi-root tree and rapid file switching;
- diagnostic bursts and stale result rejection;
- streaming proposal diff and apply/undo;
- restart with dirty documents plus pending proposal;
- worktree switch isolation;
- keyboard-only context selection and review;
- screen-reader names for tree status, diagnostics, changes, agent status, and
  evidence tier.

### Proposed follow-on: IDE-08 — agent context and proposal loop

After IDE-00 through IDE-07 establish a real IDE:

- project-bound session attachment;
- explicit context tray and disclosure manifest;
- selection/file/diagnostic/change mentions;
- code-to-turn and turn-to-code backlinks;
- version-bound single- and multi-file proposals;
- Pierre review, apply, reject, undo, and evidence attachment;
- native/external/terminal runtime identity and child topology.

### Proposed follow-on: IDE-09 — completion and next-edit intelligence

After document/language/context generations are real:

- inline completion and next-edit presentation in Monaco;
- candidate portfolio from explicit context, local parse/LSP, recent edits,
  diagnostics, Git changes/co-change, lexical and optional semantic retrieval;
- provider disclosure and cost/retention facts;
- prediction base generation, editable ranges, accept/reject, latency and
  quality telemetry under declared policy;
- no persistent index requirement and no unsupported embeddings claim.

## 18. Delivery sequence

### Phase 0 — preserve the working file-open path

Keep the already landed Command-E Files, Pierre tree, primary rail, Finder
Open With, and editor-first cold-open work green while replacing temporary
drivers.

### Phase 1 — coherent basic IDE

Deliver IDE-00 through IDE-03: project identity, bounded tree snapshots, real
Monaco models, safe open/edit/save/conflict/recovery, and theme projection.

### Phase 2 — navigation, review, and language

Deliver IDE-04 through IDE-07: file operations, search/navigation, exact diff,
LSP/Problems, accessibility, performance, and packaged release evidence.

### Phase 3 — agent context loop

Deliver proposed IDE-08: project-bound sessions, explicit context, code
backlinks, version-bound proposals, review/apply/undo, and evidence convergence.

### Phase 4 — AI editing breadth

Deliver proposed IDE-09: completion, next edit, inline generation, multi-file
apply, and selectable retrieval/indexing under explicit data policy.

### Phase 5 — deeper IDE capability

Add tasks, terminal, debug, outline, richer excerpt views, remote placement,
worktree lifecycle, and separately admitted Git mutation.

### Phase 6 — optional collaboration and extensions

Only after consistency, authority, component ABI, data policy, and containment
have independent specifications and proof plans.

## 19. Release-blocking acceptance journeys

1. **Cold Finder open.** With Desktop stopped, Open With a supported source
   file. A real Monaco editor appears in the primary main region before chat,
   provider, Git, index, or LSP hydration finishes.
2. **Command-E symmetry.** Open the same file through Command-E and the Pierre
   tree. All routes resolve the same document model and navigation history.
3. **Safe save.** Edit and save against the expected disk revision; an
   external modification produces an explicit conflict, not overwrite.
4. **Restart recovery.** Restart with dirty documents and a pending agent
   proposal. Both restore with exact status and without claiming disk save.
5. **Diagnostic-to-agent loop.** Select a versioned diagnostic, ask an agent
   to fix it, inspect the exact context manifest, review the proposal diff,
   apply, rerun diagnostics/tests, and see evidence linked to the turn.
6. **Stale proposal.** Change the document after proposal creation. Apply
   refuses or enters an explicit rebase flow; it never patches current lines
   by guess.
7. **Multi-file proposal.** Review, accept, reject, and undo at supported
   granularity across multiple files while Git status and disk save state stay
   independently truthful.
8. **Agent backlink.** Open a file/range from an agent tool result, then reveal
   the creating turn from the code. Historical generations resolve to a
   snapshot/diff or explicit unavailable state.
9. **Worktree isolation.** Two sessions on two worktrees open files with the
   same relative path. Models, diagnostics, proposals, terminals, and Git
   evidence never cross.
10. **Runtime substitution refusal.** Switching from an owned/native runtime
    to Claude Code, Codex, Pi, external protocol, or terminal preserves project
    references but does not fabricate shared session semantics or authority.
11. **Remote placement.** Attach a remote project, lose connectivity, recover,
    and prove stale attachments/results cannot mutate the current project.
12. **Mobile review.** Receive a privacy-generic alert, inspect a safe proposal
    diff and evidence, issue one durable review command, and open the exact
    project/proposal on Desktop without transferring a raw path or credential.
13. **Web supervision.** Inspect the same safe evidence and steer/pause/hand
    off without a browser IDE or workspace execution authority.
14. **Data deletion.** Delete a project/session under its policy and prove
    eligible document recovery, context excerpts, proposals, indexes, and
    derived embeddings are removed or tombstoned with explicit residuals.

## 20. Verification and performance posture

Zed's source uses virtualization, background projections, snapshotting,
cancellation, property tests, integration tests, and benchmarks throughout
the editor stack. OpenAgents should adopt that discipline against its own
packaged architecture.

### Property and bounded-model targets

- normalize/reject project paths and multi-root collisions;
- attachment-generation invalidation;
- document change sequence/gap/resync;
- UTF-8/UTF-16/range conversion across Monaco and LSP;
- excerpt source/projection mapping;
- stale agent/code-action apply refusal;
- save/reload/external-conflict/recovery state machine;
- HEAD/index/worktree/document identity;
- worktree create/attach/archive refusal conditions;
- context disclosure filtering and derived-data tombstones.

### Packaged performance budgets to measure

- cold file-open input-ready p50/p95/p99;
- large-tree first projection, expand, scroll, filter, and reveal;
- Monaco model create/switch/dispose;
- search and Problems result bursts;
- LSP startup and stale-result drop;
- streaming agent proposal diff;
- apply/undo and post-apply diagnostics;
- restart restore;
- local versus remote latency classes.

No Zed benchmark number is an OpenAgents target until measured on the same
fixture and packaged OpenAgents build. Fixture, unsigned/dev, packaged,
signed/notarized, real-provider, and owner-real evidence remain distinct.

## 21. What not to take from Zed

- Do not adopt GPUI or create a second Rust UI/state/control plane.
- Do not reimplement Monaco from Zed's rope, CRDT, SumTree, MultiBuffer, or
  Editor internals.
- Do not port the GPUI Project Panel; use Pierre behind the owned adapter.
- Do not copy GPL-default source without a separate legal decision.
- Do not make raw roots, absolute paths, or ambient CWD portable identity.
- Do not let agents mutate Monaco or renderer state directly.
- Do not treat project membership, trust, context disclosure, and tool
  authority as one switch.
- Do not claim an external ACP/terminal runtime has native OpenAgents session
  semantics it does not expose.
- Do not treat WASM as complete containment or accept wildcard host effects.
- Do not add mutable Git to the basic editor milestone.
- Do not introduce collaborative editing as an incidental Sync feature.
- Do not require repository embeddings for code understanding.
- Do not claim active embeddings because a directory or schema exists.
- Do not upload or persist a repository index without a declared custody,
  retention, deletion, and provider-disclosure contract.
- Do not move the full editor to mobile or web; preserve their supervision
  roles.
- Do not call component assembly “VS Code parity,” “Zed parity,” or “Cursor
  parity” until maintained journey corpora pass at the claimed evidence tier.

## 22. Decisions this analysis makes and leaves open

### Recommended decisions

1. Adopt Zed as the primary integrated agent-IDE reference.
2. Keep Monaco and Pierre as the practical component choices.
3. Add a typed `IdeProjectRef`/`ProjectFileRef` and composed project capability
   graph as the center of the architecture.
4. Make agent sessions project-bound but independently authorized.
5. Make context disclosure explicit and proposal application version-bound.
6. Reserve read-only excerpt-set identity before workspace search, Problems,
   review, and agent context diverge.
7. Add IDE-08 for the agent context/proposal loop and IDE-09 for completion and
   next-edit intelligence after the basic IDE packets.
8. Share typed project evidence with mobile/web while retaining their explicit
   no-full-editor/no-execution boundaries.

### Still needs an admitted implementation decision

- exact Effect service/package ownership and schema names;
- whether main persists canonical incremental text or checkpointed snapshots
  around Monaco;
- the first LSP implementation and local/remote placement split;
- exact proposal operation format and rebase policy;
- context-manifest content retention and provider disclosure UX;
- local lexical/semantic index implementations and defaults;
- worktree creation/archive retention and recovery policy;
- performance budgets after baseline measurement;
- collaboration consistency model and extension component ABI.

## 23. Source map

Most consequential Zed paths at the pin:

| Concern | Zed source |
| --- | --- |
| project and multi-root identity | `crates/project/src/project.rs`; `crates/worktree/src/worktree.rs` |
| buffer and language state | `crates/text/src/text.rs`; `crates/language/src/buffer.rs`; `crates/project/src/buffer_store.rs` |
| excerpts and editor | `crates/multi_buffer/src/multi_buffer.rs`; `crates/editor/src/editor.rs` |
| project tree | `crates/project_panel/src/project_panel.rs` |
| language capabilities | `crates/project/src/lsp_store.rs`; `crates/language`; `crates/languages` |
| Git/diff | `crates/buffer_diff/src/buffer_diff.rs`; `crates/project/src/git_store.rs`; `crates/git`; `crates/git_ui` |
| native agent/project context | `crates/agent/src/agent.rs`; `crates/agent/src/db.rs` |
| agent UI/context/review | `crates/agent_ui/src/mention_set.rs`; `crates/agent_ui/src/agent_diff.rs`; `crates/agent_ui/src/inline_assistant.rs`; `crates/agent_ui/src/conversation_view` |
| agent worktree lifecycle | `crates/agent_ui/src/thread_worktree_archive.rs` |
| external and terminal agents | `crates/acp_thread`; `crates/agent_ui/src/terminal_thread_metadata_store.rs` |
| edit context | `crates/edit_prediction_context/src/edit_prediction_context.rs`; `crates/edit_prediction_context/src/bm25_context.rs`; `crates/edit_prediction_context/src/git_log_context.rs` |
| sandbox and permissions | `crates/agent/src/sandboxing.rs`; `crates/agent/src/tool_permissions.rs` |
| persistence and local paths | `crates/db/src/db.rs`; `crates/paths/src/paths.rs`; `crates/workspace/src/persistence.rs`; `crates/editor/src/persistence.rs` |
| remote development | `crates/remote/src/protocol.rs`; `crates/remote_server`; `docs/src/remote-development.md` |

Requested OpenAgents sources that constrain this recommendation:

- `docs/transcripts/257.md`
- `docs/ide/2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md`
- `specs/desktop/desktop-trust-complete-workbench.product-spec.md`
- `specs/desktop/full-auto.product-spec.md`
- `specs/desktop/full-auto.assurance-spec.md`
- `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`
- `specs/web/openagents-com-sales-landing.product-spec.md`
- `specs/web/openagents-com-trust-surface.product-spec.md`
- `docs/teardowns/2026-07-18-zed-teardown.md`

## Final recommendation

Build the first complete OpenAgents agent-IDE loop as one causal path:

```text
attach exact project/worktree
→ open a real Monaco document in the main workspace
→ obtain versioned language and Git evidence
→ select explicit code/context for a named runtime
→ receive a version-bound agent proposal
→ review it in Pierre against exact bases
→ apply through canonical authority
→ rerun diagnostics/tests
→ save, undo, commit, and deliver as separate facts
→ preserve the evidence and project links across restart and supervision
```

That is the useful meaning of “take Zed.” It is not a Rust port and not a skin.
It is an IDE whose human, agents, tools, and remote controllers all know which
project, file, version, capability, proposal, and receipt they are talking
about—without any of them acquiring more authority merely because they share
those facts.
