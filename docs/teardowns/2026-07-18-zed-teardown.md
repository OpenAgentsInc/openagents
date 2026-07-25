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

The “Zed Agent” name does not identify a model or an external agent process.
It identifies Zed's in-process agent runtime. That runtime selects a configured
model, builds project context, runs the model/tool loop, persists threads, and
projects native events through the same thread UI abstractions used for ACP
agents.

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

## 14. Zed Agent, ACP agents, and terminal threads

### 14.1 Product and runtime boundary

Zed exposes three agent experiences:

1. Zed Agent is Zed's native, in-process agent runtime.
2. An external agent runs through Agent Client Protocol (ACP).
3. A terminal thread hosts an interactive terminal-native agent.

The source gives the native runtime the exact product ID `Zed Agent` and the
telemetry ID `zed`. `NativeAgentServer::connect` creates templates, creates a
GPUI `NativeAgent` entity, and returns a `NativeAgentConnection`. It does not
start an agent child process or an ACP transport. The connection implements the
same `AgentConnection` interface that the shared thread UI uses for external
agents. **[source]** See `crates/agent/src/native_agent_server.rs:23`,
`crates/agent/src/agent.rs:404`, and `crates/agent/src/agent.rs:2562`.

Thus, Zed Agent is not a model. It is the orchestration layer above a selected
`LanguageModel`. It owns the native loop, project context, tools, native
permissions, thread state, and persistence. A model provider owns model access
and completion behavior. An external ACP runtime usually owns its own
authentication, models, tools, and native configuration. A terminal thread
leaves those duties with its CLI or TUI. **[source]**

This distinction explains why shared UI does not imply shared runtime
semantics. Zed adapts its native runtime to ACP-shaped thread interfaces. It
does not implement the native loop by starting itself as an external ACP
agent. **[inferred]**

### 14.2 Model and provider selection

`LanguageModels` reads visible providers from `LanguageModelRegistry`. It keeps
only authenticated providers, groups recommended models, and groups the other
models by provider. The UI-facing model ID is `provider/model`, not only the
model name. Zed tries provider authentication in the background so the selector
can populate. Missing credentials and selected noisy local-provider failures
have explicit handling. **[source]** See
`crates/agent/src/agent.rs:229` and `crates/agent/src/agent.rs:303`.

A new native thread receives the registry's default model. It separately
receives the configured thread-summary model. A model change resolves the
selected ID back to an `Arc<dyn LanguageModel>`. The change also applies
thinking, effort, and speed settings and writes the default selection to user
settings. The thread therefore owns one selected model state, while the
registry owns provider discovery and configured model objects. **[source]**
See `crates/agent/src/agent.rs:724` and
`crates/agent/src/agent.rs:2385`.

The model interface supplies streaming completions, tool-format support,
context size, token use, image support, and provider identity. The native agent
does not erase these differences. It filters tools for provider compatibility
and exposes the selected provider in the UI. **[source]**

### 14.3 Session and thread lifecycle

`NativeAgent` owns these principal states:

- active sessions and shared pending session loads.
- one project state for each live Project entity.
- a local `ThreadStore`.
- templates and the model catalog.
- project and global skill state.
- filesystem and application subscriptions.

Each active session holds two entities. `Thread` is the native execution and
message state. `AcpThread` is the UI-facing thread projection. The session also
holds its project ID, save task, subscriptions, and a reference count.
**[source]** See `crates/agent/src/agent.rs:191`,
`crates/agent/src/agent.rs:203`, and `crates/agent/src/agent.rs:404`.

A new session creates `Thread` with the exact Project, ProjectContext, context
server registry, templates, and default model. Registration then:

1. creates the UI-facing `AcpThread`.
2. restores title, draft prompt, scroll position, and token use.
3. installs built-in tools and the live skill resolver.
4. subscribes to title and token changes.
5. observes thread changes and schedules persistence.
6. publishes native commands and skills to the UI.

**[source]** See `crates/agent/src/agent.rs:724` and
`crates/agent/src/agent.rs:754`.

Load is reference-counted. Concurrent requests for one stored session share one
pending task. A loaded thread replays stored native events through the same
native-to-UI bridge before the UI marks its snapshot complete. Close decrements
the count. The last close saves and removes the session. It also removes project
state after the last project session closes. **[source]** See
`crates/agent/src/agent.rs:1567` and `crates/agent/src/agent.rs:1706`.

The connection supports new, load, close, retry, cancel, truncate, title
change, session listing, deletion, and telemetry. Cancel reaches the native
running turn. Retry resumes the same native thread. Truncate updates both
native history and the UI token state. **[source]** See
`crates/agent/src/agent.rs:2564`,
`crates/agent/src/agent.rs:2898`, and
`crates/agent/src/agent.rs:2974`.

