# Zed: Native IDE, Project Graph, Remote Runtime, and Agent Workbench Teardown

Date: 2026-07-18

## Executive verdict

Zed is the strongest public **integrated IDE architecture** reference in the
current teardown set. Pierre is the better focused dependency candidate for
trees and diffs. Monaco remains the practical code-editor component for
OpenAgents Desktop. Zed supplies something neither does: a coherent account of
how an editor, worktree scanner, language layer, Git model, remote environment,
collaboration system, local database, extension host, and agent workbench fit
together without becoming unrelated panels.

The central design is a typed vertical stack:

```text
GPUI application and entity runtime
        ↓
SumTree coordinate/index substrate
        ↓
Rope → collaborative text buffer → language buffer
        ↓
MultiBuffer/excerpts/diffs → Editor
        ↓
Worktree stores → Project service graph
        ↓
workspace / project panel / LSP / Git / terminal / tasks / debug
        ↓
remote projects / collaboration / native agent / ACP agents
```

That stack is why Zed feels integrated. The project panel, editor, search,
diagnostics, Git review, remote sessions, and agents all refer to the same
project paths, buffers, versions, and service lifecycles. They are not merely
visual components pointed at an ambient current directory.

OpenAgents should **adapt that coherence, not Zed's implementation**. The
target remains Effect Native and the existing typed Electron main boundary,
Monaco for code editing, and pinned Pierre tree/diff packages behind an owned
adapter. The useful Zed lessons are multi-root project identity, one canonical
buffer/version plane, first-class excerpt views, capability-scoped language
services, exact local-versus-remote placement, unified actions, explicit local
state, project-bound agent context, and performance budgets over the complete
packaged workbench. OpenAgents should not import GPUI, rebuild Monaco in Rust,
adopt Zed's full CRDT or SCM engine, or treat a WASM extension host as complete
containment.

## Audit identity and evidence boundary

This is a source-only, point-in-time audit of the existing external-reference
checkout. No upstream build, installer, test, extension, agent, network
service, or release artifact was executed.

| Field | Audited value |
| --- | --- |
| Upstream | `zed-industries/zed` |
| Canonical URL | `https://github.com/zed-industries/zed` |
| Default branch | `main` |
| Commit | `f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4` |
| Tree | `bc5e231b224529baeb1a3cc2c8ea54eff8ac21ad` |
| Commit timestamp | `2026-07-18T22:40:55Z` |
| Commit subject | `agent_ui: Show controls after every agent message (#61245)` |
| Application version | `1.13.0` in `crates/zed/Cargo.toml` |
| Repository size | 4,206 tracked files. 239 Crate directories. About 590 MiB packed locally |
| License | GPL-3.0-or-later by default. Marked components may use Apache-2.0 |

The nearest result from `git describe` is an unrelated extension tag, not an
application release containing this commit. The commit and tree hashes above
are therefore the audit identity. The checkout was clean after the workspace
sync fast-forwarded it from `7eb8af27a6` to the pinned commit. Forty-five
commits had landed since 2026-07-16, including active Git, diff, project, and
agent UI changes. **[history]** That velocity makes unpinned statements about
Zed especially weak.

Evidence labels follow the teardown catalog convention:

- **[source]** is directly encoded at the audited tree.
- **[test]** is encoded in tests, benchmarks, or checked verification.
- **[history]** is supported by checked commit history.
- **[inferred]** is a reasoned architectural conclusion.
- **[limitation]** bounds what this source-only audit proves.

Zed's repository-local instruction files were inspected only as source
evidence. They are upstream project material, not instructions to this audit.

## 1. Repository and product shape

Zed is predominantly a Rust workspace rather than a web application wrapped in
a desktop shell. The root workspace joins roughly 239 crates spanning:

- `gpui`, platform backends, windowing, text, input, and rendering.
- `sum_tree`, `rope`, `text`, `language`, `multi_buffer`, and `editor`.
- `fs`, `worktree`, `project`, `workspace`, and `project_panel`.
- `lsp`, `git`, `git_ui`, search, outline, diagnostics, tasks, terminal, and
  debugger support.
- themes, settings, keymaps, command palette, snippets, Vim mode, and
  persistence.
- extension registry, extension host, WASI guest API, and language packages.
- remote clients, a headless remote server, collaboration, channels, calls,
  and shared projects.
- native agents, the agent UI, ACP threads, context servers, skills, edit
  prediction, and model providers.

`crates/zed` is the application assembly point. Its dependency list makes the
product thesis visible: there is one application graph, not a separate IDE and
agent application loosely sharing a window. Native platform crates cover
macOS, Linux/FreeBSD, and Windows. The repository also builds WASIp2 extension
components, a web GPUI target, and a musl remote server. **[source]**

The Rust toolchain is pinned to 1.95.0 at this revision. That detail matters
less as a target choice than as evidence of a tightly coordinated application,
framework, extension ABI, and remote-server release train. **[inferred]**

## 2. GPUI is a combined UI, state, and concurrency runtime

