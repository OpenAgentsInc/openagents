# Decks (`/deck`)

`/deck` is a **local-only** slide deck viewer built with **Effect + Effuse** (no React).

It renders a slide deck described by a **pure JSON DSL** (`dsl: "effuse.slide-deck"`) into Effuse templates, and provides a minimal presenter loop (slides + build steps) with keyboard navigation.

This is intended as a dev tool for local presentations / design reviews. The deck *content* lives in gitignored JSON files so we can ship the engine without committing private slide content.

## Where It Lives

- Route: `apps/web/src/effuse-app/routes.ts` (`/deck`)
- SSR shell: `apps/web/src/effuse-pages/deck.ts`
- Client controller (fetch JSON + keyboard loop): `apps/web/src/effuse-app/controllers/deckController.ts`
- DSL parsing/validation: `apps/web/src/effuse-deck/dsl.ts`
- Rendering (layouts/tokens/builds/nodes): `apps/web/src/effuse-deck/render.ts`
- Local deck content folder: `apps/web/public/decks/`
- Git ignore rule (deck content is not committed): `apps/web/.gitignore`

## Local-Only Behavior

`/deck` is intentionally **blocked outside local dev**:

- The route guard returns `404` unless the hostname is one of:
  - `localhost`
  - `127.0.0.1`
  - `::1`

This prevents accidental deployment and ensures deck content isn't reachable on production hosts.

## Quick Start

1. Create a local deck file (ignored by git):

   Path: `apps/web/public/decks/deck.json`

2. Start the web dev server:

   ```bash
   cd apps/web
   npm run dev
   ```

3. Open:

   - `http://localhost:3000/deck`

If the deck file is missing or invalid, `/deck` will show an error with the expected file path.

**Refresh behavior:** The controller cache-busts the deck URL on each load, so after you edit `deck.json` you can refresh the page (or press **R**) and see changes without restarting the dev server.

### What you can do

- **Layouts:** Use `title` (centered hero + footer), `title-body` (header/body/footer), or `solution` (full-bleed component behind text with a dark overlay).
- **Live components:** Embed a Storybook story or any same-origin URL via `Story` or `Embed` nodes—either inline in the body or full-bleed behind text (see below).
- **Fullscreen:** Press **F** to present. In fullscreen the slide fills edge-to-edge (no border, no on-screen exit button); press **F** again to exit.
- **Dots grid:** A dot-pattern background is rendered behind the slide content so it appears in both windowed and fullscreen views.

## Selecting A Deck File

By default, `/deck` loads:

- `/decks/deck.json` (served from `apps/web/public/decks/deck.json`)

You can select a different deck file via query params:

- `?file=<name>.json` (from `apps/web/public/decks/<name>.json`)
  - Example: `/deck?file=demo.json`
- `?src=/decks/<name>.json` (explicit path under `/decks/`)
  - Example: `/deck?src=/decks/demo.json`

Security constraints (local dev only anyway, but still constrained):

- `src` must start with `/decks/` and end with `.json`
- `file` must match `^[a-zA-Z0-9._-]+\\.json$`

## Keyboard Controls

`/deck` provides a simple presenter loop:

- Next build step (or next slide if at last step): `ArrowRight` or `Space`
- Previous build step (or previous slide if at first step): `ArrowLeft` or `Backspace`
- Next slide: `PageDown`
- Previous slide: `PageUp`
- Reload the deck JSON from disk: `R`
- Toggle fullscreen (edge-to-edge, no on-screen exit): `F`

## Deck Content: Gitignored By Design

Deck JSON files are ignored:

- `apps/web/.gitignore` contains:
  - `/public/decks/*.json`
  - `/public/decks/**/*.json`

The folder exists in git via:

- `apps/web/public/decks/.gitkeep`

## DSL Reference (v0)

This section documents **what the current engine implements** in `apps/web/src/effuse-deck/*`.

The DSL is strict about the top-level contract, slide IDs, and layout references, but intentionally light on deep node validation in v0.

### Top-Level Document

Required fields:

```json
{
  "dsl": "effuse.slide-deck",
  "version": "0.1.0",
  "deck": {
    "slides": []
  }
}
```

Supported optional fields:

- `meta`: freeform metadata (title, author, etc)
- `theme`: token bag used by the renderer
- `assets`: registry (currently only images are used)
- `layouts`: named layout node trees

