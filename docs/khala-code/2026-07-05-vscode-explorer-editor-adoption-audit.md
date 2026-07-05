# Khala Code Desktop VS Code Explorer / Editor Adoption Audit

Date: 2026-07-05
Status: audit / implementation direction. No code changed by this doc.
Scope: `clients/khala-code-desktop`, with `projects/repos/vscode` used as
read-only source material for Explorer, tree, file, and editor architecture.

## Executive Decision

Khala Code should not vendor the VS Code workbench or Explorer stack wholesale.
The right path is:

- Use Monaco as the code editor renderer. It is the extracted editor surface
  from VS Code and avoids copying `src/vs/editor` internals directly.
- Adapt VS Code Explorer ideas, not the full implementation: stable file item
  identity, lazy directory hydration, explicit selection/reveal state,
  deterministic sorting, typed file metadata, bounded refresh after file
  changes, and accessible keyboard tree behavior.
- Put a Khala-owned workspace file service between the editor UI and every
  backing source. Codex can be one optional provider later, but the editor must
  not depend on Codex app-server as its authority.
- Add an `Editor` hotbar slot to the existing Khala desktop shell, then mount a
  first read-only editor panel with a basic file tree and source viewer.

The first useful feature should be a read-only, workspace-rooted source browser:
open Khala desktop, click `Editor`, browse the current working directory, and
open a text file with syntax-highlighted source. Editing, dirty tabs, file
watchers, DnD, rename, delete, and full context menus can come later.

## Current Khala Code Shape

Khala Code Desktop is an Electrobun app with a Bun host and Vite-built DOM UI.
The desktop package currently has React available, but the active shell is
mostly hand-mounted DOM modules under `src/ui`.

Key receiving points:

- `clients/khala-code-desktop/src/ui/sidebar.ts` owns the command hotbar. Its
  current `KhalaCodeHotbarValue` union and `KHALA_CODE_HOTBAR_SLOTS` array are
  `chat`, `fleet`, `forum`, `inbox`, and `settings`.
- `clients/khala-code-desktop/src/ui/main.ts` owns panel switching. It already
  mounts fleet, forum, inbox, gym, and settings panels, then hides the chat
  transcript/composer when a non-chat panel is active.
- `clients/khala-code-desktop/src/ui/index.html` already has one hidden
  section per full-screen panel. An editor panel would fit as another sibling
  section, for example `id="editor-panel"` and `class="khala-code-editor"`.
- `clients/khala-code-desktop/src/shared/rpc.ts` currently declares legacy raw
  Codex app-server filesystem pass-through methods:
  `codexFsGetMetadata`, `codexFsReadFile`, and `codexFsWriteFile`. Treat these
  as compatibility context, not as the editor abstraction.
- `clients/khala-code-desktop/src/bun/rpc-handlers.ts` forwards those legacy
  methods to `fs/getMetadata`, `fs/readFile`, and `fs/writeFile`.
- `clients/khala-code-desktop/src/bun/rpc-handlers.ts` already uses
  `fs/readDirectory` and `fuzzyFileSearch` to power `/mention` candidates, so
  the app has prior file-discovery affordances to learn from.

The main gap is a provider-neutral editor file contract. A polished editor
should not let UI code parse unknown filesystem payloads ad hoc, and it should
not bind source browsing to one agent runtime. Add a typed Khala editor service
before rendering.

## VS Code Source Material To Adapt

The useful VS Code donor code is architectural, not copy-paste UI.

### Explorer model

Reference:

- `projects/repos/vscode/src/vs/workbench/contrib/files/common/explorerModel.ts`

Relevant ideas:

- `ExplorerModel` owns workspace roots and rebuilds roots when the workspace
  folder set changes.
- `ExplorerItem` owns stable resource identity, parent/child links, directory
  resolved state, metadata, readonly/locked flags, and child lookup.
- `ExplorerItem.fetchChildren()` lazily resolves children only when a directory
  expands, then merges disk data into the local model without throwing away
  unresolved local state.
- `ExplorerItem.getId()` composes root and resource identity so tree selection
  and expansion survive refreshes.
- The model has explicit hooks for file nesting and filtering. Khala should
  defer those, but keep the model open for them.

Khala adaptation:

- Add `KhalaCodeEditorTreeNode` / `KhalaCodeEditorRoot` schemas in a shared
  editor contract file.
- Track `path`, `name`, `kind`, `depth`, `parentPath`, `childrenLoaded`,
  `children`, `mtime`, `sizeBytes`, `readonly`, `symlink`, and `error`.
- Use stable IDs derived from `rootPath + "::" + absolutePath`, but keep raw
  absolute paths inside local-only desktop state and never public receipts.
