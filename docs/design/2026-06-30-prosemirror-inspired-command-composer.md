# ProseMirror-Inspired Command Composer

Date: 2026-06-30
Status: Design decision and implementation plan
Scope: OpenAgents and Khala composer input surfaces, especially the desktop
chat box and shared `@openagentsinc/ui` AI elements.

## Decision

Build an OpenAgents-owned command composer that borrows ProseMirror's
architecture, not its implementation.

The first production version should stay Markdown-first and textarea-backed for
editing, with a typed Effect model beside it. It should not embed the whole
ProseMirror browser view or become a generic WYSIWYG document editor. The
composer's authority should be a small immutable document/draft model, a typed
transaction pipeline, and explicit attachment records. The DOM textarea owns
native text entry, IME, selection, copy/paste, and accessibility. `three-effect`
owns the futuristic HUD shell, hover energy, attachment holograms, and scanner
motion around that editor.

That gives us the useful ProseMirror ideas:

- schema-shaped content instead of ad hoc strings;
- pure steps and transactions;
- selection mapping through edits;
- plugins for input rules, keymaps, history, attachment handling, and preview;
- a view adapter that renders state but does not own the document.

And it avoids the heavy parts we do not need for a chat composer:

- a full contentEditable browser editor;
- arbitrary deeply nested document schemas;
- menus and toolbar systems;
- collaborative rebase machinery in v1;
- the large cross-browser DOM mutation/input observer layer.

## Material Audited

Audited reference material:

- `/Users/christopherdavid/work/projects/prosemirror/README.md`
- `/Users/christopherdavid/work/projects/prosemirror/manifest.txt`
- every repo currently listed under `/Users/christopherdavid/work/projects/prosemirror/repos/`
- `/Users/christopherdavid/work/openagents/docs/design/starcraft.md`
- current OpenAgents composer/UI material in `packages/ui/src/ai-elements/`
- current OpenAgents three-effect usage patterns in `apps/autopilot-desktop/src/shared/`

Per the workspace contract, the ProseMirror repos remain read-only reference
material. This doc ports concepts, not source code.

## ProseMirror Repo Audit

The lane contains 23 reference repos:

| Repo | What It Owns | Composer Takeaway |
| --- | --- | --- |
| `.profile` | Org-level overview and package map. | Confirms the separation of core packages from optional functionality. |
| `buildhelper` | Shared package build/test helpers. | Not relevant to runtime design. Useful only as evidence that PM treats packages independently. |
| `prosemirror` | Central project and issue tracker package. | Use the project as an architecture map, not a runtime dependency. |
| `prosemirror-model` | Persistent document tree, schema, nodes, marks, fragments, DOM parse/serialize. | Borrow immutable tree plus schema validation. Simplify to chat blocks, inline marks, and attachment refs. |
| `prosemirror-transform` | Step, StepMap, Mapping, Transform, replace/wrap/split/join/mark operations. | Borrow "every edit is a typed step" and selection mapping. Skip general tree surgery in v1. |
| `prosemirror-state` | EditorState, Transaction, Selection, Plugin, PluginKey. | Borrow state fields plus plugin/transaction metadata. Keep state small and serializable. |
| `prosemirror-view` | EditorView, DOM input handling, decorations, clipboard, coordinates, observers. | Borrow view-as-adapter and decorations. Do not borrow the full contentEditable machinery. |
| `prosemirror-commands` | Command functions over state/dispatch. | Borrow command shape for submit, newline, indent, toggle code, attach, resize, preview. |
| `prosemirror-keymap` | Declarative key binding plugin. | Borrow normalized command mapping. Preserve OS shortcuts like Mod-C/Mod-V/Mod-A. |
| `prosemirror-inputrules` | Text-triggered transforms and undo of the last rule. | Borrow for Markdown triggers: lists, blockquote, code fence, mention/tool tokens. |
| `prosemirror-history` | Undo/redo grouping around transactions. | Borrow grouped undo across text, input rules, resize, and attachment operations. |
| `prosemirror-markdown` | Schema-bound Markdown parse/serialize. | Borrow round-trip discipline. Our first path can extend the existing `packages/ui` Markdown subset. |
| `prosemirror-schema-basic` | Basic paragraphs, headings, code, links, images, marks. | Borrow as the upper bound. Our composer schema is narrower. |
| `prosemirror-schema-list` | List node schema and list commands. | Borrow list input behavior, not general nested-list editing at first. |
| `prosemirror-gapcursor` | Cursor positions around non-text blocks. | Borrow the concept for attachment focus and file/image chips. |
| `prosemirror-dropcursor` | Drop target feedback while dragging. | Borrow for file/image drag targets and insertion rails. |
| `prosemirror-collab` | Versioned sendable steps and receive/rebase. | Defer. Our typed steps should leave this possible later. |
| `prosemirror-changeset` | Distills step history into human-readable changes. | Useful for draft diffs and "what changed before submit" later. Not v1. |
| `prosemirror-search` | Query plugin and match decorations. | Defer except in expanded large-text mode. |
| `prosemirror-menu` | Menu primitives over commands. | Avoid v1. Our command card should be app-native, not PM menus. |
| `prosemirror-example-setup` | Composition package for a rough editor. | Good example of plugin assembly. Do not copy its one-size setup. |
| `prosemirror-test-builder` | Schema-aware test document builders. | Borrow test ergonomics for composer state tests. |
| `website` | Documentation and demos. | Useful for behavior examples only. |

