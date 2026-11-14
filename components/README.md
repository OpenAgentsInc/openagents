# Shared Coax components

This is the component library for Coax Native, Coax Native Web, desktop apps,
realm HUDs and terminal surfaces. Start with [spec.md](spec.md) and
[rendering.md](rendering.md). Coax Native uses this library; it does not own
a second set of native-only components.

## Find the existing owner

| Need | Read |
| --- | --- |
| Keyed tree, semantic elements and actions | `core/tree.coax`, `core/elements.coax`, `kit/frames.coax` |
| Text editing, Markdown, syntax | `core/edit.coax`, `core/markdown.coax`, `core/syntax.coax` |
| Layout and text measurement/wrapping | `layout/flex.coax`, `layout/measure.coax`, `layout/text.coax`, `layout/font.coax` |
| Paint primitives, selectable text and hit regions | `paint/list.coax`, `paint/sheet.coax` |
| Colors, measures, frames and motion values | `skin/` |
| Shared easing, timelines and effects | `core/arwes/motion.coax`, `core/arwes/fx.coax` |
| Display-list controls, Markdown and game HUD | `kit/controls.coax`, `kit/markdown.coax`, `kit/hud.coax` |
| Tree-based forms, chat and settings | `kit/frames.coax`, `kit/settings.coax`, `kit/dock.coax`, `kit/work.coax` |
| GPU drawing and shader math | `backend/gpu.coax`, `backend/gpu-shader.coax` |
| Browser tree output or terminal drawing | `backend/dom.coax`, `backend/terminal.coax` |
| Existing desktop chat set and its adapters | `zeron/` |
| Gallery and examples | `gallery/`, the `/components` page |

The tree-based and display-list entry points are both real; they are not yet
one interchangeable renderer API. Reconcile their state, roles and paint as
[the rendering plan](rendering.md#one-library-several-presentations) specifies.
Do not copy a component to solve the difference in an application folder.

## Extend it

1. Locate the component/state/parser/layout owner in [docs/reuse.md](../docs/reuse.md).
2. Write Coax checks for the intended state/action and cancellation behavior.
3. Extend core/layout/paint/skin/kit in their existing direction of dependencies.
   A component reads skin roles; it contains no private colors, font names or sizes.
4. Emit semantics alongside drawing. Stable IDs and actual presented geometry
   drive focus, accessibility, hit tests and automation.
5. Add the gallery case and numeric/tree/picture evidence for each supported
   backend. Declare unsupported capabilities and their alternatives explicitly.
6. Register a new wire/stored/cross-module shape in `proto/` with one codec
   before use. Preserve installed runtime fingerprints and published ABIs.

Never add another Markdown parser, line wrapper, text edit buffer, animation
engine or application-specific GPU shader. Missing arcs/images/blur are backend
work; OS input methods and accessibility are host work. A component cannot
reach an engine, network, credential or platform SDK directly.

## Check it

Run the touched Coax units and the component/layer gates using the repository's
remote-build rules. The local entry points include
`coax/web/tool.sh components-check` and `coax/web/tool.sh layer-check`.
Use the existing headless shot/drive workflow for gallery pictures; do not open
a browser window or play sound. Actual iOS/Android editing and accessibility
must also pass before switching those devices' default renderer.

The [Arcade catalog](../docs/arcade/06-rendering-and-animation.md) is the first
complete animation acceptance set for the new rendering work. Its entries
are requirements and source findings, not claims of implemented components.
