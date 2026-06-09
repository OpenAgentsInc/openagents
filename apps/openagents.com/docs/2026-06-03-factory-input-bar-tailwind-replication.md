# Factory Input Bar Tailwind Replication Report

Date: 2026-06-03

Status: CSS structure report from the supplied Factory session DOM snippet.

Scope: This note documents how to reproduce the submitted Factory input bar in
Tailwind for OpenAgents product surface's Foldkit UI registry. It focuses on DOM structure, layout
contracts, spacing, control grouping, token mapping, and a Tailwind skeleton.
The original markup uses styled-components hash classes, so this report treats
inline styles, semantic attributes, dimensions, and visible control state as the
recoverable source of truth.

## Executive Summary

The Factory composer is a centered, two-row command dock:

1. An upper editable prompt well with a `contenteditable` input and an empty
   right action pocket.
2. A lower toolbar with attachment, model, effort/status, auto mode, normal
   mode, MCP, Skills, and keyboard-shortcut controls.

The outer contract is narrow and centered: `max-width: 768px`, `width: 100%`,
`margin: 0 auto`, and a column flex stack. Inside that, the composer shell is a
full-width column. The visible bar itself uses `background-color:
var(--surface-1)` and relies on compact 26-28px controls, 4px radii, 1px
borders, and small horizontal gaps.

For OpenAgents product surface, the closest local primitive is `Ui.workroomComposer` in
`apps/web/src/ui/index.ts`. It currently uses a taller textarea and a more
panel-like submit row. To match Factory more closely, OpenAgents product surface would need a new
compact variant, likely `workroomCompactComposer`, backed by Tailwind utility
classes and the existing dark tokens rather than a new vanilla CSS block.

## Recovered DOM Anatomy

The submitted structure reduces to this component tree:

```html
<div class="composer-frame">
  <div class="composer-stack">
    <div class="composer-surface">
      <div class="prompt-row">
        <div class="prompt-center">
          <div class="prompt-outer">
            <div
              class="prompt-editor"
              contenteditable="true"
              data-placeholder="Type your message..."
            ></div>
          </div>
        </div>
        <div class="prompt-actions"></div>
      </div>

      <div class="toolbar-row">
        <div class="toolbar-left">
          <button class="pill add">+</button>
          <input type="file" multiple />

          <div class="model-group">
            <button class="model-trigger">
              <span class="provider-icon"></span>
              <span>Gemini 3.5 Flash</span>
            </button>
            <button class="effort-trigger">
              <span class="meter-dot"></span>
              <span class="meter-dot"></span>
              <span class="meter-dot"></span>
              <span class="meter-dot"></span>
            </button>
          </div>

          <div class="mode-group">
            <button class="auto-trigger">Auto Off <span>...</span></button>
            <button class="normal-mode">Normal Mode</button>
          </div>

          <button class="mcp-trigger">MCP</button>
          <button class="skills-trigger">Skills (27)</button>
        </div>

        <div class="toolbar-right">
          <button class="shortcut-trigger">?</button>
        </div>
      </div>
    </div>
  </div>
</div>
```

The original has many neutral flex wrappers with attributes such as
`data-direction="row"` and `data-flex-row="true"`. They are not styling magic;
they map directly to Tailwind `flex`, `flex-row`, `flex-col`, `items-center`,
`justify-center`, and `flex-wrap` utilities.

## Layout Contract

Outer frame:

- `mx-auto flex w-full max-w-[768px] flex-col justify-center`
- No external padding in the submitted snippet.
- The composer should consume the parent width, not impose a fixed pixel width
  below `768px`.

Stack wrapper:

- `flex w-full flex-col`
- Keep this wrapper even if it appears redundant. It is the component boundary
  for future attachments, drag overlays, streaming state, or follow-up chips.

Surface:

- `relative w-full bg-[var(--surface-1)]`
- In OpenAgents product surface token language, this should map to `bg-[#010102]` or
  `bg-surface-base` depending on whether the compact composer should read as a
  hard panel or a softer inset surface.
- The snippet does not expose border or radius on the surface wrapper. If the
  surrounding computed CSS adds them, prefer a minimal OpenAgents product surface adaptation:
  `border border-[#222]` and no radius, or `rounded-[4px]` only if the adjacent
  Factory surface visibly rounds.

Upper prompt row:

- `flex min-h-[43px] flex-row`
- The editor holder gets all available space through `flex-1`.
- The prompt action pocket is bottom-aligned with `self-end m-2 flex gap-0.5`.
  It is empty in the captured state but should remain in the DOM for send/stop,
  microphone, or inline run controls.