Source-size scan, useful for weighting complexity:

| Layer | Reference Scale |
| --- | --- |
| `prosemirror-model` | 13 source files, about 3,589 lines |
| `prosemirror-transform` | 10 source files, about 2,112 lines |
| `prosemirror-state` | 5 source files, about 1,097 lines |
| `prosemirror-view` | 12 source files, about 6,348 lines |
| Editing extras | commands, inputrules, keymap, history, markdown, search, dropcursor, gapcursor |

The `view` package is intentionally the largest because browser editing is the
hard part. For OpenAgents, the best first move is to keep native textarea entry
and make the model smarter, rather than rebuilding ProseMirror's DOM observer
system.

## Current OpenAgents Baseline

`packages/ui/src/ai-elements/prompt-input.ts` already provides a compact
textarea composer surface, with typed props and named class constants.
`packages/ui/src/ai-elements/markdown.ts` already has a streaming-tolerant
Markdown block/inline parser for the assistant message surface. `message.ts`,
`code-block.ts`, and `diff.ts` already render assistant content through the
shared AI-elements vocabulary.

So the next composer should be an evolution:

1. Keep the prompt input as the accessible DOM editor.
2. Add a reusable composer state/model layer under it.
3. Add resize, attachment, command, history, and preview plugins.
4. Add the HUD shell through shared visual primitives, not app-local canvas art.

## Proposed Architecture

Use four layers.

### 1. Contract Layer

Home: `packages/composer-contract` or a submodule inside `packages/ui` until it
is shared by multiple apps.

Purpose: Effect Schema types for drafts, selections, attachments, and wire-safe
message submission.

Sketch:

```ts
type ComposerDoc = {
  readonly version: 1
  readonly blocks: readonly ComposerBlock[]
  readonly attachments: readonly ComposerAttachment[]
}

type ComposerBlock =
  | { readonly kind: "paragraph"; readonly text: string; readonly marks: readonly InlineMark[] }
  | { readonly kind: "code"; readonly language?: string; readonly text: string }
  | { readonly kind: "quote"; readonly text: string }
  | { readonly kind: "list"; readonly ordered: boolean; readonly items: readonly string[] }
  | { readonly kind: "attachmentRef"; readonly attachmentId: string }

type ComposerAttachment = {
  readonly id: string
  readonly kind: "image" | "file" | "text" | "snippet"
  readonly name: string
  readonly mime: string
  readonly sizeBytes: number
  readonly digest?: string
  readonly previewUrl?: string
  readonly dimensions?: { readonly width: number; readonly height: number }
  readonly status: "staged" | "uploading" | "ready" | "error"
}
```

The contract must be explicit that attachment records carry local refs and
metadata, not raw private file contents in public projections.

### 2. State And Transform Layer

Home: `packages/composer-state`.

Purpose: pure data reducers, ProseMirror-style steps, transaction metadata,
history grouping, input rules, keymaps, and selection mapping.

Sketch:

```ts
type ComposerStep =
  | { readonly _tag: "InsertText"; readonly at: TextPosition; readonly text: string }
  | { readonly _tag: "DeleteRange"; readonly range: TextRange }
  | { readonly _tag: "ReplaceRange"; readonly range: TextRange; readonly text: string }
  | { readonly _tag: "SetBlockKind"; readonly blockId: string; readonly kind: ComposerBlock["kind"] }
  | { readonly _tag: "InsertAttachment"; readonly attachment: ComposerAttachment; readonly at?: BlockPosition }
  | { readonly _tag: "RemoveAttachment"; readonly attachmentId: string }
  | { readonly _tag: "ResizeComposer"; readonly heightPx: number }

type ComposerTransaction = {
  readonly steps: readonly ComposerStep[]
  readonly selection?: ComposerSelection
  readonly meta: {
    readonly source: "input" | "paste" | "drop" | "keymap" | "program" | "submit"
    readonly time: number
    readonly addToHistory?: boolean
  }
}
```

Design rules:

- each transaction is deterministic and serializable;
- every step has an inverse where practical;
- selections map through text edits and attachment insertion/removal;
- Markdown parser output never drops user text on parse failure;
- commands are typed values, not keyword-routing strings;
- plugins may observe and append transactions, but may not mutate state in place.

