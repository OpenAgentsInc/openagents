# rust-native-desktop

The desktop adapter for [Rust Native](../rust-native/) views. An application
describes each screen as a validated `View` with its own typed intents; this
crate lays the view out in Rust, paints it in software, and shows it in a
`winit` window over a `wgpu` surface, the window pattern
[`openagents-deck`](../openagents-deck/) uses. It adds no window or GPU
dependency the workspace did not already have.

- **Layout** ([`src/layout.rs`](src/layout.rs)) places nodes in points and
  records every button's rectangle, in view order, which is also the focus
  order.
- **Text** ([`src/text.rs`](src/text.rs)) breaks lines with
  `rust_native::layout::shape::ShapingMeasurer`, the breaker the iOS
  transcript is checked against, and paints the same bundled Inter and
  JetBrains Mono outlines with `swash`.
- **Paint** ([`src/paint.rs`](src/paint.rs), [`src/canvas.rs`](src/canvas.rs))
  draws antialiased rounded rectangles, check marks, and glyphs into an
  RGBA frame. The window retains this frame and compares drawing operations
  to repaint changed regions. Clipping excludes hidden operations and limits
  rasterization to visible pixels. `capture` paints a complete view without
  a window, for tests and PNGs.
- **Window** ([`src/window.rs`](src/window.rs), the default `window`
  feature) paints only when the view, the size, or the pointer's target
  changes, sleeps until the application's next tick, and turns clicks and
  Tab/Enter/Space into a revision-bound `Activation` it resolves against
  the current view before the application sees an intent.

- **Backdrop** ([`src/backdrop.rs`](src/backdrop.rs), with `window`) is
  an optional live picture an application draws with the window's own
  `wgpu` device (`window::run_with_backdrop`). The views are then painted
  into a retained clear frame only when they change, and only changed pixel
  regions are uploaded; each frame the backdrop is
  drawn at half size, blurred, dimmed under the window's background
  (`Look`), and the views are laid over it in one pass, so every view
  keeps its full contrast. The backdrop sets its own frame rate and gets
  no frames while the window is hidden.

## What it draws

| Element or property | Desktop |
| --- | --- |
| `Stack` vertical | Children one under another, `gap` apart. With `align` `start` text, stacks, and lists fill the width; with `center` or `end` every child keeps its own width. |
| `Stack` horizontal | Buttons and surfaces keep their width; text, stacks, and lists share the rest; children centered on the row. |
| `Stack` wrap | Flows onto as many rows as needed. |
| `List` | A vertical stack with a rule between rows. The label is not drawn. |
| `Text` | `heading`, `body`, `status` (secondary color), `code` and `terminal` (JetBrains Mono; terminal never wraps), `markdown` as plain text. `align` and `weight` apply. |
| `Button` | A filled rounded rectangle; a capsule with `pill`; a link with a transparent `style.background`; a checkbox with the `unchecked` or `checked` glyph. The closed glyph set is drawn with vector strokes. A `circular` icon button draws only its glyph and keeps its label as its semantic name. Explicit `align: start` fills the available width and aligns the label to the start. |
| `Surface` | The application's `surface_size` and `paint_surface`; an unregistered resource shows its label. |
| `style.background` on a stack | A card with rounded corners. |
| `style.padding`, `gap`, `foreground` | As given (`xs` 4, `sm` 8, `md` 16, `lg` 28 points). |
| `Transcript`, `Message`, `Tool`, `Composer` | Not supported: recorded in `Scene::unsupported` and drawn as their children or label. |

The application supplies the palette through `App::theme`; the defaults are
white on black.

## Split windows

`WindowLayout::HeaderSplit` places a fixed-height header above both panes.
Its root is a vertical stack containing the header and the horizontal pane
stack. The panes scroll below the header; their docked footers remain visible.
On macOS, the native titlebar is transparent, the standard window controls sit
inside the header, and dragging an empty header area moves the window.
`App::fullscreen_changed` reports native fullscreen state before layout so an
application can adjust the space reserved for those controls.

`App::window_layout` defaults to the centered column. `WindowLayout::Split`
lays out a horizontal root stack with two vertical pane stacks. Each pane has
three children: header, scrollable body, and footer. All controls stay ordinary
semantic nodes with revision-bound activations. The layout is an adapter
option, not another view schema or a change to phone layout.

`SplitLayout` sets the leading pane's preferred width, its bounds, the content
pane's minimum width, collapse state, and whether to center body content.
The leading pane clamps to the available window width. A malformed pane tree
falls back to the column and reports `window.split` in `Scene::unsupported`.

Each body clips its drawing and pointer targets, has its own scroll offset,
and exposes a scrollbar when it overflows. Headers and footers stay fixed.
Tab scrolls an offscreen focused control into view. Dragging the seam calls
`App::resize_leading_pane`; the application owns the preferred width. Closing
or losing focus ends a drag. `App::key_bindings` maps command chords to node
keys, and the window resolves them against the current validated view.

## Composer editing foundation

[`composer::ComposerDraft`](src/composer.rs) retains a local
[`rust_native::edit::Editor`](../rust-native/src/edit.rs) across updates of the
same semantic composer. It checks callback identity and edit sequence, keeps
IME preedit through rerenders, resolves typed stop actions, and validates sends
and the composer's own choices. Preparing a send keeps the draft; acceptance
clears only the submitted edit sequence. Both APIs reimplement Zeron's retained
composer editing design for
[#9996](https://github.com/OpenAgentsInc/openagents/issues/9996).

This module runs in headless input fixtures. Window key and IME events,
clipboard access, focus and caret painting, and the application send callback
are not connected yet. The support table above still describes the window's
actual rendering capabilities.

## Use it

Implement `rust_native_desktop::App` and call
`rust_native_desktop::window::run(app, Options::default())`.
[`openagents-desktop`](../openagents-desktop/) is the first application.

```sh
cargo test -p rust-native-desktop
```