Calling GPUI a renderer undersells it. `crates/gpui/src/app.rs:679` shows an
`App` that owns the platform, text system, action registry, executors, entities,
windows, focus, keymaps, listeners, observers, globals, assets, HTTP client,
and invalidation state. `crates/gpui/src/app/entity_map.rs:414` defines typed
`Entity<T>` handles with controlled read/update operations. **[source]**

The checked `.rules` explains the operating model:

- `App`, `Context<T>`, `AsyncApp`, and `Window` expose progressively scoped
  application and view authority.
- UI and entity mutation run on one foreground thread.
- background work is explicitly spawned and must return to the relevant
  entity/application context to mutate state.
- tasks are cancellation-bearing resources: dropping a task cancels it unless
  it is awaited, detached, or stored.
- actions, focus, observation, subscriptions, and `notify` drive invalidation
  through typed contexts.

**[inferred]** GPUI reduces coordination cost because ownership, mutation,
render invalidation, actions, focus, and async lifetime share one vocabulary.
The same convenience makes it architectural gravity: importing one widget is
not importing this model.

**OpenAgents implication:** preserve the existing Effect Native and typed
Electron main architecture. Adapt Zed's explicit entity lifetime, action
identity, cancellation ownership, and scoped-context discipline into Effect
services and scopes. Do not add a second Rust UI/state runtime just to pursue
visual parity.

## 3. SumTree is the common coordinate algebra

`crates/sum_tree/src/sum_tree.rs:213` defines a persistent, cloneable
`SumTree<T>` backed by an `Arc` node. Items summarize themselves, so callers
can seek and transform positions in more than one dimension. `crates/rope` uses
it for text chunks. Text buffers use it for visible and deleted text,
fragments, and edit metadata. MultiBuffer uses it for excerpt and diff
coordinates. UI projections use related structures for rows and layouts.
**[source]**

This is more important than the fact that Zed has a B+ tree. **[inferred]** A
single summary/seek abstraction lets the system answer “where is this byte,
UTF-16 offset, point, display row, excerpt, or diff hunk?” without every layer
inventing an unrelated index. Persistent snapshots also make background
parsing, diffing, and rendering safer because readers can hold stable views
while foreground state advances.

The OpenAgents IDE should not copy SumTree. Monaco already owns the hardest
text and display coordinates, and Pierre owns its projection internals. The
portable lesson is to make coordinate and version conversion explicit:

```text
WorkContext + relative path + document generation
  ↔ Monaco model URI/version/range
  ↔ language-service URI/version/range
  ↔ Pierre file/hunk projection identity
  ↔ Git evidence revision
```

Every conversion should be typed, generation-bound, and reject stale input.
Ambient strings and line numbers without a document revision are not enough.

## 4. The editor is a vertical stack, not a widget

### 4.1 Rope and collaborative text

`crates/rope/src/rope.rs:26` represents text as a SumTree of chunks with
boundary-aware slicing and replacement. `crates/text/src/text.rs:59` then
builds a collaborative text buffer with visible/deleted ropes, fragments,
insertion slices, replica identity, Lamport ordering, global versions,
deferred operations, operation queues, undo maps, and histories. **[source]**

The important boundary is that this layer knows text operations and causal
versions, but not language servers or files. **[inferred]** That separation is
what lets collaboration, undo, snapshots, and remote synchronization remain
coherent before editor presentation is involved.

OpenAgents does not need a fresh editor CRDT for basic IDE parity. Monaco's
model should remain the renderer-side editing engine while Electron main owns
the authoritative revisioned document service, atomic persistence, recovery,
and conflict checks. A future collaborative editor must be a separately
specified consistency lane, not an incidental side effect of Sync chat.

### 4.2 Language buffer

`crates/language/src/buffer.rs:99` wraps the text buffer with file identity,
saved modification time and version, language selection, asynchronous
Tree-sitter syntax state, diagnostics partitioned by language server, remote
selections, capabilities, conflict state, and encoding/BOM information.
**[source]**

This is the first strong parity lesson: “open file” is not just path plus
string. It is a versioned document resource whose language, save base,
encoding, diagnostics, parse revision, conflicts, and capabilities evolve
independently.

### 4.3 MultiBuffer and excerpts

`crates/multi_buffer/src/multi_buffer.rs:73` composes excerpts from one or more
buffers. It tracks history, capability, excerpts, diff transforms, hunk ranges,
and word-level diffs. **[source]** Search results, references, review views, and
agent-generated context can therefore reuse editor behavior without pretending
they are one physical file.

This is one of Zed's most transferable ideas. OpenAgents should introduce an
app-owned **excerpt projection** after basic Monaco editing is stable:

- each excerpt retains source `WorkContext`, relative path, document
  generation, source range, and projection range.
- search results, references, changes, diagnostics, agent context, and review
  may assemble excerpt sets.
- edits map back only through an explicit writable capability and revision
  check.
- a combined view never becomes a synthetic filesystem authority.

The current Desktop IDE plan intentionally avoids an early multi-buffer editor.
That is correct sequencing. Zed changes the long-term model, not the first
milestone: reserve typed excerpt identities now so later review/search views do
not need a protocol rewrite.

### 4.4 Editor

