# SwiftUI, Expo UI, and the Effect Native stdlib

Date: 2026-07-09
Status: analysis / platform decision input (owner-directed)
Issues: #8597 (APP-MOBILE), effect-native#70 (SwiftUI host kind / render-swiftui),
D-MB-02 (demand register row), #8572 (EN-2 catalog demand loop)
Inputs: `projects/repos/expo/packages/expo-ui` (read-only reference, @expo/ui
56.0.14, MIT), `docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md`,
`docs/mobile/2026-07-09-expo-ui-swift-ui-audit.md`,
`docs/effect-native/DEMAND_REGISTER.md`, the effect-native repo at catalog v26,
the shipped Liquid Glass island in `apps/openagents-mobile/modules/openagents-liquid-glass`
(TestFlight build 105 receipt), and the owner's ChatGPT-app Liquid Glass
design-target screenshots.

## The question this answers

The owner's ask, in his words: "I don't want to have to go completely out of my
way to do shit on Android. I like to just specify certain things and we get the
SwiftUI version if they're Swift and Android if it's not." And: "I'm hoping
there's a unified type definition that we can pull in and use as building
blocks easily."

Short answers up front, argued below:

1. **The pattern the owner wants already exists and works.** Expo's `@expo/ui`
   package is a living proof: one TypeScript component spec that lowers to
   real SwiftUI on iOS and real Jetpack Compose on Android. We read its full
   source. The mechanism is sound and we should steal its lowering mechanics.
2. **But `@expo/ui` is not the unified type definition the owner hopes to pull
   in wholesale.** Its own internal doctrine (its checked-in `CLAUDE.md`) is
   "mirror the native API shape with as little abstraction as possible" —
   which produces three deliberately *divergent* type trees (universal,
   `swift-ui`, `jetpack-compose`), with only a small lowest-common-denominator
   universal layer genuinely shared.
3. **The unified type definition the owner wants is the Effect Native catalog
   itself.** That is literally what the catalog is for: one Schema-typed
   component set, renderers below it doing per-platform lowering. Expo UI is
   the best available reference for *how the renderer lowers*, not a
   replacement for *what the components are*.
4. **Recommended path: hybrid.** Consume `@expo/ui` inside the React Native
   renderer as an implementation detail (its MIT-licensed universal components
   and SwiftUI/Compose trees become lowering targets for our catalog tags),
   while effect-native#70 grows the owned host-kind seam and eventually the
   `render-swiftui` lane. App code never imports `@expo/ui`; the catalog stays
   the contract; migration off Expo later is invisible to every screen.

## 1. SwiftUI for a reader who doesn't know SwiftUI

The owner said he doesn't know much about SwiftUI. Here is the working mental
model, in Effect Native terms, because the two systems are structurally the
same idea.

### 1.1 It is a typed declarative view tree driven by state

A SwiftUI screen is a value: a tree of typed view structs (`VStack`, `Text`,
`Button`, `List`) computed from state. When state changes, the framework
recomputes the tree and diffs it against what is on screen. There is no
imperative "set this label's text"; you re-describe the whole view and the
runtime reconciles.

```swift
struct Home: View {
  @State var taps = 0
  var body: some View {
    VStack {
      Text("Taps: \(taps)")
      Button("Ping") { taps += 1 }
    }
  }
}
```

That is exactly Effect Native's shape, one abstraction lower:

```ts
Stack({ direction: "column" }, [
  Text({ content: `Taps: ${state.taps}` }),
  Button({ label: "Ping", onPress: IntentRef("Pinged") }),
])
```

The differences that matter:

- **SwiftUI's tree is Swift code; EN's tree is Schema-validated data.** SwiftUI
  views are compiled structs with closures; EN views are serializable values
  with typed `IntentRef`s instead of callbacks. EN can log, replay, conformance-
  test, and render the same tree on four renderers. SwiftUI can only ever be
  Apple's renderer.
- **SwiftUI is Apple-only.** iOS, iPadOS, macOS, watchOS, tvOS, visionOS. There
  is no Android story at all. Its Android sibling in spirit is Jetpack Compose
  — also a typed declarative tree driven by state, also platform-locked.
- **State drives rendering in both.** SwiftUI uses `@State` / `@Observable` /
  `ObservableObject`; EN uses `SubscriptionRef` + a view stream. The Liquid
  Glass island already bridges the two: an `ObservableObject` whose
  `@Published` fields are projected from the EN program's `SubscriptionRef`.
- **Styling is modifiers, not stylesheets.** `.padding(16)`,
  `.background(...)`, `.glassEffect(...)` are chained functions that wrap the
  view. EN's typed style objects resolve to exactly this kind of modifier
  chain when lowered (the renderer audit specs the mapping table).

