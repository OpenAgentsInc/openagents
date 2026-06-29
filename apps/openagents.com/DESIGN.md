# OpenAgents Autopilot Design

OpenAgents Autopilot inherits the Vortex design direction: Effect Institute-style
dark surfaces adapted into OpenAgents operational graphics.

## Non-Negotiables

- Dark-only by default. Do not add light variants unless explicitly requested.
- Use pure black foundations, warm off-white text, subtle borders, and compact
  mono typography.
- Prefer command surfaces, tables, registers, strips, panes, event tapes, and
  status rows over cards and marketing sections.
- Keep copy short in product chrome. Put detail in logs, receipts, drill-downs,
  docs, or briefings.
- Do not expose internal agent codenames in product chrome. The Sites
  fulfillment supervisor is `Adjutant` internally, but public, customer,
  operator, and share UI should call that surface `Autopilot`.
- No decorative gradients, bokeh, ornamental SVG backgrounds, or generic
  assistant/chat-first framing.
- Do not vendor Foldkit source or copy Vortex components wholesale. Port the
  design constraints and tokens into local primitives.
- Prefer Tailwind utility classes and Foldkit UI registry components for all
  product UI. Vanilla CSS is a last resort for global base rules, browser
  resets, third-party integration constraints, or selectors that cannot be
  expressed cleanly in Tailwind.
- When adapting OpenCode-style chat/session UI, preserve the chat-centric
  workroom shape with Tailwind-backed rails, timelines, composers, docks, and
  context panes. Do not add new bespoke CSS blocks for those surfaces.

## Foundation Tokens

```css
--oa-bg: #000000;
--oa-panel: #010102;
--oa-panel-active: #141414;
--oa-hover: #080808;
--oa-border-subtle: #222222;
--oa-border-active: #333333;
--oa-text: #f1efe8;
--oa-text-strong: rgba(255, 255, 255, 0.9);
--oa-text-muted: rgba(255, 255, 255, 0.6);
--oa-text-faint: rgba(255, 255, 255, 0.35);
```

## Semantic Accent Tokens

Use these only for small functional state accents:

```css
--oa-highlight: #ffb400;
--oa-positive: #00c853;
--oa-negative: #d32f2f;
--oa-warning: #ff6f00;
--oa-info: #2979ff;
```

## Typography

- Preferred mono: Commit Mono.
- Fallbacks: Berkeley Mono, then `ui-monospace`.
- Operational UI should be mono-first.
- Avoid negative letter spacing. Slight positive mono spacing is acceptable.
- Use larger type only for true landing or maintenance surfaces.

## Current Landing Rule

The temporary public surface is intentionally minimal and dark: a compact
Autopilot beta entry with GitHub login plus a first-class agent path. The
agent path may expose public discovery links and copyable dry-run instructions,
but must not imply prompt files, public docs, or profiles grant authority.
