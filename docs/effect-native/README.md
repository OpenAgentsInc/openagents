# Effect Native

**A framework for building native applications using Effect.**

That is the same sentence React Native uses for itself
("a framework for building native applications using React"), with one word
changed — and the change is the whole point. React Native takes **React** (a
model for describing UI and state) and runs it on native platforms. Effect
Native takes **[Effect](https://effect.website)** (a model for describing
*whole programs* — state, effects, concurrency, typed data, services,
resource lifetimes, and UI) and runs it on native platforms. **The UI is one
part of an Effect Native app; the rest of the app is Effect.**

So Effect Native is more than a UI toolkit. It is how we build entire
applications — services, state, logic, data, and interface — as one typed
Effect program, and render that program's UI faithfully on the web, on
phones, and on desktop through small, swappable "renderers." (The base is
Effect **v4**, the `effect-smol` line.)

## The problem it solves

Most companies building for web and mobile end up maintaining the same
button, the same form, the same list — three or four times over, in
different languages and frameworks. Every change has to be made everywhere.
The versions drift apart. Bugs multiply. And as the pace of change speeds
up (in our case, AI agents making thousands of edits a day), that
duplication becomes a wall you hit hard.

Effect Native removes the duplication at its root: there is **one
definition** of each component, and everything else is a thin translation
layer.

## How the UI layer works, in three parts

The whole application is an Effect program (services, state, logic, data —
see the framing doc above). This section describes its **UI layer** — the
part that turns that program's interface into pixels.

1. **The component set.** A small, fixed catalog of building blocks —
   text, buttons, stacks (layouts), fields, lists, cards — each described
   as *typed data* rather than platform code. A screen is just an
   arrangement of these building blocks. Because it's typed data, the
   computer (and our tests) can check that every screen is valid before it
   ever runs. Invalid UI simply can't be built.

2. **The runtime.** A small engine (built on [Effect](https://effect.website),
   the typed foundation the rest of our software already uses) that reads a
   screen's description, connects it to live data, and turns taps and
   clicks into well-defined actions. This is where the app's behavior
   lives, once, shared by every platform.

3. **The renderers.** Thin, per-platform adapters that take the shared
   description and draw it using that platform's real, native building
   blocks — HTML on the web, native controls on a phone. The renderer is
   the *only* platform-specific code. Everything above it is shared.

The important idea: **the component set and its behavior are the stable
contract; the renderer is a detail you can have several of, and replace one
at a time.** Today a phone screen might be drawn with React Native; if we
later want it drawn with true native code (Swift on iOS, Kotlin/Compose on
Android) for extra polish or speed, we swap the renderer underneath — the
components and the app on top don't change at all.

## Why "Native," why "Effect"

- **Effect** — because the whole system is built on typed, composable
  Effect programs. The UI stays inside the same safe, checkable world as
  the rest of our code, instead of being an untyped exception to it.
- **Native** — because the goal isn't "web pages pretending to be an app."
  Each platform renders with its own genuine components, and the design is
  built so that the rendering can go all the way down to true native code
  where it matters.

## What this gives us

- **Write once, run everywhere** — a real version of a much-abused phrase,
  because only the thin renderer differs between platforms.
- **Safety at speed** — the type system catches broken UI at the boundary,
  which matters enormously when changes come fast.
- **Resilience** — if a framework we depend on changes or falls out of
  favor, we swap a renderer, not the product.
- **Trustworthy AI-authored UI** — because screens are validated typed
  data, an AI agent building a screen can't produce something malformed; it
  can only assemble valid, known components.

## Status

Effect Native is an **owner decision as of 2026-07-08** and is being
adopted greenfield-first: new surfaces are built on it immediately, while
existing, shipping apps migrate onto it gradually — never rewritten just to
move them.

## Documents in this folder

- **[What Effect Native is — a framework for native apps using Effect](./2026-07-08-effect-native-is-a-framework-for-native-apps-using-effect.md)**
  — the definitive framing: "Effect is to Effect Native what React is to
  React Native." A role-for-role parallel (React→Effect, Fabric→renderers,
  the reconciler→Effect's fiber runtime, hooks→`Ref`/`SubscriptionRef`/`Atom`,
  Context→`Layer`, PropTypes→`Schema`), why Effect is a *stronger* foundation
  than React (typed errors, structured concurrency, DI, resource safety,
  schema boundaries — things React lacks), and the technical seam: **Effect
  Native is a `platform-native` host adapter**, mirroring how
  `platform-browser`/`-node`/`-bun` already extend Effect. Pins **Effect v4
  (effect-smol)** as the base. *Read this first — it reframes everything
  below as layers of one application framework.*

