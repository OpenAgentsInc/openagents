---
status: "accepted"
date: 2026-06-30
decision-makers: OpenAgents maintainers
consulted: AGENTS.md, INVARIANTS.md, docs/design/starcraft.md, projects/prosemirror/README.md, projects/prosemirror/manifest.txt, projects/prosemirror/repos, packages/ui/src/ai-elements, apps/autopilot-desktop/src/shared
informed: OpenAgents contributors, agents, Khala Code desktop operators, OpenAgents web operators, and three-effect maintainers
---

# Adopt a ProseMirror-inspired command composer

## Context and Problem Statement

OpenAgents and Khala need a richer chat input that can handle Markdown, code,
large pasted text, images, files, resizing, streaming turns, and command-like
agent workflows without breaking native text entry, copy/paste, focus, or
accessibility.

The ProseMirror reference lane under
`/Users/christopherdavid/work/projects/prosemirror/repos/` shows a strong
architecture for rich editors: persistent schema-shaped documents, pure
transforms, transactions, selections, plugins, keymaps, input rules, history,
Markdown round-tripping, and optional collaboration/search packages. It also
shows how much complexity lives in a full browser `contentEditable` view.

OpenAgents already has a small `@openagentsinc/ui` AI-elements prompt input and
Markdown renderer. It also has an owner mandate and invariant boundary for
promoting rich visual language through `@openagentsinc/three-effect` instead of
adding app-local visual renderers. The StarCraft design study in
`docs/design/starcraft.md` also points the composer toward a command-console HUD
rather than a neutral SaaS textarea.

The decision needed: should OpenAgents vendor/adopt ProseMirror directly, keep
incrementally patching app-local textareas, or build an OpenAgents-owned
composer that borrows the ProseMirror architecture while staying simpler and
native to Effect, Foldkit, and three-effect?

## Decision Drivers

* Preserve native textarea behavior for text entry, IME, copy, paste, selection,
  OS shortcuts, and accessibility.
* Support Markdown, code blocks, large text, image/file attachments, resizing,
  command keys, and always-available typing during streaming.
* Keep document/draft state typed with Effect Schema and deterministic pure
  transitions.
* Reuse the existing `@openagentsinc/ui` AI-elements direction instead of
  inventing an app-local one-off composer.
* Promote futuristic HUD visuals through `@openagentsinc/three-effect`, not a
  parallel DOM/canvas/WebGL renderer.
* Borrow ProseMirror's proven separation of model, transform, state, view,
  plugins, commands, keymaps, history, and Markdown without inheriting the full
  editor surface.
* Avoid ad hoc keyword routing from composer text into tool/model decisions.
* Avoid writing raw private prompt or file content into public projections,
  docs, traces, fixtures, or receipts.

## Considered Options

* Build an OpenAgents-owned ProseMirror-inspired command composer.
* Vendor or embed ProseMirror directly.
* Keep a plain textarea with app-local feature patches.
* Build a new full `contentEditable` WYSIWYG editor from scratch.

## Decision Outcome

Chosen option: "Build an OpenAgents-owned ProseMirror-inspired command
composer", because it gives OpenAgents the useful editor architecture while
staying aligned with the repository's Effect, Foldkit, and three-effect
boundaries.

The accepted shape is:

* a Markdown-first, textarea-backed editable substrate;
* a typed Effect Schema contract for drafts, selections, attachments, steps, and
  submit payloads;
* a pure state/transform layer with ProseMirror-like steps, transactions,
  input rules, keymaps, commands, and history;
* a shared `@openagentsinc/ui` command-composer element that renders the text
  well, attachment rail, status strip, resize controls, command controls, and
  accessibility mirrors;
* a `three-effect` HUD projection for the futuristic command-console shell,
  focus energy, dropcursor effects, attachment holograms, and scanner motion.

The first production version must not embed the whole ProseMirror browser view
or become a generic WYSIWYG document editor. It should keep source Markdown as
the user's authoritative editable content and use structured state beside it.

### Consequences

* Good, because native text entry remains reliable while the composer gains
  structured model behavior.
* Good, because typed transactions make resize, attachments, input rules, undo,
  and submit behavior testable without a browser.
* Good, because future collaboration or changeset work remains possible through
  stable steps without shipping it in v1.