### 14.4 Project context and the system prompt

Each live project state owns a `ProjectContext`, a context-server registry, a
skill catalog, load issues, and a refresh task. Project, context-server, and
worktree-trust changes request a refresh. The refresh only replaces
ProjectContext when the model-visible value changes. This preserves prompt
cache stability for irrelevant filesystem events. **[source]** See
`crates/agent/src/agent.rs:846` and `crates/agent/src/agent.rs:930`.

The context builder reads all visible worktrees. For each worktree, it records
root information and reads the first supported project instruction file.
Supported compatibility names include `.rules`, `AGENTS.md`, `CLAUDE.md`, and
other agent instruction files. Zed also watches the user's personal
`~/.config/zed/AGENTS.md` or the platform equivalent. Blank, absent, or failed
personal instruction reads do not enter the prompt. **[source]** See
`crates/agent/src/agent.rs:1019`,
`crates/agent/src/agent.rs:1214`, and
`crates/agent_settings/src/user_agents_md.rs:23`.

Each model request rebuilds the system prompt from:

- current ProjectContext and worktree instructions.
- personal `AGENTS.md`.
- the selected model name.
- the current date and platform.
- the exact enabled tool names.
- the current sandbox state.

The request then appends saved conversation history and any pending assistant
message. It marks the last request message for provider caching. The request
also carries thread ID, prompt ID, completion intent, function schemas,
temperature, thinking settings, and speed. **[source]** See
`crates/agent/src/templates.rs:8` and
`crates/agent/src/thread.rs:3963`.

User-provided context is not only the system prompt. The Agent Panel can add
files, directories, symbols, diagnostics, branch diffs, URLs, images,
selections, skills, and earlier threads. The message editor converts these
items into ACP content blocks and native message content. The selected model
then receives those blocks in the request history. **[source]** See
`docs/src/ai/agent-panel.md` and `crates/agent/src/agent.rs:2862`.

This architecture binds context assembly to the current project and thread. It
does not prove a least-disclosure provider policy. Rich explicit mentions,
project instructions, tool results, and thread history can all reach the
selected provider. **[limitation]**

### 14.5 The native model and tool loop

`Thread::send` appends one user message and starts a running turn. A turn owns
the enabled tool map, an event stream, cancellation state, streaming tool input
channels, and its foreground task. **[source]** See
`crates/agent/src/thread.rs:2470` and
`crates/agent/src/thread.rs:2638`.

The loop performs these steps:

1. compact context if the configured threshold requires it.
2. read the current model and current enabled tools.
3. build a model request with history and function schemas.
4. call `LanguageModel::stream_completion`.
5. race model events, tool completion futures, and cancellation.
6. append tool results to the native assistant message.
7. call the model again until the turn stops or fails.

The loop reads the model, profile, and tools again between tool rounds.
Mid-turn changes can therefore affect the next completion request. Independent
tool futures can run in parallel. A tool that supports streamed input can start
before its complete JSON input arrives. Unknown tools and invalid JSON become
structured tool results rather than implicit success. **[source]** See
`crates/agent/src/thread.rs:2702`,
`crates/agent/src/thread.rs:2817`, and
`crates/agent/src/thread.rs:3434`.

Completion events include text, thinking, redacted thinking, reasoning detail,
tool input, token use, refusal, maximum-token stops, and normal stops. Native
`ThreadEvent` adds tool authorization, subagent, retry, compaction, and
user-message events. `NativeAgentConnection::handle_thread_events` maps that
stream into `AcpThread` mutations, tool cards, approval prompts, token updates,
and stop reasons. This function is the main boundary between the native core
and the shared agent UI. **[source]** See
`crates/agent/src/thread.rs:868`,
`crates/agent/src/thread.rs:3311`, and
`crates/agent/src/agent.rs:2163`.

Stop cancels the foreground task and active child work. A queued message
normally waits for turn completion. Native steering can request a stop at the
next message boundary, usually between a tool result and the next model
response. Zed does not offer this guarantee for external agents because it
does not own their turn loop. **[source]**

### 14.6 Built-in tools, profiles, and context servers

The registered built-in set covers:

- file and directory read, list, find, grep, copy, create, move, delete, edit,
  and write operations.
- terminal, URL fetch, and hosted web search.
- project diagnostics, definitions, references, rename, and code actions.
- skill load, nested subagent spawn, sibling-thread creation, and agent/model
  listing.

The exact enabled set is smaller than the registered set. The active profile
must enable a tool. The selected provider must support its schema. Feature
flags can remove it. Restricted workspaces remove disallowed tools. The runtime
selects either the sandboxed or plain terminal implementation but exposes the
canonical name `terminal`. **[source]** See
`crates/agent/src/thread.rs:2091` and
`crates/agent/src/thread.rs:4030`.

