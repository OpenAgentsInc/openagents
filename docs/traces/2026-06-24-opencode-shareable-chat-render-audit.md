# opencode shareable-chat web render — audit for `/trace/{uuid}` (#6209)

> **Purpose.** opencode renders shared coding sessions at a public web URL whose
> styling we want to repurpose for the OpenAgents agent-trace page
> (`GET /trace/{uuid}`, spec in `docs/traces/README.md`, epic #6209). This audit
> records exactly how opencode does it, with file references, and maps its
> session shape onto our **ATIF** trace so we know what to adopt and what to skip.
>
> Source studied: `projects/repos/opencode` (read-only reference clone).
> Our side: `apps/qa-runner/src/atif.ts` (ATIF-v1.7 types) and
> `apps/qa-runner/src/atif-html.ts` (the standalone trace renderer already in
> repo). Public-safe; no secrets.

---

## 1. The share feature: URL shape, routing, data flow

### Public URL

```
https://opencode.ai/s/{id}
```

- Route file: `packages/web/src/pages/s/[id].astro`. `{id}` is the share id
  minted server-side (`Share.id`, see `packages/opencode/src/share/share-next.ts`
  → `ShareSchema = { id, url, secret }`).
- It is an **Astro page inside the Starlight docs site** (`template: "splash"`,
  `hasSidebar: false`, no table of contents, `pagefind: false`). They did not
  build a separate "share app" — they hung one route off the marketing/docs
  Astro site and mounted a single client component.
- The page is explicitly marked `noindex, nofollow, noarchive, nosnippet`
  (privacy/abuse posture for unlisted-by-id sharing — relevant to our open
  "visibility" question).

### Two-phase render: SSR shell, then live client hydrate

opencode renders the page in two passes, which is the key architectural idea:

1. **SSR (Astro frontmatter, `s/[id].astro`)** fetches a *snapshot* from the
   server: `await fetch(`${apiUrl}/share_data?id=${id}`)` → `{ info, messages }`.
   It uses that only to populate `<head>` for link previews — the title, the
   model list, and a **dynamic OG/Twitter social-card image**:
   ```ts
   const ogImage = `${config.socialCard}/opencode-share/${encodedTitle}.png?model=${modelParam}&version=${version}&id=${id}`
   ```
   (title is base64 + double-URI-encoded; `model` collapses to
   "A & B" or "A & N others"). Returns a real `404` when `info` is missing.

2. **Client hydrate** mounts the Solid component with `client:only="solid"`:
   ```astro
   <Share id={id} api={apiUrl} info={data.info} messages={messages} client:only="solid" />
   ```
   `Share.tsx` then opens a **WebSocket** to
   `wss://…/share_poll?id={id}` and streams the session in live, with
   auto-reconnect (2s backoff) and a connection-status pill at the bottom. So a
   shared session that is still running updates in real time, and a finished one
   simply replays from the stream.

> Implication for us: ATIF traces are *finished, immutable* records (D1/R2 per
> `docs/traces/README.md`), so we do **not** need the WebSocket. We want phase 1
> (SSR shell + OG card + real 404) and a static render of the steps; skip phase 2.

### Streamed data model (NOT ATIF)

The server pushes keyed records over the socket; the client folds them into a
store (`Share.tsx`, `socket.onmessage`):

- `session/info` → the `Session.Info` (title, version, time, directory).
- `session/message/{id}` → a `MessageV2.Info` (role `user`/`assistant`, plus
  assistant `cost`, `tokens{input,output,reasoning,cache}`, `modelID`,
  `providerID`, `time`, `path`).
- `session/part/{messageID}/{id}` → a `MessageV2.Part`, appended/replaced into
  that message's `parts[]`.

`share-next.ts` confirms the wire `Data` union: `session | message | part |
session_diff | model`, keyed by
`message/{id}`, `part/{messageID}/{id}`, etc. There is also a `v1→v2` shim
(`fromV1` in `Share.tsx`) for legacy shares.

So opencode's unit is **message + parts**, where a "part" is one of:
`text` (user or assistant), `reasoning`, `tool` (with
`state.{status,input,output,metadata,time}`), `file` (attachment),
`step-start` (a provider/model boundary), plus filtered-out housekeeping parts
(`snapshot`, `patch`, `step-finish`, synthetic/empty text, pending/running
tools). The filtering happens in `Share.tsx` before render.

---

## 2. How a session is laid out (the timeline)

Two files do all the work:

- `packages/web/src/components/Share.tsx` — page shell, header, the
  `<For>` over messages → `<For>` over filtered parts → `<Part>`, plus a
  trailing **summary row** (connection status + aggregate cost/token stats).
- `packages/web/src/components/share/part.tsx` — renders one part. This is the
  reusable heart of it.

### Layout primitives

- **Header.** Big clamped title (`-webkit-line-clamp: 3`), then a wrap-flex
  stats row of `[icon] value` items: opencode version, one chip per distinct
  model (provider icon + model id), and a relative timestamp (Luxon
  `DATETIME_MED`, with full timestamp in `title`).
- **Vertical timeline.** Every part is a two-column grid:
  `[decoration] [content]`. The decoration column is a fixed-width gutter
  containing (a) a clickable **anchor icon** that becomes a `#`/✓ on
  hover/copy and copies a deep link to that exact part, and (b) a thin vertical
  **rail bar** connecting parts. Content is `flex 1 1 auto; min-width: 0` so it
  truncates instead of overflowing.
- **Per-part-type icon.** A `Switch` in `part.tsx` picks a glyph by role +
  part type + tool name (user circle, paperclip for file, brain for reasoning,
  command-line for bash, pencil for edit, doc-plus for write, magnifier for
  grep/glob, globe for webfetch, robot for task, sparkles fallback). All icons
  are inline SVG sized 18px.
- **Deep links per part.** Each part has `id = ${messageID}-${index}`; the page
  reads `window.location.hash` on mount and smooth-scrolls to it. Clicking the
  anchor copies `origin+pathname+search+#id` to the clipboard and shows a
  "copied" tooltip. (Great primitive: **every step is individually linkable**.)

### How each content type renders

- **User text** → `ContentText`: a `<pre>` in a surface-colored rounded box,
  3-line clamp with a "show more" button when it overflows
  (`createOverflow()` measures `scrollHeight > clientHeight`), plus a copy
  button.
- **Assistant text** → `ContentMarkdown`: full markdown via `marked` +
  `marked-shiki`, rendered to HTML and injected with `innerHTML`. Subtle
  blue-bordered box; clamps to 3 lines until expanded. A timestamp **footer**
  is appended on the last assistant part.
- **Reasoning** → a "THINKING" tool-title + collapsible markdown behind a
  show/hide-details button.
- **Tool calls** → `data-component="tool"` with a `data-tool="{name}"` attr and
  a dedicated sub-component per tool (`GrepTool`, `EditTool`, `BashTool`,
  `ReadTool`, `WriteTool`, `TodoWriteTool`, `WebFetchTool`, `TaskTool`,
  `GlobTool`, `ListTool`, `FallbackTool`). Each renders a **TOOL-TITLE row**
  (`[NAME-in-uppercase] [target/arg in bold]`) and an optional collapsible
  result. A `FallbackTool` flattens arbitrary args into a `path → value`
  inline-grid so unknown tools still render cleanly.
- **Tool result + duration footer.** Completed tools get a `ToolFooter` showing
  duration (only if `> MIN_DURATION = 2000ms`), formatted via Luxon diff.
- **bash** → `ContentBash`: a terminal "chrome" — a header bar with three SVG
  dots (drawn via a `--term-icon` data-URI mask) + a "Shell" label, then the
  shiki-highlighted command and output.
- **edit/write/read** → shiki-highlighted code; **edit** uses `ContentDiff`.
- **todowrite** → a bordered checklist; each item draws its own checkbox via
  pure CSS (`clip-path` checkmark for completed, inset box-shadow fill for
  in-progress) colored by status.
- **errors** → `ContentError`: red `Error:` label + dimmed body, 7-line clamp.
- **Trailing summary row** reuses the timeline grid (a status dot in the
  decoration column) and lays the aggregate **cost / input / output / reasoning
  tokens** as a wrap-flex `label value` list, with `&mdash;` placeholders.

---

## 3. The styling (what the user calls "great")

### Approach

- **Plain CSS Modules + data-attributes**, not Tailwind. Components emit
  semantic `data-component` / `data-slot` / `data-status` / `data-tool` attrs;
  the `.module.css` files style by those attrs. This keeps markup readable and
  the styling fully decoupled. (`share.module.css`, `part.module.css`, plus one
  small module per content type: `content-markdown/code/diff/bash/text/error`.)
- **CSS custom properties for the whole palette**, inherited from Starlight and
  redefined for light/dark in `packages/web/src/styles/custom.css`. Everything
  references `--sl-color-*` tokens: `--sl-color-text`,
  `--sl-color-text-secondary`, `--sl-color-text-dimmed`,
  `--sl-color-bg-surface`, `--sl-color-divider`, `--sl-color-blue-high/low`,
  `--sl-color-green/red/orange`. **Light/dark is free** because the page only
  ever names tokens — there is essentially no hard-coded color in the share CSS.
- **Width tiers as variables** drive the whole layout rhythm:
  ```css
  --sm-tool-width: 28rem;  /* bash, todos, tool results        */
  --md-tool-width: 40rem;  /* text/markdown/tool-title          */
  --lg-tool-width: 56rem;  /* diffs / edit (needs the room)     */
  ```
  Different content gets a different comfortable max-width instead of one
  blanket column. This is a big part of why it reads well.

### Typography & spacing

- Mono-ish, tight, dense: base `14px` body; tool/meta text at `0.75rem`–
  `0.875rem`; uppercase tool/section labels with negative letter-spacing
  (`text-transform: uppercase; letter-spacing: -0.5px`); `line-height` 1.5–1.6
  for code/pre. Title is `2.75rem`, weight 500, `letter-spacing: -0.05em`.
- Consistent `gap`-based vertical rhythm (`gap: 2.5rem` between parts container,
  `0.625rem` inside the timeline, `1rem` inside a part's content).
- Numeric stats: opencode uses `Intl.NumberFormat`/`currency` (`common.tsx`)
  for cost/tokens; our renderer should use `font-variant-numeric: tabular-nums`
  (it already does).

### Code, markdown, diffs

- **Syntax highlighting via Shiki** (`codeToHtml`) with the dual theme
  `{ light: "github-light", dark: "github-dark" }`, rendered to HTML strings and
  injected with `innerHTML`. Shiki emits CSS-variable-driven dual-theme spans, so
  no JS theme toggle is needed.
- **Markdown via `marked` + `marked-shiki`** (`content-markdown.tsx`), with a
  custom link renderer forcing `target="_blank" rel="noopener noreferrer"`. The
  markdown CSS (`content-markdown.module.css`) is a tidy mini-reset: spacing for
  `p/ul/ol/li`, `code` gets visible backtick pseudo-elements when inline,
  tables get borders, `pre` wraps (`white-space: pre-wrap; word-break`).
- **Diffs (`content-diff.tsx` + `.module.css`)** are the standout. opencode
  parses the unified patch (`diff` lib `parsePatch`), pairs consecutive
  removals/additions into rows, and renders a **side-by-side 2-column grid**
  on desktop and a **stacked single column on mobile** (`@media (max-width:
  40rem)` swaps `[data-component="desktop"]` / `["mobile"]`). Added/removed
  lines get `--sl-color-green-low`/`--sl-color-red-low` backgrounds and a
  `±` gutter marker via `::before` + `content`. Per-line shiki highlight is
  preserved by overriding `--shiki-dark-bg` to the row's tint.

### Progressive disclosure & interaction polish

- **Overflow-aware "show more/less"** everywhere via the shared
  `createOverflow()` hook (measures real overflow, only shows the toggle when
  needed) — text, markdown, bash output, errors all clamp by default
  (`-webkit-line-clamp`) and expand on demand. The *last* part auto-expands
  (`expand={props.last}`).
- **Copy buttons** on text/markdown blocks (`copy-button.tsx`).
- A floating **scroll-to-bottom button** that only appears while scrolling down,
  away from the bottom, and auto-hides after a beat unless hovered
  (`Share.tsx` `checkScrollNeed` + `IntersectionObserver` sentinel).
- **Pure-CSS status affordances**: the todo checkboxes (`clip-path` checkmark),
  the connection-status dot colored by `data-status`, the terminal dots drawn
  from a masked SVG data-URI. No icon font, no images for chrome.
- **Responsive**: a single `@media (max-width: 30rem)` shrinks padding/title;
  the diff has its own `40rem` breakpoint. Everything else flows from
  flex/grid + `min-width: 0`.

---

## 4. opencode's session shape vs our ATIF trace

opencode serializes a session as **`Session.Info` + `MessageV2[]` where each
message has `parts[]`** (`message/part` records). We use **ATIF-v1.7**
(`apps/qa-runner/src/atif.ts`): a `Trajectory { agent, steps[], final_metrics }`
where each `AtifStep { step_id, source, message, reasoning_content,
tool_calls[{tool_call_id, function_name, arguments}], observation{results[]},
metrics }`. The mapping is clean:

| opencode (MessageV2 + parts)                         | ATIF (our trace)                                  |
| ---------------------------------------------------- | ------------------------------------------------- |
| `message.role: "user"` + a `text` part               | `AtifStep.source: "user"`, `message` = the goal   |
| `message.role: "assistant"` + `text` part            | agent `AtifStep.message` (narration)              |
| `reasoning` part                                      | `AtifStep.reasoning_content`                       |
| `tool` part (`tool`, `state.input`)                  | `AtifStep.tool_calls[].{function_name, arguments}`|
| `tool` part `state.output` / `state.metadata`        | `AtifStep.observation.results[].content`          |
| `tool` part `state.time.{start,end}` (duration)      | `AtifStep.metrics` / `final_metrics.extra`        |
| assistant `cost` / `tokens{input,output,reasoning}`  | `metrics.{prompt_tokens,completion_tokens,cost_usd}` + `final_metrics` |
| `modelID` / `providerID`                             | `agent.model_name` (public `openagents/khala` id) |
| `file` part (attachment)                             | multimodal `ContentPart` / artifacts (screenshots/video) |
| `step-start` provider/model boundary                 | (no equivalent — ATIF is already step-keyed)      |

**Key structural differences (and why ours is simpler to render):**

1. **One step vs message+parts.** opencode splits a turn across many `parts` and
   has to *filter* housekeeping parts (`snapshot`, `patch`, `step-finish`,
   synthetic text, pending/running tools) at render time. ATIF already collapses
   a turn into one `AtifStep` whose tool call + observation are first-class
   fields. We do **not** need opencode's part-filtering layer.
2. **tool_call ↔ observation correlation is explicit in ATIF**
   (`observation.results[].source_call_id` → `tool_call.tool_call_id`).
   opencode embeds output inside the tool part's `state`. ATIF's correlation is
   strictly better for rendering "the call → the result that came back."
3. **Public-safety is built into ATIF emission** (`assertAtifPublicSafe`,
   forbidden-key tripwire with a narrow allowlist). opencode's share has no such
   redaction layer (it serves whatever the session contained). We must keep our
   tripwire; do not copy opencode's "stream the raw session" posture.
4. **Live vs immutable.** opencode streams an in-progress session over a socket.
   Our traces are immutable D1/R2 records, so we render statically (SSR or
   pre-rendered) — no WebSocket, reconnect, or `share_poll` equivalent.
5. **opencode keys tools by a fixed allowlist** (bash/edit/read/grep/…) with a
   `FallbackTool`. Our tools are computer-use verbs (navigate/click/type/
   readText/waitFor/assert/screenshot/done — see `atif-html.ts` `toolGlyph`).
   The *pattern* (per-tool component + a generic fallback that flattens args)
   ports directly; the tool list does not.

We already have a renderer (`atif-html.ts`) styled to our own
`apps/openagents.com/DESIGN.md` (pure-black, `#f1efe8`, Commit Mono,
command-surface/timeline aesthetic). The opencode audit should *inform* the
`/trace/{uuid}` page, not replace our visual identity — adopt opencode's
*structural* ideas, keep our palette.

---

## 5. Recommendations for #6209

**Adopt (high value, ports cleanly onto ATIF + our DESIGN.md):**

1. **Two-phase render: SSR shell + live nothing.** Keep our trace immutable, but
   copy opencode's `s/[id].astro` pattern — SSR fetch the trajectory to populate
   `<head>` (title, model, **dynamic OG/Twitter social card**), real `404` when
   the uuid is unknown, and `noindex,nofollow` until we settle visibility. The
   social-card image is what makes a shared trace look good in a PR/DM/Forum
   thread; it is cheap and high-leverage.
2. **Per-step deep links.** Give every `AtifStep` a stable
   `#step-{step_id}` anchor, a hover "copy link to this step" affordance, and
   on-mount hash-scroll. This is opencode's best primitive — "drop a link to the
   exact step where it went wrong." Our `atif-html.ts` timeline already has step
   ids; just add the anchor + clipboard + scroll.
3. **Per-tool render with a generic fallback.** Mirror opencode's
   `Switch`-by-tool + `FallbackTool` (flatten unknown `arguments` into a
   `key → value` grid). Our renderer already has `toolGlyph` + `argsTable`;
   extend it so any future `function_name` renders cleanly without code changes.
4. **Side-by-side diff component for edit/write tools** (`content-diff.tsx`):
   parse unified patch, pair add/remove rows, 2-col desktop / stacked mobile,
   `±` gutter marker, green/red row tints. If/when traces carry code edits this
   is the single most impressive widget to steal; recolor to our tokens.
5. **Overflow-aware progressive disclosure** (`createOverflow()` pattern):
   clamp long observations/reasoning to N lines, show "show more" *only when it
   actually overflows*, auto-expand the last/active step. We already collapse
   reasoning via `<details>`; extend the same to long observation content.
6. **Shiki dual-theme syntax highlighting** for any code/observation content
   (`{ light: github-light, dark: github-dark }` → CSS-variable spans). Lets one
   render serve light/dark with zero JS.
7. **Width tiers + data-attribute styling.** Define `--sm/--md/--lg` content
   widths and style by semantic `data-*` attrs rather than ad-hoc classes. Makes
   the dense timeline readable and keeps markup self-documenting.
8. **Aggregate summary row** (cost / tokens / steps / duration) as a labeled
   wrap-flex strip with `—` placeholders — opencode's footer summary; we already
   have the header strip + final-metrics strip, keep them.

**Skip / do differently:**

- **WebSocket live streaming** (`share_poll`, reconnect, status pill). Our
  traces are immutable; render statically.
- **opencode's raw-session-over-the-wire posture.** Keep our ATIF public-safe
  tripwire (`assertAtifPublicSafe`); never serve a raw session.
- **The opencode tool allowlist + opencode-specific icons.** Use our computer-use
  verbs and our mono-glyph palette (no icon font, no SVG icon set — consistent
  with `atif-html.ts`).
- **Starlight/Astro-docs coupling and `--sl-color-*` tokens.** Our `/trace`
  surface lives in `apps/openagents.com`, not a Starlight docs site; use our
  DESIGN.md tokens (`--oa-*`) we already defined in `atif-html.ts`.
- **part/message split + part-filtering layer.** ATIF is already step-keyed;
  render `steps[]` directly.
- **luxon + the i18n message-bag indirection** (`ShareI18nProvider`, ~50 string
  keys). Overkill for our single-locale trace page; inline labels.

**Net:** the `/trace/{uuid}` page should be our `atif-html.ts` render promoted to
a real `apps/openagents.com` route, plus opencode's **(a) per-step deep links,
(b) SSR head + dynamic OG card + 404, (c) overflow-aware show-more, (d) the
side-by-side diff component, and (e) Shiki dual-theme code** — all recolored to
our `--oa-*` palette, fed from the public-safe ATIF projection in D1/R2.