`crates/editor/src/editor.rs:924` shows the resulting `Editor`: it owns a
MultiBuffer, display map, selections, scroll state, completion and diagnostic
presentation, project semantics, collaboration state, snippets, code actions,
edit predictions, breadcrumbs, gutters, navigation history, and other
projections. **[source]**

**[inferred]** Zed's editor quality comes from this stratification, not from a
single magical text-control API. For OpenAgents, Monaco supplies the lower
editing and display machinery. The host still needs explicit document,
project, language, Git, review, and agent-context services around it.

## 5. Worktree and Project form the IDE authority graph

### 5.1 Worktree: filesystem truth and scanning

`crates/worktree/src/worktree.rs` supports local and remote worktrees. A local
worktree owns filesystem events, scanning, ignore stacks, Git repository
discovery, and entry snapshots. `FS_WATCH_LATENCY` is 100 ms at line 82.
Snapshots retain absolute roots internally while indexing entries by stable ID
and relative path. Scan IDs distinguish observed progress from completed scans.
**[source]**

That shape closely validates OpenAgents' existing `WorkContext` boundary:
absolute roots belong to privileged host state, while clients should refer to
a root by opaque identity plus normalized relative path. Zed itself persists
and uses absolute paths more broadly than OpenAgents should expose, but its
`ProjectPath` gives the right interaction identity.

### 5.2 ProjectPath: multi-root identity

`crates/project/src/project.rs:427` defines `ProjectPath` as a worktree ID plus
an `Arc<RelPath>`. **[source]** The ID disambiguates the same relative path in
different roots and survives UI projection better than concatenating display
names.

OpenAgents should standardize the equivalent everywhere in Editor mode:

```text
ProjectFileRef = {
  workContextId,
  rootId,
  relativePath,
  attachmentGeneration
}
```

The renderer never receives the raw root as canonical identity. Rename, move,
save, search, reveal, compare, diagnostics, and Git requests all consume the
same reference and return a new generation where applicable.

### 5.3 Project: semantics-aware service aggregation

`crates/project/src/project.rs:214` owns the higher graph: worktrees, buffer
store, language-server store, Git store, debugger, tasks, terminals, context
servers, agents, environment, search, collaboration, and settings. A Project
may be local, shared, or a collaboration guest. `WorktreeStore`, `BufferStore`,
and `LspStore` each have local and remote modes rather than making the UI branch
on every operation. **[source]**

The buffer store deduplicates loads so one open buffer exists for each project
path/ID. The LSP store tracks server status/capabilities and versioned per-
buffer results such as semantic tokens, colors, lenses, folds, links, symbols,
inlay hints, and diagnostics. Requests are cancellable. **[source]**

**[inferred]** This is Zed's primary architectural win: placement and semantics
sit behind the same Project contract. The editor asks for capabilities. The
project decides whether the service is local, remote, shared, pending, failed,
or unavailable.

OpenAgents should adapt this into an Effect service graph owned by Electron
main—not one giant mutable Project object and not renderer IPC per feature. A
`WorkspaceService` should compose filesystem, document, language, Git,
terminal, task, debug, search, and agent-context capabilities for one exact
WorkContext attachment. Each capability advertises lifecycle and placement.

## 6. Project Panel: tree parity is a state machine

`crates/project_panel/src/project_panel.rs:138` is far more than a directory
list. Its state covers visible entries, expansion and unfolded state,
selection, edits, drag/drop, marks, sorting, focus, and project subscriptions.
Actions include:

- expand, collapse, recursive expand, and directory folding.
- create file/directory, rename, duplicate, move, delete/trash, and undo.
- copy/cut/paste and drag/drop.
- reveal, open terminal, search, and compare marked files.
- hidden/ignored-file controls.
- Git status and diagnostic navigation/decorations.
- keyboard and accessibility behavior.

Visible entries are rebuilt from worktree and Git snapshots on a background
executor, sorted and folded, then rendered through GPUI's virtualized
`uniform_list` at line 7061. **[source]**

**[inferred]** The panel stays fast because the visible flattened tree is a
projection over canonical worktree state. It does not rescan the filesystem
from React render or make each row its own authority.

For the Pierre tree adapter, Zed supplies a concrete parity checklist:

1. multi-root identity and root reorder.
2. paged/incremental scanning with explicit incomplete/error state.
3. virtualized flattening and stable selection/scroll anchoring.
4. folded single-child directories and sticky ancestor context.
5. Git, diagnostic, conflict, hidden, ignored, symlink, and remote badges with
   non-color cues.
6. keyboard navigation, typeahead, focus restoration, and screen-reader names.
7. create/rename/move/delete/copy/cut/paste/drag intents through main-owned
   authority, expected revisions, and undo records.
8. reveal/search/compare/terminal commands through the one command registry.

Pierre remains the chosen presentation package. Zed is the behavior and state
reference. Do not port `ProjectPanel` or its GPUI dependencies.

## 7. Language intelligence: parsing and LSP are capabilities

Zed combines Tree-sitter language packages with a language registry and an LSP
store. Parsing is associated with buffer snapshots. Language servers produce
versioned diagnostics, semantic tokens, symbols, links, lenses, code actions,
folds, and inlay hints. A server has lifecycle, capability, and status rather
than being inferred from whether a spinner vanished. **[source]**

