# Arcade audit 06: animation parity and Coax Native rendering

2026-11-14, #13607. Owner request: preserve **every animation** in Arcade as
a reusable Coax component, including dormant/demo effects, and assess GPU
versus native controls for iOS, Android, HTML pages and several inline worlds.
This is a source audit and architecture recommendation; no port, performance
benchmark, binary or OTA is delivered by this document.

Sources: read-only Arcade `3f10e1ebf84658411af12c8ff3527e34eb34a5fd`, arclib
`542ecec`; OpenAgents `96825b529e`. No Arcade install/build scripts were run.
Arcade code paths below are relative to its `app/`; `assets/` paths are
relative to the clone root. The executable design
requirements belong to [components/rendering.md](../../components/rendering.md).

## What Arcade actually uses

The manifest declares React Native 0.71.8, Reanimated `^3.3.0` and React
Native Skia `^0.1.193`, plus Gesture Handler, React Navigation, bottom sheets
and native media. These are manifest ranges, not proof of installed versions.
The same revision's `yarn.lock` resolves Reanimated **3.4.0**, Skia
**0.1.197**, Gesture Handler **2.9.0**, native-stack **6.9.13** and bottom
sheet **4.4.7**. Use those versions for reference traces, not today's defaults.
All directly imported `Animated` objects found in app code come from
**Reanimated**, not React Native's core `Animated`. Some Skia components use
its older `useTiming`, `useSpring`, `runTiming` and value APIs. Other effects
use JS timers, native transitions or an animated image.

