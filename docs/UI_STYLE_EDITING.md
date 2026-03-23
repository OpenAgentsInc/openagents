# UI Style Editing Guide

This file explains where to edit shared UI styles without digging through pane rendering code.

## Main File

Edit this file for shared Autopilot UI styling:

- [ui_style.rs](/Users/kevinfremon/code/OpenAgents/openagents/apps/autopilot-desktop/src/ui_style.rs)

Think of it like a CSS variables file.

## What Each Section Controls

### `text`

Use this section for shared typography roles.

These are the main tokens:

- `HEADER_*`
  - Larger pane/window title text
- `SECTION_HEADING_*`
  - Section titles like `\\ SELL COMPUTE` or `\\ LOG STREAM`
- `FORM_LABEL_*`
  - Left-side labels like `Today`, `Network`, `Status`
- `FORM_VALUE_*`
  - Right-side values like `CONNECTED`, `MAINNET`, `0 BTC`
- `SUPPORTING_*`
  - Smaller helper or supporting copy

### `input`

Use this section for shared input field chrome.

These are the main tokens:

- `BACKGROUND_COLOR`
- `BACKGROUND_ALPHA`
- `BORDER_COLOR`
- `BORDER_WIDTH`
- `CORNER_RADIUS`

### `button`

Use this section for shared button styling.

These are the most important tokens:

- `PRIMARY_*`
  - Main CTA-style shared buttons
- `SECONDARY_CORNER_RADIUS`
- `TERTIARY_CORNER_RADIUS`
- `DISABLED_CORNER_RADIUS`

## Notes

- Some pane headers still come from the shared HUD pane component, not `ui_style.rs`.
- Some inputs still inherit defaults from the shared `TextInput` component.
- `ui_style.rs` is now the main Autopilot-specific style layer on top of the base `wgpui` theme.

## Best Workflow

1. Change a value in `ui_style.rs`
2. Run:

```bash
cargo check -p autopilot-desktop --bin autopilot-desktop
```

3. Launch the app and inspect the change

## If You Want Even More Control

The next layer we can centralize too is:

1. pane header text in the shared HUD pane component
2. shared `TextInput` defaults into an Autopilot-specific preset
