# VS Code teardown: reusable TypeScript substrate and the new agent workbench

Date: 2026-07-18

Status: source-grounded teardown and adaptation evidence. This document does
not authorize product code, dependency admission, a Code-OSS fork, or a change
to the accepted IDE packet order.

## Executive verdict

VS Code is two different references hiding in one repository.

The first is a small, unusually valuable set of deliberately reusable
TypeScript packages: Monaco, URI and Language Server Protocol primitives,
standalone JSON/HTML/CSS/Markdown language services, xterm, TextMate and
Oniguruma tokenization, Tree-sitter WASM, Ripgrep packaging, and later Debug
Adapter Protocol types. OpenAgents can evaluate these as ordinary pinned
dependencies behind owned Effect services and renderer adapters.

The second is the Code-OSS application framework: Explorer, text-file models,
commands, context keys, workbench layout, extension hosts, settings, remote
development, source control, testing, and now a large agent-sessions layer.
That code is open and written mostly in TypeScript, but it is not a collection
of independent npm libraries. Its useful units are architectural patterns and
behavioral specifications. Importing internal `vs/*` modules would quietly
adopt a substantial fraction of the workbench.

The correct OpenAgents split is therefore:

- **consume** Monaco and a narrowly admitted package portfolio;
- **adapt** VS Code's document, lifecycle, command, language, terminal,
  remote, and agent-session patterns into OpenAgents-owned contracts;
- **retain** Zed as the main integrated agent-IDE architecture reference,
  Pierre as the tree/diff projection choice, and Cursor as the product-breadth
  and fork-delta comparison;
- **reject** a Code-OSS fork, internal `vs/*` imports, an extension-host clone,
  and a second authority plane beside Effect Native, WorkContext, the generated
  engine protocol, and Electron main.

The most important new finding is that current VS Code is itself moving toward
an agent-first overlay. `src/vs/sessions/` is a new top-level layer above the
workbench; `src/vs/platform/agentHost/` runs Copilot, Claude, and Codex harnesses
through a provider-neutral protocol, session database, worktree isolation,
checkpoint, changeset, approval, and remote-host system. This validates the
idea that agent sessions should be a distinct plane over an editor substrate.
It does not justify copying that plane: its largest UI handler alone is 4,862
lines and its dependency closure reaches most of the workbench.

## Evidence boundary and pins

This was a read-only source audit. No VS Code build, extension, agent SDK,
downloaded runtime, installer, product service, or test suite was executed.
The external checkout was refreshed with the workspace's targeted sync lane
before inspection.

### VS Code source pin

| Field | Value |
| --- | --- |
| Repository | `microsoft/vscode` |
| Local checkout | `projects/repos/vscode` |
| Branch | `main` |
| Commit | `f4e18ff9f2d0f5dcea01d00ec73bed52be18f488` |
| Tree | `065a78b57b3fe4845a4ae22905b0df92848f9ac4` |
| Commit time | `2026-07-18T20:32:49-07:00` |
| Commit subject | `Merge pull request #326406 from microsoft/connor/fix-local-ipc-tool-timeout` |
| Nearest description | `1.106.0-20797-gf4e18ff9f2d` |
| Root package metadata | `code-oss-dev`, version `1.130.0`, private ESM package |
| Repository license | MIT; dependencies retain their own licenses and notices |

The root package version and Git description are development metadata from the
pinned checkout, not a claim about the latest stable Visual Studio Code
release.

The 53-file source corpus used for the focused architecture and package audit
has aggregate SHA-256
`14ee1bd16db9b86d198eb1fe1625ece9a7d5b747035ed9e964f6cf7aba7d2c53`.
It covers the layer rules; Monaco build seam; Explorer, tree, file, text-file,
theme, extension, and language-service surfaces; sessions specifications;
agent-host protocol, persistence, worktree, checkpoint, changeset, and handler
code; and agent-SDK packaging.

### OpenAgents target pin at authoring

| Field | Value |
| --- | --- |
| Repository | `OpenAgentsInc/openagents` |
| Branch | `main` |
| Commit | `b86850ed8bcf528f29a51e38b9167292ac2f608e` |
| Tree | `741f8a040a0d2bb7f1df540b6830dcb626107560` |

The 17-file target corpus has aggregate SHA-256
`61e60ba29e86f2ac2a10eacfcc2ead54282aff5aaf5309cbbf065861432f668a`.
It includes `AGENTS.md`, `INVARIANTS.md`, `FASTFOLLOW.md`, the Desktop and
Effect Native package/renderer/document surfaces, and the prior VS Code,
Cursor, Pierre, and Zed analyses.

VS Code is not presently registered as a source in `FASTFOLLOW.md`. This is an
owner-directed source study using the Fast Follow evidence discipline, not a
new formal source registration or implementation candidate.

### Evidence labels