So when the audit says "the EN renderer's job is lowering," the concrete
meaning is: walk the EN data tree, and for each catalog node emit the
corresponding SwiftUI struct with the corresponding modifier chain — the same
job `render-dom` does with DOM elements and atomic CSS, and `render-rn` does
with RN host components (see the fd1ccc5 Button variant lowering fix for what
that looks like per-component).

### 1.2 What SwiftUI offers that we actually care about

The capability surface, filtered to what matters for OpenAgents mobile:

- **Native controls with system behavior for free.** Buttons with press
  effects, pickers (wheel/menu/segmented), toggles, sliders, steppers, date
  pickers, `List` with swipe actions and reordering, pull-to-refresh, context
  menus with previews, share sheets, confirmation dialogs, searchable lists.
  Every one respects Dynamic Type, VoiceOver, dark mode, and locale without
  extra work. This is the fidelity gap RN components always chase.
- **Liquid Glass and materials (iOS 26).** The design system the owner's
  ChatGPT screenshots show. First-class API: `.glassEffect(.regular.tint(...),
  in: .capsule)` on any view, `.buttonStyle(.glass)` / `.glassProminent`,
  `GlassEffectContainer` to make neighboring glass shapes blend and morph into
  each other, `glassEffectID` + `@Namespace` for morph transitions when glass
  controls appear/disappear. Pre-26 devices fall back to `.ultraThinMaterial`
  blur — our shipped island already does exactly this two-tier treatment.
- **Presentation machinery.** `.sheet` with `presentationDetents` (the
  ChatGPT-style bottom sheets), `.popover`, `.fullScreenCover`, `.alert`,
  `.confirmationDialog`, `Menu`, `ContextMenu`. These are windowing-system
  level behaviors that are painful to fake in RN.
- **Navigation chrome.** `NavigationStack`/`NavigationSplitView` (sidebar +
  detail — the structural pattern behind flyout-style navigation), `.toolbar`
  placements, `TabView`, `.searchable`. Note: in an RN-hosted app *we* own
  navigation, so this tier is mostly out of reach inside islands (see 1.3).
