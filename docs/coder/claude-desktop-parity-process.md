# Claude Desktop -> Coder Desktop UI Parity Process

This document describes the repeatable workflow for matching Claude Desktop's
structural layout, spacing, proportions, and typographic hierarchy in
`apps/coder-desktop` while strictly preserving **Coder's own monochrome palette,
branding, and logos**.

## Core Rule: Structure Only, Never Brand or Colors

- **Copy**: layout hierarchy, pane widths, row heights, paddings, insets, border
  radii, icon sizes, font-size ratios, and control placement.
- **Keep Coder's own**: monochrome Arcade palette (`components/skin/mono.coax`,
  `components/skin/arcade.coax`), Coder's icons/branding, and Coax component
  primitives (`components/app/`, `components/kit/`).

---

## Step 1: Extract Reference Styles & Markup from Claude Desktop

Claude Desktop ships Electron ASAR archives at:

```text
/Applications/Claude.app/Contents/Resources/app.asar
/Applications/Claude.app/Contents/Resources/ion-dist.asar
```

1. Extract CSS bundles (or read directly from the ASAR header/payload in Python)
   into `/tmp/claude-css/`:
   - `ion-dist/assets/v1/shared-styles-*.css` — desktop frame tokens (`--df-*`,
     `.dframe-root`, `.dframe-sidebar`, `.df-row`, `.df-titlebar`, etc.).
   - `app/` CSS & JS bundles — composer, toolbar chips, and webview layout.
2. Query exact design tokens with Python regex over the extracted CSS:
   ```bash
   python3 -c '
   import re
   css = open("/tmp/claude-css/ion-dist/assets/v1/shared-styles-7U9dpGAh.css").read()
   for m in re.finditer(r"(\.dframe-[^{]+|\.df-[^{]+)\{([^}]+)\}", css):
       print(m.group(1), "=>", m.group(2))
   '
   ```

## Step 2: Measure Reference Screenshots

Capture side-by-side macOS Retina screenshots of Claude Desktop and Coder
Desktop (e.g. `Screenshot ...png` on Desktop) and measure rendered geometry in
logical points (`px / 2` on 2x Retina):

1. **Pixel band scans (Pillow)**: sample background fills and horizontal/vertical
   non-background spans to verify actual rendered widths, row heights, insets,
   and radii (e.g., sidebar width `280pt`, row height `32pt`, row inset `8pt`,
   row radius `8pt`, composer bottom-left toolbar chip alignment).
2. **Text bounds & labels (macOS Vision OCR via Swift)**: run
   `VNRecognizeTextRequest` on cropped sidebar and composer regions to verify
   text cap heights, baselines, horizontal padding (`16pt`), and control order.

## Step 3: Write Measures in Points

Coder Desktop lays out in points (GitHub #13737): `cp_scale(s)` is the text
size alone (`CP_ZOOMS`, 1.0 by default), so a measured point value goes into
`cp_skin()` (`apps/coder-desktop/page/skin.coax`) or `page/state.coax` as is.
Font sizes are the atlas's, in points, in `CP_FONT_SIZES`; the check
`every font slot comes out at Zeron's type ramp` in
`page/checks/layout-checks.coax` holds them to Zeron's ramp.

| Element | Points | Where |
| --- | --- | --- |
| Default sidebar width | `272` (242-400) | `CP_SIDE` |
| Sidebar row height, heading | `32`, `26` | `k.app.side_row`, `k.app.side_head` |
| Row inset and radius, text padding | `8`, `8`, `16` | `k.app.row_inset`, `k.app.row_radius`, `k.app.side_pad` |
| Conversation column, side room | `660`, `32` | `CP_COLUMN`, `CP_PAD` |
| Composer chip height / radius / icon | `28` / `8` / `14` | `k.cmp.chip_h`, `k.cmp.chip_radius`, `k.cmp.chip_icon` |

## Step 4: Implement in Coax Components & `apps/coder-desktop/page/`

1. Keep reusable structural improvements in `components/app/` and
   `components/kit/` (for example, `app_side_action` and hover states in
   `components/app/nav.coax`, and live `row_y` returned in `ComposerBox` from
   `components/kit/composer.coax`).
2. Wire page-specific layout in `apps/coder-desktop/page/layout/` (`cp_layout`,
   `cp_sidebar`, `cp_conversation`, `cp_pane`) and `page/skin.coax` (`cp_skin`).
3. Never hardcode Claude colors; always reference Coder's skin tokens
   (`k.canvas`, `k.surface`, `k.raised`, `k.hover`, `k.menu_divider`, `k.ink`,
   `k.muted`, `k.faint`).

## Step 5: Verify with Tests, Build, and Visual Diff

Run all Coax unit tests for touched components and the desktop page:

```bash
coax/bin/coax test components/app/nav.coax
coax/bin/coax test components/kit/composer.coax
coax/bin/coax test apps/coder-desktop/page.coax
```

Build the macOS app bundle to verify C/Swift/Coax integration:

```bash
./apps/coder-desktop/build.sh
```

Finally, launch `apps/coder-desktop/build/Coder.app` and compare against the
Claude Desktop reference screenshot to confirm alignment, spacing, and visual
hierarchy.
