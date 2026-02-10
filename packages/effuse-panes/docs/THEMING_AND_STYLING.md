# Theming and Styling

The DOM adapter (`mountPaneSystemDom`) injects a small CSS bundle into the root container.

The goal is to provide:

- usable defaults (dark industrial theme, close to WGPUI)
- no external CSS dependencies
- a small surface area for host overrides

## Theme Tokens (`PaneSystemTheme`)

`PaneSystemTheme` is a plain object of CSS color strings:

- `background`: base background (canvas + pane body)
- `surface`: active/raised surface (hotbar item active background)
- `border`: chrome border color
- `accent`: active highlight (active pane border)
- `text`: primary text color
- `mutedText`: muted text color (reserved for future usage)

Default is `DEFAULT_PANE_SYSTEM_THEME`, derived from `crates/wgpui/src/theme/mod.rs`.

Example override:

```ts
mountPaneSystemDom(root, {
  theme: {
    ...DEFAULT_PANE_SYSTEM_THEME,
    accent: "#4A9EFF", // match WGPUI accent::BLUE
  },
});
```

## Layout Tokens

`PaneSystemConfig` includes explicit layout numbers that match Autopilot Desktop constants:

- `paneTitleHeight` (28)
- `paneResizeHandle` (10)
- `gridDotDistance` (32)

Hotbar layout:

- `hotbarHeight` (52)
- `hotbarFloatGap` (18)
- `hotbarItemSize` (36)
- `hotbarItemGap` (6)
- `hotbarPadding` (6)
- `hotbarCornerRadius` (8)

You can override any of these.

Note:

- The hotbar `bottom` positioning uses `env(safe-area-inset-bottom, 0px)` so it does not overlap the iOS home indicator.

## Dotted Background

By default (`enableDotsBackground: true`), the root is given `data-oa-dots="1"`.

This enables a CSS `radial-gradient(...)` dot grid background.

Canvas panning updates the CSS `background-position` so dots "move" with the pan.

If you want a solid background, set:

```ts
root.removeAttribute("data-oa-dots");
```

or set `enableDotsBackground: false` in `mountPaneSystemDom(root, { ... })`.

or override the injected CSS with your own selectors.

## Overriding Styles

The adapter injects:

```html
<style data-oa-pane-style="1"> ... </style>
```

inside the root.

If you need to override styling:

1. Prefer config (`theme`, `hotbar*`, `paneTitleHeight`, etc).
2. Otherwise, add your own CSS with selectors that are stable:
   - `[data-oa-pane-system]`
   - `[data-oa-pane]`
   - `[data-oa-pane-title]`
   - `[data-oa-pane-content]`
   - `[data-oa-hotbar]`
   - `[data-oa-hotbar-item]`

Example: make panes slightly translucent:

```css
[data-oa-pane] {
  background: rgba(10, 10, 10, 0.92);
}
```

## Accessibility Notes

The pane chrome is intentionally minimal today:

- Close button is a real `<button>` with `aria-label="Close pane"`.
- Hotbar items are real `<button>` elements with `title` for hover text.
- Root is made focusable (`tabIndex=0`) so keyboard shortcuts work after click.

If you build a more complete UI, you may want:

- visible focus rings
- ARIA role semantics for panes (e.g. `role="dialog"` for some panes)
- keyboard navigation between panes

Those are currently host responsibilities.
