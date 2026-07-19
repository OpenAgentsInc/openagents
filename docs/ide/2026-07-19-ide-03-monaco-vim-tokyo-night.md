# IDE-03 delivery: Monaco, built-in Vim, and Tokyo Night

Date: 2026-07-19  
Issue: [#9018](https://github.com/OpenAgentsInc/openagents/issues/9018)  
Implementation receipt commit: `53ca9a3413f11f5517ec6c134a84c637556f2c83`  
Status: delivered implementation evidence; no IDE-07, Zed-quality, or Cursor-parity claim

## Result

Every production Files and Finder document route now mounts a real, editable
Monaco model in the primary editor region. The workbench and editor use one
owned Tokyo Night semantic projection from native-window creation through
first HTML paint, React/Effect Native chrome, Pierre states, and Monaco. Vim is
a built-in first-party controller, is off by default, persists as a Desktop
preference, and can be toggled without replacing the document or losing its
draft or selection.

The old controlled textarea is absent from the production React document
route. `makeStubCodeEditorDriver()` remains only in the explicitly selected
Effect Native compatibility renderer; it is not a supported production open
route and owns no document state.

## Authority and data flow

```text
Finder / Pierre / Files command
              |
              v
Effect workspace document service
  documentRef + generation + disk revision + draft + recovery
              |
       Schema-decoded projection
              v
app-local Monaco runtime -------- app-local VimModeController
  model/view/edit mechanics         focused key interpretation only
              |
       versioned edit event
              v
Effect reducer: fence generation/sequence, resync gaps, persist recovery
              |
       typed save/close command
              v
workspace service: revision/conflict/grant authority
```

Monaco never receives the workspace root, bridge, grant, filesystem service,
disk revision authority, or last durable bytes. A model is keyed by a branded
opaque `IdeDocumentRef`; the mutable relative path is only a label and language
hint. Finder main-process admission reduces an absolute OS-selected file to a
containing-directory grant plus one validated relative basename before the
renderer receives a typed command.

The document service remains the second unsaved copy and source of truth for:

- draft bytes, encoding, EOL, expected disk revision, dirty and conflict state;
- recovery version 3, including opaque identity, generation, incremental
  sequence, selection, and every open draft;
- save, save all, save as, close, conflict resolution, revocation, and stale
  generation refusal; and
- the distinction between model version, document event sequence, and durable
  disk revision.

Monaco sends a full current value alongside bounded incremental changes. The
Effect reducer accepts the next sequence, rejects stale generations, and uses
the full value as deterministic resynchronization after a sequence gap. A
rename or move can change the path label without changing model authority.

## Runtime and lifecycle

The production editor is one lazy ESM island:

- fixed entry URLs:
  `openagents-app://renderer/ide-editor/editor.js` and `editor.css`;
- content-hashed editor, JSON, CSS, HTML, and TypeScript worker assets beneath
  the same packaged private scheme;
- no Monaco implementation graph, model, view, listener, or worker on an
  ordinary chat-only launch;
- one model per opaque document ref and multiple view instances per model;
- selection state scoped by `IdeEditorViewRef`, while registers, local marks,
  and bounded dot-repeat state are shared by the document model; and
- idempotent view, model, listener, Vim-handler, worker, page-hide, and runtime
  finalization.

The runtime exposes only `attach`, `resources`, and `dispose`. It is a mechanic,
not an application service. Its inputs, events, Vim status, resource snapshot,
benchmark receipt, and packaged-journey receipt are all Effect Schema values;
TypeScript types are derived from those schemas.

The build keeps the editor out of `boot.js` and emits a fixed CSS entry so the
loader never has to discover a Vite manifest or loosen CSP at runtime. Main's
private protocol admits only the bounded `ide-editor/**` asset subtree. The
packaged proof observes the loaded private-scheme stylesheet and a live editor
worker with no network dependency.

## Editing and Vim behavior

Monaco owns public editing mechanics: undo/redo, find/replace and go-to-line
controllers, multi-cursor, selection, clipboard/IME input, folding, matching,
indent guides, line numbers, minimap, word wrap, and accessibility mode.
Desktop commands remain authoritative for save, save all, split, close, and
the persistent Vim toggle.

The first-party `VimModeController` is deliberately app-local instead of a VS
Code extension-host dependency. It projects Normal, Insert, Visual, Visual
Line, Visual Block, Replace, and operator-pending states into an `aria-live`
status. Its initial admitted surface includes core character/word/line/document
motions, counts, delete/change/yank, line operations, local marks, named and
unnamed registers, paste, dot repeat, forward character find/till, join,
indent/outdent, case toggle, undo/redo, `/`, and bounded `:write`, `:quit`,
`:q!`, `:wq`, and `:x` routing. Save and close variants emit typed document
events and therefore cannot bypass revision/conflict/dirty guards.

Normal mappings suspend during IME composition. Escape returns to Normal and
collapses selection; blur clears pending modal state so editor mappings cannot
capture global keys. `Ctrl-V` is resolved before plain `v`. Disabling or
disposing Vim removes both key and blur subscriptions, and packaged teardown
requires the handler count to return to zero. Volatile modal state does not
survive restart; only the user's on/off preference does.

IDE-04 still owns the complete keybinding/conflict inspector and broader
settings surface. Unsupported Vim/global mapping expansion must remain an
explicit bounded controller decision rather than acquiring filesystem,
clipboard, shell, or extension-host authority.

## Tokyo Night

`src/ide/tokyo-night-theme.ts` is the sole semantic color authority for this
rung. The native BrowserWindow background and first HTML paint use `#1a1b26`,
then the same projection supplies Effect Native/React tokens, Pierre adapter
variables, and the Monaco theme before model creation. Changing a UI option or
Vim state never recreates the model. Focus, warning, error, selected, dirty,
and unavailable states also retain shape, text, border, or status cues instead
of depending on color alone.

Tokyo Night remains the only selectable theme through IDE-07. This delivery
does not claim light, high-contrast light/dark, system-following, or arbitrary
theme import support; those remain IDE-18 work.

## Verification receipts

Environment: Apple Silicon macOS, Node 24.13.1, packaged Electron/Forge
`darwin-arm64`, commit `53ca9a3413f11f5517ec6c134a84c637556f2c83`.

| Observation | p50 | p95 | p99 | Written p95 gate |
| --- | ---: | ---: | ---: | ---: |
| open one 1 MB document | 0.128 ms | 0.243 ms | 0.319 ms | 10 ms |
| incremental edit in 1 MB document | 0.049 ms | 0.059 ms | 0.063 ms | 10 ms |
| sequence-gap resync in 1 MB document | 0.001 ms | 0.003 ms | 0.020 ms | 15 ms |
| decode/recover 12 open tabs | 13.539 ms | 14.691 ms | 14.862 ms | 40 ms |

The controller benchmark used twelve isolated one-megabyte documents and 101
edit/recovery samples. It measured a 105,415,352-byte heap delta because the
bounded benchmark deliberately retains one hundred full 1 MB undo/recovery
snapshots (105,414,504 measured bytes); this is retained-copy cost, not a worker/listener leak. Active
resource delta was zero. The build measured 1,964,887 bytes of ordinary boot
JavaScript with no Monaco graph, 4,689,027 bytes of editor JavaScript, 487,927
bytes of editor CSS, and 9,359,261 bytes of worker assets.

The schema-decoded benchmark is
`apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-monaco.json`.

The packaged LaunchServices journey opens `ide03.ts` through the actual macOS
`open-file` route, reaches the input-ready editor without a Sessions shell,
types through Monaco's native accessible EditContext, observes canonical
recovery, toggles Vim, mounts two views, and reloads. It proves:

- editable Monaco, recovery, Vim toggle, and exactly two split views;
- no production textarea and no absolute workspace root in rendered output;
- fixed private-scheme CSS plus a live packaged worker; and
- after page-hide/runtime stop: zero models, views, workers, listeners, and Vim
  handlers.

Its receipt and screenshot are:

- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-packaged-journey.json`
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-packaged-editor.png`

Verification also includes Desktop typecheck, the complete 2,664-test Desktop
suite, IDE boundary checks, focused document/gap/recovery/preferences/startup/
accessibility/behavior-contract tests, production build, Forge package staging,
and the ASAR closure gate (66 entries, 59 unpacked entries, two byte-verified
closure components).

## Scope boundary and next packet

This closes IDE-03 only. It does not promote the proposed Desktop or Cursor
AssuranceSpecs and does not establish the daily-use basic-IDE rung. IDE-04 must
now add one navigation history, quick open/search/breadcrumb/outline routes,
durable editor groups, preview/pin/reorder/reopen behavior, complete typed file
operations, and inspectable settings/keybinding precedence without creating a
second document or command authority.