Remote-development documentation exposes a useful placement split: the local
client retains the UI, model calls, Tree-sitter parsing, unsaved changes, and
recent-project state. The remote server owns source files, language servers,
tasks, and terminals. Project settings span both. **[source]**

OpenAgents should adapt the capability contract, not necessarily that exact
placement:

- `unconfigured | starting | ready | degraded | stopped | failed` is visible.
- results carry source document generation and service generation.
- cancellation and supersession are ordinary protocol events.
- URI translation is main-owned and does not leak raw roots.
- one provider may be local, another remote, or absent without changing editor
  intent schemas.
- diagnostics, definitions, references, rename, formatting, and code actions
  return typed losses or unsupported results, never silent no-ops.

Tree-sitter and LSP are complementary: parsing provides cheap local structure.
LSP provides project semantics. Neither should become a hidden permission path
for launching arbitrary binaries or downloading servers.

## 8. Git and diffs: exact index state is hard

`crates/buffer_diff/src/buffer_diff.rs:22` models diff snapshots with hunk and
pending-hunk trees and word diffs. Hunk secondary status explicitly
distinguishes unstaged, partially staged, staged, and pending stage/unstage.
`crates/project/src/git_store.rs:99` connects buffers, worktrees, repositories,
HEAD/index bases, staged/unstaged/uncommitted diffs, object IDs, and optimistic
pending index edits. **[source]**

`git_ui` then implements a mutable source-control workbench: file and hunk
stage/unstage, range actions, checkout/discard flows, commit/amend, fetch,
push, and persistent commit drafts. **[source]** A very recent commit in the
audited history fixed an ambiguous-hunk staging corruption. **[history]**

The inference is cautionary. Staging is not a boolean on a file. It is a
three-base state machine over worktree, index, and HEAD with optimistic actions
that can become stale while the user edits. Rich UI does not reduce the need
for exact evidence.

OpenAgents should:

- keep Pierre as diff projection, not Git authority.
- preserve the current read-only Git review MVP until mutation has its own
  admitted packet.
- carry repository identity, HEAD OID, index evidence, document generation,
  hunk identity, operation generation, and pending status in future mutations.
- re-read and prove the post-state after stage/unstage/discard/commit.
- represent partial stage and ambiguous/stale hunks explicitly.
- receipt mutations and keep acceptance/publish authority outside the diff UI.

Do not copy Zed's command-backed Git implementation wholesale or imply that
basic VS Code parity requires writable SCM on day one.

## 9. Workspace, commands, panes, and navigation

Zed composes items into panes and pane groups, restores windows and workspace
layouts, and gives actions stable typed identities. Focus and key contexts
resolve commands to the active entity while the application registry owns the
catalog. **[source]**

This validates two existing OpenAgents choices:

1. one canonical command registry should drive palette, menus, keyboard,
   buttons, slash commands, mobile, and agent-proposed actions.
2. Editor mode is a primary workbench mode, not a file viewer squeezed into
   the ancillary right panel.

The useful Zed parity set is open/recent workspaces, multi-root management,
tabs and splits, pinned/preview tabs, dirty/conflict state, breadcrumbs,
back/forward navigation, symbol/file quick-open, command palette, workspace
search, problems, outline, terminal/tasks/debug, and restore. They should land
as typed projections in the sequence of the existing IDE plan, not as a
wholesale pane framework.

## 10. Remote development is placement behind the Project contract

Zed's remote system runs a matching headless server near the source. The local
client connects over SSH, installs or selects the exact server version, uses a
daemon/proxy for reconnection, and exchanges length-prefixed protobuf
envelopes. Source, language servers, tasks, and terminals are remote. UI and
local recovery state remain local. **[source]**

The strongest lesson is not SSH. It is that remote and local Project stores
implement comparable capabilities, so editor surfaces do not grow separate
remote-only business logic. **[inferred]**

OpenAgents should bind every workbench request to an explicit placement and
attachment generation. A remote workspace needs independently visible host,
runtime, WorkContext, protocol/component compatibility, containment,
credentials, latency/offline state, and recovery class. Exact-version server
matching is useful operationally but should live in the signed component
graph. Downloading and launching a helper is an admitted component action, not
an implementation detail.

Zed's documented reconnectable daemon is not evidence of host-portable
canonical sessions or OpenAgents-grade execution receipts. **[limitation]**

## 11. Collaboration and shared projects

The text layer carries causal collaborative operations, while Worktree,
Project, BufferStore, and LSP paths have shared or remote variants. Zed can
share a project and buffers with collaborators. Its documentation warns that
project sharing grants collaborators access to files within the project.
**[source]**

OpenAgents should adapt live shared cursors, presence, project-relative
references, and one versioned buffer plane only after collaboration authority
is explicit. Room membership must not imply filesystem, terminal, Git,
language-server launch, agent execution, secret, publication, or acceptance
rights. The multAIplayer teardown's singular execution attachment and bounded
room projection remain the stronger authority model.