- **[UI layer: decision, architecture & roadmap](./2026-07-08-effect-native-one-ui-substrate-analysis.md)**
  — the owner decision to pivot, the three-layer architecture (typed
  component set → Effect runtime → swappable renderers), the greenfield-first
  vs migrate-on-touch discipline, a ruthlessly-small v0, and the phased
  full-conversion roadmap **EN-0…EN-9** (foundation → greenfield web →
  catalog → mobile → web product → desktop → canvas → native renderers →
  terminal → governance). *The main doc; read first.*

- **[Foldkit vs Effect Native](./2026-07-08-foldkit-vs-effect-native.md)**
  — Foldkit is our Elm/MVU Effect framework with a *fixed DOM renderer* and
  a *whole-app* mandate; Effect Native is a *renderer-agnostic component
  substrate*. Different layers. **Learn from Foldkit:** interactions-as-data
  (not callbacks), impossible-states-unrepresentable via Schema, exhaustive
  match, the Command/Subscription/Mount effect taxonomy, and **Ports** as the
  typed embed/migration boundary. **Diverge on:** renderer abstraction, the
  component-not-app unit, incremental adoption, tokens + a prop-driven
  catalog. Names a real option — host Effect Native's web surface *inside*
  Foldkit via Ports to reuse its mature rendering.

- **[React Native vs Effect Native](./2026-07-08-react-native-vs-effect-native.md)**
  — RN is two things fused: an excellent *native rendering engine*
  (Fabric/Yoga/JSI) **and** the *React programming model*. They separate
  cleanly (RN's own out-of-tree seam proves it). **Use** RN as Effect
  Native's mobile *renderer* (adapter #1 — reuse the shipping primitives,
  Yoga layout, host components); **leave** React as the authoring model
  (no JSX screens, no hooks/component-local state, no callbacks-in-view).
  Native Swift/Compose is a per-component escape, never a from-scratch
  Fabric replacement.

- **[three-effect vs Effect Native](./2026-07-08-three-effect-vs-effect-native.md)**
  — our Three.js library is really two things: a large standalone-worthy
  *domain library* (Verse world, VFX, HUD, Drei ports) and a small
  *renderer kernel* (reconciler + frame clock + scope). **Split, don't
  choose:** keep the domain library standalone; **fold the kernel into the
  Effect Native canvas renderer** (EN-6), reimplemented on real Effect
  `Scope`/`Stream`/`Layer`; retire its Foldkit adapter. Realizes the
  "Effect" the name currently only claims, without rewriting the live Verse.

- **[Styling: Tailwind, StyleX & native](./2026-07-08-styling-tailwind-stylex-effect-native.md)**
  — a Tailwind *class string* is a web-coupled contract; a StyleX-style
  *typed style object* is a portable value that lowers to CSS on web and RN
  objects on native, with a deterministic no-cascade merge. **Verdict:**
  adopt **StyleX's typed-object model** (the contract, merge, typed
  tokens/themes, typed per-component style contracts) + carry **Tailwind's
  design tokens** (the scale/taste, as typed tokens not classes) + **lower
  per renderer**. Not class-strings-as-contract; not per-platform native CSS
  (that forks the source of truth). NativeWind is the proof the lowering
  works and the map of the CSS-runtime seams.

The reference codebases these analyses draw on live under
`projects/repos/` (hyperview, react-native, tailwindcss, nativewind, stylex)
and the workspace siblings `foldkit`/`three-effect`.

## The one-sentence version

Effect Native lets us design a component once, as typed data, and render it
faithfully on the web, on phones, and on desktop — so we build our
interfaces in one resilient, typed place instead of duplicating them across
platforms.