- **[source]** is directly present in the pinned checkout.
- **[lock]** is present in a checked-in package manifest or lockfile.
- **[history]** comes from the local Git history at the pin.
- **[inferred]** is a reasoned conclusion from multiple source observations.
- **[proposal]** is an OpenAgents-specific recommendation, not an upstream
  behavior claim.
- **[limitation]** states what this source-only pass cannot prove.

## 1. Repository shape: disciplined layers, not a reusable monorepo

VS Code's source organization is explicit:

```text
base → platform → editor → workbench → code/server
                                   ↘ sessions
```

`sessions` may import `workbench`; `workbench` may not import `sessions`.
Within each layer, `common`, `browser`, `node`, `electron-browser`,
`electron-utility`, and `electron-main` encode runtime availability. Services
use decorator-identified dependency injection, and contribution entry points
control what is loaded. The sessions layer adds its own enforced rule: ordinary
contributions cannot import provider internals. [source]

At the pin, the TypeScript scale is:

| Layer | `.ts` files | Lines |
| --- | ---: | ---: |
| `vs/base` | 467 | 150,243 |
| `vs/platform` | 1,946 | 416,155 |
| `vs/editor` | 852 | 278,158 |
| `vs/workbench` | 3,726 | 1,304,343 |
| `vs/code` | 18 | 6,148 |
| `vs/server` | 25 | 5,777 |
| `vs/sessions` | 489 | 152,290 |

There are 148 `package.json` files outside `node_modules`, but this does not
mean VS Code offers 148 supported application libraries. Most manifests belong
to built-in extensions, build/test tools, remote packaging, or isolated
servers. The root is one private `code-oss-dev` application package. [source]

The architecture is worth copying conceptually:

- dependency direction is a product invariant, not a convention;
- runtime environment appears in the folder/API boundary;
- services are injected and delayed instead of imported as ambient globals;
- feature code registers through contribution points;
- entry points determine load and startup cost;
- tests are colocated at every layer. The pin contains 1,396 `*.test.ts` or
  `*.integrationTest.ts` files under base/platform/editor/workbench/sessions.

OpenAgents already has stronger typed Effect and process-authority boundaries.
The lesson is to make its IDE layering comparably visible and enforceable, not
to replace Effect services with VS Code's service container. [inferred]

## 2. Monaco is the deliberate public editor seam

The repository itself explains the right consumption boundary.
`build/monaco/README-npm.md` says `monaco-editor-core` is a building block and
ordinary consumers should use `monaco-editor`, which adds language support.
`src/vs/editor/editor.api.ts` exports a deliberately small root API, and
`src/tsconfig.monaco.json` excludes Node, Electron, telemetry, and terminal
surfaces. [source]

The standalone editor API exposes the useful mechanics directly:

- code, diff, and multi-file-diff editor construction;
- stable text models identified by URI;
- language assignment and tokenization;
- markers and marker-change events;
- commands, editor actions, and keybinding rules;
- themes;
- web workers;
- link and editor openers;
- model/editor lifecycle events.

This seam is why Monaco is reusable and Explorer is not. Monaco contains a text
model, editor mechanics, and language-provider surface without pretending to
own files, Git, sessions, permissions, or application navigation. [inferred]

The accepted OpenAgents pin remains `monaco-editor@0.55.1`. The VS Code source
pin does not justify silently changing it: `build/monaco/package.json` uses a
placeholder development version, while an unrelated Copilot extension carries
an old private `monaco-editor@0.44.0`. Neither is the product dependency pin
OpenAgents should inherit. [source]

## 3. Explorer is behavior to adapt, not a package to install

The earlier Khala audit remains directionally correct.
`ExplorerModel` owns workspace roots, while `ExplorerItem` owns stable resource
identity, parent/child relations, resolved-directory state, metadata, and
merge behavior. `ExplorerView` composes the model with the workbench tree,
menus, configuration, editor, workspace, clipboard, progress, theme, and file
services. `AsyncDataTree` supplies lazy resolution, refresh, filter/find,
selection/focus, drag and drop, compression, and view-state restoration.
[source]

The cost is the boundary. At the pin:

| Surface | Direct file size | Direct imports | Approximate local import closure |
| --- | ---: | ---: | ---: |
| `explorerModel.ts` | 526 lines | 19 | 322 files |
| `explorerView.ts` | 1,145 lines | 53 | 781 files |
| `asyncDataTree.ts` | 1,739 lines | 23 | 156 files |

The closure numbers come from a lexical traversal of local relative imports;
they are an architectural pressure gauge, not a compiler-accurate bundle
graph. [limitation]

OpenAgents should continue using `@pierre/trees` for projection and its own
workspace index/file service for authority. The VS Code donor behaviors are:

- stable identity independent of a rendered row;
- lazy directory hydration with explicit unresolved state;
- merge rather than wholesale replacement after refresh;
- cancellation and stale-result rejection;
- deterministic sorting, filter, selection, reveal, focus, and keyboard laws;
- bounded persistence of expansion and view state;
- file operations routed through one typed authority service.

Those behaviors belong in the Pierre adapter and WorkContext contracts, not in
a port of `AsyncDataTree`. [proposal]

## 4. The file service is the real workbench substrate

`IFileService` and the provider contracts in `vs/platform/files/common/files.ts`
are more important than the Explorer renderer. The surface models provider
registration, capabilities, atomic read/write/delete, streams, stat/resolve,
watchers, file changes, copy/move/clone, readonly state, correlation, encoding,
ETags, operation events, and typed error/result classes. [source]

The lesson is capability-first I/O. A local disk provider, remote provider,
virtual document, untitled buffer, notebook, agent attachment, or test provider
can participate without teaching Explorer which backend it is using.

OpenAgents already has the safer receiving boundary: relative opaque path refs,
grant refs, revision refs, a main-owned workspace service, typed watch events,
and no renderer `file://` authority. It should add missing capabilities there
rather than substituting VS Code URIs and provider interfaces wholesale.
[proposal]

## 5. Text-file models separate editor mechanics from document truth

`TextFileEditorModel` is 1,217 lines with 33 direct imports because a mature
editable file is not just a string. It coordinates dirty state, save/revert,
encoding, preferred contents, backup and working-copy lifecycle, external
changes, conflict/error states, autosave, language, accessibility, and model
disposal. Its approximate local import closure is 348 files. [source]

The useful law is:

```text
filesystem/provider revision
  → application document model
    → Monaco text model
      → one or more editor widgets
```

An editor widget is never the file authority. Monaco may own an attached
model's undo stack, selections, decorations, folds, and scoped edit mechanics;
the app still owns workspace identity, version admission, save, recovery,
external-change reconciliation, permissions, and retention. [inferred]

This is compatible with the current OpenAgents document contract. The main
remaining correction is to replace full-value textarea events and duplicated
app undo arrays with versioned Monaco edit/model mechanics while keeping the
bounded canonical draft and revision boundary. [proposal]

## 6. Commands, context keys, menus, and keybindings form one action graph

VS Code does not let each surface invent an unrelated action system. Command
IDs, keybindings, menus, enablement expressions, context keys, and palette
visibility converge. Explorer, editor, terminal, Git, Problems, and agent
sessions contribute actions through the same graph. [source]

Two constraints are especially useful:

- runtime state belongs in models/services; context keys are projections for
  declarative enablement, not a hidden state database;
- capabilities gate actions structurally. The new session UI derives multi-chat
  and fork actions from advertised capabilities rather than provider-name
  switches.

OpenAgents should keep one typed Desktop command registry and project its
effective enablement into Monaco, Pierre, menus, shortcuts, voice proposals,
mobile, and web. Monaco's own command/action APIs should be adapter endpoints,
not a second command authority. [proposal]

## 7. The language stack has reusable lower layers and VS Code-only clients

The checked-in built-in extensions reveal three different boundaries.

### Directly reusable protocol and data packages

The lockfiles pin `vscode-uri`, `vscode-jsonrpc`,
`vscode-languageserver-protocol`, and
`vscode-languageserver-textdocument`. These are ordinary MIT-licensed protocol
and data libraries. They can be evaluated behind an OpenAgents language-host
service. [lock]

### Directly reusable standalone language services

The JSON, CSS, HTML, and Markdown extensions depend on separately published
language-service packages. The JSON/CSS/HTML servers do not import the `vscode`
extension API. Their parsers, completion, hover, validation, symbols, links,
formatting, and document models can be used in a worker or host process if
their exact package/version/browser constraints pass an admission spike.
[source]

### Not directly reusable as an application client

`vscode-languageclient` declares a VS Code engine and its extension-side code
imports the `vscode` runtime API. Installing it in OpenAgents would imply
emulating extension-host services. The reusable seam is the lower
JSON-RPC/LSP packages, with OpenAgents owning process lifecycle, URI mapping,
cancellation, request versioning, diagnostics, policy, and sandbox placement.
[source]

For TypeScript, Monaco's worker can provide the first file/local-project rung.
A full repository language experience requires a separately managed tsserver
or language-server process. VS Code's TypeScript extension is a behavior and
protocol reference, not a drop-in library. [proposal]

## 8. Focused packages worth evaluating

The following versions are observed at the VS Code source pin. They are
candidate evidence, not automatic OpenAgents pins.