* Good, because visual ambition can live in `three-effect` while the DOM remains
  responsible for text, labels, buttons, and accessibility.
* Good, because the same composer can serve Khala desktop, OpenAgents web, and
  future shared surfaces.
* Bad, because the team must build a small editor core instead of only wiring a
  dependency.
* Bad, because textarea-first editing will not provide full inline WYSIWYG
  decorations in v1.
* Bad, because three-effect HUD polish requires coordination with the sibling
  `three-effect` repo before app consumption.

### Confirmation

Compliance is confirmed by:

* composer contract/state tests for every step type, inverse where practical,
  selection mapping, history grouping, and attachment operations;
* Markdown parse/serialize tests that preserve unsupported user text rather
  than dropping it;
* keyboard tests proving OS text shortcuts pass through and explicit composer
  keymaps work;
* browser or desktop smoke tests for autofocus, focus-after-submit, streaming
  while typing, resize, drag/drop, paste, large prompt, Markdown render, and
  stop/retry behavior;
* three-effect visual smoke tests proving the HUD is visible, framed, and
  reduced-motion compatible;
* code review rejecting app-local WebGL/canvas HUD renderers, direct
  ProseMirror vendoring, and public projection of raw private prompt or file
  content.

## ProseMirror Audit Summary

The audited ProseMirror lane contains 23 reference repos:

| Repo | What It Owns | OpenAgents Composer Takeaway |
| --- | --- | --- |
| `.profile` | Project overview and package map. | Confirms the separation of core packages from optional editor features. |
| `buildhelper` | Package build/test helpers. | Not runtime-relevant; reinforces package independence. |
| `prosemirror` | Central project and issue tracker package. | Use as an architecture map, not a runtime dependency. |
| `prosemirror-model` | Persistent document tree, schema, nodes, marks, fragments, DOM parse/serialize. | Borrow immutable schema-shaped content; simplify to chat blocks, inline marks, and attachment refs. |
| `prosemirror-transform` | Step, StepMap, Mapping, Transform, replace/wrap/split/join/mark operations. | Borrow "every edit is a typed step" and selection mapping; skip general tree surgery in v1. |
| `prosemirror-state` | EditorState, Transaction, Selection, Plugin, PluginKey. | Borrow state fields, transaction metadata, plugin shape, and serialized selections. |
| `prosemirror-view` | EditorView, DOM input handling, decorations, clipboard, coordinates, observers. | Borrow view-as-adapter and decoration ideas; do not borrow the full contentEditable machinery. |
| `prosemirror-commands` | Command functions over state and dispatch. | Borrow command shape for submit, stop, newline, indent, attach, resize, preview, undo, and redo. |
| `prosemirror-keymap` | Declarative keyboard binding plugin. | Borrow normalized keymap assembly while preserving Mod-C/Mod-V/Mod-A and platform shortcuts. |
| `prosemirror-inputrules` | Text-triggered transforms and undo of the last rule. | Borrow Markdown triggers for lists, blockquotes, code fences, mentions, and tool tokens. |
| `prosemirror-history` | Undo/redo grouping around transactions. | Borrow grouped undo for typing, input rules, resize, and attachments. |
| `prosemirror-markdown` | Schema-bound Markdown parse/serialize. | Borrow round-trip discipline; seed from the existing `packages/ui` Markdown subset. |
| `prosemirror-schema-basic` | Basic paragraphs, headings, code, links, images, and marks. | Use as an upper-bound reference; the composer schema remains narrower. |
| `prosemirror-schema-list` | List schema and commands. | Borrow list input behavior, not arbitrary nested list editing at first. |
| `prosemirror-gapcursor` | Cursor positions around non-text blocks. | Borrow the idea for keyboard focus before/after attachment chips. |
| `prosemirror-dropcursor` | Drop target feedback while dragging. | Borrow the dropcursor concept for file/image drag targets and insertion rails. |
| `prosemirror-collab` | Versioned sendable steps and receive/rebase. | Defer; stable steps keep this possible later. |
| `prosemirror-changeset` | Human-readable changes distilled from step history. | Useful later for draft diffs or "what changed before submit"; not v1. |
| `prosemirror-search` | Query plugin and match decorations. | Defer except for expanded large-text mode. |
| `prosemirror-menu` | Menu primitives over commands. | Avoid v1; build an app-native command card instead. |
| `prosemirror-example-setup` | Example plugin composition. | Use as setup inspiration; do not copy its one-size package. |
| `prosemirror-test-builder` | Schema-aware test document builders. | Borrow test ergonomics for composer state tests. |
| `website` | Documentation and demos. | Behavior reference only. |