Zed demonstrates collaborative editing mechanics. It does not prove group
E2EE, metadata privacy, tenant containment, or portable execution authority.
**[limitation]**

## 12. Themes: one typed editor plane, live reloaded

`crates/theme/src/registry.rs:67` holds theme and icon-theme objects behind a
thread-safe registry with defaults, metadata, listing, insertion, removal, and
extension-load state. The theme schema models appearance, syntax, UI, editor,
and terminal colors. User JSON themes are loaded and watched for changes.
system light/dark mappings select variants. Zed can import VS Code themes.
**[source]**

This reinforces the Pierre/Monaco plan:

- the Effect Native product theme remains canonical for the whole shell.
- a validated resolved editor theme projects into Monaco, Pierre diffs/trees,
  syntax highlighting, terminal, minimap, and code-adjacent chrome.
- light/dark system behavior, live preview, and icon themes use stable IDs.
- colors are tested with contrast, color-vision simulation, and non-color
  status cues.
- untrusted theme JSON is parsed against a bounded schema—never injected as
  arbitrary CSS or executable extension code.

Zed's registry is an architecture reference, not a reason to introduce a
second product-theme authority.

## 13. Extensions: WASM narrows the guest, not the host effect

Zed extensions run through Wasmtime's component model/WASIp2 host.
`crates/extension_host/src/wasm_host.rs` enables epoch interruption and
preopens an extension-specific work directory. The guest API is versioned.
Externally consequential capabilities are grouped as process execution, file
download, and npm installation. A grant check intersects manifest allowance
with configured permission. **[source]**

That double intersection is good. The defaults are not: the audited default
settings grant wildcard commands, downloads, and npm operations. **[source]**
WASM limits direct guest memory and filesystem access, but a powerful host
proxy can still execute arbitrary programs or fetch moving content.
**[inferred]**

OpenAgents should adapt the component boundary, versioned API, per-extension
work directory, cancellation/fuel/epoch controls, and manifest-plus-owner
intersection. It should strengthen them with:

- deny-by-default command, network, package, filesystem, secret, spend, and
  publication grants.
- content-addressed signed components and dependencies.
- no ambient host environment or credentials.
- brokered tools mapped to canonical typed intents.
- effective containment and effect receipts.
- compatibility, staged activation, last-known-good rollback, and revocation.

Do not claim “sandboxed” from Wasmtime alone, and do not use Zed's permissive
defaults as parity requirements.

## 14. Native agent, ACP agents, and terminal threads

Zed exposes three agent experiences:

1. a native Zed Agent integrated with Project, tools, context servers, skills,
   model providers, and a local thread store.
2. external agents through Agent Client Protocol, where the external runtime
   retains its own authentication, model, configuration, tools, and session
   semantics.
3. terminal threads for interactive terminal-native agents.

The Threads Sidebar groups parallel threads by project/worktree and the agent
UI uses the workbench's files, diagnostics, terminal, and diff context.
`crates/agent/src/agent.rs` tracks sessions and per-project context, watches
global and project skills, respects worktree trust for project skills, and
supports parent session IDs for subagents. **[source]**

This taxonomy is valuable confirmation for OpenAgents' harness architecture:
a model provider, an owned native loop, a real external runtime adapter, an
in-process emulation policy, and a terminal projection are different things.
They may share portable UI parts without sharing authority or state semantics.

Zed's native tool set covers file reads/writes/edits/moves, search, terminal,
LSP definitions/references/diagnostics/code actions, web, skills, and child
threads. Its local sandboxing uses macOS Seatbelt, Linux Bubblewrap, and Windows
WSL Bubblewrap, protects Git directories, and supports persistent/thread/once
grants plus requested network/path/unsandboxed escalation. Shell command
permissions use precedence rules and fail-closed parsing for dangerous
substitution/interpolation cases. **[source]**

OpenAgents should adapt project/worktree grouping, capability-aware context,
native-versus-external runtime visibility, child topology, and inline workbench
evidence. It should preserve its stricter canonical tool authority, WorkContext
grants, provider-private event envelope, loss-accounted portable projection,
and effective-containment receipts. An external ACP agent's native config or
MCP server must not become host authority by inheritance.

## 15. Edit prediction and context assembly

Zed's edit-prediction context is unusually instructive because it does **not**
depend on a persistent repository embedding index at this pin.

- `RelatedExcerptStore` finds nearby identifiers and follows LSP definitions
  with debounce and caching.
- the BM25 path enumerates tracked files, chunks them into overlapping line
  windows, and builds an in-memory lexical index for a collection. Active path,
  recent edits, and cursor context receive different query weights.
- Git-log context derives a file co-change graph from recent commits.
- recent opens/views, edit history, current uncommitted diff, diagnostics,
  repository identity, and editable regions contribute structured context.
- prompt encodings support fill-in-the-middle and several explicit edit-region
  formats, then apply predicted diffs back to exact regions.

**[source]** At the audited pin, the BM25 implementation uses 40-line chunks
with 10-line overlap, selects up to 12 chunks and at most three per file, caps
individual files at 1,000,000 bytes, and derives co-change data from up to
5,000 commits.
These are implementation parameters, not target requirements.