Profiles control availability, not approval. The shipped Write, Ask, and
Minimal profiles select different built-in and context-server tools. A profile
can also select a default model. Tool permission settings separately decide
allow, deny, or confirm for an available permission-gated call. **[source]**
See `crates/agent_settings/src/agent_profile.rs:104` and
`assets/settings/default.json`.

Context-server tools enter the same enabled-tool map. Per-profile MCP settings
can enable all server tools or named tools. If two servers or a built-in tool
use the same name, Zed prefixes duplicate server tool names when the provider's
tool-name limit permits it. MCP tools then use the same model request and tool
result loop. **[source]**

### 14.7 Permission and sandbox layers

Native tool authorization has more than one layer:

1. the profile decides whether the model can see the tool.
2. workspace trust can remove dangerous tools.
3. hardcoded rules reject selected catastrophic terminal commands.
4. tool rules apply deny, confirm, allow, and regex input matches.
5. the UI resolves a pending approval.
6. terminal sandbox policy limits the resulting process.

Terminal rules parse supported command chains so an allowed prefix cannot hide
a denied later command. When protected terminal input uses unsafe or
unsupported substitution, interpolation, or chaining, the parser fails closed.
Some dangerous recursive deletions have hardcoded denial and cannot be allowed
by settings. **[source]** See
`crates/agent/src/tool_permissions.rs:14`.

The approval UI can grant one call, the rest of the thread, or a persistent
setting where the permission type supports that scope. A settings change can
resolve a pending request. Third-party MCP tools receive their per-tool default
decision. They do not receive the full built-in input-regex policy. **[source]**
See `crates/agent/src/thread.rs:5539` and
`crates/agent/src/thread.rs:6173`.

At the audited pin, terminal sandboxing applies to local projects on macOS,
Linux, and Windows unless the user persistently allows unsandboxed execution.
macOS uses Seatbelt. Linux uses non-setuid Bubblewrap. Windows uses Bubblewrap
inside WSL. The baseline grants writes to project worktrees, isolates temporary
storage, and protects discovered Git metadata from writes. **[source]** See
`crates/agent/src/sandboxing.rs:1`,
`crates/agent/src/sandboxing.rs:34`, and
`crates/sandbox/README.md`.

A terminal call can request exact network hosts, any network host, exact write
paths, all filesystem writes, or fully unsandboxed execution. Persistent
settings and per-thread grants merge as allowlists. The UI can grant an
escalation once, for the thread, or persistently. Git metadata remains
protected while the command stays sandboxed. **[source]**

This sandbox is not a complete agent sandbox. It limits the native terminal
path. The `fetch` tool is permission-gated but does not use the terminal OS
sandbox or its network grants. Native file tools use separate path and
permission checks. Language servers, build hooks, Git actions outside the agent
terminal, and later user commands can execute content outside this sandbox.
Zed's own documentation makes this limit explicit.
**[limitation]**

### 14.8 Skills, instructions, and slash commands

Skills and always-on instructions are different inputs. Personal and project
instruction files enter every relevant system prompt. A skill is a named,
on-demand instruction package with frontmatter and a body. **[source]**

Zed loads global skills from `~/.agents/skills`. It loads project skills from
`.agents/skills` only for trusted worktrees. Skill bodies stay on disk until
invocation. The catalog and load issues refresh when relevant files or trust
change. The agent installs a dynamic `skill` tool, so the model can request a
skill by name. The user can invoke the same skill with a slash command.
**[source]** See `crates/agent/src/agent.rs:596`,
`crates/agent/src/agent.rs:1019`, and
`crates/agent/src/agent.rs:1967`.

A skill invocation reads the body, wraps it in a visible `<skill_content>`
envelope, appends the user's remaining content, and runs the normal native
loop. The UI shows the injected skill content. This makes model-driven and
user-driven invocation use one conversation representation. **[source]**

Native slash commands also include `/compact` and context-server prompts.
Ambiguous MCP prompt names receive server prefixes. Prompts that need multiple
arguments do not enter the simple slash-command list. An MCP prompt is fetched,
its user and assistant messages are converted into thread messages, and the
same native loop continues. **[source]** See
`crates/agent/src/agent.rs:1502` and
`crates/agent/src/agent.rs:1826`.

### 14.9 Child agents and sibling threads

`spawn_agent` creates a native child `Thread`. The child receives a parent
session ID and a depth. It shares the Project, ProjectContext, context-server
registry, and linked action log. It initially inherits model behavior, profile,
thinking, speed, and summarization settings. A configured subagent model can
override its model. **[source]** See
`crates/agent/src/thread.rs:1286` and
`crates/agent/src/agent.rs:3028`.