### `deck` Settings

Supported:

- `deck.size`: `{ width, height }` (preferred)
- `deck.aspectRatio`: `"16:9"` (fallback if `size` not provided)
- `deck.background`: string or `{ "$token": "…" }`
- `deck.defaultSlideLayout`: name from `layouts`
- `deck.slides`: non-empty array

### Slides

Each slide:

```json
{
  "id": "intro",
  "layout": "title-body",
  "background": { "$token": "color.bg" },
  "regions": {
    "header": [],
    "body": [],
    "footer": []
  }
}
```

Rules:

- `id` is required and must be unique across the deck.
- `layout` is optional; if set it must exist in `layouts`.
- If `layout` is omitted, `deck.defaultSlideLayout` is used (if present).
- Slides provide content via either:
  - `regions` (when using a layout with `Slot` nodes), or
  - `content` (freeform node list).
- Layouts can define different slot names. The **solution** layout uses a `background` slot (in addition to `header`, `body`, `footer`) for full-bleed content behind the text overlay.

### Layouts + `Slot`

`layouts` is a map: `{ [name]: DeckNode }`.

`Slot` is a special node type used only inside layout trees:

```json
{ "type": "Slot", "props": { "name": "body" } }
```

During render:

- the layout tree is cloned
- each `Slot(name)` is replaced with a `Fragment` containing `slide.regions[name]` (or empty if absent)

### Built-in layout patterns

Three layout patterns are used in the default deck; you can define them in `layouts` and reference them with `slide.layout`:

| Layout       | Slots                    | Use case |
|-------------|--------------------------|----------|
| **title**   | `header`, `body`, `footer` | Hero slide: centered title/subheadline (put content in `body` with a centered `Column`), byline in `footer`. Root Column uses `fill: true` and `justify: "space-between"` so footer sits at the bottom. |
| **title-body** | `header`, `body`, `footer` | Standard content slide: heading in header, main content in body, slide number or chrome in footer. |
| **solution** | `background`, `header`, `body`, `footer` | Full-bleed component behind text: put a `Story` or `Embed` in `background`; it is rendered in a full-screen layer with a semi-transparent black overlay and your header/body/footer on top. Root Column needs `position: "relative"`. See “Full-bleed component with text overlay” below. |

### Theme Tokens + `$token`

Nodes can reference theme tokens using:

```json
{ "$token": "space.2" }
```

Tokens are resolved from:

- `theme.tokens`

Currently, token resolution is applied to a small set of props:

- layout props: `gap`, `padding`
- sizing props: `width`, `height`
- color props: `background`, `color`

### Nodes

Nodes are component-like objects:

```json
{
  "type": "Text",
  "props": { "style": "h1" },
  "children": ["Hello"]
}
```

Shortcuts:

- string children are treated as text (no need to wrap in `Inline`)

### Builds (Progressive Reveal)

Any node can add build rules:

```json
{
  "type": "ListItem",
  "props": { "build": { "in": 2, "out": null } },
  "children": ["Appears on step 2"]
}
```

Rules:

- Steps are 1-based.
- `in` is the first visible step (defaults to 1 if omitted/invalid).
- `out` is the first step where it is hidden (`null`/missing means "never hides").
- The slide’s total steps is computed as the max `in/out` across all nodes.

### `$ref` (Not Resolved Yet)

The DSL spec mentions `$ref`. The v0 engine currently:

- parses `$ref` nodes, but
- renders them as a placeholder `[ref:…]`

If you need `$ref` resolution, implement it in `apps/web/src/effuse-deck/render.ts` (centralized), and extend validation in `apps/web/src/effuse-deck/dsl.ts`.

## Implemented Node Types (v0)

The current renderer supports the following `type` values:

### Structure

- `Fragment`
  - props: none
  - children: rendered inline (no wrapper)
- `Row`
  - props: `gap`, `align`, `justify`
- `Column`
  - props: `gap`, `align`, `justify`, `fill` (boolean; when true, column gets `flex-1` to fill available height—use for full-height layouts like title slides), `position` (optional; e.g. `"relative"` so absolute children are positioned relative to it)