**[inferred]** Zed is assembling a portfolio of cheap, provenance-rich context
signals rather than betting the editor on one semantic index. OpenAgents should
model candidate context as typed, budgeted records with source, revision,
reason, sensitivity, audience, and truncation. Open buffers, recent edits,
diagnostics, definitions, Git changes, and co-change history can feed the
central semantic selector or a structured query planner. BM25 may be a bounded
subretriever after route selection. It must not become ad hoc user-intent or
tool routing.

Any context sent to a provider should have a previewable disclosure record and
respect account, repository, secret, ignore, and telemetry policy. A useful
prediction is not permission to upload the repository.

## 16. What Zed stores locally

`crates/paths/src/paths.rs` gives an unusually concrete inventory. On macOS,
Zed uses:

- application data: `~/Library/Application Support/Zed`.
- user configuration: `~/.config/zed`.
- state: `~/.local/state/Zed`.
- logs: `~/Library/Logs/Zed`.
- OS cache/temp locations with Zed-specific subdirectories.

Named local artifacts include:

- settings, global settings, backups, keymap, tasks, debug configuration, and
  an `AGENTS.md` path.
- installed/staging/build extension directories and extension index metadata.
- language packages, debug adapters, external agents, Copilot, Prettier,
  remote servers, and development-container assets.
- themes, icon themes, snippets, prompts, and prompt overrides.
- database, logs, crash state, terminal and editor state, and update/download
  caches.

`crates/db/src/db.rs:41` opens an application SQLite database at a channel-
specific path under the data directory. It uses WAL, a 500 ms busy timeout,
foreign keys, case-sensitive `LIKE`, and `synchronous=NORMAL`. Domain
migrations compose through an inventory mechanism. Registered domains include
workspace layout, editor state, command palette, keymap, search, terminal,
Vim, previews, onboarding, thread metadata, terminal-thread metadata, and Git
graph/diff data. **[source]**

Workspace persistence includes windows, pane groups, panes/items, local and
remote projects/connections, toolchains, breakpoints, trusted worktrees, and
bookmarks. Editor persistence includes file path or unsaved contents, language,
mtime, scroll, selections, and folds. Zed therefore retains unsaved editor
contents and substantial project/workspace history, not only preferences.
**[source]**

Agent thread content uses a separate `threads/threads.db`. Thread rows contain
IDs, parent IDs, folder paths, titles, timestamps, type, and data. Full thread
JSON is compressed with zstd level 3. Recursive deletion also removes child
threads and associated sandbox temp directories. **[source]**

`paths.rs` defines an `embeddings_dir` described as semantic-search embedding
storage. A repository-wide call-site search at this exact tree found no use
beyond the definition itself. **[source]** The honest conclusion is:

- there is a reserved or legacy embeddings path.
- this source pin does **not** show Zed currently building or persisting a
  repository embedding index there.
- edit-prediction retrieval observed in this audit is in-memory BM25 plus
  LSP, Git, diagnostics, and recent-activity signals.

Claiming that Zed stores code embeddings from the directory name alone would
overstate the evidence. **[limitation]** Source also cannot prove which dormant
paths a particular installed release has populated on a user's machine.

### OpenAgents local-state requirement

Editor parity needs a user-visible data inventory, not hidden database growth.
For each store, declare exact purpose, data classes, root/path sensitivity,
encryption, retention, quota, export, deletion, backup/Sync eligibility, crash
behavior, and whether an external runtime can read it. Unsaved files, workspace
roots, agent histories, terminal transcripts, trust grants, search history,
language caches, indexes, and telemetry queues need separate controls.
Renderer projections should continue to receive relative file references. Raw
absolute paths and provider-private histories must not leak into public receipts
or owner Sync by convenience.

## 17. Performance and verification posture

The audited core crates contain broad unit, property, integration, and
benchmark coverage. Examples include:

- large random edit and MultiBuffer tests.
- Rope benchmarks including many small appends.
- project-panel sorting benchmarks over a realistic repository snapshot.
- extension compilation benchmarks.
- large locator/search cases and explicit project-search limits.
- virtualized project-panel rows and background projection rebuilding.

**[test]** A lexical survey across the selected core crates found thousands of
test/property annotations, but count is not quality proof. The source also
documents scaling edges. Remote documentation warns that very large directory
counts remain problematic. **[limitation]** The recent Git hunk fix shows that
deep test suites do not make mutable SCM trivial.

OpenAgents should borrow the verification style:

- property-test path normalization, coordinate conversion, stale generations,
  edit application, and tree projections.
- model buffer/save/reload/conflict and Git index transitions.
- benchmark cold open, large tree expansion, search, Monaco model switching,
  diff rendering, LSP result bursts, restore, and remote latency.
- gate the packaged application with accessibility and p50/p95/p99 frame/input
  budgets.
- keep fixtures for ignored files, symlinks, multi-root collisions, encodings,
  huge/minified files, partial Git stage, offline remote, and corrupt recovery
  state.

No Zed benchmark number is an OpenAgents target until the same workload is
measured in the packaged OpenAgents architecture. **[limitation]**