- Load roots from `input.workingDirectory` first. Later add explicit extra roots
  from Khala workspace selection, Fleet/Pylon workspaces, remote sessions, or
  other provider adapters behind the same contract.

### Explorer data source, renderer, and interaction

Reference:

- `projects/repos/vscode/src/vs/workbench/contrib/files/browser/views/explorerView.ts`
- `projects/repos/vscode/src/vs/workbench/contrib/files/browser/views/explorerViewer.ts`

Relevant ideas:

- `ExplorerDataSource` is lazy. `hasChildren()` is cheap, and `getChildren()`
  can return a promise for directory hydration.
- `ExplorerView.createTree()` separates filter, renderer, sorter, drag/drop,
  identity provider, keyboard navigation label provider, and open behavior.
- `onDidOpen` ignores directories and calls the editor service only for file
  resources.
- `FilesRenderer` centralizes row rendering, labels, ARIA labels, active
  descendants, inline rename input, and compact-folder navigation.
- `FilesFilter` respects `files.exclude`, `.gitignore`, and "show opened files
  even if hidden."
- `FileSorter` is a separate unit, not hard-coded into rendering.

Khala adaptation:

- Start with a small `mountKhalaCodeEditorPanel(container, options)` DOM module,
  matching the existing `mountFleetPanel` / `mountCodexSettingsPanel` style.
- Build a semantic `tree`/`treeitem` surface with buttons, not generic divs.
- Implement keyboard basics in the first pass: Up/Down, Left/Right collapse and
  expand, Enter opens file, Home/End, and visible focus rings.
- Keep row height stable and bounded. VS Code uses 22px rows; Khala can use a
  denser product UI row, but target pointer hit areas should remain ergonomic.
- Do not take VS Code's workbench context-key system, context menu plumbing,
  DnD, inline rename, compact folder navigation, or contribution registry in
  the first implementation.

### Tree/list primitives

Reference:

- `projects/repos/vscode/src/vs/base/browser/ui/tree/asyncDataTree.ts`
- `projects/repos/vscode/src/vs/base/browser/ui/list/listWidget.ts`
- `projects/repos/vscode/src/vs/base/browser/ui/list/listView.ts`

Relevant ideas:

- VS Code's tree is virtualized, async, cancellable, keyboard-navigable, and
  deeply integrated with list focus/selection.
- It separates item identity, render templates, async data, filtering, sorting,
  and drag/drop.

Khala adaptation:

- Do not port these primitives for the initial feature. They bring a large
  platform surface and are tightly coupled to VS Code's base libraries.
- If large-repo rendering becomes slow, use an existing small virtualization
  library or write a bounded Khala-specific virtual tree after measuring.
- Preserve the same separations in local code so virtualization can replace the
  simple renderer later without changing the editor contract.

### File service

Reference:

- `projects/repos/vscode/src/vs/platform/files/common/files.ts`
- `projects/repos/vscode/src/vs/platform/files/node/diskFileSystemProvider.ts`
- `projects/repos/vscode/src/vs/platform/files/node/diskFileSystemProviderServer.ts`

Relevant ideas:

- VS Code has an explicit `IFileStat` shape with file/directory/symlink,
  readonly/locked/executable, size, mtime, ctime, and optional children.
- File watcher events are typed as added, updated, and deleted.
- Provider capabilities are explicit, including read/write, path case
  sensitivity, trash, and readonly behavior.

Khala adaptation:

- Add typed editor RPCs backed by a Khala-owned provider interface:
  - `editorProviderList()`
  - `editorWorkspaceRead()`
  - `editorDirectoryRead({ path })`
  - `editorFileRead({ path, maxBytes? })`
  - later `editorFileWrite({ path, dataBase64, expectedMtime? })`
  - later `editorWatchStart({ rootPath })` / `editorWatchStop(...)`
- Keep all paths root-bounded to the selected workspace. Refuse path traversal
  and return typed errors such as `outside_workspace`, `binary_file`,
  `file_too_large`, `not_found`, and `provider_unavailable`.
- Implement the first provider as a local desktop workspace filesystem adapter
  owned by Khala. Additional adapters can cover remote worktrees, Fleet/Pylon
  sessions, and Codex compatibility without changing the editor UI.
- Add Effect Schema decoders for every host/provider payload before UI code
  uses it.

### Code editor

Reference:

- `projects/repos/vscode/src/vs/editor/standalone/browser/standaloneEditor.ts`
- `projects/repos/vscode/src/vs/editor/standalone/browser/standaloneCodeEditor.ts`
- `projects/repos/vscode/src/vs/editor/browser/widget/codeEditor/codeEditorWidget.ts`

Relevant ideas:

- `standaloneEditor.create(domElement, options)` is the small public-style API
  for creating an editor in an arbitrary container.
- `createModel(value, language, uri)` creates a text model and infers language
  from a URI or first line when no language is supplied.
- `StandaloneEditor` owns an auto-created model unless a model is passed in.
- `CodeEditorWidget` has the event and accessibility surface Khala wants:
  content changes, cursor movement, layout changes, focus/blur, key events,
  paste/copy/cut, and scroll changes.

Khala adaptation:

- Add `monaco-editor` as a desktop dependency and load it dynamically inside
  the editor panel so the default Chat view does not pay editor startup cost.
- Define a Khala theme that maps Monaco colors to existing
  `--oa-color-khala-*` tokens.
- For v1, open one model at a time in read-only mode:
  `readOnly: true`, `automaticLayout: true`, `minimap.enabled: false`, and
  `largeFileOptimizations: true`.
- Add language inference by extension in Khala's wrapper, then let Monaco refine
  where it can.
- Later, keep one Monaco model per open tab and dispose models when tabs close.

## What Not To Take

Avoid these from VS Code until there is a specific measured need:

- Full workbench DI, context keys, menu registry, action registry, storage
  service, telemetry service, and theme service.
- Explorer DnD and clipboard file operations.
- Inline create/rename/delete UI.
- Compact folders and file nesting as default behavior.
- VS Code's extension host, icon theme machinery, and file decoration provider.
- Raw Electron disk providers. Khala is Electrobun and should expose its own
  bounded desktop file provider instead of duplicating VS Code's provider layer
  or inheriting a Codex-specific bridge.

## License And Reuse Boundary

`projects/repos/vscode` is MIT-licensed, so Khala can study and adapt the code
with attribution. The recommended path is still concept-level adaptation:
Monaco as a dependency, locally written Explorer/data-source wrappers, and tests
that encode the behavior Khala actually needs.

If a future patch copies a VS Code function or class verbatim, preserve the
license notice required by the upstream license and keep the copied surface
small enough that it can be audited during review. Do not copy VS Code product
identity, marketplace/extension integration, telemetry plumbing, or workbench
registration scaffolding into Khala Code.

## Required Khala Changes

### 1. Dependency and bundling

- Add `monaco-editor` to `clients/khala-code-desktop/package.json`.
- Configure Vite/Bun bundling for Monaco workers. Use dynamic import and worker
  URL setup appropriate to Vite.
- Confirm Electrobun packaged assets include Monaco worker chunks.
- Add a build smoke that opens the editor panel in packaged/dev preview without
  blank editor canvas.

### 2. Typed editor contracts

Add shared schemas, likely in `clients/khala-code-desktop/src/shared/editor.ts`:

- `KhalaCodeEditorWorkspace`
- `KhalaCodeEditorTreeNode`
- `KhalaCodeEditorDirectoryReadRequest`
- `KhalaCodeEditorDirectoryReadResult`
- `KhalaCodeEditorFileReadRequest`
- `KhalaCodeEditorFileReadResult`
- `KhalaCodeEditorError`

Then wire RPC names in `src/shared/rpc.ts` and `src/bun/rpc-handlers.ts`.

### 3. Editor panel shell

Add:

- `clients/khala-code-desktop/src/ui/editor-panel.ts`
- `clients/khala-code-desktop/tests/editor-panel.test.ts`

The first UI should have:

- left file tree, root label, refresh control, loading state, and error state;
- right editor surface with empty state, file title, path, size/language
  metadata, and Monaco container;
- no nested cards;
- stable panel dimensions so tree rows, tabs, and editor loading do not shift
  the shell;
- all visible controls using `@openagentsinc/ui/icon-dom`, matching the current
  house icon rule.

### 4. Hotbar integration

Update:

- `src/ui/sidebar.ts`: add `editor` to `KhalaCodeHotbarValue` and
  `KHALA_CODE_HOTBAR_SLOTS`.
- `src/ui/main.ts`: import/mount the editor panel, include it in
  `setActiveView`, and decide whether Chat composer hides while Editor is open.
- `src/ui/index.html`: add the hidden `editor-panel` section.
- `src/ui/styles.css`: add editor layout tokens/classes using existing Khala
  color and spacing tokens.
- `tests/sidebar.test.ts`, `tests/app-shell.test.ts`, and any hotbar shortcut
  assertions.

Slot order decision:

- Lowest-risk: append Editor as slot 6 to avoid changing existing Option+1..5
  muscle memory.
- Product-forward: put Editor at slot 2 after Chat, then shift Fleet and later
  panels. This is better long term, but it needs explicit UX contract/test
  updates because hotkeys change.