| Package | Observed version | License in lock | What VS Code uses it for | OpenAgents disposition |
| --- | --- | --- | --- | --- |
| `monaco-editor` | OpenAgents accepted `0.55.1`; VS Code builds core from source | MIT | editor/model/language API | **Adopt already decided; implement the app adapter** |
| `vscode-uri` | `3.1.0` | MIT | standards-compatible URI values | **Evaluate only at LSP boundary; never expose absolute paths to renderer** |
| `vscode-jsonrpc` | `8.2.0` stable lineage; `9.0.0-next.12` in newer built-ins | MIT where declared | LSP transport | **Evaluate; prefer a compatible stable set** |
| `vscode-languageserver-protocol` | `3.17.5` / `3.17.6-next.18` | MIT | typed LSP messages | **Evaluate behind Effect schemas** |
| `vscode-languageserver-textdocument` | `1.0.12` / `1.0.13` | MIT | versioned text documents | **Evaluate; do not make it canonical document storage** |
| `vscode-json-languageservice` | `6.0.0-next.2` | MIT | JSON intelligence | **Worker spike; avoid prerelease admission without reason** |
| `vscode-css-languageservice` | `7.0.0-next.1` | MIT | CSS/SCSS/LESS intelligence | **Worker spike** |
| `vscode-html-languageservice` | `6.0.0-next.1` | MIT | HTML intelligence | **Worker spike** |
| `vscode-markdown-languageservice` | `0.5.0` | MIT | Markdown links/symbols/validation | **Later spike; compare against existing Markdown stack** |
| `@xterm/xterm` | `6.1.0-beta.288` | MIT | terminal emulator | **Strong later candidate behind typed Terminal host** |
| xterm search/serialize/WebGL add-ons | matching beta line | MIT | terminal UX/performance | **Admit only with xterm and packaged GPU/accessibility proof** |
| `@vscode/ripgrep-universal` | `1.18.0` | MIT | platform Ripgrep distribution | **Benchmark against current search worker; main process only** |
| `@vscode/tree-sitter-wasm` | `0.3.1` | MIT | tokenization and command parsing | **Later parse/symbol candidate, not required for Monaco launch** |
| `@vscode/vscode-languagedetection` | `1.0.23` | MIT | model-based language detection | **Defer; extension/path mapping is cheaper and more explainable** |
| `vscode-textmate` | `9.3.2` | MIT | TextMate grammar execution | **Defer; avoid duplicating Monaco/Pierre/Shiki tokenization** |
| `vscode-oniguruma` | `1.7.0` | review package notice | regex WASM for TextMate | **Only with a TextMate decision** |
| `@vscode/diff` | `0.0.2-7` | MIT | editor line diff computation | **Do not add now; Pierre/Monaco already own presentation paths** |
| `@vscode/codicons` | `0.0.46-21` | CC-BY-4.0 | product/editor icon font | **Reject for product identity and extra attribution plane** |
| `@vscode/sqlite3` | `5.1.12-vscode` | BSD-3-Clause | native SQLite | **Reject; OpenAgents already owns a SQLite runtime** |
| `@vscode/debugadapter` / `@vscode/debugprotocol` | `1.68.0` in Copilot extension | MIT | DAP adapter/protocol | **Later DAP study, not basic editor scope** |

`@vscode/diff` has only a handful of source call sites. `@xterm/xterm` has 87,
reflecting a large terminal integration surface. Package adoption removes the
need to write an emulator; it does not provide the surrounding commands,
process lifecycle, shell integration, accessibility, reconnection, or security
policy. [source][inferred]

## 9. Themes are a translation plane, not a product shell

VS Code combines workbench colors, TextMate token rules, semantic token rules,
file icons, and product icons. `ColorThemeData` loads theme JSON, resolves
included token files, merges user customizations, and produces editor token
colors. Monaco exposes `defineTheme`/`setTheme`. [source]

OpenAgents should keep one Effect Native-owned product theme and generate
bounded projections for Monaco, Pierre, Shiki, xterm, and any Problems/diff
surface. Loading arbitrary VS Code themes or icon themes would expand the
extension/file/resource trust surface and undermine consistent product chrome.
[proposal]

## 10. The extension host is an ecosystem, not a TypeScript dependency

The public extension API declaration is 21,235 lines. The main extension-host
protocol is 4,205 lines before implementation, scanning, lifecycle,
activation, proposed APIs, remote placement, web workers, storage, settings,
workspace trust, and marketplace/update flows are counted. [source]

The `vscode` module imported by extensions is supplied by the VS Code extension
host. It is not an ordinary application package. Reusing built-in extension
source generally requires either:

1. porting the useful logic below the extension API;
2. consuming a separately published server/library package; or
3. implementing a compatible extension host.

Only the first two fit the accepted OpenAgents plan. A future OpenAgents plugin
system should be a versioned, capability-limited component/tool ABI behind
canonical brokers, not accidental VS Code extension compatibility. [proposal]

## 11. Remote development demonstrates placement symmetry

