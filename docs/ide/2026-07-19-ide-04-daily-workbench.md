# IDE-04 daily workbench implementation and verification

Date: 2026-07-19  
Roadmap packet: IDE-04  
Issue: [#9019](https://github.com/OpenAgentsInc/openagents/issues/9019)  
Depends on: IDE-02 path index and Pierre Explorer. IDE-03 Monaco document runtime

## Result

Desktop now has one schema-first daily editing workbench over the IDE-02 and
IDE-03 foundations. Explorer, workspace-search, and Quick Open activation reuse
the same opaque canonical document ref and carry the admitted path-index
project/root/worktree identity into one bounded navigation history. Tabs have
preview/pinned state, reorder and bulk-close operations, dirty guards, a bounded
closed stack, and recovery. A split creates a second group over the same
document model, with independent view-state slots. Breadcrumbs and Outline are
explicit projections. Outline truthfully says that its language service is not
admitted until IDE-06.

The implementation also closes the settings/commands seam. The Desktop command
registry is schema-derived and contains the new Quick Open, tab, group,
navigation, split, and Vim product commands. The durable keybinding projection
now identifies source, platform, context, conflict kind, and its relationship to
the separate Vim mapping layer. Editor settings resolve `workspace > user >
default`, reject unknown import values, and project only a fixed allowlist into
Monaco. Tokyo Night remains the only admitted theme.

## Authority graph

```text
path-index identity + bounded projection
  ├─ Pierre tree ── typed IdeExplorerCommand ── workspace operation host
  ├─ workspace search ─┐
  └─ Quick Open ───────┼─ typed open intent ── canonical document reducer
                       │                         ├─ one Monaco model
                       └─ navigation entry       ├─ one/two group views
                                                 └─ recovery v4

settings default/user/workspace ── validated effective options ── Monaco
Desktop command registry ── typed intent ── same reducer/operation authority
```

React, Monaco, and Pierre never receive an absolute root or direct filesystem
function. The path index supplies public-safe relative refs and exact identity.
the workspace service owns grant checks, ignore/secret/symlink policy,
expected-revision comparison, collisions, mutations, and success. Monaco owns
editing mechanics only. Effect-owned reducers and schema-derived state remain
canonical for bytes, dirty state, workbench state, and recovery.

## Navigation

`IdeNavigationEntrySchema` records an entry ref, origin, project, root,
worktree, document, document generation, relative path, selection, availability,
and bounded reason. Equal relative paths in two worktrees therefore cannot
alias. A stale target is retained and marked `unavailable`. Traversal does not
silently retarget it. The history retains the newest 100 entries.

The current origins are wired as follows:

| Origin | Result |
| --- | --- |
| Pierre Explorer | exact index identity + canonical open |
| Quick Open | bounded fuzzy path ranking + preview open + exact identity |
| workspace text/path search | canonical open tagged `workspace_search` |
| breadcrumb | typed path projection. Activation remains a later refinement |
| recent restore | recovery v4 preserves documents/groups/history/settings |
| Outline / quick symbol | explicit `Unavailable(language_service_not_admitted)` until IDE-06 |
| Problems / Git / agent backlinks | vocabulary reserved in the same origin schema. Producers land with their owning packets |

Quick Open is path-only, deterministic, capped at 100 decoded relative refs,
and does not inspect withheld nodes. The UI displays twelve at a time and uses
the complete admitted path-index projection rather than filesystem traversal.

## Tabs, groups, and recovery

- Ordinary Explorer opens are pinned. Quick Open creates a preview unless the
  target is already pinned. Opening another preview replaces the clean preview.
  editing or double-clicking pins it.
- Tabs expose pin/unpin, left/right reorder, close, close others, close right,
  close all, and reopen closed. Dirty targets remain open and surface the same
  explicit discard confirmation used by normal close.
- A split has stable primary/secondary group refs and reuses the exact document
  ref/model. Group state stores document membership, active document,
  selection, scroll slot, and folded-line slots. Selection events stay fenced
  by document generation and view ref.
- Rename/move retargets paths without replacing document identity or dirty
  bytes. External change/conflict and grant-loss behavior continues through the
  IDE-03 document reducer. Delete cannot erase a dirty in-memory draft.
- Recovery moved from v3 to v4. It includes document refs, generations,
  sequences, selection, preview/pinned mode, active path, split/groups,
  navigation, closed refs, and settings. v2 and v3 migrate without manufacturing
  filesystem authority. Recovery is bounded to 12 tabs, 20 closed refs, 100
  history entries, and one megabyte per draft.

## Commands, bindings, and Vim precedence

`DesktopCommandDefinitionSchema` is now the contract for every registry row.
the raw handwritten TypeScript definition type was removed. New stable commands
include Quick Open, editor navigation, pin toggle, active/other/right/all close,
next group, split, Vim, save, and save-all. Pierre file actions already dispatch
the same typed Explorer command union used by the operation handler: open,
reveal, create file/folder, rename, expected-revision move/copy/duplicate/delete,
open in terminal, compare, refresh, retry, and rescan.

The first-party settings UI shows command ID/title, effective chord, default or
user source, platform, scope/context, exact-conflict state, and whether an editor
command is in the separately scoped Vim layer. Exact conflicts remove all
claimants from the effective runtime until the user edits or resets the durable
binding. Ordinary app/menu commands precede editor mechanics. Modal dialogs,
approvals, accessibility/IME handling, and Vim remain explicit scoped layers
rather than being flattened into duplicate Monaco bindings. Toggling Vim updates
the existing mounted views and never remounts or duplicates a document model.

## File operations

The workbench uses the IDE-02 typed operation lane rather than adding React
callbacks. The Explorer toolbar supplies root-level New file/New folder. Pierre
supplies rename, drag/move, duplicate, reveal, compare, open-in-terminal, and
policy-admitted delete. The contract and handler also support copy and
destination-parent operations. Every mutation carries relative refs and, where
applicable, the expected revision. The host classifies collision, stale
revision, permission denial, revoked grant, and unavailability, then the index
reconciles only a confirmed result.

## Settings allowlist

The workbench schema supports these stable settings and rejects every other ID:

| Setting | Default | Projection |
| --- | --- | --- |
| Vim | off | first-party Vim controller |
| minimap / word wrap | off | Monaco boolean options |
| line numbers, bracket matching, indentation guides | on | Monaco allowlist |
| multi-cursor, accessibility support | on | Monaco allowlist |
| tab size / insert spaces | 2 / on | Monaco model options. Tab size 1–16 effective bound |
| font size / line height | 12 / 18 | Monaco options. 8–40 And 12–64 bounds |
| render whitespace | selection | fixed enum |
| rulers | 80, 120 | at most eight columns, each 1–400 |
| sticky scroll | on | Monaco allowlist |
| theme | Tokyo Night | fixed. No contribution or selector |

Autosave and format-on-save remain unadmitted. Import accepts only the versioned
settings export schema. Invalid JSON, unknown IDs, unsafe values, or over-limit
arrays yield a visible error and never reach Monaco.

## Verification

The focused verification command is:

```sh
cd apps/openagents-desktop
pnpm run verify:ide-04
```

It builds the production app, runs the scale benchmark, typechecks, executes
the workbench/Monaco/editor/browser/Pierre/command/binding/DOM/accessibility and
behavior-contract suites, and runs the IDE boundary checker. The focused sweep
covers 149 assertions. The complete Desktop regression sweep also passed: 269
files, 2,631 passed assertions, 39 intentionally skipped, and 2,670 total
assertions.

The public-safe benchmark receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-04-workbench.json`:

| Fixture / observation | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Quick Open, 50,000 indexed paths, 32 queries | 2.951 ms | 3.167 ms | 3.297 ms |
| navigation push, 10,000 pushes, retained 100 | 0.146 ms | 0.173 ms | 0.296 ms |

Admission envelopes are 250 ms Quick Open p95, 2 ms navigation-push p95, and
exactly 100 retained history entries. The benchmark stays intentionally
headless and uses only public-safe synthetic relative paths.

After `package:mac`, `ide:monaco-packaged-journey` now also exercises the IDE-04
workbench through the actual macOS LaunchServices `open-file` route: keyboard
Quick Open, a preview open, double-click pin, mounted-model reuse, Vim, split,
recovery v4 reload, private-scheme/offline assets, absolute-root withholding,
and zero resource teardown. It writes
`2026-07-19-ide-04-packaged-workbench.json` beside the benchmark and updates the
IDE-03 screenshot with the daily-workbench chrome.

The packaged receipt from hardened core commit `78ac9e74fe26505ef94e72ed8d8e892abf7ab996`
records `quickOpenReady`, `previewOpened`, `previewPinned`, recovery version 4,
two split views, and absolute-root withholding as true. The paired Monaco
receipt records edit, Vim, recovery reload, private-scheme/offline assets, no
legacy textarea, and zero models/views/workers/listeners after close.

## Explicit non-goals and next packets

- IDE-05 owns the versioned Pierre review plane across Git, documents,
  conflicts, checkpoints, proposals, and candidate comparisons.
- IDE-06 owns language services, real symbols, diagnostics, formatting,
  code-action authority, and symbol-backed Outline/quick-symbol results.
- IDE-07 owns the packaged daily-editor acceptance matrix. IDE-10 owns
  terminal/PTTY mechanics, and IDE-12 owns safe Git mutation/worktree delivery
  beyond the read-only review surface.
- Arbitrary Monaco extensions/settings/themes, autosave, format-on-save,
  collaboration, debugging, and direct filesystem/process access remain absent.

Those boundaries are visible product states, not simulated successes.
