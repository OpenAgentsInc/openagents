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
| Physical springs | `core/spring.coax` |
| Animated frames, activity/progress rings, switches, drawer/reply surfaces | `kit/animated.coax` |
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
engine or application-specific GPU shader. Missing images/blur are backend
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

## Try it

- **iPhone:** Settings → Components (TestFlight 1086 or later). The shared Apple
  GPU carrier embeds a Metal view; it does not open a browser. Gallery options
  also provides native accessible section, motion and skin controls. Touches
  arbitrate tap/scroll/slider in Coax, cancellation never activates a control,
  and the display link stops on dismissal or backgrounding.
- **Web:** [/components-gpu](https://openagents.com/components-gpu) runs the same
  gallery with WebGPU. [/components](https://openagents.com/components) is the
  browser-rendered companion, with a link to GPU + motion.
- **Mac/Linux/Android:** the existing `gallery/{apple,linux,android}/build.sh`
  entry points still load `gallery/page.coax` and the common shader. These
  carriers share the atlas and native entry in `gallery/linux/main.c`; the iOS
  app compiles its embedded profile rather than copying it.

The motion lab has replay, pause, reduced motion, three visual skins and tap
reversal. It adds reusable corner frames, activity/progress arcs, spring
switches and drawer/reply surfaces. The existing four-skin controls/Markdown,
HUD cards and 25 ARWES specimens remain selectable. GPU arcs now lower to at
most 64 existing antialiased segments; this is a bounded approximation.

This is an inspectable first library release, not completion of all 28 Arcade
animation cases. Image textures, general transforms/blur, inline GPU islands,
multiple live world views and complete GPU semantic/text parity remain in
[rendering.md](rendering.md). Native gallery code is installed code: changes
to its compiled components require a build. Portable app screens and their
Settings link continue through signed Updates. Gallery controls are accessible
through the native menu; per-primitive text selection and VoiceOver semantics
are not qualified, so this does not switch the production chat renderer.