`src/vs/server/node/` separates remote filesystem, extension scanning/hosts,
terminal channels, environment, connection tokens, web client serving, and now
remote agent-host management. The browser/workbench consumes services whose
implementation may live locally or remotely. [source]

The behavior to adapt is placement-transparent identity with explicit
capabilities. OpenAgents already has the stronger vocabulary: project,
worktree, WorkContext, runtime placement, provider session, and receipt. A
remote project must expose the same file/language/terminal/Git interfaces while
retaining different authorization and disclosure scopes. [proposal]

## 12. VS Code's new Agents Window

At this source pin, VS Code has added `src/vs/sessions/` as a distinct
top-level application layer above the workbench. It is not a chat panel bolted
into the default layout. It creates a sessions-first workbench with fixed
chrome, a session list, one or more session views, optional editor/detail pane,
Changes and Files views, and a hidden panel for terminal/log surfaces.
[source]

The dependency law is explicit:

```text
workbench ──X──▶ sessions
sessions ─────▶ workbench and lower layers
core/contrib ──X──▶ provider internals
provider implementations ─────▶ shared session services
```

This is the most relevant architectural comparison to Cursor's graft. VS Code
is retaining its classic editor/workbench while building an agent-first shell
as an overlay layer. [inferred]

### Model, view, and provider separation

The sessions architecture separates:

- `ISessionsManagementService`: aggregate sessions, create/send/CRUD, recency,
  drafts, and provider routing;
- `ISessionsService`: active/visible session views, focus, layout,
  back/forward, and persistence;
- `ISessionsProvidersService`: provider registration;
- `ISession`: observable provider-neutral session facade;
- providers: local chat, Copilot, local Agent Host, and remote Agent Host.

A provider-scoped session ID is distinct from the provider's raw session ID,
the logical session type, the resource URI scheme, and the language-model
target. This lets one logical agent type span local and remote hosts without
colliding routing or model catalogs. [source]

Chats have explicit interactivity: Full, ReadOnly, or Hidden. Archived and
subagent histories can remain visible without exposing a composer or mutating
actions. Provider capabilities gate multiple chats, forking, deletion, and
other actions; core UI does not switch on provider name. [source]

### Layout and responsive projection

The desktop agent shell keeps Sessions as the flexible center, Files/Changes
as a detail surface, editors explicitly revealable, and the panel hidden until
needed. Per-session layout state remembers auxiliary/panel visibility and
editor working sets. The phone projection uses separate mobile subclasses,
default-denies unsupported views, replaces the sidebar with a drawer, and
offers dedicated Changes/diff overlays. It explicitly admits that Files and
Terminal still need phone-specific surfaces. [source]

The useful lesson is not the exact chrome. It is that one session can restore
its editor working set, changes view, terminal, and approval state without
making the renderer the session authority. [inferred]

## 13. Agent Host: harnesses behind a generated state protocol

`src/vs/platform/agentHost/` is a separate utility-process runtime and shared
platform service. It discovers agents, connects local/SSH/WSL/tunnel hosts,
normalizes sessions, exposes approvals and configuration, tracks tools and
turns, manages terminals/Git/resources, and projects state to clients.
[source]

The protocol copy under `common/state/protocol/` is generated from a sibling
Agent Host Protocol repository. At the pin it contains 11,380 lines across
root, session, chat, terminal, changeset, annotations, OTLP, and resource-watch
channels plus action/notification version registries. Generated registries
make missing version metadata a type error. [source]

The in-memory state manager keeps session states, chat states, opaque
provider-data blobs, client subscriptions, active turns, changesets, and
resource channels. Reducers project actions into snapshots. Agent-specific
SDK state stays behind an `IAgent` boundary. [source]

This is close to OpenAgents' desired “one canonical runtime event plane, many
clients” shape, but it is not a reusable package and should not compete with
OpenAgents' existing generated protocol and harness adapters. The reusable
lesson is generated/versioned state plus opaque provider continuation data,
not the AHP types themselves. [proposal]

### Multi-chat ownership

VS Code distinguishes:

- harness SDK session;
- chat channel within that session;
- orchestrator session containing the chat catalog and persisted state.

The orchestrator records and routes; the harness creates and drives chats.
User-created and harness-spawned chats enter one catalog path. Provider data is
opaque. Session URIs and chat URIs are structurally distinct. Capability flags
gate multi-chat and fork UI. Persisted peer chats and transient spawned chats
have different restore semantics. [source]

Those invariants are excellent adaptation material for OpenAgents' parent/child
session topology. They also expose a warning: the pinned Codex adapter is still
single-chat while Claude and Copilot have peer chats, so a common UI cannot
pretend every runtime has identical semantics. [source][inferred]

### Harness adapters and scale

