# Accessibility And Non-Interactive Mode Audit

Date: 2026-06-11

This is system #60 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should support screen readers, keyboard-only use,
reduced motion, color contrast, structured output, CI use, scripting, and
headless operation.

## Target

Build an accessibility and non-interactive mode system that treats automation
and assistive technology as first-class runtime surfaces.

## User-Visible Capability

Users should be able to:

- Run the agent in a fully interactive terminal.
- Run commands in CI or scripts without TUI rendering.
- Receive JSON or structured text output.
- Use keyboard-only navigation.
- Use high-contrast and no-color modes.
- Avoid spinners and motion.
- Get accessible status labels, not color-only state.
- Fail fast when a required prompt cannot be answered non-interactively.

## Mode Model

Modes:

- Interactive TUI.
- Plain terminal.
- Non-interactive command.
- JSON output.
- CI mode.
- Screen-reader-friendly mode.
- Headless service mode.

Each mode should declare whether prompts, approvals, notifications, and remote
bridges are available.

## Bun/Effect Boundary

Use Effect services for:

- `InteractionModeService`: resolves the active interaction mode.
- `AccessibleRendererService`: maps runtime events to accessible text.
- `NonInteractivePolicyService`: decides whether a run can proceed without
  prompts.
- `StructuredOutputService`: emits JSON and machine-readable receipts.
- `TerminalCapabilityService`: detects color, width, TTY, and input features.

Use Schema for structured output envelopes and non-interactive errors.

## Safety Rules

- Non-interactive mode cannot assume approval.
- If approval is required and no resolver exists, stop with a typed blocker.
- JSON output must not include private payloads beyond requested scope.
- Color cannot be the only status indicator.
- Keyboard traps are not allowed.
- Progress spinners have plain-text alternatives.
- CI mode should default to no live spend, no push, no deploy, and no
  provider-account mutation.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has agent API parity goals, terminal-agent system
docs, and Pylon command/TUI surfaces. The terminal-agent README does not yet
include an accessibility/non-interactive mode audit.

Related open issue anchors:

- #4773 API parity contract.
- #4768 overnight unattended proof smoke.
- #4772 MVP exit review.

No terminal automation claim should be green until non-interactive prompts,
approval blockers, structured outputs, and accessible status rendering are
tested.

## Tests

Minimum coverage:

- Run the same command in TUI, plain, JSON, and CI modes.
- Stop with a typed blocker when approval is unavailable.
- Verify no color-only statuses.
- Render screen-reader-friendly progress.
- Reject private payload leakage in JSON output.
- Disable animations in reduced-motion mode.
- Preserve exit codes for scripting.
- Validate structured output against Schema.

## Decision

Accessibility and non-interactive operation are runtime contracts, not UI
afterthoughts. The same typed events should support humans, scripts, CI, and
assistive technology.