Child depth is bounded. The parent keeps weak handles to running children so
cancellation can propagate. Child sessions use the same persistence and event
pipeline. The database keeps the parent ID and child context. Recursive parent
deletion removes child threads and their sandbox temporary directories.
**[source]**

Sibling-thread tools are different from nested subagents. They ask a UI-owned
host to create another panel thread or list available agents and models. That
new thread can use a different agent runtime. This preserves the difference
between delegated child context and independent parallel work. **[source]**
See `crates/agent/src/agent.rs:369` and
`crates/agent/src/agent.rs:3237`.

### 14.10 UI, review, and persistence projection

The Agent Panel and Threads Sidebar are projections over shared agent thread
interfaces. The native connection supplies extra capabilities through checked
downcasts, such as native skills and exact native thread control. The panel
groups parallel threads by project and worktree. Each thread has its own model
context, history, running turn, and action log. **[source]**

The native event bridge drives streamed text, thinking blocks, tool cards,
terminal output, diffs, permission choices, retries, compaction markers, and
stop state. The action log connects file edits to inline and multi-buffer
review. The UI can follow file reads and edits. It can restore checkpoints and
accept or reject agent changes. **[source]** See
`crates/agent_ui/src/agent_panel.rs`,
`crates/agent_ui/src/conversation_view.rs`,
`crates/agent_ui/src/conversation_view/thread_view.rs`, and
`crates/agent_ui/src/agent_diff.rs`.

Every non-empty native thread change schedules a local save. Close saves again,
and application quit flushes all live non-empty threads concurrently. Stored
state includes native messages, title, summary, token use, model/profile,
thinking settings, draft, scroll position, parent context, and sandbox state.
Section 16 gives the database and retention details. **[source]**

The source includes shared agent-server end-to-end tests and focused tests for
model selection, project context, skill trust, slash commands, permissions,
compaction, tool replay, thread save/load, close, quit flush, reference counts,
and child deletion. This audit did not execute those tests or a provider-backed
turn. It proves encoded design and test intent, not installed runtime behavior.
**[test] [limitation]**

### 14.11 OpenAgents implication

This taxonomy confirms OpenAgents' harness architecture. A provider, native
loop, external runtime adapter, terminal projection, and UI thread are separate
components. Portable UI can be shared without sharing authority or private
state semantics.

OpenAgents should adapt:

- explicit native-versus-external runtime identity.
- one project-bound context and event plane.
- model/provider identity that remains visible.
- a stable turn state machine with cancellation and steering boundaries.
- tool availability separate from tool approval and containment.
- child topology separate from independent sibling work.
- local persistence with visible retention and deletion semantics.
- inline workbench evidence and review tied to exact file revisions.

OpenAgents should preserve its stricter canonical tool authority, WorkContext
grants, provider-private event envelope, loss-accounted portable projection,
and effective-containment receipts. It must not inherit host authority from an
external ACP agent, MCP server, selected model, profile, skill, or shared UI
type.

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
JSON is versioned and compressed with zstd level 3. The stored native data
includes messages, summaries, token use, model/profile selection, thinking
state, parent context, draft prompt, scroll position, and sandbox state.
Top-level history excludes child sessions but retains their parent links.
Thread changes schedule a save. Last close saves again, and application quit
flushes all live non-empty threads. Recursive deletion also removes child
threads and associated sandbox temp directories. **[source]** See
`crates/agent/src/db.rs:29`, `crates/agent/src/thread_store.rs:12`, and
`crates/agent/src/agent.rs:1706`.

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
| agent identity and lifecycle | `crates/agent/src/native_agent_server.rs`. `crates/agent/src/agent.rs`. `crates/acp_thread` |
| native model/tool loop | `crates/agent/src/thread.rs`. `crates/agent/src/templates.rs`. `crates/language_model` |
| agent context | `crates/agent_settings/src/user_agents_md.rs`. `crates/prompt_store`. `crates/context_server`. `crates/agent/src/tools/context_server_registry.rs` |
| native tools and children | `crates/agent/src/tools.rs`. `crates/agent/src/tools`. `crates/agent/src/outline.rs` |
| agent permissions and sandbox | `crates/agent/src/tool_permissions.rs`. `crates/agent/src/sandboxing.rs`. `crates/sandbox`. `crates/settings_ui/src/pages/tool_permissions_setup.rs` |
| native thread persistence | `crates/agent/src/db.rs`. `crates/agent/src/thread_store.rs`. `crates/paths/src/paths.rs` |
| agent UI and review | `crates/agent_ui/src/agent_panel.rs`. `crates/agent_ui/src/conversation_view.rs`. `crates/agent_ui/src/conversation_view/thread_view.rs`. `crates/agent_ui/src/agent_diff.rs` |
| agent tests | `crates/agent/src/tests`. `crates/agent/src/native_agent_server.rs`. `crates/agent/src/thread_store.rs` |
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