The pin contains dedicated Claude, Copilot, and Codex adapters. Approximate
TypeScript footprints are 12,456 lines for Claude, 15,910 for Copilot, and
18,527 for Codex, with Codex spread across 670 files because the generated
protocol creates many small types. The workbench-side
`AgentHostSessionHandler` is 4,862 lines with 81 direct imports and an
approximate 1,810-file local import closure. [source]

This is a concrete demonstration that “support a harness” means far more than
spawn a CLI. It includes authentication, model/config discovery, permissions,
elicitation, tool display, history/replay, file edits, subagents, MCP, usage,
errors, local tools, and lifecycle reconciliation. [inferred]

OpenAgents already owns a broader harness research and adapter program. VS
Code's code is valuable as a gap checklist, not a replacement interface.
[proposal]

## 14. Worktrees, checkpoints, changesets, and review

The current Agent Host has moved isolation above individual harness adapters.
One host-owned `WorktreeIsolation` service supports folder/worktree config,
branch completion, deferred first-send creation, shared metadata, archive/
unarchive cleanup and recreation, serialized lifecycle, and user-visible
announcements across Copilot, Claude, and Codex. Worktrees live in a sibling
`<repo>.worktrees` directory. [source]

The checkpoint service captures the working tree through a temporary index and
writes hidden Git refs under:

```text
refs/agents/<sanitized-session-id>/checkpoints/turn/<N>
```

It creates a baseline and parent-chained per-turn commits with `commit-tree`,
reuses the parent ref for no-op turns, records exact refs in the session DB,
and deletes them before session data is destroyed. These commits are reachable
for recovery without appearing as branches or tags. [source]

The changeset service publishes branch-, session-, uncommitted-, and per-turn
diff views as subscribable protocol resources. It combines Git-derived and
file-edit-tracker evidence, persists static results, lazily recomputes
subscribed views, and exposes aggregate additions/deletions/files to session
lists. [source]

This is one of the strongest source-grounded agent-review patterns in the
repository. OpenAgents should adapt:

- host-owned isolation shared across runtimes;
- exact worktree provenance and serialized lifecycle;
- checkpoint refs that include terminal edits, not only recognized edit tools;
- session/branch/turn changesets as separate named evidence;
- lazy subscription and summary projection;
- cleanup that knows exactly which refs it owns.

OpenAgents should strengthen it with its existing receipts, placement,
retention, destructive-action refusal, and project/worktree identity. A hidden
ref is recoverable evidence, not by itself a durable execution receipt.
[proposal]

## 15. Agent Host persistence is intentionally split

Every Agent Host session gets
`{userDataPath}/agentSessionData/{sessionId}/session.db` plus an `attachments/`
directory. SQLite migrations currently define:

- turns and SDK event IDs;
- file edits with before/after blobs, rename metadata, and line counts;
- arbitrary session metadata;
- per-turn checkpoint refs;
- per-chat drafts;
- reviewed-file URI/content-nonce pairs;
- host-injected local turns.

Session summary caches are also stored through the workbench storage service so
the list can render before the host starts. Local chat sessions keep a separate
profile-scoped summary store and lazily load chat models. Agent-host OTel can
write a separate WAL-mode SQLite database under
`<userData>/agent-host/otel/agent-host-traces.db`. [source]

The architecture optimizes cold-list rendering and separates large blobs from
observable state, but it creates multiple truth classes: provider history,
orchestrator catalog, session database, workbench summary cache, hidden Git
refs, attachments, and optional telemetry DB. [inferred]

OpenAgents should learn from both sides: lazy summaries and on-demand blobs are
good; every persisted class still needs a declared inventory, authority,
retention, export, and verified deletion law. [proposal]

## 16. Skills, instructions, hooks, MCP, and harness customizations

VS Code's AI customization manager aggregates agents, skills, instructions,
prompts, hooks, plugins, and MCP servers across workspace, user, extension,
built-in, plugin, and remote-host stores. A harness descriptor defines which
roots, storage sources, sections, file filters, providers, and actions apply.
The active session worktree becomes the sessions-window project root. [source]

This is a useful answer to a common product mistake: storage location and
runtime consumer are separate concepts. The same file can exist in a workspace
but be ignored by a given harness; a remote harness can contribute its own
items, disable controls, and plugin actions. [inferred]

OpenAgents should retain its own typed skill/tool/plugin admission and policy
planes, but it should adopt the explicit tuple:

```text
artifact identity + storage source + consuming harness + project scope
+ enabled scope + provenance + effective version
```

It should not infer compatibility merely from a filename such as `AGENTS.md`
or `CLAUDE.md`. [proposal]

## 17. Agent SDK packaging is stronger than ambient npm installation

VS Code pins Claude and Codex SDKs twice: root development types and isolated
`build/agent-sdk/agents/<sdk>` manifests/lockfiles. CI builds one deterministic
tarball per platform, hashes it, uploads it to a versioned CDN path with
HEAD-before-upload idempotence, stamps a versioned URL template into
`product.json`, and downloads/caches the SDK on first use. A test rejects drift
between build and runtime pins. Web builds omit the Node-only SDK surface.
[source]