Prompt center:

- `flex flex-1 items-center justify-center`
- This centers the editable well vertically within the 43px minimum height.

Prompt editor shell:

- The unknown styled wrapper likely supplies width, padding, and placeholder
  styling. A faithful Tailwind approximation:
  `w-full min-w-0 px-3 py-2`
- The editor itself should be:
  `min-h-[27px] w-full whitespace-pre-wrap break-words bg-transparent text-[14px] leading-[20px] text-[#f1efe8] outline-none empty:before:text-white/35`

`contenteditable` placeholder support cannot be expressed fully with core
Tailwind utilities because it needs the `[contenteditable][data-placeholder]:
empty:before` selector. In OpenAgents product surface, prefer a textarea unless exact Factory parity
is needed. If parity is required, add the selector inside a registry-owned
component class, not in a page-level CSS block.

Lower toolbar row:

- `flex flex-row justify-between p-2`
- The left group is `flex flex-wrap items-center gap-x-1 gap-y-2`
- The right keyboard-shortcut group is a small non-flexing trailing island.
- The original toolbar allows wrapping, which is important below 768px. Do not
  force a single-line toolbar on mobile.

## Control System

All visible controls follow the same compact button shell:

- `type="button"`
- height `26px` for plain pills, `28px` for the model group
- horizontal padding usually `8px`
- right margin `4px` on most pills
- no visible padding on the outer button; padding lives in the inner row
- text appears around 13-14px
- controls are icon+label rows with `items-center`

Base button approximation:

```txt
inline-flex h-[26px] items-center justify-center border border-[#333]
bg-[#080808] px-2 text-[13px] leading-none text-white/60
hover:bg-[#141414] hover:text-[#f1efe8]
focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400]
disabled:cursor-not-allowed disabled:text-white/35
```

Use `rounded-[4px]`, not large pill radii. The captured model group explicitly
uses `border-radius: 4px`.

Model group:

- Outer group:
  `mr-1 flex h-7 items-center rounded-[4px] border border-[#333]`
- Model trigger:
  `ml-0.5 h-7 px-0`
- Trigger inner row:
  `flex h-7 items-center gap-1 pl-2 pr-0.5`
- Model label should truncate:
  `max-w-[180px] truncate text-[13px]`
- Effort trigger sits inside the same bordered group and should not add a
  second outer border unless active/focused.

Meters:

- Four model-effort marks are tiny vertical bars or dots in the original
  styled class.
- Auto mode uses three inactive marks.
- Tailwind approximation:
  `h-[10px] w-[3px] rounded-[1px] bg-white/35`
- Inactive auto marks:
  `bg-white/20`

File input:

- The file input is visually hidden by its styled class.
- Tailwind equivalent:
  `sr-only`
- Preserve the original accept list if implementing this exactly:
  images, PDF, plain text, Markdown, JSON, YAML, XML, CSV, DOCX, and XLSX.

Toolbar wrap behavior:

- Left toolbar:
  `flex min-w-0 flex-wrap items-center gap-x-1 gap-y-2`
- Nested groups:
  `flex flex-wrap items-center gap-x-1 gap-y-2`
- Mode group:
  `flex items-center gap-1`
- Right toolbar:
  `flex shrink-0 items-center`

## Tailwind Skeleton

This is the direct Tailwind component shape for a visual match:

```html
<form class="mx-auto flex w-full max-w-[768px] flex-col justify-center">
  <div class="flex w-full flex-col">
    <div class="relative w-full border border-[#222] bg-[#010102]">
      <div class="flex min-h-[43px] flex-row">
        <div class="flex flex-1 items-center justify-center">
          <div class="w-full min-w-0 px-3 py-2">
            <div
              contenteditable="true"
              data-placeholder="Type your message..."
              class="min-h-[27px] w-full whitespace-pre-wrap break-words bg-transparent text-[14px] leading-5 text-[#f1efe8] outline-none"
            ></div>
          </div>
        </div>
        <div class="m-2 flex self-end gap-0.5"></div>
      </div>

      <div class="flex flex-row justify-between p-2">
        <div class="flex min-w-0 flex-wrap items-center gap-x-1 gap-y-2">
          <button
            type="button"
            class="mr-1 inline-flex h-[26px] items-center px-2 text-[13px]"
          >
            +
          </button>
          <input class="sr-only" type="file" multiple />

          <div
            class="mr-1 flex h-7 items-center rounded-[4px] border border-[#333]"
          >
            <button
              type="button"
              class="ml-0.5 inline-flex h-7 items-center pl-2 pr-0.5 text-[13px]"
            >
              <span class="mr-1 size-3.5 shrink-0"></span>
              <span class="max-w-[180px] truncate">Gemini 3.5 Flash</span>
            </button>
            <button
              type="button"
              class="inline-flex h-[26px] items-center px-2"
            >
              <span class="flex items-center gap-0.5">
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/35"></span>
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/35"></span>
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/35"></span>
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/35"></span>
              </span>
            </button>
          </div>

          <div class="flex items-center gap-1">
            <button
              type="button"
              class="mr-1 inline-flex h-[26px] items-center gap-2 px-2 text-[13px]"
            >
              <span>Auto Off</span>
              <span class="flex items-center gap-0.5">
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/20"></span>
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/20"></span>
                <span class="h-2.5 w-[3px] rounded-[1px] bg-white/20"></span>
              </span>
            </button>
            <button
              type="button"
              class="mr-1 inline-flex h-[26px] items-center px-2 text-[13px]"
            >
              Normal Mode
            </button>
          </div>

          <button
            type="button"
            class="mr-1 inline-flex h-[26px] items-center gap-1 px-2 text-[13px]"
          >
            <span class="size-2 rounded-full bg-white/35"></span>
            <span>MCP</span>
          </button>
          <button
            type="button"
            class="inline-flex h-[26px] items-center px-2 text-[13px]"
          >
            Skills (27)
          </button>
        </div>

        <div class="flex shrink-0 items-center">
          <button
            type="button"
            aria-label="Show keyboard shortcuts"
            class="inline-flex h-[26px] items-center px-2 text-[13px]"
          >
            ?
          </button>
        </div>
      </div>
    </div>
  </div>
</form>
```

The skeleton intentionally omits the Google SVG path. In implementation, use a
small provider icon component or existing provider logo asset rather than
embedding raw copied SVG path data in the composer primitive.

## OpenAgents product surface Token Mapping

Factory token or behavior to OpenAgents product surface mapping:

- `var(--surface-1)`: `bg-[#010102]` for the panel, or `bg-surface-base` for a
  softer translucent surface.
- `var(--border-1)`: `border-[#333]` on grouped controls and `border-[#222]` on
  the outer composer.
- Primary text: `text-[#f1efe8]`.
- Secondary toolbar labels: `text-white/60`.
- Muted or inactive meters: `bg-white/20` or `text-white/35`.
- Hover state: `hover:bg-[#141414]`.
- Active/inset state: `bg-[#080808]`.
- Focus ring: `focus-visible:outline-[#ffb400]` because OpenAgents product surface uses yellow as a
  small functional highlight.

## Implementation Notes For Foldkit

Recommended registry shape:

```ts
export const workroomCompactComposer = <Message>(input: {
  editor: Html
  modelControl: Html
  effortControl: Html
  autoControl: Html
  modeControl: Html
  mcpControl: Html
  skillsControl: Html
  shortcutControl: Html
  attachmentInput: Html
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => ...
```

Keep the primitive focused on layout. Let page code provide typed controls and
messages for model selection, auto mode, mode selection, MCP, skills, and
attachments.

For an exact Factory editor, the hard part is not the flex layout; it is the
`contenteditable` placeholder and input synchronization. In Foldkit, a textarea
is simpler and already covered by `forms/textareas`. Use `contenteditable` only
if OpenAgents product surface needs rich text chips, slash commands, inline mentions, or attachment
tokens inside the prompt well.

## Raw CSS Limits

The supplied HTML includes class names such as `sc-dYwGCk ktlXaG` and
`sc-bMTdWJ kIznWb`. Those are generated styled-components classes, not stable
source identifiers. Without authenticated browser access to computed styles or
the app's JavaScript/CSS bundles, the exact declarations behind those hashes
cannot be recovered from the snippet alone.

What is recoverable with high confidence:

- hierarchy and row grouping;
- widths, min heights, margin, padding, gaps, and flex behavior from inline
  styles;
- control labels, ARIA labels, titles, and file accept list;
- button dimensions and group borders where inline styles expose them;
- the intended centered `768px` composer width.

What remains inferred:

- exact font family, font weight, and text color;
- exact button hover/focus/active states;
- exact prompt placeholder selector;
- whether the outer surface has computed border radius or shadow;
- the visual form of the styled meter marks and MCP icon.

For production OpenAgents product surface work, those inferred values should be resolved against the
local design contract in `DESIGN.md`, not copied from Factory wholesale.
