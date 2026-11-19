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

## Step 3: Convert Points to Coax Layout Units

Coder Desktop renders with `cp_scale(s) = ARCADE_PHONE_SCALE * zoom`, where
`ARCADE_PHONE_SCALE = 1.2` (`components/skin/arcade.coax`) and default zoom is
`1.0`. Therefore:

```text
coax_units = target_points / ARCADE_PHONE_SCALE   // divide by 1.2
```

Apply desktop-specific geometry overrides in `cp_skin()` inside
`apps/coder-desktop/page/skin.coax` (keeping shared skins intact for other apps):

| Element | Claude Desktop (pt) | Coder `cp_skin()` (Coax units at 1.2x) |
| --- | --- | --- |
| Default sidebar width (`CP_SIDE`) | `280pt` | `234` (`280 / 1.2`) |
| Sidebar row height (`side_row`) | `32pt` | `32.0 / u` |
| Sidebar section heading (`side_head`) | `26pt` | `26.0 / u` |
| Row horizontal inset & radius (`row_inset`, `row_radius`) | `8pt` | `8.0 / u` |
| Row text horizontal padding (`side_pad`) | `16pt` | `16.0 / u` |
| Sidebar icon size (`app.icon`) | `15pt` | `15.0 / u` |
| Composer toolbar chip height / radius / icon | `28pt` / `8pt` / `14pt` | `28.0 / u`, `8.0 / u`, `14.0 / u` |

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
