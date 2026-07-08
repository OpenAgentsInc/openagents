# React Native vs. Effect Native — What to Use, What to Leave

Date: 2026-07-08
Status: analysis / design input. Companion to the Effect Native decision
(`2026-07-08-effect-native-one-ui-substrate-analysis.md`), the README, and
the Foldkit comparison. Grounded in a deep read of the real
`facebook/react-native` monorepo (`projects/repos/react-native`) on
2026-07-08.

> **Framing note (2026-07-08):** the deepest version of this comparison —
> "Effect is to Effect Native what React is to React Native," i.e. Effect
> Native is a `platform-native` host adapter for Effect the way React Native
> is a native host for React — is in
> `2026-07-08-effect-native-is-a-framework-for-native-apps-using-effect.md`.
> This doc is the renderer-level half: which parts of RN to use as a backend,
> and which to leave. **Effect is our React** — the app is authored in Effect;
> RN only paints.

The short version: **React Native and Effect Native are not the same layer,
and the right relationship is "use one, inside the other."** React Native is
two things fused together — a genuinely excellent **native rendering engine**
(Fabric + Yoga + JSI + the shadow tree + per-platform mounting) *and* the
**React programming model** (hooks, JSX, component-local state, context). The
rendering engine is decades of solved, hard native work we should *use*. The
programming model is exactly the "React land" the owner wants out of. The
whole point of this doc: **keep React Native as a rendering backend, drop
React as the app architecture.** Those two can be separated, and separating
them is the design.

## 1. What React Native actually is (précis)

RN's pitch is "learn once, write anywhere": you write React components; RN
renders **real native views** (`UIView` / `android.view.View`), not a webview
or a canvas. Under the "New Architecture," a component tree becomes pixels
through a precise pipeline:

- **JS runs in Hermes** (Meta's mobile JS VM) on a dedicated JS thread.
- **The React Fiber reconciler** diffs the tree — the shipped
  `ReactFabric` renderer is *React itself with a React Native host config*
  (the exact analog of `react-dom`). This host config is the seam that makes
  RN "just React."
- **JSI** (`ReactCommon/jsi`) gives synchronous, zero-copy C++↔JS calls
  (replacing the old async JSON "bridge").
- Each element becomes an immutable C++ **ShadowNode**; the **Shadow Tree**
  is committed off the main thread; **Yoga** (Meta's Flexbox engine,
  `ReactCommon/yoga`) computes layout.
- A **Differentiator** produces a mutation list (create/insert/update/…),
  and the **mounting layer** applies it on the UI thread to real native
  views (`RCTMountingManager.mm` on iOS; JNI/Fabric on Android).
- **TurboModules + Codegen**: native capabilities are exposed to JS through
  machine-generated, statically-typed JSI interfaces built from spec files.

The core is small in *surface* (~20 JS core components: View, Text, Image,
ScrollView, TextInput, Pressable, Switch, ActivityIndicator, lists…) but
**enormous in depth**: every primitive exists three times — JS wrapper +
C++ shared core (`ReactCommon/react/renderer/components`) + per-platform
native (Obj-C++ and Kotlin/JNI) — plus Yoga, the concurrent C++ renderer,
codegen, two JS VMs, and old/new-architecture interop.

## 2. The crux: "React Native" (renderer) ≠ "React" (programming model)

This distinction is the entire analysis, so it gets its own section.

- **The rendering engine** — Fabric, Yoga, the shadow tree, JSI, the
  mounting layer, TurboModules — is a *host-agnostic native UI runtime*. It
  turns a tree of typed host-component descriptions + layout + events into
  native pixels, fast, on iOS and Android. Reproducing this from scratch
  (a cross-platform Flexbox engine, an immutable diffing tree, per-platform
  view managers, a concurrent scheduler) is a multi-year, multi-team effort.
  **This is the part worth using.**
- **The programming model** — JSX, hooks (`useState`/`useEffect`),
  component-local state, context, the React lifecycle — is how *app authors*
  talk to that engine today. It is untyped-by-default relative to our world,
  encourages state scattered across component-local hooks, and is precisely
  the substrate that produces "React slop on React slop" at scale. **This is
  the part to leave.**

The proof that these separate cleanly is in RN's own repo: **the reconciler
host-config seam is how out-of-tree renderers exist.** react-native-windows
and react-native-macos plug a different native backend under the same JS;
**react-native-web** swaps the host entirely so the same component API
renders to the DOM. RN is already "one component tree, many renderers" — it
just happens to fix the *authoring* model to React. Effect Native fixes the
authoring model to a typed component set + intents, and can render *through*
RN's engine underneath.

## 3. React Native vs. Effect Native — side by side

| | **React Native** | **Effect Native** |
|---|---|---|
| **What it is** | A native rendering engine **+** the React programming model | A typed component substrate that needs a renderer |
| **Layer** | Renderer + authoring model (fused) | Authoring contract (component set + intents), renderer-agnostic |
| **Owns** | Fabric, Yoga, shadow tree, JSI, mounting, TurboModules | The catalog, the intent algebra, the Effect runtime, tokens |
| **State model** | React hooks / component-local state / context | One typed model in Effect; no component-local state |
| **Interactions** | Callbacks (`onPress={() => …}`) | Named typed intents (data, not closures) |
| **Styling** | `StyleSheet` inline objects → Yoga Flexbox + native attrs | Typed props + design tokens → compiled to the renderer's styles |
| **Typing** | TypeScript on top of a JS/React core (optional, erasable) | Effect Schema at the core; invalid UI can't be represented |
| **Platforms** | iOS + Android native (out-of-tree: web, Windows, macOS) | Web + mobile + desktop + canvas via swappable adapters |
| **Adoption** | The framework you build the app in | Incremental; wraps existing renderers, migrates on touch |
| **Relationship** | A candidate **renderer** for Effect Native (adapter #1) | Sits **above** a renderer; can use RN's engine as one |

The table's punchline: they are not on the same rung. RN answers "how do
pixels happen on a phone." Effect Native answers "how do I define UI once,
typed, and render it anywhere." Effect Native's mobile renderer *is* React
Native's engine — used as a backend, not as the app.

## 4. The relationship: React Native as Effect Native's mobile renderer

This is the concrete design, matching EN-0/EN-3 in the roadmap:

- An Effect Native **component** (Stack, Text, Button, Field, List…) maps,
  in the RN adapter, to an RN **host component** (`View`, `Text`,
  `Pressable`, `TextInput`, `ScrollView`, `FlatList`). We reuse the 94
  shipping khala-mobile primitives as the first adapter's targets — zero new
  native work.
- Effect Native **layout** (Stack row/column + spacing tokens) compiles to
  **Yoga** Flexbox — we adopt Yoga's model rather than invent one; it's the
  layout engine already under our mobile app.
- Effect Native **intents** replace React callbacks: the view tree carries
  named typed intents; the runtime (Effect) dispatches them. No closures in
  the data, no `useState` in the components.
- **State lives in the Effect runtime**, bound from Khala Sync — not in
  hooks or context. RN's job shrinks to "render this host tree and emit
  these events," which is exactly what its engine is best at.
- The seam is philosophically identical to react-native-web's: same
  component contract above, different host below. We're doing to RN what
  react-native-web does to the DOM — treating RN as a *rendering target* for
  a component contract we own.

Net: **we get RN's entire native rendering investment for free, and pay none
of the React-authoring tax**, because our authors write Effect Native
components and intents, and only the thin adapter speaks React/RN.

## 5. What to USE from React Native, and what to LEAVE

The owner's direct question — use what, not what:

**USE (it's decades of solved native work; do not reinvent it):**
- **The Fabric rendering engine as the mobile renderer** (adapter #1). Do
  not attempt to render native pixels ourselves for v0 — that road is
  Fabric+Yoga+mounting, and it is enormous.
- **RN's core host components** (View/Text/Image/ScrollView/TextInput/
  Pressable/Switch/lists) as the primitives our catalog maps onto.
- **Yoga** as the layout model our Stack/spacing tokens compile to.
- **The out-of-tree host-config seam** as both the *proof* that renderer
  plurality works and the *mechanism* for it — Effect Native's RN adapter is
  an application of the same idea.
- **Codegen's spirit** (typed native interfaces generated from specs) as
  inspiration for how our native (Swift/Compose) renderers, if built, expose
  typed component bindings.
- **react-native-web** as a *reference* (and possible shortcut) for the web
  renderer — living proof "same component API → DOM" works.

**LEAVE (this is the "React land" that produces brittle software at scale):**
- **React's authoring model as our app architecture** — no JSX-authored
  screens, no `useState`/`useEffect` as the state model, no
  component-local state, no context-as-state. State is one typed Effect
  model; interactions are typed intents.
- **Callbacks in the view** — intents, not closures. This is what keeps the
  tree serializable, replayable, and agent-safe (the Foldkit lesson).
- **RN/React as the *contract*** — RN is a backend below our contract, never
  the contract itself. App code must not import React or RN directly; only
  the adapter does. A lint boundary enforces it (EN-9).
- **The RN component *API* as our public component API** — we expose the
  Effect Native catalog with typed props + tokens, not RN's prop shapes;
  the RN prop mapping is an adapter detail that can change per platform.

## 6. The "go fully native" question (RN vs Swift/Compose)

The Effect Native roadmap keeps a native Swift (iOS) / Jetpack Compose
(Android) renderer as the EN-7 fidelity upgrade. Honest framing against RN:

- Dropping RN entirely and rendering via Swift/Compose means **reimplementing
  what Fabric + Yoga + the mounting layer already do**: a cross-platform
  layout engine, a diffing/commit model, per-platform view management, event
  plumbing. That is a multi-quarter effort *per platform* and should not be
  a v0 or even v1 ambition.
- Because Effect Native's contract is renderer-agnostic, native rendering can
  be **per-component and incremental** — swap the five highest-value
  components (e.g. a perf-critical list, a bespoke animated control) to a
  native renderer while everything else stays on the RN adapter. That's the
  right shape: native as a *targeted escape hatch*, not a wholesale RN
  replacement.
- Practically: **RN is likely the mobile renderer for a long time**, and
  that's fine — the architecture just guarantees we're never *locked* to it.
  The value of renderer-agnosticism here is insurance and per-component
  fidelity, not a plan to delete RN.

## 7. Honest take

React Native is the strongest evidence *for* the Effect Native thesis, not
against it: it already proves that a single typed component tree can render
to genuinely different backends (native iOS/Android, DOM via
react-native-web, Windows/macOS) through a clean host-config seam. RN's only
limitation, from our vantage, is that it welds that renderer plurality to
React's authoring model — the exact model that turns high-velocity codebases
brittle.

So the recommendation is precise: **adopt React Native as Effect Native's
mobile rendering backend and reject React as the way we author UI.** Use
Fabric, Yoga, the host components, and the out-of-tree seam; author in the
typed Effect Native component set with intent-based interactions and
Effect-held state. Keep the native Swift/Compose renderer as a per-component
escape for fidelity, never a from-scratch replacement for RN's engine. This
captures the entire native-rendering investment of the React Native ecosystem
while keeping our UI inside the typed, resilient, agent-safe world the
Effect Native decision exists to protect.

## 8. Open questions

1. RN adapter surface: how thin can the RN adapter be — a direct map from
   catalog components to host components, or does it need an intermediate
   (e.g. reuse the reconciler host-config approach vs. a plain render
   function per component)?
2. Do we render through the full RN framework (Metro/Expo runtime, as today)
   or, longer term, drive Fabric more directly — and is that ever worth the
   cost over just using RN-the-framework as the backend?
3. Web renderer: thin direct-DOM vs. react-native-web (reuse RN's component
   API on web, but drag React along) vs. host-inside-Foldkit (the Foldkit
   comparison's Option A). Decide at EN-1 with landing + Sarah as the test.
4. Yoga everywhere: do we adopt Yoga/Flexbox as Effect Native's *canonical*
   layout model across all renderers (so web and native lay out identically),
   or let each renderer use its platform-native layout under the same tokens?
5. For the eventual native renderers, is codegen-from-catalog (RN's pattern:
   generate typed native bindings from the component spec) the right way to
   keep Swift/Compose renderers in sync with the catalog?