### 3. DOM/Foldkit View Adapter

Home: `packages/ui/src/ai-elements/command-composer.ts` and CSS beside it.

Purpose: render the textarea, attachment rail, footer controls, preview toggle,
command card, resize handles, counters, and a11y mirrors.

The editable substrate remains a native textarea in v1. It should have:

- autofocus on mount where the host app requests it;
- stable focus restore after submit;
- native Mod-C, Mod-V, Mod-X, Mod-A, undo, redo, and text selection;
- Shift-Enter newline, Mod-Enter submit, Escape collapse/cancel;
- Tab indentation only inside fenced code contexts or when explicitly toggled;
- drag/drop and paste handlers that create typed attachment transactions;
- `aria-label`/`aria-describedby` and a visible accessible file list;
- mobile-safe text size, minimum 16px for the actual input on mobile.

The view may render a Markdown preview or syntax overlay, but source text remains
authoritative. That keeps IME and OS behavior boring in the best possible way.

### 4. Three-Effect HUD Layer

Home for visual primitives: `/Users/christopherdavid/work/three-effect` first,
then consume through `@openagentsinc/three-effect` in web and desktop apps.

Purpose: make the composer feel like a command console without making WebGL the
editing authority.

Follow the current OpenAgents pattern from `chat-world-scene.ts`,
`verse-khala-effect.ts`, and `pylon-base-scene.ts`: pure projection functions
build typed visualization descriptors, and renderers consume those descriptors.

Sketch:

```ts
type ComposerHudProjection = {
  readonly focus: "idle" | "focused" | "submitting" | "error"
  readonly heightRatio: number
  readonly attachmentNodes: readonly {
    readonly id: string
    readonly kind: ComposerAttachment["kind"]
    readonly status: ComposerAttachment["status"]
  }[]
  readonly commandSlots: readonly {
    readonly id: string
    readonly active: boolean
    readonly disabled: boolean
  }[]
}
```

Three-effect can render:

- cyan/gold edge energy on the existing border, not an extra inner focus slab;
- a subtle scanning plane behind the textarea;
- attachment chips as small holographic nodes in a rail;
- drag/drop targeting as a dropcursor beam;
- resize state as a mechanical grip and calibrated height marks;
- command slots as a bottom-right command card, inspired by RTS HUDs.

DOM still renders text, labels, buttons, tooltips, and file metadata.

## Feature Decisions

### Resizing

Support three resize modes:

- auto-grow from a compact minimum to a comfortable maximum;
- manual drag handle that stores `heightPx` in composer state;
- expanded command-console mode for large prompts, logs, and file review.

CSS target:

```css
--composer-min-height: 128px;
--composer-max-height: clamp(220px, 32vh, 420px);
--composer-console-height: clamp(180px, 24vh, 260px);
```

Use `ResizeObserver` for layout feedback, but write the canonical size through
a typed `ResizeComposer` transaction. Do not let hover states or counters resize
the control.

### Markdown

The composer should be source-first Markdown, not full WYSIWYG.

Support in v1:

- paragraphs;
- fenced code blocks with language hints;
- inline code, strong, emphasis, safe links;
- quotes;
- ordered and unordered lists;
- horizontal rules;
- image/file attachment refs rendered as chips or previews.

Input rules:

- `- `, `* `, `1. ` create list intent;
- `> ` creates quote intent;
- triple backticks enter code mode;
- pasting a huge block can offer "attach as text file" while preserving the
  user's source text unless they accept conversion.

The existing `packages/ui/src/ai-elements/markdown.ts` can be the first parser
seed, but the composer should eventually use a shared parser/serializer with
round-trip tests.

### Big Text

Large prompts and pasted logs must not freeze the chat box.

Rules:

- editing remains possible at 100k characters;
- parsing/highlighting work should be chunked or worker-backed;
- preview can virtualize blocks and defer syntax highlighting;
- pasted content above a configured threshold becomes a staged `text`
  attachment offer, not an automatic destructive rewrite;
- token/size counters are derived projections, not blockers to typing.

### Attachments

Attachments are first-class typed records, not Markdown strings glued onto the
textarea.

Required states:

- staged;
- uploading;
- ready;
- error;
- removed through undoable transaction.

Required UI:

- drag/drop anywhere on the composer shell;
- paste image/file support;
- image thumbnails with dimensions when known;
- file chips with MIME, size, and status;
- remove, retry, and open/preview controls;
- accessible list and keyboard focus for each attachment;
- gapcursor-like focus before/after attachment chips.

### Commands And Keymaps

Use a ProseMirror-like command signature:

```ts
type ComposerCommand = (
  state: ComposerState,
  dispatch?: (transaction: ComposerTransaction) => void,
) => boolean
```

Baseline commands:

- submit;
- stop current turn without disabling typing;
- insert newline;
- indent/outdent code;
- attach files;
- toggle preview;
- expand/collapse composer;
- clear draft;
- undo/redo;
- open command card.

Keymaps should be platform-aware and should pass through OS text commands unless
the composer has an explicit reason to handle them.

### History

History should group adjacent typing transactions and keep non-text operations
undoable:

- text insert/delete;
- input-rule transforms;
- attachment add/remove;
- manual resize;
- preview/expanded mode if it changes durable composer state.

Submitting should not clear undo history until the app has either accepted the
draft or intentionally archived it.

### Collaboration

Do not build collaborative editing in v1. Do make the step model stable enough
that a future `sendableSteps`/`receiveSteps` design is plausible.

### Search

Do not build normal search in v1. Add search only for expanded large-text mode
or attachment preview if users actually need it.

## StarCraft Style Application

`docs/design/starcraft.md` argues for a command console, not neutral SaaS chrome.
For the composer, that means:

- bottom-anchored, command-console posture;
- stable regions: text well, attachment rail, command card, status strip;
- hard-edged gunmetal/black glass base;
- cyan focus energy and amber command accents;
- green terminal highlights for counters/status when useful;
- no one-note purple/blue haze;
- no copied Blizzard names, assets, icons, or layout one-for-one reproduction.

The best visual language is a Terran/Protoss-informed hybrid:

- Terran for physical paneling, handles, latches, and dense command affordances;
- Protoss for thin luminous borders, crystalline attachment nodes, and psionic
  focus energy;
- StarCraft II for readability, high-DPI restraint, and keyboard-forward speed.

Important constraint: focus should light the existing border. It should not add
a new inner blue rectangle that changes spacing or competes with the text well.

## Implementation Sequence

### Phase 0 - Decision Record

This doc.

### Phase 1 - Composer State Core

- Add Effect Schema draft, attachment, selection, step, transaction types.
- Add pure reducer and tests for insert/delete/replace, selection mapping,
  attachment insertion/removal, and resize.
- Add Markdown parse/serialize round-trip tests for the supported subset.

### Phase 2 - Shared UI Composer

- Promote `prompt-input` into a `command-composer` AI element.
- Keep textarea editing, but wire it to composer state.
- Add attachment rail, drag/drop/paste staging, resize handles, focus restore,
  submit/stop behavior, and keymap handling.
- Keep all controls accessible and keyboard reachable.

### Phase 3 - Khala Desktop Integration

- Replace Khala's app-local chat input with the shared composer.
- Turn all local tools on by default through the existing chat runtime, while
  keeping the composer writable during streaming.
- Test autofocus, submit focus restore, copy/paste, long prompt, image paste,
  file drag, resize, Markdown render, and stop/retry.

### Phase 4 - Three-Effect HUD Polish

- Add missing composer HUD primitives in `/Users/christopherdavid/work/three-effect`.
- Consume the pinned `@openagentsinc/three-effect` package from web/desktop.
- Add canvas pixel smoke tests for visible focus energy, attachment nodes,
  dropcursor beam, and reduced-motion behavior.

### Phase 5 - Advanced Editor Features

- Optional expanded large-text search.
- Optional typed changeset summary before submit.
- Optional collaborative draft state if real multi-user composing appears.

## Tests And Acceptance

Minimum acceptance for v1:

- pure state tests cover every `ComposerStep`;
- Markdown parse/render tests preserve unsupported syntax as text instead of
  deleting it;
- attachment tests cover image, file, large text, retry, removal, and undo;
- keyboard tests prove Mod-C/Mod-V/Mod-A, Shift-Enter, Mod-Enter, Escape, and
  undo/redo behavior;
- Playwright or desktop smoke covers autofocus, focus-after-submit, streaming
  while typing, resizing, file drop, image paste, and mobile-width layout;
- three-effect visual smoke proves the HUD is nonblank, correctly framed, and
  reduced-motion compatible;
- no public projection or docs path records raw private prompt or file content.

## Non-Goals

- Do not vendor ProseMirror.
- Do not ship a full contentEditable WYSIWYG editor for v1.
- Do not add a parallel app-local WebGL/canvas visual system outside
  `three-effect`.
- Do not use ad hoc keyword routing for model/tool/user intent from the
  composer text.
- Do not make Markdown formatting or attachment processing block typing.

## Open Questions

- Should the shared package be `packages/composer-state` immediately, or should
  it start under `packages/ui` until a second app consumes it?
- Should large pasted text default to source text plus an attachment offer, or
  should it automatically stage as a text attachment above a hard threshold?
- Should the first three-effect pass be a subtle composer frame only, or should
  attachment holograms ship in the same release?

My recommendation is to start with state plus DOM composer in one release, then
ship the three-effect HUD pass immediately after the behavior is stable.