Reanimated worklets evaluate values/styles on a CPU UI runtime rather than
the app's React JS thread. Drawing/compositing is a separate job performed by
native views or Skia and ultimately the graphics stack. `Animated.View`
does not mean application logic or spring integration executes on a GPU.
The [Reanimated 3.x glossary](https://docs.swmansion.com/react-native-reanimated/docs/3.x/fundamentals/glossary/)
and [worklets documentation](https://docs.swmansion.com/react-native-reanimated/docs/guides/worklets/)
describe that distinction. It is not evidence that GPU UI is faster than UIKit.

## Complete source animation catalog

Each row is a port acceptance case. **Live** means wired into the reference
app; **dormant** means code present but unreachable. Shared wrappers are
listed even when they reuse another row's animation. Platform/library-owned
behavior is recorded without inventing its unspecified curve or duration.
All rows need a runnable gallery specimen, normal and reduced-motion cases,
stable semantic IDs and interruption/lifecycle checks. Product inclusion is
separate: a dormant tab-bar specimen does not add tabs to our app.

Pinned Reanimated 3.4.0 source supplies the otherwise implicit defaults:
[`withTiming`](https://raw.githubusercontent.com/software-mansion/react-native-reanimated/3.4.0/src/reanimated2/animation/timing.ts)
uses 300 ms and in/out quadratic easing. This applies to the unspecified
return, reply-bar, switch and menu timing calls; an explicit duration keeps
that easing unless the caller overrides it.
[`withSpring`](https://raw.githubusercontent.com/software-mansion/react-native-reanimated/3.4.0/src/reanimated2/animation/spring.ts)
defaults to mass 1, damping 10, stiffness 100, velocity 0, overshoot unclamped,
rest displacement 0.01 and rest speed 2; its per-frame delta is capped at
64 ms. A10 overrides mass/clamping; A14 uses the defaults. Its 2000 ms
duration default is not a two-second limit for these physics-configured calls:
duration mode activates only with the relevant caller configuration. These
are source facts, not measured device traces, and must not be substituted
for Skia's separate spring/timing or the native navigation/sheet curves.

| ID | Source and reachability | Motion / implementation read in source | Shared port requirement |
| --- | --- | --- | --- |
| A01 | `components/Frame/Frame.tsx`, `AnimatedRectBorder.tsx`, `FrameSquare.tsx`, `Scaler.tsx`; dormant demos/tab bar | Reanimated `withTiming`, 200 ms, bridged to Skia values; four corner squares scale around their corners; horizontal/vertical border segments scale from their stated origins | Transform origins and independently animated frame parts, enter and exit; preserve source geometry in reference specimen rather than substituting a static generic frame |
| A02 | `Frame/Frame.tsx`; same reachability | Skia `runTiming`: highlight 0.2 to 0.7 over 200 ms; fill alpha = max(scale × highlight, always-background ? 0.1 : 0) | Animated fill/opacity, reversal from current value; retain the 200 ms transition rather than the earlier plan's immediate fill step |
| A03 | `ArwesButton/index.tsx`; dormant | Gesture tap begin/release changes A02 highlight; invokes app callback through `runOnJS`; max tap duration 5000 ms | Shared button behavior plus A01/A02, cancellation and exactly one action; no app callback inside a shader |
| A04 | `TextInput/index.tsx`; dormant | Focus/blur toggles A01 corner visibility while `alwaysShowBorder` and `alwaysShowBackground` keep border/fill | Animated focus chrome with a real platform/DOM editor; preserve composition while focus visuals change |
| A05 | `ActivityIndicator/index.tsx`, `AnimatedArc.tsx`; live small variant | Skia `useTiming`, 500 ms linear loop; two 90-degree arcs; rotation = progress × pi; blurred ring behind an opaque inner circle | Arc geometry/rotation and mask blur; do not replace with a stock spinner and call it complete |
| A06 | Same files; large variant available, distinct live use not established | Additional opposite-direction outer arcs, radius multiplier 1.8; Skia looping spring mass 1.5, initial velocity 100, damping 15 | Separate spring-loop specimen with recorded reference values; unused `showOpacityAnimation`/`showScaleAnimation` props do not imply additional animations |
| A07 | `SwipeableItem/index.tsx`; live DMs/channels | Gesture pan activates outside ±10 px, clamps to the permitted ±100 px direction; translates message; `withTiming(0)` on release | Shared gesture capture, scroll arbitration, direction/RTL cases, bounded return motion; source uses the library's default timing where unspecified |
| A08 | Same; live | Reply icon fades via `withTiming` above progress 0.3 and translates by message translation / 10 | Independent opacity and parallax track; no per-frame app/host round trip |
| A09 | `AnimatedDonut/index.tsx`, `SwipeableItem`; live | Skia round-cap circular path trim; sweep end = progress squared, radius 17.5, stroke 3 | Arc/path trim primitive, including zero/full sweep and antialiasing at small size |
| A10 | `SwipeableItem`; live | On reaching full swipe: spring alpha to 0, scale to 2, mass 0.5, overshoot clamped; reset latch when progress reaches 0; reply action on release at full progress | Independent gesture and spring state, one completion, cancellation/reversal; preserve visual completion timing without duplicating reply submission |
| A11 | `DirectMessageReply.tsx`; live | `withTiming` progress controls opacity and height up to 60 px; collapses on clear | Animated layout plus opacity, preserving transcript scroll anchor and keyboard docking |
| A12 | `ReText/index.tsx`, reply bar; live | `useAnimatedProps` changes noneditable TextInput text from shared values, bypassing a React rerender | Update glyph/content bindings when content changes; no fake typing animation or separate text editor |
| A13 | `ScreenWithSidebar.tsx`; live | Gesture Handler slide drawer, width 70, open/close speed 2; progress updates while dragging and `withTiming` 250 ms on settling | Drawer/panel transition in shared motion; existing OpenAgents drawer size/layout stays; original 70 px retained in reference specimen |
| A14 | `DrawerIconButton.tsx`; live | Reanimated spring morphs three bars into an arrow; widths 18 to 12/16, shifts/margins, rotations ±45 degrees, color interpolation | Vector/icon morph plus grayscale token interpolation. Drawer also writes the shared progress: normalize one owner instead of copying competing writers |
| A15 | `screens/HomeMessagesScreen.tsx`; live | Reanimated `FadeIn.delay(100 × index).duration(800)` | Keyed row entry and stagger, bounded realization; new messages must not restart all visible fades |
| A16 | `components/Toggle.tsx`; live switch; checkbox/radio variants available | Check/radio on-layer opacity via `withTiming`; switch on-fill opacity and knob start/margin via `withTiming` | Shared switch/choice motion and layout-relative positions; state/value accessible without depending on color |
| A17 | `TouchableFeedback.tsx`; live reply cancel | Tap worklet drives background-color `withTiming`, 100 ms; finalize restores it; action on touches-up | Interpolated pressed fill, release/cancel behavior, high-contrast and reduced-motion alternatives |
| A18 | `BlurredPopup/BlurredPopupProvider.tsx`, `TouchablePopupHandler.tsx`; live on profile edit | Long press at 500 ms with haptic; snapshot background drawn at blur radius /3 and full radius; radius 0 to 10 in 200 ms; sharp lifted child; menu opacity via `withTiming`; close delays blur-out 200 ms | Owned offscreen background, two blur layers, sharp child, menu input fence, focus restore and light haptic. Preserve blur rather than replacing the effect with a generic menu |
| A19 | `navigators/TabBar.tsx`; dormant | Frame visible after 250 ms; A01 enters it; tabs fade after 500 + 50 × index ms over 300 ms | Staggered toolbar/tab specimen; reusable motion does not require changing drawer navigation |
| A20 | `BackgroundGradient.tsx`; dormant | Reanimated repeat/reverse, radius 0 to 10 over 2000 ms indefinitely, copied to Skia BlurMask over a sweep-gradient rounded rectangle | Sweep gradient plus blur/pulse, grayscale profile and original reference palette; pause offscreen |
| A21 | `screens/SplashScreen/SplashScreen.tsx`, `HudNavigator.tsx`; dormant | JS delay 500 ms, blur increases by 2 every 20 ms; wrapper removes splash at 2000 ms | Image blur timeline and exit/removal. Source closes over initial blur: its `blur < 300` check does not reliably cap recursion. Port intended bounded behavior, record deviation, never copy timer bug |
| A22 | `Faerie/AnimatedTyping.tsx`; dormant | Initial delay 50 ms, nominal one character per 1 ms timer, cursor toggles every 250 ms, completion clears cursor | Shared text reveal/caret timeline; reveal complete graphemes using the shared text helpers. Record Unicode adaptation from JS code units; no 1000 Hz rendering/timer loop |
| A23 | `Faerie/Message.tsx`, `assets/images/typing-message.gif`; live loading row | Animated-image playback. Asset metadata inspected: 19 frames, each 50 ms, 950 ms cycle | Decoded/cached image frames with source cadence for parity; a procedural replacement needs explicit visual equivalence. The app can use its existing quieter waiting indicator separately |
| A24 | `screens/ProfileScreen.tsx`; live | Copy confirmation switches to “Copied” and resets after 500 ms | Timed transient state, not a shader effect; cancellation on unmount, one accessibility announcement |
| A25 | `navigators/AppNavigator.tsx`, `AuthNavigator.tsx`; live | React Navigation native-stack `slide_from_right` | Portable route enter/exit specimen and back/cancel behavior; platform presentation may differ only as a declared adapter, not silent omission |
| A26 | `BottomSheetModal` consumers: Invite, AddContact, RelayManager; OfferForm dormant | Library-owned drag/snap transitions at source detents (35/36, 50, 75, 100 percent depending on screen) | Shared pane/sheet animation, drag interruption, keyboard/scroll arbitration and modal focus; source app does not specify a spring curve |
| A27 | `Button`, `ListItem`, `TouchableOpacity` consumers and recording controls; live | Pressed opacity/fill states, some library-owned opacity transitions (0.8/0.9); native ActivityIndicator in several screens | Shared pressed and busy motion variants; distinguish immediate state changes from animations and retain both |
| A28 | `Screen.tsx`, tab navigator, message forms; live | Platform keyboard avoidance, tab hide-on-keyboard, scrolling; source does not specify the OS keyboard curve | Follow native keyboard/inset observations and preserve scroll position. Do not replace OS keyboard motion with a guessed spring |

Static companion effects also need paint support: `Spotlight.tsx` is a radial
gradient, the title wordmark has a static glow, and `CityBackground.tsx` has
a dimmer. They are not additional animated timelines. The initial app boot
splash's delayed hide is not A21's unreachable Skia blur splash. Preserve
source assets/provenance/licenses in reference fixtures; production mono
tokens and the OpenAgents mark remain the product design.

The audit covered imports, animation/value/gesture calls, timers, navigation,
sheets and the GIF, not just filenames containing “Animated”. Arcade has no
reduced-motion handling. Add it on every target. Older audit wording that
claims A21 certainly stops at blur 300 is corrected here by reading the closure.
Skia/library-owned transitions and the pinned defaults still need numeric
traces or captured reference frames before claiming exact parity;
this source-only audit cannot certify their device behavior.

## Rendering assessment

Recommend **GPU visuals plus native services**, using the existing component
layers, GPU primitives and generated shaders. UIKit and Android remain hosts
for input methods, accessibility and system UI. They do not each get a new
implementation of the permanent component visuals and animation curves.
DOM remains the web text/form backend, with explicit GPU islands for custom
components, effects and world views.

The CPU evaluates layout, interaction and motion; GPU work covers primitive
drawing, transforms, filters and composition. The performance advantage we
seek is bounded incremental work and avoiding app-level per-frame bridge/tree
updates, not moving a handful of spring equations into compute shaders.
Both speed and power benefits must be measured against the current native UI.

This fits the existing architecture, but is not already available end to end:

- The first gallery now renders arcs through bounded shared GPU segments
  and includes A05/A09-inspired activity/progress specimens. Images remain
  unsupported. Full A05/A09/A21/A23 parity, sweep/radial gradients, transform
  groups and background/image blur need more than the box-shadow primitive.
- `core/arwes/motion.coax` supplies much of easing/scheduling/text reveal;
  `core/spring.coax` now adds analytic damped springs with interruption. These
  still do not form a complete Reanimated-style spring/gesture runtime.
- The production iOS app uses a UIKit/JSC projection; 1086 adds a shared Metal
  component gallery in Settings. This qualifies an inspectable visual carrier,
  not full chat/IME/semantics parity. Existing Vulkan Android drawing is not
  proof of text input, TalkBack or mobile app parity.
- The native world host has `phCurrent` with a one-page-at-a-time comment.
  Three inline worlds need explicit per-view state, async callbacks and
  resource scheduling. Three view-controller allocations do not solve it.
- GPU components must emit semantic and text geometry alongside pixels.
  Otherwise this repeats the usability problems of a canvas-only UI.

## Changes to the earlier audit and plan

The fixed source descriptions in 01 to 04 stay historical. This assessment
supersedes recommendations to drop glow/blur/typing animations, replace the
custom spinner or popup with a system approximation, or build each control's
permanent appearance in Swift first. **Every A row is required in the library
and gallery**, even when a product screen uses a quieter variant.

Retain `components/` as the single library. Reconcile the current DOM-tree
and display-list component families rather than copying them again. Native
GPU visuals and browser GPU islands consume the same paint/motion definitions;
browser text retains its DOM semantics and selection. A scene view is an
engine-owned output composed with that UI, with explicit input and lifecycle.

Phase 0 now has a component/rendering foundation before the Arcade screen
rebuild: contracts, paint/effect gaps, the complete motion gallery, native
text/semantics, and three-scene composition. Nostr protocol/service work is
independent. Do not hold up routine current-app fixes, but do not ship a second
per-platform widget library as the permanent answer.

The “one TestFlight build, then Updates” objective must be earned: install
the compositor, permitted effects and text/semantic/resource contracts together,
then deliver components and animation descriptions within those capabilities.
New native effects/pipelines/contracts or new world frameworks can require
another build. No current runtime fingerprint gains these powers via a doc.

## Required evidence before the default changes

Use [the rendering stages and gates](../../components/rendering.md#delivery-order-and-acceptance).
At minimum: every A row in the shared gallery; same fixed-time values across
Metal/Vulkan/WebGPU; platform-specific image goldens; real iOS and Android
editing/AutoFill/accessibility; DOM around GPU islands; three simultaneous
scene views; scrolling and popup coverage; cancellation/background/device loss;
and CPU/GPU/input/thermal/memory measurements against the shipped UI.

Cross-origin realms keep their sandbox and typed messages. The compositor
cannot sample another origin's canvas or forward account credentials merely
to implement blur or texture sharing. The distinction between trusted same-host
scene textures and isolated realm frames is part of the acceptance test.
