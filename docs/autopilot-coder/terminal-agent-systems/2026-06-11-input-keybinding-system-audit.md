# Input And Keybinding System Audit

Date: 2026-06-11

This is system #22 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should process raw terminal input, prompt editing,
keybindings, chords, vim-style editing, paste handling, and shortcut conflicts.

## Target

Build an input system with two clear layers:

- Prompt editing semantics for text, cursor, history, paste, and multiline
  input.
- Action keybinding semantics for commands, modals, navigation, permissions,
  and global shortcuts.

These layers should cooperate without scattered key handlers fighting each
other.

## User-Visible Capability

The user should be able to:

- Type and edit long prompts.
- Insert multiline text intentionally.
- Navigate prompt history.
- Use configurable shortcuts by UI context.
- Use chorded shortcuts.
- Use command palette and modal navigation.
- Paste text, images, and large content safely.
- Use optional vim-style editing.
- See shortcut conflicts and reserved-key warnings.
- Keep terminal-specific shortcut fallbacks.

Input should feel predictable across terminals, SSH, tmux, and local shells.

## Core Design

Define an `InputRouter` that converts raw terminal events into typed actions or
text edits.

Suggested service boundary:

```ts
interface InputRouter {
  parse(event: RawTerminalInput): Effect.Effect<InputEvent, InputError>
  resolve(request: KeybindingResolveRequest): Effect.Effect<KeybindingResolution, InputError>
  edit(request: PromptEditRequest): Effect.Effect<PromptEditResult, InputError>
  loadKeymap(request: KeymapLoadRequest): Effect.Effect<KeymapLoadResult, InputError>
}
```

The terminal shell should send raw input to this service. The service should
return either a text edit, an action, a chord state update, or an ignored event.

## Context Model

Keybindings should resolve against active contexts:

- Global.
- Chat input.
- Autocomplete.
- Confirmation or permission dialog.
- Transcript.
- History search.
- Task view.
- Footer navigation.
- Message selection.
- Diff review.
- Model or settings picker.
- Plugin or extension management.
- Generic select list.

Context order and priority should be explicit. User overrides should be loaded
after defaults, with null unbinding support where allowed.

## Keybinding Shape

Each binding should include:

- Context.
- Chord.
- Action id.
- Display label.
- Source.
- Platform applicability.
- Feature gate or capability requirement.
- Reserved status.
- Conflict warnings.

Use a schema-validated configuration file for user bindings. Invalid bindings
should produce warnings and fall back to defaults, not break input.

## Prompt Editing Semantics

Prompt editing should support:

- Character insertion.
- Cursor left and right.
- Word movement.
- Line start and end.
- Wrapped-line up and down.
- Logical-line up and down.
- Backspace, delete, and word delete.
- Kill ring and yank.
- Undo.
- History navigation.
- Searchable history.
- Multiline insertion.
- External editor handoff.
- Prompt stash and restore.
- Inline ghost text or suggestions.

Editing behavior should be independent of action keybindings where possible.
Some text-editing safety shortcuts may remain hard-coded when they depend on
double-press timing or terminal quirks.

## Paste And Attachment Handling

Paste handling should support:

- Bracketed paste where available.
- Plain text paste.
- ANSI stripping.
- Line-ending normalization.
- Large paste refs instead of huge inline prompt text.
- Image paste refs.
- Clipboard failure notifications.
- Attachment cleanup when refs are removed.
- Preservation of pasted content when editing queued prompts.

Large pasted content should become a typed attachment ref. The visible prompt
should stay small and editable.

## Vim Mode

Optional vim-style editing should be a state machine:

- Insert mode.
- Normal mode.
- Counts.
- Motions.
- Operators.
- Text objects.
- Find and repeat-find.
- Replace.
- Dot repeat.
- Undo integration.

Vim mode should operate on the prompt buffer only. It should not bypass
permission, command, or modal keybinding rules.

## Terminal Compatibility

The input system should handle:

- Terminals that cannot send some modifier keys.
- SSH or tmux coalesced input.
- Backspace variants.
- Meta versus alt ambiguity.
- Super or command keys only when supported.
- Shift-enter and alternate newline paths.
- Mouse wheel events routed to scroll state.
- Option-key text on macOS when meta mode is disabled.

Compatibility warnings should be actionable and tied to the detected terminal.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for input routing and keymap loading.
- `Schema` for raw input, parsed input, keymaps, actions, edit operations, and
  warnings.
- `Ref` for chord state, prompt buffer state, and vim command state.
- `Layer` for terminal input adapters and config stores.
- `Stream` for raw input event flow.
- `Queue` for resolved actions.
- `Schedule` for keymap reload debounce.

Parsing and resolving should be pure wherever possible so tests can cover the
hard terminal cases.

## Safety Rules

- Do not let text input submit while a modal owns focus.
- Do not let untrusted keymap config bind reserved exits or interrupts.
- Do not execute command bindings outside allowed contexts.
- Do not insert raw terminal escape sequences into prompts.
- Do not inline huge pasted content into model context without attachment
  policy.
- Do not let image or attachment refs survive after the visible ref is deleted.
- Do not let vim mode bypass approval or destructive-action policy.
- Do not treat unsupported terminal shortcuts as user intent.

## Tests

Minimum regression coverage:

- Parse common control, alt, shift, arrow, page, home, and end keys.
- Resolve defaults by active context.
- Apply user overrides and null unbindings.
- Detect duplicate, invalid, and reserved bindings.
- Resolve chord start, chord match, unbound chord, and cancellation.
- Preserve text editing when no action binding matches.
- Navigate history only when cursor movement cannot proceed.
- Insert newline through configured newline paths.
- Normalize text paste and collapse large paste into refs.
- Add and remove image refs safely.
- Run vim counts, motions, operators, and undo from fixtures.
- Handle SSH coalesced enter and backspace variants.

## OpenAgents Translation Notes

When promoted, map input actions to OpenAgents terminal adapter events,
operator UX commands, approval refs, policy refs, and private/local-only
settings. Verify live issue state before claiming keybinding behavior is
implemented.

## Decision

Input should be a typed router with distinct text-editing and action-resolution
layers. Configurable keymaps, terminal compatibility, paste refs, and optional
vim mode should all feed one event shape before the terminal shell or agent
runtime acts.