- **Animation.** `withAnimation`, spring physics, `matchedGeometryEffect`
  (shared-element morphs — how ChatGPT's pill controls melt into each other),
  content transitions, symbol effects on SF Symbols.
- **SF Symbols.** ~6,000 system icons with weights, palettes, and built-in
  animation, addressable by string name. The icon language of the ChatGPT look.
- **Charts, Gauges** (`Swift Charts`, `Gauge`) for dashboard-ish surfaces.
- **Widgets, Live Activities, App Intents.** Home-screen widgets, lock-screen
  Live Activities (a fleet-run progress ticker on the lock screen is an
  obvious future), Siri/Shortcuts entry points. These are SwiftUI-only — RN
  cannot render them at all because they run in separate system processes.

### 1.3 What is reachable from an RN-hosted app vs full native

Our mobile app is Effect Native on an RN/Expo host (#8597 locks this). The
boundary rules:

- **Reachable from RN-hosted islands:** everything in a bounded subtree —
  glass effects, native controls, sheets, menus, animations, SF Symbols. The
  mechanism is a `UIHostingController` mounted inside a UIKit view that RN
  lays out (Expo's `Host`, our island's `ExpoView`). Yoga/flexbox sizes and
  places the island; SwiftUI owns everything inside it.
- **Awkward from islands:** whole-screen navigation transitions, toolbar
  morphing tied to `NavigationStack`, `.searchable` pinned to a system nav
  bar. RN owns the navigation container, so system-level chrome behaviors
  only work if an entire screen (or the whole app shell) becomes one SwiftUI
  surface — which is precisely what the `render-swiftui` lane (effect-native
  ROADMAP Phase 5, audit EN-S0..S6) would eventually give us.
- **Unreachable from RN entirely:** WidgetKit widgets, Live Activities,
  watchOS. These need separate native targets regardless of framework
  (Expo's config-plugin ecosystem can scaffold the targets, and expo-ui is
  already growing Live Activity modifiers like `activityBackgroundTint`, but
  the widget UI itself is Swift). When we want a fleet Live Activity, it is a
  small owned Swift target fed by the same typed state, not an EN renderer
  problem.

## 2. Expo UI dissected: the proof the dream works

`@expo/ui` 56.0.14 (MIT, 650 Industries; version rides the Expo SDK train) is
the closest existing thing to "specify once, get SwiftUI on iOS and Compose on
Android." We read the full package. Structure:

| Entry point | Platform | Contents |
| --- | --- | --- |
| `@expo/ui` (`src/universal/`) | iOS + Android + web | ~20 cross-platform components with one shared type per component |
| `@expo/ui/swift-ui` (`src/swift-ui/`) | iOS only | ~50 components mirroring SwiftUI's own API |
| `@expo/ui/swift-ui/modifiers` | iOS only | ~90 typed SwiftUI modifier factories |
| `@expo/ui/jetpack-compose` (`src/jetpack-compose/`) | Android only | ~55 components mirroring Material 3 Compose |
| `@expo/ui/jetpack-compose/modifiers` | Android only | Compose modifier factories |
| `@expo/ui/community/*` | both | 8 drop-in replacements for popular RN libraries (bottom-sheet, datetime-picker, menu, picker, slider, segmented-control, pager-view, masked-view) |

### 2.1 The universal layer is the owner's dream, implemented

Each universal component is one shared TypeScript type plus three files:
`index.ios.tsx` (lowers to the `swift-ui` tree), `index.android.tsx` (lowers
to the `jetpack-compose` tree), `index.tsx` (web/RN fallback). One spec, two
native lowerings, chosen automatically by Metro's platform resolution. The
universal `Button` is the canonical example:

```ts
// shared types.ts — ONE definition
export type ButtonVariant = 'filled' | 'outlined' | 'text';
export interface ButtonProps extends UniversalBaseProps {
  children?: React.ReactNode;
  label?: string;
  onPress?: () => void;
  variant?: ButtonVariant;
}
```

```tsx
// index.ios.tsx — lowers variant to a SwiftUI buttonStyle
const variantButtonStyle = {
  filled: 'borderedProminent', outlined: 'bordered', text: 'plain',
};
return <SwiftUIButton onPress={onPress} label={label}
  modifiers={[buttonStyle(variantButtonStyle[variant]), ...universalModifiers]} />;
```

```tsx
// index.android.tsx — lowers variant to a Material 3 component choice
const variantComponentMap = {
  filled: ComposeButton, outlined: OutlinedButton, text: TextButton,
};
return <ButtonComponent onClick={onPress} enabled={!disabled}>{content}</ButtonComponent>;
```

This is *structurally identical* to Effect Native's catalog-and-renderers
split, one level up the stack: the universal `ButtonProps` is the "catalog
entry," and the platform files are the "renderers." Note it is even the same
per-component decision our own fd1ccc5 fix made in `render-rn` — a `variant`
in the shared contract, a platform-appropriate visual lowering below it.

The shared style surface is deliberately narrow. `UniversalStyle` is a
`Pick<ViewStyle>` of exactly the properties that map cleanly to both SwiftUI
modifiers and Compose modifiers: padding*, backgroundColor, borderRadius/
Width/Color, opacity, width, height. Anything richer goes through the
`modifiers?: ModifierConfig[]` escape hatch, which accepts *platform-specific*
modifier factories. That narrowness is honest, and it is the same conclusion
EN's typed style tokens already embody.

### 2.2 How the native lowering works (both platforms, same pattern)

Per component, both platforms follow one shape — TypeScript wrapper calls
`requireNativeView('ExpoUI', 'Button')`; native side declares a typed props
class and a declarative view that reads it:

```swift
// iOS: props class + SwiftUI view (ios/Button/)
open class ButtonProps: UIBaseViewProps, Observable {
  @Field public var label: String?
  @Field public var systemImage: String?
  @Field public var role: ButtonRole?
  var onButtonPress = EventDispatcher()
}
public struct Button: ExpoSwiftUI.View {
  @ObservedObject public var props: ButtonProps
  public var body: some View {
    SwiftUI.Button(props.label ?? "", role: props.role?.toNativeRole()) {
      props.onButtonPress()
    }
  }
}
```

```kotlin
// Android: Record props + composable (android/.../button/Button.kt)
class ButtonColors : Record {
  @Field val containerColor: Color? = null
  @Field val contentColor: Color? = null
}
@Composable fun FunctionalComposableScope.ButtonContent(props, onClick) {
  androidx.compose.material3.Button(onClick, colors = props.colors.compose) {
    Children(UIComposableScope(rowScope = this))
  }
}
```

Cross-cutting machinery worth stealing:

- **`Host` boundary.** RN lays out one UIKit/Android view; inside it a
  `UIHostingController` (iOS) or `ComposeView` (Android) renders the native
  subtree. Children under `Host` are *virtual* nodes, not one UIKit wrapper
  per leaf. `matchContents` reports measured native size back into RN's
  shadow tree. `RNHostView` is the inverse door (RN views inside a native
  subtree).
- **Modifiers as data.** `padding({all: 16})` returns `{$type: 'padding',
  all: 16}`; a native `ViewModifierRegistry` maps `$type` strings to real
  `ViewModifier`s applied in order. ~90 iOS modifiers are exposed, including
  the full Liquid Glass set: `glassEffect({glass: {variant: 'regular' |
  'clear' | 'identity', interactive, tint}, shape})`, `glassEffectId`,
  `buttonStyle('glass' | 'glassProminent')`, `buttonBorderShape`, plus
  `matchedGeometryEffect`, presentation detents, list row styling, and a
  broad accessibility set.
- **`useNativeState`.** A shared observable object both JS and native can
  read/write, so keystrokes and slider drags don't round-trip through the JS
  thread. Conceptually EN's `SubscriptionRef` doing the same job; theirs
  needs `react-native-worklets` for synchronous UI-thread writes.

### 2.3 Are the types complete and unified? Honestly: no — by design

This is the crux for the owner's "unified type definition I can pull in" hope.

- The **universal layer is genuinely unified** but small: Host, Row/Column,
  Text, Button, Switch, Checkbox, Slider, Picker, BottomSheet, TextInput,
  List/ListItem, Collapsible, FieldGroup, Icon, ScrollView, Spacer,
  RNHostView, useNativeState. Roughly twenty components.
- The **platform trees diverge on purpose.** expo-ui's own `CLAUDE.md`
  doctrine is to mirror native API shapes with minimal abstraction. So iOS
  gets `Form`/`Section`/`Gauge`/`Chart`/`SwipeActions`/`GlassEffectContainer`/
  `TabView`/`ContextMenu`; Android gets `Card`/`Chip`/`Surface`/
  `NavigationBar`/`Carousel`/`FloatingActionButton`/`Snackbar`/
  `SegmentedButton`/`PullToRefreshBox`. Even shared concepts split shapes:
  iOS expresses button variants as a `buttonStyle` modifier including
  `'glass'`; Android expresses them as five distinct Material components.
  Universal `Picker`'s `appearance: 'wheel'` silently degrades to a dropdown
  on Android; `BottomSheet` fractional snap points degrade to half/full.
- **Maturity:** no in-repo alpha warnings (those live on the docs site), but
  the CHANGELOG shows high churn — breaking changes routinely per minor,
  rapid patch cadence, iOS 26 features landing weekly. The type surface is
  fast-moving and not frozen. It is SDK-locked (56.x ⇒ Expo SDK 56), so
  adopting it pins upgrade cadence to the SDK train we already ride.
- **License:** MIT (repo root, 650 Industries). No obstacle to depending on
  it or to porting its patterns into owned code with attribution.

Conclusion: expo-ui **proves the lowering pattern** and ships useful typed
building blocks, but the "unified definition" only exists at its
lowest-common-denominator universal layer. The richer trees are platform
mirrors, exactly what the EN renderer audit warns against adopting as public
API ("if we copy Expo's public JSX/modifier surface, we recreate the framework
coupling Effect Native exists to avoid").

## 3. The mental model: how it all relates

One table, since this is the part the owner asked to wrap his head around:

| Layer | SwiftUI world | Compose world | Expo UI | Effect Native |
| --- | --- | --- | --- | --- |
| Unified component spec | — (Apple-only) | — (Android-only) | `src/universal/*` types (small) | **The catalog** (`@effect-native/core`, v26) — this is ours and it is the point |
| Per-platform lowering | `body: some View` | `@Composable` | `index.ios.tsx` / `index.android.tsx` | Renderers: `render-dom`, `render-rn`, `render-canvas`, future `render-swiftui` |
| State → view | `@State`/`Observable` | `remember`/state | React props + `useNativeState` | `SubscriptionRef` + view stream |
| Events | closures | lambdas | callbacks + `EventDispatcher` | **typed `IntentRef` → `IntentRegistry`** (serializable, loggable, replayable) |
| Styling | modifier chain | `Modifier` chain | `$type` modifier records | typed style objects + tokens, lowered by each renderer |
| Foreign view door | `UIViewRepresentable` | `AndroidView` | `Host` / `RNHostView` | `Host({kind, props, onEvent})` — closed registry, currently six kinds |

Two readings of that table:

1. **SwiftUI relates to EN as a target, not a rival.** SwiftUI and Compose are
   both "typed declarative view trees driven by state" — the same species as
   EN's catalog — but each is platform-captive. EN's catalog is the
   renderer-independent statement of the same idea; a SwiftUI renderer's whole
   job is the mechanical walk from catalog node to SwiftUI struct, exactly
   like expo-ui's `ios/` directory does per component today.
2. **Expo UI relates to EN as a sibling one level up.** Their universal layer
   is a JSX-and-callbacks catalog; ours is a data-and-intents catalog. Theirs
   trades governability for developer familiarity; ours trades familiarity
   for replay, conformance, and agent-safety. Their native lowering machinery
   (Host mounting, virtual children, props classes, modifier registry) is
   excellent and largely reusable *below* our contract.

## 4. The cross-platform dream, honestly assessed

Can we "specify once, get SwiftUI on iOS and Compose on Android"? **Yes, with
one honest qualifier.** Expo-ui demonstrates the mechanism end to end in
production code, MIT-licensed, with TypeScript types for both lowerings. The
qualifier: a single spec only lowers cleanly where the platforms share a
concept. Buttons, toggles, sliders, pickers, lists, sheets, text fields,
progress — clean. Liquid Glass specifically is an *Apple* design language:
Compose has no glass API, so the Android lowering of `surface: "glass"` is an
approximation (Material 3 tonal surface + blur/scrim), not the same pixels.
That is fine — it is exactly the owner's stated expectation ("we get the
SwiftUI version if they're Swift and Android if it's not") — but the spec
must say "glass surface" (semantic), never `.glassEffect` (Apple API), or
Android becomes a lie.

### The three options, ranked

**Option 1 — consume `@expo/ui` directly as the EN native lowering layer.**
`render-rn` lowers catalog tags to `@expo/ui` universal components (and
per-platform trees where needed) instead of bare RN `Pressable`/`View`.
*Fastest to native fidelity*: their types become our building blocks, glass
buttons and native sheets arrive this week, and because the dependency lives
inside the renderer, no app code ever sees it. Risks: SDK-locked churn and
routine breaking changes land in our renderer's lap; their universal layer's
shape (JSX children, callback events, `ObservableState`) has to be adapted to
EN's data/intent model at every seam; and their LCD layer misses things we
need (drawer, toolbar, icon button as a first-class thing on iOS).

**Option 2 — mirror the pattern inside effect-native (the stdlib play).**
Build owned expo-module-style native views per component family — our Swift
props classes, our composables — with the EN catalog as the single type
definition, per the renderer audit (EN-S0..S6: generate Swift types from the
catalog Schema, exhaustive decoding, conformance suite). *The right long-term
shape*: fully owned, catalog-governed, no third-party churn, generated rather
than hand-mirrored. Cost: it is real native engineering across two platforms,
and the audit itself says to start it only when a real screen demands native
fidelity — building fifty components speculatively would be catalog rot.

**Option 3 — hybrid (recommended).** Adopt `@expo/ui` now, strictly *behind*
EN typed lowerings, and migrate to owned lowerings as effect-native#70 and the
EN-S lanes mature:

- The **catalog stays the only public contract.** New components (Toggle,
  Slider, Picker, Menu, IconButton, Toolbar, Drawer — section 5) enter through
  the normal demand loop (#8572 register → upstream GAPS → version bump), with
  typed props, intents, and style — never expo-ui prop shapes.
- **`render-rn` lowers catalog tags to `@expo/ui` components on native**, the
  way it lowers to RN host components today. `Button({variant})` maps to their
  universal Button (or `swift-ui` Button + `buttonStyle('glass')` when the
  resolved style says glass); `Sheet` maps to their BottomSheet;
  `List`/`SectionList` rows to their List/ListItem. Web and desktop renderers
  are untouched.
- **Expo churn is firewalled** at one layer: when `@expo/ui` breaks, we fix
  renderer internals; no screen changes. When the owned `render-swiftui` (or
  per-family native host kinds via the #70 driver seam) lands, we swap the
  lowering component-by-component and *delete* the expo-ui path —
  convert-and-delete, the same discipline as every EN migration.
- **License is clean** (MIT) and we already ship Expo modules in this app, so
  the dependency adds no new platform commitment.

This is also what the evidence supports: the renderer audit's recommendation
("use Expo's `UIHostingController` and virtual-child lessons... RN renderer
remains the shipping mobile path"), the demand-register discipline (no
parallel primitives — expo-ui inside the renderer is a lowering detail, not a
primitive), and the owner's velocity requirement (the ChatGPT-look home screen
should not wait for a generated Swift renderer).

Migration path in one line: **catalog contract now → expo-ui lowering now →
#70 host-driver seam next → owned render-swiftui per EN-S0..S6 → expo-ui
deleted.** Each step is invisible above the renderer line.

## 5. The concrete type surfaces for the EN stdlib

What should the catalog grow, informed by expo-ui's shipped surface and
prioritized for our actual product screens — the ChatGPT-look design target
(glass pills, nav flyout, Recents, floating composer), Sarah mobile home,
fleet supervision, and approvals?

Current catalog (v26) already has: Stack, Text, Button, Image, TextField,
List, SectionList, Card, Spacer, Link, Modal, Sheet, Transcript, StatusBanner,
GraphFigure, marketing/pager extensions, and `Host` with six kinds
(`code-editor`, `terminal`, `canvas`, `voice-input`, `on-device-model`,
`media-video`). The gaps below are ranked. "expo-ui" column says what exists
to lower onto today; SwiftUI/Compose columns say the eventual owned lowering.

### P0 — the ChatGPT-look structural set (demanded by the mobile Home)

| Proposed EN surface | expo-ui today | SwiftUI lowering | Compose lowering | Notes |
| --- | --- | --- | --- | --- |
| `Button` glass variant (style token `surface: "glass"` + existing `variant`) | universal Button + `buttonStyle('glass'/'glassProminent')` (iOS 26+), capsule via `buttonBorderShape` | `.buttonStyle(.glass)`, `.tint`, capsule shape; `.ultraThinMaterial` fallback pre-26 | Material 3 tonal/elevated button; blur approximation only | The "Chat" pill with icon. Semantic token, never an Apple API name in the contract |
| `IconButton` (new tag: `icon`, `accessibilityLabel`, `onPress`, shape) | swift-ui Button + `systemImage` + glass + circle shape; compose `IconButton` exists first-class | `Button` + `Image(systemName:)` + `.glassEffect(in: .circle)` | `IconButton`/`FilledIconButton` | The circular search/settings buttons. Needs a typed icon vocabulary (SF Symbol name on iOS, Material Symbol on Android) — expo-ui ships `sf-symbols-typescript` for exactly this |
| Glass surface/container (style capability, not a component: `surface: "glass"` on Stack/Card + optional `GlassGroup` for morphing) | `GlassEffectContainer` + `glassEffect`/`glassEffectId` modifiers | `GlassEffectContainer`, `.glassEffect(_:in:)`, `glassEffectID` for morphs | no equivalent; `Surface` + haze-style blur/scrim | The layered glass-over-content depth look. Container morphing (pills melting together) is iOS-only sugar; contract must degrade honestly |
| `Drawer` / nav flyout (new tag: edge, open state via intent, scrim) | **not shipped** in any tree | owned: overlaid glass panel (`.glassEffect` panel + offset animation); full-native later via `NavigationSplitView` sidebar | Material 3 `ModalNavigationDrawer` (in Compose proper, not expo-ui) | The ChatGPT left flyout. First implementation is honest EN composition (animated overlay Stack + List) with the panel itself glass-styled; a native host kind only if gesture fidelity demands it |
| `ListItem` row contract (leading icon, label, trailing accessory, `selected` state) | universal List/ListItem; swift-ui `List`+`Label`; compose `ListItem` | `List` row with `Label`, `.listRowBackground` for selected highlight | `ListItem` with `leadingContent` | The flyout rows (icon+label, selected-row highlight) and the Recents list. Extends existing List/SectionList rather than a parallel list |
| `Toolbar` / floating composer (composition contract: glass capsule containing TextField + IconButtons) | swift-ui HStack+TextField+glassEffect; compose `HorizontalFloatingToolbar` exists first-class | `HStack` in `.glassEffect(in: .capsule)`; `matchedGeometryEffect` for expand-on-focus | `HorizontalFloatingToolbar` | The floating composer bar with embedded mic/voice controls. Mic button dispatches through the existing `voice-input` host kind — the composer chrome is catalog; the audio capture is Host |
| `Sheet` native lowering upgrade (detents: half/full/fraction) | universal BottomSheet (`snapPoints: 'half'\|'full'\|{fraction}\|{height}`) | `.sheet` + `presentationDetents`, `presentationBackground` for glass sheets | `ModalBottomSheet` (no fractional snap) | Catalog `Sheet` exists; this is a lowering-fidelity upgrade plus a detents prop. Approvals live here |

### P1 — Sarah home, fleet supervision, approvals

| Proposed EN surface | expo-ui today | SwiftUI lowering | Compose lowering | Notes |
| --- | --- | --- | --- | --- |
| `Toggle` (new tag) | universal Switch (`value`, `onValueChange`, `label`) | `Toggle` | `Switch` | Approval gates, settings. Intent-dispatching, not callback |
| `Menu` / `ContextMenu` (new tag: typed items → intents) | swift-ui Menu + ContextMenu (with `.Trigger`/`.Preview`/`.Items`); compose DropdownMenu | `Menu`, `.contextMenu` with preview | `DropdownMenu` | Per-run actions (pause/drain/stop) on fleet rows; long-press on Recents |
| `Picker` incl. segmented (new tag: `appearance: 'segmented'\|'menu'\|'wheel'`) | universal Picker; community segmented-control; compose SegmentedButton rows | `Picker` + `.pickerStyle(.segmented)` | `SingleChoiceSegmentedButtonRow` | Fleet filter (running/queued/done); wheel degrades on Android per expo-ui precedent |
| `Progress` (new tag: linear/circular, determinate/indeterminate) | swift-ui ProgressView + `progressViewStyle`; compose Circular/LinearProgress + `LoadingIndicator` | `ProgressView` | `CircularProgressIndicator` etc. | Run progress on fleet cards |
| `Gauge` (new tag) | swift-ui Gauge (with value labels); no compose equivalent | `Gauge` | owned canvas/arc composition | Capacity/quota dials. Low cost given expo-ui reference, iOS-first honest |
| `Badge` (prop on ListItem/Tab rather than component) | compose Badge/BadgedBox; iOS via `badge` modifier | `.badge(_:)` | `BadgedBox` | Unread approvals count |
| `SearchField` (new tag or TextField role) | compose SearchBar/DockedSearchBar; iOS via TextField-in-glass (native `.searchable` needs nav ownership) | TextField + glass capsule now; `.searchable` under render-swiftui | `SearchBar` | The flyout's search affordance |
| `Tabs` (new tag: items → intents) | swift-ui TabView; compose NavigationBar/NavigationBarItem | `TabView` | `NavigationBar` | Only if the app shell adopts tabs; drawer-first shape may not need it |

### P2 — complete-the-set (demand-gated per #8572; do not build speculatively)

| Proposed EN surface | expo-ui today | Notes |
| --- | --- | --- |
| `DateTimePicker` | universal/community DatePicker both platforms | Scheduling runs/reminders — no current screen demands it |
| `Slider` / `Stepper` | universal Slider; swift-ui Stepper | Budget/concurrency dials |
| `SwipeActions` on ListItem | swift-ui SwipeActions; compose via gestures | Archive/approve from a row |
| `PullToRefresh` (List prop) | `refreshable` modifier (iOS); compose PullToRefreshBox | Fleet list refresh |
| `Snackbar`/toast | compose Snackbar; iOS owned overlay | Catalog `StatusBanner` may already cover it |
| `Alert`/`ConfirmationDialog` | swift-ui Alert/ConfirmationDialog; compose AlertDialog | Destructive-action confirms (stop fleet run) |
| `Chart` | swift-ui Chart (Swift Charts); no compose equivalent | Dashboards later; `GraphFigure`/canvas covers current need |
| `ShareLink`, `ColorPicker`, `Grid`, `Popover` | swift-ui only | No product demand yet |

Stdlib admission rule stays exactly the EN-2 loop: a real converting screen
names the gap → register row → upstream issue → catalog version bump →
conformance across renderers (with explicit per-renderer support limits where
a platform genuinely lacks the concept, e.g. Gauge on Android). The table
above is the pre-negotiated priority order, not a bulk import.

## 6. The ChatGPT look as a first EN screen

Mapping the owner's screenshots ("the buttons, the nav fly out, and that kind
of stuff") to a concrete composition for the mobile Home, piece by piece:

- **Layered background.** Plain EN: root `Stack` with the Protoss-blue theme
  surface; content scrolls under the chrome. No native code.
- **Top glass pill cluster** (the "Chat" pill + circular icon buttons). P0
  catalog pieces: `Button` with glass surface + `IconButton`, grouped so the
  iOS lowering can wrap them in `GlassEffectContainer` (pills blend/morph);
  Android renders tonal buttons. Until those catalog tags land, this cluster
  is the natural *second* SwiftUI island — same `UIHostingController` +
  props-projection + typed-intent shape as the shipped Liquid Glass island,
  three buttons dispatching three intents.
- **Nav flyout drawer.** EN composition, not native: an overlay `Stack`
  animated from the leading edge, glass-surfaced, containing `SearchField`
  (interim: TextField-in-glass), a `List` of `ListItem` rows (icon + label +
  selected highlight), a Recents `SectionList`, and a pinned bottom row
  (account + settings `IconButton`s). Open/close are typed intents; the scrim
  tap dispatches close. This is deliberately *not* a SwiftUI island first —
  drawer state interleaves with the whole screen's layout and navigation,
  which is the thing islands are worst at.
- **Recents list.** Catalog `SectionList` + the P0 `ListItem` row contract.
  Rows' long-press `ContextMenu` (P1) for rename/delete.
- **Floating composer.** The P0 `Toolbar`/composer contract: glass capsule,
  `TextField`, mic `IconButton`. Mic press hands off to the existing
  `voice-input` host kind; the chrome is pure catalog. On iOS 26 the capsule
  gets `.glassEffect`; the expand-on-focus morph is `matchedGeometryEffect`
  sugar inside the lowering, absent on Android without contract change.
- **Approvals sheet.** Catalog `Sheet` with the P0 detents upgrade: half-detent
  glass sheet listing pending approvals (`ListItem` + `Toggle`/`Button`
  intents). Native `presentationDetents` on iOS, `ModalBottomSheet` on
  Android.

The important discipline: every piece above is state-in-props,
intents-out — the same `SubscriptionRef` → view stream → `IntentRegistry`
spine the Home program already runs. Whether a given piece renders through
bare RN, an `@expo/ui` lowering, or a SwiftUI island is a renderer decision
that can change per piece without touching the program.

## 7. From the Liquid Glass interim to catalog-native (what this means for #70)

Where we actually are: the shipped interim (TestFlight build 105) is exactly
the audit's "interop case 2" — an expo-module island
(`apps/openagents-mobile/modules/openagents-liquid-glass`): an `ExpoView`
owning a `UIHostingController<LiquidGlassRoot>`, four serializable props
projected from the Home program's `SubscriptionRef`, one `onGlassTap` event
that dispatches the typed `GlassPinged` intent through the *same*
`IntentRegistry` the RN renderer's reporter uses, real `.glassEffect` /
`.buttonStyle(.glass)` on iOS 26 with an `.ultraThinMaterial` fallback, and an
honest non-iOS fallback when the native module is absent. One source of
truth, two renderers reading it. That is the correct interim and it validated
every load-bearing assumption.

What it is not: catalog-native. The island is app-local wiring
(`loadLiquidGlassView()` in the screen shell), invisible to the catalog,
unreproducible by other EN apps, and per-island bespoke. D-MB-02 records this
precisely: interim = shell-boundary island; gap = "SwiftUI host kind +
`render-rn` host-driver seam (or the `render-swiftui` lane)"; upstream =
effect-native#70.

The path, in order:

1. **Land the `render-rn` host-driver seam (effect-native#70, ask 2).**
   `render-dom` already has a Scope-bound driver registry for host kinds;
   `render-rn` has none — unsupported kinds render loud markers. Mirror the
   `DomHostDriver` shape: `makeRNRenderer({ hostDrivers })` where an app
   registers a driver (native component + props codec + event→intent map) for
   a registered kind. This is small, renderer-local, and unblocks everything.
2. **Register the SwiftUI island kind (ask 1).** Either one reviewed kind
   (`ios.swiftui-island`) with per-island Schema-decoded props, or — better
   per the audit's "no arbitrary prop bag" rule — per-family kinds as they
   are demanded. The Liquid Glass island then becomes
   `Host({ kind, props, onEvent: IntentRef("GlassPinged") })` *in the catalog
   tree*, and the app-local `loadLiquidGlassView` wiring is deleted. D-MB-02
   closes when the register row's convert-and-delete lands.
3. **Grow the glass chrome behind catalog tags, not more islands.** As the P0
   table's tags land (glass Button/IconButton, ListItem, Toolbar, Sheet
   detents), `render-rn` lowers them — via `@expo/ui` per the Option 3
   decision — and each bespoke island or RN approximation is deleted. Islands
   remain only for genuinely foreign things, which is the `Host` contract's
   whole point.
4. **`render-swiftui` when a real screen demands whole-surface fidelity**
   (EN-S0..S6: renderer contract → generated Swift catalog types → static
   catalog → forms → lists/overlays → typed foreign Host → conformance +
   simulator screenshot receipts). The trigger per the audit and roadmap is
   demand — most likely candidate: the Home screen wanting system-level glass
   navigation chrome (toolbar morphing, `.searchable`, sidebar) that islands
   cannot reach. At that point `@expo/ui` lowerings for iOS are replaced by
   owned generated Swift, and Expo remains only the host shell.

## 8. Decision summary

- **Mental model:** SwiftUI ≈ EN's catalog idea, platform-captive; Compose is
  Android's same idea; the EN catalog is the renderer-independent version, and
  renderers lower to each — exactly the per-component job expo-ui's `ios/` and
  `android/` directories perform.
- **The dream is real:** one TS spec → SwiftUI + Compose is proven shipping
  architecture (expo-ui universal layer), with the honest caveat that the
  shared spec must stay semantic (glass *surface*, not glass *API*) and
  degrade gracefully where a platform lacks the concept.
- **Adopt:** Option 3 (hybrid). `@expo/ui` (MIT) becomes a lowering target
  inside `render-rn` — never a public API, never imported by app code. Its
  types inform our catalog's new tags; our catalog remains the unified type
  definition.
- **Stdlib:** grow the catalog by the P0/P1 tables (glass Button/IconButton,
  glass surface capability, Drawer, ListItem, Toolbar/composer, Sheet detents;
  then Toggle, Menu, Picker, Progress, Gauge, Badge, SearchField), through the
  normal EN-2 demand loop with per-renderer support limits stated.
- **Now:** land the effect-native#70 `render-rn` host-driver seam and convert
  the Liquid Glass island to a typed `Host` kind (closes D-MB-02's interim);
  build the ChatGPT-look Home from catalog pieces per section 6.
- **Later, on demand:** owned `render-swiftui` per EN-S0..S6 replaces the
  expo-ui iOS lowerings component-by-component; convert-and-delete.
