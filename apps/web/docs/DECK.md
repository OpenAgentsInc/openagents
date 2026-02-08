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

### Layouts + `Slot`

`layouts` is a map: `{ [name]: DeckNode }`.

`Slot` is a special node type used only inside layout trees:

```json
{ "type": "Slot", "props": { "name": "body" } }
```

During render:

- the layout tree is cloned
- each `Slot(name)` is replaced with a `Fragment` containing `slide.regions[name]` (or empty if absent)

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
  - props: `gap`, `align`, `justify`
- `Box`
  - props: `padding`, `border` (truthy = show border), `background`, `color`, `width`, `height`
- `Spacer`
  - props: `size`
- `Divider`
  - props: `thickness`, `color`

### Text

- `Text`
  - props: `style` (`h1|h2|h3|body|caption|code`), `align`, `color`
  - note: sizes are currently hardcoded per `style` (tokens not wired for font sizes yet)
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

Unknown node types render as an error placeholder.

## Minimal Example Deck (`apps/web/public/decks/deck.json`)

Create the file with something like:

```json
{
  "dsl": "effuse.slide-deck",
  "version": "0.1.0",
  "meta": { "title": "Effuse Deck (Local)" },
  "theme": {
    "tokens": {
      "color.bg": "oklch(0.13 0 0)",
      "space.2": 16
    }
  },
  "layouts": {
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
        "id": "s1",
        "regions": {
          "header": [
            { "type": "Text", "props": { "style": "h1" }, "children": ["Hello, deck"] }
          ],
          "body": [
            {
              "type": "List",
              "props": { "ordered": false, "gap": 12 },
              "children": [
                { "type": "ListItem", "children": ["First point"] },
                { "type": "ListItem", "props": { "build": { "in": 2 } }, "children": ["Second point (step 2)"] }
              ]
            }
          ],
          "footer": [
            { "type": "Footer", "props": { "right": [{ "type": "SlideNumber" }] } }
          ]
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