- `Layer`
  - props: `zIndex` (number; default 0), `background` (optional CSS background, e.g. `rgba(0,0,0,0.75)`), `pointerEvents` (optional; set to `false` so the layer doesn’t capture clicks), `inset` (optional; default true; layer is `absolute inset-0` to fill parent)
  - Use for stacked layouts: put a full-bleed embed (Story/Embed) in a Layer at z-0, a semi-transparent overlay Layer at z-10 with `pointerEvents: false`, and content in a Layer at z-20. Parent Column should have `position: "relative"`.
- `Graph`
  - Purpose: draw an SVG graph of nodes + edges at explicit positions (good for background “systems diagrams” on title/problem slides).
  - props:
    - `width`, `height` (optional; default `deck.size` if present, else `1920x…` derived from aspect ratio)
    - `opacity` (0..1; default `0.65`)
    - `zIndex` (number; default `-1` so it renders behind slide content)
    - `inset` (boolean; default true; renders as `absolute inset-0`)
    - `fit` (`"stretch"|"contain"|"cover"`; default `"stretch"`, maps to SVG `preserveAspectRatio`)
    - `preset` (`"dots"|"dashes"|"dots-slow"|"dashes-fast"|"pulse"`; default `"dots-slow"`)
    - `pointerEvents` (boolean; default false)
    - `className` (string; optional)
  - children: `GraphNode` + `GraphEdge`
- `GraphNode`
  - props:
    - `nodeId` (string; required; identifier used by edges)
    - `nodeType` (`"root"|"leaf"|"skeleton"`; default `"leaf"`) controls sizing + node chrome
    - `x`, `y` (required): position in the graph canvas
      - number = pixels (in the graph canvas coordinate system)
      - string `"15%"` = percent of canvas width/height
      - string `"120px"` = pixels
    - `anchor` (`"top-left"|"top-right"|"bottom-left"|"bottom-right"|"center"`; default `"top-left"`) interprets `x/y`
    - `label` (string; default `nodeId`)
    - `subtitle` (string; optional)
    - `status` (`"ok"|"live"|"running"|"pending"|"error"`; optional)
    - `badge` (`{ "label": "...", "tone": "neutral|info|success|warning|destructive" }`; optional)
- `GraphEdge`
  - props:
    - `from`, `to` (required; node ids)
    - `preset` (optional; same set as `Graph.preset`; overrides per-edge)
    - `color` (optional CSS color; overrides per-edge)
- `Box`
  - props: `padding`, `border` (truthy = show border), `background`, `color`, `width`, `height`
- `Spacer`
  - props: `size`
- `Divider`
  - props: `thickness`, `color`

### Text

- `Text`
  - props: `style` (`h1|h2|h3|body|caption|code`), `align`, `color`
  - note: sizes and fonts are hardcoded per `style` in the renderer (e.g. h1 large sans, h2 Square721, caption for bylines). Tokens are not wired for font sizes yet.
- `Inline`
  - props: `text`
- `List`
  - props: `ordered` (boolean), `gap`
  - children should be `ListItem`
- `ListItem`
  - props: (none)

### Code

- `CodeBlock`
  - props: `language`, `code`

### Media

- `Image`
  - props: `url` OR `assetId`, `alt`
  - `assetId` resolves `assets.images[assetId].url` if present

### Chrome

- `Header`, `Footer`
  - props: `left`, `center`, `right` (each can be a node or an array of nodes/strings)
- `SlideNumber`
  - props: `format` (`current|total|current/total`)

### Live embeds (synced with Storybook)

- `Embed`
  - props: `src` (required), `title` (optional), `minHeight` (optional, px; default 320)
  - Renders an iframe. Use for any same-origin URL (e.g. app routes). Deck is local-only so embedding is safe.
- `Story`
  - props: `storyId` (required)
  - Renders the story **inline** (no iframe): looks up the story by id via `getStoryById`, calls `story.render()`, and embeds the result in the slide. Stays in sync with `apps/web/src/storybook/stories/`. Full height/width when inside a `Layer` (wrapper and story root use `h-full`). Use stories that size with `h-full` (e.g. `autopilot-dashboard-preview`) for full-bleed.

### Ways to show live components

You can show a Storybook story or embedded URL in two ways:

1. **Inline in the slide body**  
   Use a `Story` or `Embed` node inside the `body` region (e.g. under a `title-body` layout). `Story` renders the story template inline (no iframe); `Embed` uses an iframe with `minHeight`. Good for “here’s a component” slides.

