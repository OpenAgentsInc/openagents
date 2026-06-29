# Theme And Visual Design System Audit

Date: 2026-06-11

This is system #59 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should manage colors, spacing, icons, density, status
badges, accessibility contrast, and cross-surface visual consistency.

## Target

Build a visual design system for terminal, web companion, mobile companion,
and operator views that keeps status legible, dense, and consistent without
mixing visual style with runtime policy.

## User-Visible Capability

Users should be able to:

- Choose a theme or follow system theme.
- Use high-contrast and reduced-motion modes.
- Distinguish status, warning, failure, approval, and success states.
- Read long diffs, logs, and tables comfortably.
- Keep visual state consistent across terminal and companion surfaces.
- Export or reset visual preferences.

## Design Tokens

Core tokens:

- Color roles for text, subtle text, background, border, accent, warning,
  danger, success, info, and disabled states.
- Spacing scale.
- Density modes.
- Typography roles.
- Status icon roles.
- Diff colors.
- Progress and attention colors.
- Focus rings.
- Motion preferences.

Status colors should be redundant with labels and icons for accessibility.

## Bun/Effect Boundary

Use Effect services for:

- `ThemePreferenceService`: stores user and managed theme choices.
- `ThemeTokenService`: resolves tokens for terminal and web renderers.
- `StatusVisualService`: maps typed runtime states to visual roles.
- `VisualAccessibilityService`: checks contrast and motion settings.

Use Schema for tokens, themes, density, and accessibility modes.

## Safety Rules

- Visual status cannot imply success without runtime receipt state.
- Failure, warning, and waiting states must remain distinguishable in
  monochrome and high contrast.
- Managed theme policy may require high contrast or branding constraints.
- Theme files cannot execute code.
- Remote or plugin-provided themes are data, not trusted instructions.
- Visual truncation must not hide critical warnings.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has Pylon TUI surface and theme tests, web product
surfaces, and public-status projection requirements. The terminal-agent README
does not yet include a theme/visual design audit.

Related anchors:

- #4765 decision queue and notifications for status clarity.
- #4772 MVP exit review for public surface readiness.
- #4773 API parity contract because status names must match visual states.

No visual surface should show green/success for planned, partial, stale, or
unreceipted behavior.

## Tests

Minimum coverage:

- Resolve built-in, system, high-contrast, and managed themes.
- Check contrast for status colors.
- Render status states with labels and icons.
- Preserve warnings under narrow terminal width.
- Reject executable theme material.
- Map runtime states to visual roles deterministically.
- Snapshot key terminal and companion views.
- Verify reduced-motion mode.

## Decision

The visual system should make runtime truth easier to scan. It must not encode
product claims that the event, artifact, and receipt layers do not support.