## 18. What Zed does especially well

1. **One typed Project graph.** Files, buffers, language intelligence, Git,
   tasks, terminals, debug, remote placement, collaboration, and agents share
   identity and lifecycle.
2. **One coordinate substrate.** Persistent snapshots and summary/seek
   structures connect bytes, points, rows, excerpts, and diffs.
3. **First-class excerpts.** Search, references, review, and context can be
   editor views without pretending to be physical files.
4. **Local and remote symmetry.** Store interfaces absorb placement
   differences instead of duplicating every UI path.
5. **Native performance discipline.** Virtualization, background projections,
   cancellation, snapshots, benchmarks, and explicit limits are architectural.
6. **Agent/IDE integration.** Agent context consumes real project, diagnostic,
   LSP, Git, terminal, file, and thread state.
7. **Concrete local-state ownership.** Paths and domain databases make much of
   retained state inspectable in source.
8. **Versioned extension surface.** WASM components and a host API create a
   clearer compatibility boundary than arbitrary renderer plugins.

## 19. Limits, costs, and risks

1. **Architecture gravity.** GPUI, SumTree, text CRDT, Project, and Editor are
   mutually reinforcing. Selectively adopting internals is expensive.
2. **Large integrated surface.** Hundreds of crates and coordinated release
   targets impose substantial build, migration, and ownership cost.
3. **Broad extension effects.** WASM memory isolation coexists with wildcard
   default process/download/npm host capabilities.
4. **Absolute local paths.** Zed appropriately needs host paths internally but
   persists them in places OpenAgents must keep out of renderer/public planes.
5. **Mutable Git complexity.** Partial staging and optimistic index operations
   remain a corruption-prone state machine.
6. **Remote component lockstep.** Exact client/server matching simplifies
   compatibility but adds download, installation, and recovery obligations.
7. **Collaboration is broad authority.** Shared-project access is not the same
   as least-privilege collaboration.
8. **Agent context has disclosure risk.** Rich retrieval signals can move code,
   diagnostics, history, and repository identity toward model providers.
9. **A defined embeddings directory is not evidence of active embeddings.**
   Product claims must follow call sites and runtime evidence.
10. **License boundary.** The repository's GPL-default licensing makes source
    study safe for architecture learning but wholesale code reuse a legal and
    product decision. Pierre/Monaco package licenses and exact pins must be
    evaluated separately.

## 20. Exact OpenAgents adaptation

### Adapt now into the basic IDE packet

1. Make `ProjectFileRef`/equivalent multi-root identity canonical across tree,
   Monaco, search, language, Git, terminal, review, and agent context.
2. Keep raw roots private to Electron main and bind every request to
   `WorkContext` plus attachment generation.
3. Define one revisioned document service: load/save base, dirty state,
   encoding, conflict, recovery, and expected-revision mutation.
4. Treat Pierre tree flattening as a projection over host-owned snapshots.
   add folded directories, stable virtualization, sticky context, keyboard and
   accessibility, Git/diagnostic/conflict decorations, and explicit scan state.
5. Give Monaco stable per-project URIs/model identity and make all language and
   diff ranges carry document generation.
6. Define language capability lifecycle and typed unsupported/degraded/error
   outcomes before wiring LSP features.
7. Preserve one command registry across shell, palette, tree, editor, menus,
   shortcuts, mobile, and model-proposed actions.
8. Project one validated editor theme into Monaco, Pierre, terminal, syntax,
   minimap, and adjacent chrome under the Effect Native product theme.
9. Inventory and expose local IDE data/retention before adding search indexes,
   unsaved recovery, terminal transcripts, or external-agent stores.
10. Group agent threads by canonical project/worktree and expose exact runtime,
    placement, context, child graph, and workbench evidence.

### Reserve in the contracts, implement after basics

1. Excerpt sets that can power workspace search, references, Problems, review,
   agent context, and later editable multi-buffer views.
2. Local/remote service placement behind the same workspace capability
   interfaces.
3. Writable tree operations with typed intents, undo, and expected revisions.
4. Mutable Git with three-base evidence, partial-stage identity, post-state
   proof, and receipts.
5. Tasks, terminals, debugger, outline, symbols, and Problems as services—not
   special renderer IPC.
6. Extension components behind deny-by-default canonical tool brokers.
7. Collaborative editing only after consistency, membership, projection,
   execution attachment, and authority are independently specified.

### Study

- Zed's Project/Store split as a comparison fixture for the Effect service
  graph.
- MultiBuffer/excerpt navigation and edit mapping.
- project-panel folding, sticky scroll, drag/drop, focus, and accessibility
  behavior.
- local/remote LSP and task placement under one capability interface.
- edit-prediction candidate selection and provenance-aware disclosure.
- agent thread organization across native, ACP, and terminal runtimes.
- property tests and performance fixtures for large workspaces.

### Reject

- adopting GPUI or a second Rust UI runtime for Desktop.
- reimplementing Monaco's text editor from Zed internals.
- importing the Zed Project Panel instead of using Pierre behind the owned
  adapter.