At the pin:

- `@anthropic-ai/claude-agent-sdk` is exactly `0.3.198`;
- `@openai/codex` is exactly `0.142.0`;
- `@github/copilot-sdk` is a normal root dependency at
  `^1.0.7-preview.0` and is packaged differently.

The pattern worth adapting is an exact, independently inspectable runtime
artifact closure with version/hash/platform identity and fail-loud drift—not
the Microsoft CDN or product descriptor. OpenAgents receipts should bind the
effective harness artifact, not only the adapter package version. [proposal]

## 18. Cursor's graft onto VS Code

The Cursor teardown establishes a different strategy on the same substrate.
Installed Cursor 3.11.13 reports a VS Code 1.125 base and retains the classic
workbench/Electron application. Its product delta includes a separate Glass
agent UI bundle, 17 `cursor-*` extensions, native agent/retrieval/sandbox/
browser/worktree sidecars, Cursor services and object stores, remote codebase
embeddings, and extensive local session/checkpoint/history state. [bundle]

The likely reason for this structure is straightforward. [inferred]

- Keeping upstream VS Code intact preserves editor, extension, language,
  terminal, settings, and remote breadth.
- Extensions are a comparatively low-conflict place to add commands, views,
  provider integration, and compatibility shims.
- Native sidecars isolate fast indexing, process control, sandboxing, and
  runtime-specific responsibilities from the renderer.
- A separate agent-first bundle lets Cursor move product UX faster than the
  classic workbench contribution system.
- The cost is duplicated state, startup/default regressions, merge burden, and
  a difficult-to-explain local/remote data inventory.

Current VS Code's new `sessions` layer reaches a similar “agent overlay above
the editor” conclusion without putting it in third-party extensions. The two
systems therefore corroborate the boundary, not the implementation choice:

```text
stable editor substrate
  + agent-session shell
  + runtime adapters/sidecars
  + worktree/review/persistence services
```

OpenAgents should use that shape with owned TypeScript packages rather than a
fork: Effect Native and the Desktop React projection above Monaco/Pierre;
typed main-process project/document/Git/terminal/language services; separate
real harness adapters; and one canonical project/evidence graph. [proposal]

## 19. What to take, study, and reject

### Take as dependencies after explicit admission

- `monaco-editor@0.55.1` through the already accepted lazy app adapter;
- a compatible stable `vscode-uri` + `vscode-jsonrpc` + LSP protocol/document
  set for the language-host boundary;
- selected standalone language services after worker/package proof;
- xterm and only the add-ons justified by the Terminal contract;
- optionally Ripgrep's platform package after a benchmark and release-matrix
  spike;
- DAP protocol packages only when debugging becomes an admitted product rung.

### Adapt as architecture and behavior

- layer and runtime dependency rules;
- file-provider capabilities and operation events;
- stable document models above editor widgets;
- one command/context/menu/keybinding graph;
- lazy tree merge, reveal, selection, and accessibility laws;
- LSP lifecycle, cancellation, versioning, and Problems projection;
- local/remote placement symmetry;
- sessions model/view/provider separation;
- explicit session/chat/provider/runtime identity;
- capability-gated multi-chat and read-only histories;
- shared worktree isolation, hidden-ref checkpoints, changesets, and review;
- harness customization source/consumer separation;
- exact platform runtime artifacts and drift tests.

### Study behind explicit spikes

- Tree-sitter WASM for local symbols or safe command parsing;
- TextMate/Oniguruma only if Monaco/Pierre/Shiki cannot cover a required grammar;
- language detection only if unknown-file UX warrants its model/runtime cost;
- Ripgrep packaging versus the current OpenAgents search worker;
- xterm WebGL, image, ligature, search, and serialization add-ons;
- remote filesystem/language/terminal protocol behavior;
- DAP adapter and debug state once the basic IDE rung is complete.

### Reject

- forking Code-OSS or Cursor's private delta;
- importing unpublished `src/vs/*` modules;
- porting Explorer, `AsyncDataTree`, workbench layout, settings, or extension
  host wholesale;
- treating `vscode-languageclient` as a standalone app library;
- Codicons as OpenAgents product identity;
- a second SQLite/native runtime where OpenAgents already owns one;
- provider-name switches in shared UI;
- agent mutation of Monaco or renderer state;
- a second agent protocol/session authority beside OpenAgents;
- claims of VS Code/Cursor parity based only on Monaco, a tree, and a terminal.

## 20. Failure modes to avoid

