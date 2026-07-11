# Design ports

In-repo provenance for presentation-language ports into the OpenAgents
Desktop Effect Native views. Ports translate a reference app's visual
anatomy into typed style objects on the shared `@effect-native/tokens`
vocabulary — never Tailwind/CSS class strings (owner decision 2026-07-08),
never vendored reference code, always our catalog icon set and the uniform
Protoss-blue theme.

## 2026-07-11 — opencode tool/message card language → chat transcript cards

Owner directive (verbatim): "Make a design pass through the
projects/repos/opencode desktop app. any of its tool/message card
formatting, we should port its tailwind stuff to our Effect Native, i want
our component slooking just like theirs but adapted to our starcraft blue
etc, and using the openai apps sdk icons we are."

Behavior contract: `openagents_desktop.chat.opencode_card_design_language.v1`
(`src/contracts/ux-contracts.ts`).

Source receipts (read directly from the local reference clone at
`~/work/projects/repos/opencode`; the coordinated extraction spec
`opencode-card-design-spec.md` had not landed when this pass shipped, so the
receipts below are first-hand):

- `packages/session-ui/src/components/basic-tool.css`
  - `[data-component="tool-trigger"]`: dense single-line tool row — 16px
    icon/indicator slot, 8px gaps, title `14px` medium (`--text-strong`),
    inline single-line ellipsized subtitle `14px` regular muted
    (`basic-tool-tool-subtitle`), optional args muted, trailing action slot.
  - `[data-component="task-tool-card"]` (subagent/task tools only): boxed
    row — `padding: 8px 12px`, `border-radius: 6px` (8px in the new layout),
    thin (0.5–1px) weak border, subtle translucent raised background.
  - `BasicTool` (`basic-tool.tsx`): details are a collapsible body,
    **closed by default**; pending status shows an inline indicator in the
    16px slot.
- `packages/session-ui/src/components/message-part.css`
  - assistant column gap 12px; user rows end-aligned `max-width
    min(82%, 64ch)`; `[data-component="diagnostics"]` failure text uses the
    danger foreground as content (not raw payloads); tool output sections
    are separate, selectable, below the trigger.

Translation to Effect Native (this repo, `src/renderer/shell.ts`
`toolCardMessage` + `questionCardMessage`, `src/renderer/tool-cards.ts`):

| opencode | Effect Native (typed tokens) |
| --- | --- |
| tool-trigger row, 8px gap | `Stack` row, `gap: "2"` |
| 16px icon slot | catalog `Icon` `size: "sm"` (16px), our icon set |
| title 14px medium `--text-strong` | `Text` `variant: "label"` (14/500) `weight: "medium"` `color: "textPrimary"` |
| inline muted ellipsized subtitle | `Text` `variant: "body"` `color: "textMuted"` on the same row, pre-bounded by the humanizer (160 chars) |
| pending indicator / completion state | toned catalog `Badge` chip (Running/OK/Failed — neutral/success/danger) |
| collapsible details, closed by default | bounded raw args/result behind the compact `details` toggle (`DesktopToolCardToggled`) |
| task-tool-card box (8px/12px, 6px radius, thin border, translucent bg) | catalog `Card` `padding: "2"` `radius: "md"` `borderWidth: 1` `borderColor: "border"` `surface: "glass"` — applied to agent-class tools only (`Agent`, `mcp__codex__*`) |
| diagnostics danger text | failure text as `Text` `color: "danger"` content line |
| light/dark theming | NOT ported — uniform Protoss-blue dark theme only |

Deliberate deviations:

- opencode shows no explicit "OK" chip on completed tools; our cards keep
  the toned status chip because the EP250 tool-card behavior contract
  requires the started → ok/failed chip on the same updating card.
- opencode's whole trigger row toggles its collapsible; our expand
  affordance is a dedicated compact keyboard-focusable button (the same
  compact details pattern the message rows use).
- The question card (no opencode equivalent) reuses the same family:
  header chip, dense option Buttons with dim caption descriptions.