- treating rich tree/diff/editor UI as workspace, Git, or review authority.
- allowing absolute roots or ambient current directories into renderer state.
- claiming WASM alone is containment or accepting wildcard host effects.
- mutable Git in the first basic-editor milestone.
- shared-project membership as execution authority.
- persistent repository upload/indexing without an explicit data contract.
- claiming active local embeddings from an unused directory definition.
- copying GPL-default source without a separately reviewed legal decision.

## 21. Revised Desktop IDE architecture

```text
Effect Native desktop shell
  ├─ canonical command registry
  ├─ chat / agents / approvals / receipts
  └─ primary Editor mode
       ├─ Pierre tree projection
       ├─ Monaco editor models
       ├─ Pierre diff projection
       ├─ Problems / outline / search / terminal projections
       └─ optional excerpt-set projections
                    │ typed generation-bound intents
                    ▼
Electron main workspace capability graph
  ├─ WorkContext + roots + ProjectFileRef resolver
  ├─ worktree snapshots / watch / search / file mutations
  ├─ revisioned document and recovery service
  ├─ language / LSP / parsing capability lifecycle
  ├─ Git evidence and later mutation service
  ├─ terminal / task / debug service
  ├─ local-state inventory and retention
  └─ local or remote placement adapter
                    │ canonical context/evidence only
                    ▼
OpenAgents runtime / HarnessAgent / external peers
  ├─ provider-private native event envelope
  ├─ portable loss-accounted projection
  ├─ project-bound tools through ordinary authority
  └─ execution and delivery receipts
```

This is “Zed coherence” with OpenAgents components and trust boundaries. It
also corrects an older teardown shorthand: files and code editing do not belong
only in a generic right-panel surface manager. The right panel can still host
ancillary review, evidence, terminal, and agent views. The existing Files mode
should become a first-class primary Editor mode with rail, top bar, and main
editing region.

## 22. Source map

The most consequential evidence paths at the pinned tree are:

| Concern | Source paths |
| --- | --- |
| application assembly | `Cargo.toml`. `crates/zed/Cargo.toml`. `crates/zed/src/main.rs` |
| GPUI state/runtime | `.rules`. `crates/gpui/src/app.rs`. `crates/gpui/src/app/entity_map.rs` |
| indexed text substrate | `crates/sum_tree/src/sum_tree.rs`. `crates/rope/src/rope.rs`. `crates/text/src/text.rs` |
| editor stack | `crates/language/src/buffer.rs`. `crates/multi_buffer/src/multi_buffer.rs`. `crates/editor/src/editor.rs` |
| filesystem/project | `crates/worktree/src/worktree.rs`. `crates/project/src/project.rs`. `crates/project/src/worktree_store.rs`. `crates/project/src/buffer_store.rs` |
| language services | `crates/project/src/lsp_store.rs`. `crates/language`. `crates/languages` |
| file explorer | `crates/project_panel/src/project_panel.rs` |
| Git/diffs | `crates/buffer_diff/src/buffer_diff.rs`. `crates/project/src/git_store.rs`. `crates/git/src/repository.rs`. `crates/git_ui` |
| workspace/persistence | `crates/workspace/src/persistence.rs`. `crates/editor/src/persistence.rs`. `crates/db/src/db.rs`. `crates/paths/src/paths.rs` |
| themes | `crates/theme/src/registry.rs`. `crates/theme/src/theme.rs`. `crates/theme/src/schema.rs`. `docs/src/themes.md`. `docs/src/extensions/themes.md` |
| extensions | `crates/extension_host/src/wasm_host.rs`. `crates/extension`. `crates/extension_api`. `assets/settings/default.json` |
| remote/collaboration | `crates/remote/src/protocol.rs`. `crates/remote_server`. `crates/collab`. `docs/src/remote-development.md`. `docs/src/collaboration` |
| agents | `crates/agent/src/agent.rs`. `crates/agent/src/db.rs`. `crates/agent/src/sandboxing.rs`. `crates/agent/src/tool_permissions.rs`. `crates/agent_ui`. `crates/acp_thread` |
| edit context | `crates/edit_prediction_context/src/edit_prediction_context.rs`. `crates/edit_prediction_context/src/bm25_context.rs`. `crates/edit_prediction_context/src/git_log_context.rs`. `crates/edit_prediction` |

## Final recommendation

Use Zed as the architecture and parity reference for the **whole basic IDE
loop**, while keeping Pierre and Monaco as the practical projection/editor
choices. The first OpenAgents milestone should prove one coherent path:

```text
attach WorkContext
→ render multi-root virtualized tree
→ open one revisioned Monaco document
→ save with expected revision and recovery
→ receive versioned diagnostics/navigation
→ inspect a Pierre diff against exact Git evidence
→ expose the same context to an agent through canonical authority
→ restart and restore honestly
```

Then deepen it with workspace search, Problems, symbols, excerpt sets,
terminals/tasks/debug, remote placement, and separately admitted mutation.
Zed's lesson is that parity comes from one state graph all of those surfaces
share. OpenAgents' improvement is to make that graph typed across processes,
Effect-owned, least-privilege, local-state-visible, projection-safe, and
receipted.