| Failure | Why the source makes it likely | OpenAgents guard |
| --- | --- | --- |
| “Internal TypeScript is reusable” | Explorer/workbench files have very large local closures | only import published packages; port concepts through owned contracts |
| Two document owners | Monaco can look authoritative while disk/recovery is elsewhere | versioned main document boundary + one model binding per document ref |
| Two command systems | Monaco and React can each register shortcuts | one typed command registry projected into adapters |
| LSP path leakage | LSP uses real URIs while renderer uses opaque refs | main-owned URI translation; diagnostics return document refs/generations |
| Native package release breakage | Ripgrep/xterm WebGL/SQLite/PTY have platform assets | exact lock, staged artifact inventory, packaged six-target proof |
| Duplicate tokenizers | Monaco, Shiki/Pierre, TextMate, and Tree-sitter overlap | assign one purpose to each admitted parser/tokenizer |
| Provider semantic flattening | Claude/Copilot/Codex multi-chat behavior differs | capability and runtime identity in every projection |
| Hidden-ref overclaim | Git checkpoint refs are useful but not receipts | bind refs to project/worktree/session/turn evidence and retention policy |
| Persistence sprawl | summaries, DBs, attachments, SDK stores, Git refs diverge | one local-data inventory, retention/export/delete verification |
| Fork merge treadmill | Cursor carries upstream plus a growing private platform | stock packages + owned app shell, no Code-OSS fork |

## 21. OpenAgents-specific conclusion

VS Code should become the main **TypeScript component/protocol donor** and a
secondary **behavior specification**, not the main integrated architecture.
The reference stack is now coherent:

| Question | Primary reference |
| --- | --- |
| How should the editor, project, language, Git, terminal, and agent share one model? | Zed architecture |
| Which practical code editor should OpenAgents ship? | Monaco |
| Which focused tree/diff renderers should it evaluate? | Pierre |
| Which TypeScript protocols and focused libraries can be reused? | VS Code package ecosystem |
| What is the product-breadth floor and fork caution? | Cursor |
| Who owns authority, placement, policy, persistence, and receipts? | OpenAgents Effect Native/main/runtime contracts |

The immediate product implication is deliberately narrow: complete the
existing Monaco adapter packet, then use this package ledger to avoid
reimplementing LSP protocol, JSON/HTML/CSS language intelligence, terminal
emulation, and later DAP. Do not turn the ledger into an excuse to import the
workbench. [proposal]

## Source map

| Area | Primary pinned source |
| --- | --- |
| Layer rules | `.github/instructions/source-code-organization.instructions.md`; `src/vs/sessions/LAYERS.md` |
| Monaco public seam | `build/monaco/`; `src/vs/editor/editor.api.ts`; `src/vs/editor/standalone/browser/standaloneEditor.ts`; `src/tsconfig.monaco.json` |
| Explorer/tree | `src/vs/workbench/contrib/files/common/explorerModel.ts`; `.../browser/views/explorerView.ts`; `src/vs/base/browser/ui/tree/asyncDataTree.ts` |
| File and document model | `src/vs/platform/files/common/files.ts`; `src/vs/workbench/services/textfile/common/textFileEditorModel.ts` |
| Themes | `src/vs/platform/theme/common/themeService.ts`; `src/vs/workbench/services/themes/common/colorThemeData.ts` |
| Extensions | `src/vscode-dts/vscode.d.ts`; `src/vs/workbench/api/common/extHost.protocol.ts`; `src/vs/workbench/services/extensions/common/extensions.ts` |
| Language packages | `extensions/{json,css,html,markdown}-language-features/**/package*.json` and server sources |
| Sessions architecture | `src/vs/sessions/README.md`; `SESSIONS.md`; `LAYOUT.md`; `MOBILE.md`; `SESSIONS_LIST.md`; provider docs |
| Agent state/protocol | `src/vs/platform/agentHost/common/state/`; `node/agentHostStateManager.ts`; `MULTI_CHAT_ARCHITECTURE.md` |
| Persistence | `common/sessionDataService.ts`; `node/sessionDataService.ts`; `node/sessionDatabase.ts`; `OTEL.md` |
| Isolation/checkpoint/review | `node/shared/worktreeIsolation.ts`; `common/node agentHostCheckpointService.ts`; `common/node agentHostChangesetService.ts` |
| Harness UI bridge | `src/vs/workbench/contrib/chat/browser/agentSessions/agentHost/agentHostSessionHandler.ts` |
| Runtime artifact packaging | `build/agent-sdk/` and packaging sections of `build/gulpfile.vscode.ts` |
| Cursor fork comparison | `docs/teardowns/2026-07-11-cursor-product-teardown.md` |

## Final recommendation

Install packages, not a workbench. Port laws, not dependency graphs. Keep one
OpenAgents project/document/evidence authority and let Monaco, Pierre, LSP
libraries, language services, xterm, and later DAP act as replaceable typed
projections or protocol helpers around it.