The scan showed the `prosemirror-view` package is larger than the model,
transform, and state layers because browser editing is where most complexity
lives. That is the central reason this ADR chooses textarea-native editing plus
a typed model, rather than a ProseMirror view embed.

## Accepted Implementation Scope

The implementation should land as a sequence:

1. Composer contract and state core: Effect Schema draft, attachment, selection,
   step, transaction, reducer, history, input-rule, keymap, and Markdown
   round-trip tests.
2. Shared UI command composer: promote `prompt-input` into a richer
   `command-composer` AI element with textarea editing, resize controls,
   attachment rail, status strip, command controls, accessibility mirrors, and
   keymap wiring.
3. Attachment and large-text behavior: paste/drop staging for images/files,
   text-attachment offers for huge pasted content, thumbnails/chips, retry,
   remove, undo, and non-blocking parse/highlight work.
4. Three-effect HUD primitives: composer frame, existing-border focus energy,
   scanner plane, dropcursor beam, attachment nodes, command-card affordances,
   and reduced-motion behavior in the sibling `three-effect` package.
5. Khala desktop integration: replace the app-local chat box with the shared
   composer, keep typing available during streaming, restore focus after submit,
   and test autofocus/copy/paste/resize/attachments/Markdown/stop-retry.

## Pros and Cons of the Options

### OpenAgents-owned ProseMirror-inspired command composer

* Good, because it keeps the model and transactions typed with Effect Schema.
* Good, because it preserves native textarea reliability.
* Good, because it lets web and desktop share one composer contract and UI.
* Good, because it keeps futuristic visuals inside `three-effect`.
* Bad, because OpenAgents must own a small editor state package.
* Bad, because full WYSIWYG behavior is deferred.

### Vendor or embed ProseMirror directly

* Good, because ProseMirror is mature and solves hard browser editing problems.
* Good, because collaboration, history, Markdown, and selection packages already
  exist.
* Bad, because the full browser view is much larger than the composer problem.
* Bad, because it introduces a generic editor dependency where OpenAgents needs
  a typed command-message draft model.
* Bad, because app-specific attachment/tool/receipt/privacy boundaries would
  still need significant wrapping.

### Plain textarea with app-local feature patches

* Good, because it is fast to iterate in one surface.
* Good, because native entry remains reliable.
* Bad, because resize, attachments, history, Markdown, keymaps, and streaming
  behavior would drift between apps.
* Bad, because features would remain hard to test as pure transitions.

### New full contentEditable WYSIWYG editor from scratch

* Good, because it could eventually offer rich inline editing.
* Bad, because it repeats ProseMirror's hardest browser work without its years
  of edge-case coverage.
* Bad, because it risks breaking the basic chat promise: type, paste, submit,
  and keep typing.

## More Information

* `docs/design/starcraft.md`
* `projects/prosemirror/README.md`
* `projects/prosemirror/manifest.txt`
* `projects/prosemirror/repos/prosemirror-model/src`
* `projects/prosemirror/repos/prosemirror-transform/src`
* `projects/prosemirror/repos/prosemirror-state/src`
* `projects/prosemirror/repos/prosemirror-view/src`
* `projects/prosemirror/repos/prosemirror-inputrules/src`
* `projects/prosemirror/repos/prosemirror-history/src`
* `projects/prosemirror/repos/prosemirror-markdown/src`
* `packages/ui/src/ai-elements/prompt-input.ts`
* `packages/ui/src/ai-elements/markdown.ts`
* `packages/ui/src/ai-elements/message.ts`
* `packages/ui/src/ai-elements/code-block.ts`
* `packages/ui/src/ai-elements/diff.ts`
* `apps/autopilot-desktop/src/shared/chat-world-scene.ts`
* `apps/autopilot-desktop/src/shared/verse-khala-effect.ts`
* `apps/autopilot-desktop/src/shared/pylon-base-scene.ts`
* `INVARIANTS.md`
* `AGENTS.md`