2. **Full-bleed behind text (Solution-style)**  
   Use the **solution** layout (or a custom layout that follows the same pattern): the component goes in the `background` region and is drawn in a full-screen layer; a dark overlay (e.g. 75% black) sits on top; header/body/footer render above that so the title and copy stay readable and the component is visible behind. See the next subsection and the `solution` layout in `apps/web/public/decks/deck.json`.

### Full-bleed component with text overlay (e.g. Solution slide)

To get “component fullscreen behind, text on top”:

1. Use a layout whose root is a **Column** with `fill: true` and `position: "relative"`.
2. Add three **Layer** children in order:
   - **Layer** (z-0): contains a single **Slot** named `background`. Put your `Story` or `Embed` in the slide’s `background` region so it fills the slide.
   - **Layer** (z-10): `background: "rgba(0,0,0,0.75)"`, `pointerEvents: false`, no children. This is the semi-transparent overlay so text contrasts; `pointerEvents: false` keeps the header/body/footer interactive.
   - **Layer** (z-20): contains a **Column** with the usual **Slot**s for `header`, `body`, and `footer`. Your title and body copy go here and appear on top of the overlay.

The `solution` layout and the `solution` slide in `apps/web/public/decks/deck.json` implement this. The dashboard Storybook story (`autopilot-dashboard-preview`) is in `background`; “Solution” and the body text are in `header` and `body`.

Unknown node types render as an error placeholder.

## Minimal Example Deck (`apps/web/public/decks/deck.json`)

Create the file with something like the following. It defines a **title** layout (full-height column with centered title/subheadline and footer at bottom), a **title-body** layout for content slides, and optionally a **solution** layout for full-bleed component + text overlay slides.

**Title slide:** title "OpenAgents", subheadline "The Agents Platform", name/role/email in the bottom right. **Problem slide:** headline + body copy. **Solution slide:** uses the `solution` layout with a Story in `background` and "Solution" + body text in header/body. **Self-improve slide:** graph-only visualization of what an end-to-end DSE run can look like (runtime parts + compile/promote loop), rendered with `Graph` nodes (no extra overlay text).

