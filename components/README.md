# Shared Coax components

This is the component library for Coax Native, Coax Native Web, desktop apps,
realm HUDs and terminal surfaces. Start with [spec.md](spec.md),
[rendering.md](rendering.md) and, for native presentation,
[gpu-components.md](gpu-components.md). Coax Native uses this library; it does not own
a second set of native-only components.

## Find the existing owner

| Need | Read |
| --- | --- |
| Keyed tree, semantic elements and actions | `core/tree.coax`, `core/elements.coax`, `kit/frames.coax` |
| A GPU-drawn component's frame context: sheet, measure, skin, semantic table, keyed state, input, clock | `kit/frame.coax` (`UiFrame`, `uiframe_begin`, `uiframe_row`, `uiframe_end`); see [gpu-components.md](gpu-components.md#frame-context) |
| Semantic rows for hit tests, focus order, accessibility and `coax drive`, with a per-surface capacity | `core/elements.coax` (`ElTable`, `elt_row`, `elt_hit`, `elt_focus_step`, `elt_tree`) |
| State kept by a component's stable ID across frames: entry and press motion, a spring, scroll offset, expanded or collapsed | `core/keyed.coax` (`KeyedStore`, `keyed_see`) |
| Display list, hit regions and semantic rows committed as one generation; overflow refused and counted | `paint/surface.coax` (`UiSurface`, `surface_begin`, `surface_commit`) |
| Text editing, Markdown, syntax | `core/edit.coax`, `core/markdown.coax`, `core/syntax.coax` |
| Layout and text measurement/wrapping | `layout/flex.coax`, `layout/measure.coax`, `layout/text.coax`, `layout/font.coax` |
| Paint primitives, selectable text and hit regions | `paint/list.coax`, `paint/sheet.coax` |
| Colors, measures, frames and motion values | `skin/` |
| Physical springs | `core/spring.coax` |
| Animated frames, activity/progress rings, switches, drawer/reply surfaces | `kit/animated.coax` |
| Measured native controls painted in one shared GPU scene | `native/scene.coax`, `native/apple.{c,h}`, `skin/arcade.coax`; SDK projection: `apps/shared/native/NativeArcade.swift` |
| Shared easing, timelines and effects | `core/arwes/motion.coax`, `core/arwes/fx.coax` |
| Real bounded line series/response plots | `kit/plot.coax`; twelve-float world HUD adapter: `metaverse-engine/ui/plot.coax` |
| Display-list chip: icon, chosen value, opens a choice | `kit/chip.coax` (`ctx_chip`) |
| Display-list touch composer: multiline text, model chip, Send and Stop | `kit/composer.coax` (`ctx_composer`); geometry in `layout/native-composer.coax` |
| Display-list forms: field, full-width action, quiet action, notice, heading | `kit/form.coax` (and its `ctx_*` forms); phone sizes in `skin/arcade.coax` (`arcade_phone_skin`) |
| Display-list controls, Markdown and game HUD | `kit/controls.coax` (and its `ctx_*` forms on the frame context), `kit/markdown.coax`, `kit/hud.coax` |
| Tree-based forms, chat and settings | `kit/frames.coax`, `kit/settings.coax`, `kit/dock.coax`, `kit/work.coax` |
| The hotbar every game and realm uses (page tree): slots that are tools, toggles or actions, in groups, pressed by pointer or key, a slot's options in a row over it while it is selected, skinned by genre as data (`HbSkin`; `hb_scifi` first) | `kit/hotbar.coax`, its skins and stylesheet in `skin/hotbar.coax` |
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

The **Reference** gallery section names all 25 atom categories observed in the
Claude iOS captures and provides a visual specimen for each. The thumbnail is
an icon placeholder because the GPU gallery does not render image textures;
fields are static, the scrim is a swatch, and the system status bar is the
host's actual bar rather than a simulated one. The section also includes
interactive model-choice and notification rows, a project context pill and a
usage meter, plus centered screen and sheet headers, a settings picker, and
icon-led settings, project, session and action-menu rows, account choices, a
removable file tile and a command snippet. Segmented actions, selectable
repository rows, an empty state and a usage card are included as well. The
shared kit wraps explanatory copy and makes the whole row a touch target. The file removal is local to the
gallery; the command snippet is a visual specimen. It
uses the shared control kit and a separate reference skin; it is not a product
skin or a pixel-parity qualification. Its colors are approximate and its text
still uses the gallery atlas. Select it from Gallery options on iPhone or from
the gallery navigation on the web. The reference section retains its color
even when the rest of the phone gallery is monochrome.

The motion lab has replay, pause, reduced motion, three visual skins and tap
reversal. It adds reusable corner frames, activity/progress arcs, spring
switches and drawer/reply surfaces. The existing four-skin controls/Markdown,
HUD cards and 25 ARWES specimens remain selectable. GPU arcs now lower to at
most 64 existing antialiased segments; this is a bounded approximation.

This is an inspectable library release, not completion of all 28 Arcade
animation cases. Image textures, general transforms/blur, inline GPU islands,
multiple live world views and complete GPU semantic/text parity remain in
[rendering.md](rendering.md). Native gallery code is installed code: changes
to its compiled components require a build. Portable app screens and their
Settings link continue through signed Updates. Gallery controls are accessible
through the native menu; per-primitive gallery text selection and VoiceOver
semantics are not qualified.

## Mobile application port (1087)

The default iOS application now uses the shared Arcade GPU chrome, beyond the
gallery. `native/scene.coax` consumes stable keys, actual platform-measured
rectangles, ancestor clips and control states. It paints the existing kit's
frames, composer pill, round action, activity arcs, spring progress and rules
through the common GPU backend. `skin/arcade.coax` supplies a white/grayscale
profile. The native gallery also enforces grayscale, including its alternate
skins; the web gallery retains its reference palettes.

`NativeArcadeSurface` is one transparent Metal carrier per presentation surface,
not a canvas per button. UIKit supplies text shaping/selection, SF Symbol glyphs,
text input/IME, OTP AutoFill, measured layout, scrolling, hit targets and
accessibility. Native component wrappers contain the semantics and platform
effects; they no longer draw iOS frames with CAShapeLayer. AppKit/GTK keep their
existing adapters. This is a component port with native text services, not a
claim that every character is rasterized by the GPU.

Login, the chat composer/transcript/tool rows, drawer, Projects, Usage, Settings,
wallet extras, world library/create/edit forms, floating world menu, progress,
update offer and component-gallery navigation all use this path. Authored game
scenes keep their own colors. Entry, press/focus, two-arc activity and progress
spring motion are shared Coax; reduced motion resolves to stable states.
The remaining A01–A28 work and performance qualification stay in rendering.md.
