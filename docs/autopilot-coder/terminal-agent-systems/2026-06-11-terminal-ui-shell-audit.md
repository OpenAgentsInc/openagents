# Terminal UI Shell Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #21 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should render its interactive shell: messages,
scrollback, prompt input, modals, status, background activity, search, and
resize behavior.

## Target

Build a terminal product surface that is separate from the agent runtime.

The UI shell should render runtime events, collect user input, and display
state without becoming the source of truth for conversation, tools, tasks,
permissions, or repository policy.

## User-Visible Capability

The user should be able to:

- Read streaming assistant output and tool progress.
- Type and edit long prompts.
- Scroll through long transcripts.
- Jump to new messages after scrolling away.
- Search the visible transcript.
- Open modal panels for commands, history, settings, model choice, and tasks.
- See status, cost, mode, diagnostics, and background activity.
- Use a fallback non-fullscreen layout when the terminal cannot support the
  richer shell.
- Resize the terminal without corrupting layout.

The shell should feel responsive even during long runs.

## UI State Boundaries

Keep these state classes separate:

- Durable session state: messages, task records, approvals, tool events.
- Runtime stream state: partial model output, active tool progress, spinners.
- UI-only state: scroll position, hover state, selected row, modal visibility.
- Input state: prompt value, cursor, paste refs, history search.
- Attention state: notifications, waiting-for-user markers, unread count.

Only durable session state should be used for resume. UI-only state can be
discarded or restored opportunistically.

## Core Design

Define a `TerminalShellService` for input/output orchestration and a pure view
model layer for rendering.

Suggested service boundary:

```ts
interface TerminalShellService {
  run(request: TerminalShellRunRequest): Effect.Effect<TerminalShellExit, TerminalShellError>
  events: Stream.Stream<TerminalUiEvent, TerminalShellError>
  dispatch(event: TerminalUserEvent): Effect.Effect<void, TerminalShellError>
  snapshot(): Effect.Effect<TerminalViewModel, TerminalShellError>
}
```

The UI should subscribe to runtime events and emit user events. It should not
invoke model or tool calls directly.

## Screen Regions

Use explicit regions:

- Transcript viewport.
- Bottom input dock.
- Status footer.
- Floating unread or jump controls.
- Modal overlay.
- Background task panel.
- Permission and approval panel.
- Search overlay.
- Notification area.

Each region should declare max height, resize behavior, focus ownership, and
whether it can cover transcript content.

## Transcript View

The transcript view should support:

- Virtualized rendering for long sessions.
- Stable item keys.
- Incremental height measurement.
- Sticky scroll at the bottom.
- Scroll-away detection.
- Unread divider and jump-to-bottom action.
- Search index warmup.
- Search match navigation.
- Per-message expansion when useful.
- Copy/select behavior when supported.
- Plain-text export.

The view should index user-visible text. Hidden control messages should not
show up in transcript search.

## Modal And Overlay Model

Modals should be focus-scoped.

Rules:

- A modal owns its keybindings while active.
- Overlays should not leak navigation keys to the prompt input.
- Search, picker, history, permission, and settings panels should share a
  consistent focus contract.
- Closing a modal should restore the previous focus target.
- Modal content should be scrollable when taller than the terminal.
- Modal rendering should degrade in non-fullscreen mode.

## Resize And Terminal Capability

The shell should detect terminal capabilities:

- Alternate screen support.
- Mouse tracking.
- Bracketed paste.
- Image paste support.
- Modifier-key protocol support.
- Hyperlink support.
- Clipboard integration.
- Color depth.
- Terminal width and height.

Capability gaps should change behavior, not crash the shell.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for shell lifecycle.
- `Schema` for UI events, view models, terminal capabilities, and layout state.
- `Stream` for runtime-to-UI event flow.
- `Queue` for user input events and notifications.
- `Ref` for UI-only mutable state.
- `Layer` for terminal drivers and test renderers.
- `Scope` for alternate-screen, mouse tracking, and terminal cleanup.

The view renderer can be React, Ink, Textual-style, or another retained-mode
terminal UI. The domain boundary should remain renderer-independent.

## Safety Rules

- Do not let UI-only state become the authority for task or approval state.
- Do not render secrets in status bars, notifications, or public exports.
- Do not search hidden or private messages in user-visible transcript search.
- Do not let modal key events submit prompts unexpectedly.
- Do not assume mouse or modifier support exists.
- Do not keep terminal alternate-screen state dirty on crash.
- Do not allow oversized prompt or transcript content to freeze rendering.
- Do not let background task output resize stable controls.

## Tests

Minimum regression coverage:

- Render a long transcript with virtualization.
- Preserve sticky scroll during streaming output.
- Show unread divider after scroll-away and new messages.
- Jump to bottom without losing search state.
- Search only visible transcript text.
- Open and close each modal while preserving focus ownership.
- Resize terminal width and invalidate wrapped-line measurements.
- Render fallback layout without alternate-screen support.
- Handle mouse wheel and keyboard scroll events.
- Keep prompt dock height bounded for large input.
- Clean up terminal mode on interruption.

## OpenAgents Translation Notes

When promoted, map terminal shell state to OpenAgents operator UX surfaces,
session refs, task refs, approval refs, projection visibility, and public-safe
receipts. Verify live issue state before claiming terminal shell behavior is
implemented.

## Decision

The terminal shell should be a renderer over typed runtime events. It should
own layout, focus, search, and input chrome while leaving durable agent state,
permissions, task state, and receipts to the runtime services.