```json
{
  "dsl": "effuse.slide-deck",
  "version": "0.1.0",
  "meta": { "title": "OpenAgents" },
  "theme": {
    "tokens": {
      "color.bg": "oklch(0.13 0 0)",
      "space.2": 16
    }
  },
  "layouts": {
    "title": {
      "type": "Column",
      "props": {
        "gap": { "$token": "space.2" },
        "fill": true,
        "justify": "space-between",
        "align": "center"
      },
      "children": [
        { "type": "Slot", "props": { "name": "header" } },
        { "type": "Slot", "props": { "name": "body" } },
        { "type": "Slot", "props": { "name": "footer" } }
      ]
    },
    "title-body": {
      "type": "Column",
      "props": { "gap": { "$token": "space.2" } },
      "children": [
        { "type": "Slot", "props": { "name": "header" } },
        { "type": "Slot", "props": { "name": "body" } },
        { "type": "Slot", "props": { "name": "footer" } }
      ]
    }
  },
  "deck": {
    "aspectRatio": "16:9",
    "background": { "$token": "color.bg" },
    "defaultSlideLayout": "title-body",
    "slides": [
      {
        "id": "title",
        "layout": "title",
        "regions": {
          "header": [],
          "body": [
            { "type": "Column", "props": { "fill": true, "justify": "center", "align": "center", "gap": 28 }, "children": [
              { "type": "Graph", "props": { "opacity": 0.55, "preset": "dots-slow" }, "children": [
                { "type": "GraphNode", "props": { "nodeId": "runtime", "label": "Runtime", "subtitle": "tools + receipts", "nodeType": "leaf", "x": "15%", "y": "12%" } },
                { "type": "GraphNode", "props": { "nodeId": "compiler", "label": "Compiler", "subtitle": "Signatures + Modules", "nodeType": "leaf", "x": "78%", "y": "16%" } },
                { "type": "GraphNode", "props": { "nodeId": "market", "label": "Market", "subtitle": "budgets + lanes", "nodeType": "leaf", "x": "80%", "y": "82%" } },
                { "type": "GraphNode", "props": { "nodeId": "verify", "label": "Verification", "subtitle": "tests + replay", "nodeType": "leaf", "x": "12%", "y": "82%" } },
                { "type": "GraphNode", "props": { "nodeId": "autopilot", "label": "Autopilot", "subtitle": "product surface", "nodeType": "root", "anchor": "center", "x": "50%", "y": "52%" } },

                { "type": "GraphEdge", "props": { "from": "autopilot", "to": "runtime" } },
                { "type": "GraphEdge", "props": { "from": "autopilot", "to": "compiler" } },
                { "type": "GraphEdge", "props": { "from": "autopilot", "to": "market" } },
                { "type": "GraphEdge", "props": { "from": "autopilot", "to": "verify" } }
              ] },
              { "type": "Text", "props": { "style": "h1" }, "children": ["OpenAgents"] },
              { "type": "Text", "props": { "style": "h2" }, "children": ["The Agents Platform"] }
            ] }
          ],
          "footer": [
            { "type": "Footer", "props": {
              "right": [
                { "type": "Column", "props": { "gap": 2, "align": "end" }, "children": [
                  { "type": "Text", "props": { "style": "caption" }, "children": ["Christopher David"] },
                  { "type": "Text", "props": { "style": "caption" }, "children": ["Founder & CEO"] },
                  { "type": "Text", "props": { "style": "caption" }, "children": ["chris@openagents.com"] }
                ] }
              ]
            } }
          ]
        }
      },
      {
        "id": "problem",
        "regions": {
          "header": [
            { "type": "Text", "props": { "style": "h1" }, "children": ["Problem"] }
          ],
          "body": [
            {
              "type": "Text",
              "props": { "style": "body" },
              "children": ["There is no single place to deploy and run the best AI agents—the market is fragmented across runtimes, tools, and vendors."]
            }
          ],
          "footer": [
            { "type": "Footer", "props": { "right": [{ "type": "SlideNumber" }] } }
          ]
        }
      },
      {
        "id": "self-improve",
        "layout": "title",
        "regions": {
          "header": [],
          "body": [
            {
              "type": "Column",
              "props": { "fill": true, "position": "relative" },
              "children": [
                {
                  "type": "Graph",
                  "props": { "opacity": 0.95, "preset": "dashes-fast", "zIndex": 0 },
                  "children": [
                    { "type": "GraphNode", "props": { "nodeId": "user", "label": "User", "subtitle": "review gmail", "nodeType": "leaf", "x": "14%", "y": "18%", "status": "ok" } },
                    { "type": "GraphNode", "props": { "nodeId": "ui", "label": "Autopilot UI", "subtitle": "Effuse chat", "nodeType": "leaf", "x": "14%", "y": "32%", "status": "live" } },
                    { "type": "GraphNode", "props": { "nodeId": "worker", "label": "Worker", "subtitle": "Convex-first", "nodeType": "leaf", "x": "14%", "y": "46%", "status": "running" } },
                    { "type": "GraphNode", "props": { "nodeId": "convex", "label": "Convex", "subtitle": "messageParts", "nodeType": "leaf", "x": "14%", "y": "64%", "status": "ok" } },
                    { "type": "GraphNode", "props": { "nodeId": "assistant", "label": "Assistant", "subtitle": "streamed text", "nodeType": "leaf", "x": "14%", "y": "82%", "status": "ok" } },

                    { "type": "GraphNode", "props": { "nodeId": "sig_select", "label": "dse.signature", "subtitle": "SelectTool.v1", "nodeType": "leaf", "x": "36%", "y": "28%", "status": "ok", "badge": { "label": "212ms", "tone": "info" } } },
                    { "type": "GraphNode", "props": { "nodeId": "tool_connect", "label": "dse.tool", "subtitle": "gmail.connect", "nodeType": "leaf", "x": "36%", "y": "42%", "status": "ok" } },
                    { "type": "GraphNode", "props": { "nodeId": "tool_list", "label": "dse.tool", "subtitle": "gmail.listThreads", "nodeType": "leaf", "x": "36%", "y": "56%", "status": "ok", "badge": { "label": "2 calls", "tone": "neutral" } } },
                    { "type": "GraphNode", "props": { "nodeId": "sig_write", "label": "dse.signature", "subtitle": "WriteResponse.v1", "nodeType": "leaf", "x": "36%", "y": "70%", "status": "ok", "badge": { "label": "rcpt", "tone": "success" } } },

                    { "type": "GraphNode", "props": { "nodeId": "observe", "label": "Observe", "subtitle": "receipts + hashes", "nodeType": "leaf", "x": "64%", "y": "24%", "status": "ok" } },
                    { "type": "GraphNode", "props": { "nodeId": "label", "label": "Label", "subtitle": "judge / expected", "nodeType": "leaf", "x": "84%", "y": "24%", "status": "pending" } },
                    { "type": "GraphNode", "props": { "nodeId": "eval", "label": "Evaluate", "subtitle": "reward bundle", "nodeType": "leaf", "x": "64%", "y": "40%", "status": "ok", "badge": { "label": "0.59", "tone": "warning" } } },
                    { "type": "GraphNode", "props": { "nodeId": "compile", "label": "Compile", "subtitle": "MIPRO / GEPA", "nodeType": "leaf", "x": "84%", "y": "44%", "status": "ok", "badge": { "label": "24 cand", "tone": "info" } } },
                    { "type": "GraphNode", "props": { "nodeId": "promote", "label": "Promote", "subtitle": "canary rollout", "nodeType": "leaf", "x": "64%", "y": "60%", "status": "ok", "badge": { "label": "10%", "tone": "success" } } },
                    { "type": "GraphNode", "props": { "nodeId": "monitor", "label": "Monitor", "subtitle": "quality/cost", "nodeType": "leaf", "x": "84%", "y": "64%", "status": "running", "badge": { "label": "p95", "tone": "warning" } } },
                    { "type": "GraphNode", "props": { "nodeId": "rollback", "label": "Rollback", "subtitle": "pointer-only", "nodeType": "leaf", "x": "64%", "y": "78%", "status": "pending" } },
                    { "type": "GraphNode", "props": { "nodeId": "policy", "label": "Policy Registry", "subtitle": "active compiled_id", "nodeType": "leaf", "x": "84%", "y": "82%", "status": "ok", "badge": { "label": "c_8a1b", "tone": "neutral" } } },

                    { "type": "GraphEdge", "props": { "from": "user", "to": "ui" } },
                    { "type": "GraphEdge", "props": { "from": "ui", "to": "worker" } },
                    { "type": "GraphEdge", "props": { "from": "worker", "to": "sig_select" } },
                    { "type": "GraphEdge", "props": { "from": "sig_select", "to": "tool_connect" } },
                    { "type": "GraphEdge", "props": { "from": "tool_connect", "to": "tool_list" } },
                    { "type": "GraphEdge", "props": { "from": "tool_list", "to": "sig_write" } },
                    { "type": "GraphEdge", "props": { "from": "sig_write", "to": "assistant" } },
                    { "type": "GraphEdge", "props": { "from": "assistant", "to": "convex" } },
                    { "type": "GraphEdge", "props": { "from": "convex", "to": "observe" } },

                    { "type": "GraphEdge", "props": { "from": "observe", "to": "label" } },
                    { "type": "GraphEdge", "props": { "from": "label", "to": "eval" } },
                    { "type": "GraphEdge", "props": { "from": "eval", "to": "compile", "preset": "pulse" } },
                    { "type": "GraphEdge", "props": { "from": "compile", "to": "promote", "preset": "pulse" } },
                    { "type": "GraphEdge", "props": { "from": "promote", "to": "policy" } },
                    { "type": "GraphEdge", "props": { "from": "policy", "to": "worker", "preset": "pulse" } },
                    { "type": "GraphEdge", "props": { "from": "promote", "to": "monitor" } },
                    { "type": "GraphEdge", "props": { "from": "monitor", "to": "rollback" } },
                    { "type": "GraphEdge", "props": { "from": "rollback", "to": "policy" } }
                  ]
                }
              ]
            }
          ],
          "footer": []
        }
      }
    ]
  }
}
```

Then visit `http://localhost:3000/deck` and use `→` / `←` to step the builds.

## Extending The Engine

To add a new node type:

1. Update `apps/web/src/effuse-deck/render.ts`:
   - add a `case "<TypeName>"` to `renderNode`
   - define prop parsing + mapping to Effuse template HTML
2. Update validation in `apps/web/src/effuse-deck/dsl.ts` if you want stricter enforcement.
3. Add/extend tests in `apps/web/tests` (contract-level tests are preferred).