### 5. Safety and privacy boundaries

- Treat the editor as local-only. Do not include opened paths, file contents,
  or selected file metadata in public run receipts, product promises, telemetry,
  or outside-user evidence.
- Bound all file reads to the active workspace/root unless the user explicitly
  adds another root.
- Cap file reads in v1. A sensible first cap is enough for source browsing but
  should refuse huge files before Monaco receives them.
- Detect binary files before rendering and show a safe unsupported state.
- Keep writes out of v1 unless conflict handling and dirty-state tests are in
  place.

## Implementation Phases

### Phase 1: read-only source browser

Goal: click Editor, browse files, open source code.

Work:

- Add typed workspace/directory/file-read RPCs.
- Add editor hotbar slot and hidden panel section.
- Add `editor-panel.ts` with lazy directory expansion and Monaco read-only
  viewer.
- Add focused DOM tests for tree rendering, keyboard open, typed errors, and
  binary/large-file handling.
- Add one visual smoke for Editor panel nonblank render.

Do not include editing, save, rename, delete, drag/drop, file watchers, or
multi-tab dirty state.

Phase 1 implementation log:

- #8431 added the provider-neutral editor file service foundation:
  `src/shared/editor.ts`, `src/bun/editor-file-service.ts`, and the
  `editorProviderList`, `editorWorkspaceRead`, `editorDirectoryRead`, and
  `editorFileRead` RPC methods. The first provider is a Khala-owned local
  workspace adapter; Codex app-server is not part of the editor file authority.
- #8432 added the desktop Editor hotbar slot as Option/Alt+6, the empty editor
  panel shell, `?view=editor` routing, and stable split-pane layout tokens
  while preserving the existing Chat, Fleet, Forum, Inbox, and Settings slots.
- #8433 replaced the empty shell with a lazy workspace tree and read-only
  Monaco source pane. Monaco, workers, and editor CSS load only from the editor
  panel path, the tree supports keyboard navigation, and binary/oversized
  reads stay in a safe unsupported state.

### Phase 2: workspace ergonomics

Goal: make browsing feel like a real coding tool.

Work:

- Add fuzzy file open using existing `fuzzyFileSearch`.
- Add active-file reveal and persist expansion/selection in local storage.
- Add `.gitignore` / exclude support using a small typed ignore matcher.
- Add tabs or an "open files" strip.
- Add refresh debounce inspired by VS Code's `ExplorerService` delayed
  file-change reaction.

### Phase 3: editing and agent handoff

Goal: make source viewing useful inside the coding workflow.

Work:

- Add write RPC with expected metadata conflict checks.
- Add dirty markers, save/revert, Cmd+S, and before-close protection.
- Connect selected file/range to the composer as a bounded context attachment or
  prompt insertion, without silently sending file contents.
- Connect source-control action prompts and diff-review annotations to opened
  files.

### Phase 4: VS Code parity candidates

Only after measurement or user demand:

- virtualized tree;
- compact folders;
- file nesting;
- context menu actions;
- DnD;
- inline rename/create/delete;
- watcher-backed live tree updates;
- multi-root workspace switching;
- diff editor.

## Technical Quality Audit

| Dimension | Current Readiness | Finding |
| --- | ---: | --- |
| Accessibility | 3/4 | Hotbar already uses labels, titles, `aria-pressed`, and keyboard shortcuts. The editor tree still needs explicit tree semantics and keyboard navigation. |
| Performance | 2/4 | No editor dependency is loaded today, which keeps Chat light. Monaco and tree hydration must be lazy to avoid slowing boot. |
| Theming | 3/4 | Khala has strong dark tokens in `styles.css`; Monaco needs a token-mapped theme rather than default `vs-dark`. |
| Responsive layout | 2/4 | Current full-screen panels fit the shell pattern. The editor needs stable split-pane constraints and small-width behavior. |
| Anti-pattern risk | 3/4 | The risk is over-porting VS Code chrome or adding decorative panels. Keep it utilitarian, dense, and task-first. |

Health score: 13/20, acceptable for a first implementation once the typed file
contracts are added. The biggest risk is not UI polish; it is letting unknown
filesystem payloads and unbounded local paths leak into the renderer.

## Recommended Next Step

Implement Phase 1 as a small vertical slice:

1. `editorWorkspaceRead`, `editorDirectoryRead`, and `editorFileRead` typed RPCs.
2. `Editor` hotbar slot and `editor-panel` shell integration.
3. Monaco read-only renderer with a token-mapped Khala dark theme.
4. DOM tests plus one visual smoke proving the file tree and source pane render.

That gives Khala Code the "basic filetree and seeing source code" experience
without committing to VS Code's full workbench architecture.
