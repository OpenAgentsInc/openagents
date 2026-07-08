# Foldkit vs. Effect Native — What They Are, How They Differ, What to Learn

Date: 2026-07-08
Status: analysis / design input. Companion to the Effect Native decision
(`2026-07-08-effect-native-one-ui-substrate-analysis.md`) and README.
Grounded in a deep read of Foldkit (`projects/repos/foldkit` — our own
Effect/TypeScript UI framework) on 2026-07-08.

The short version: **Foldkit and Effect Native answer different questions
on different layers, and Effect Native should treat Foldkit as its richest
source of proven ideas — because Foldkit is the deepest expression of
"Effect-native UI" we have ever built.** Foldkit is a *whole-stack web
application framework* with a fixed DOM renderer and a mandated app
architecture. Effect Native is a *renderer-agnostic component substrate*
whose unit is a typed component and whose promise is "define once, render
anywhere." One is not a newer version of the other; Effect Native re-homes
Foldkit's best instincts into a layer Foldkit deliberately never occupied.

## 1. What Foldkit is (précis)

Foldkit ("the frontend framework for correctness") implements **the Elm
Architecture (Model-View-Update) in TypeScript on Effect**, with the thesis
that app *architecture* should be a removed decision — every Foldkit app has
identical structure by construction. Concretely:

- **One Model** (an Effect **Schema**), **one Message union** (past-tense
  facts like `ClickedIncrement`), **one `update`** returning
  `[Model, Command[]]`, and a **view** function from Model to a typed
  hyperscript tree. State transitions are an exhaustive `Match` over the
  Message union — a forgotten case fails to compile.
- **Views are typed hyperscript, no JSX, no compile step**: `h.div([...],
  [...])`, and crucially **handlers take a Message *value*, not a
  callback** — `h.OnClick(ClickedIncrement())`. That single choice is what
  makes interactions loggable, replayable, and testable.
- **Rendering is a real virtual DOM via Snabbdom**, patched to the browser
  DOM with keyed diffing, plus manual memoization (`createLazy`).
- **Side effects are declared as data** in a clean taxonomy the runtime
  executes: **Command** (a named Effect returning a Message),
  **Subscription** (Effect `Stream`s gated by a slice of the Model),
  **Mount** (imperative DOM/3rd-party work with cleanup), **ManagedResource**
  (Model-driven acquire/release for sockets/audio), **CustomElement**.
  Errors fold back as Messages; nothing throws.
- **Ports** — a Schema-typed boundary (`Runtime.embed`) that lets a Foldkit
  widget run *inside any host, including React*, without the host reading
  the Model or dispatching Messages.
- Batteries: typed bidirectional routing, DevTools with time-travel + an
  MCP server, Story/Scene testing, state-preserving HMR, and a `@foldkit/ui`
  primitive kit (Button/Dialog/Combobox/…) composed as **Submodels +
  OutMessage envelopes**, not prop-driven components.

What it is **not**: no mobile / React Native / native; **no renderer
abstraction** (Snabbdom-over-DOM is hardwired); web/SPA only; no
component-local state or hooks; all-or-nothing adoption; no design-token
system (Tailwind class strings); no prop-driven component catalog; no
server-driven UI and no per-request SSR yet. (Correcting an earlier loose
claim: Foldkit has **no** three-effect/3D relationship — it ships a
declarative 2D `canvas` module but targets only the DOM.)

## 2. The core difference: different questions, different layers

| | **Foldkit** | **Effect Native** |
|---|---|---|
| **Question it answers** | "How do I structure a correct web *application*?" | "How do I define a UI *component set* once and render it on any platform?" |
| **Unit of the system** | The whole app (one Model / Message / update) | The component + the view-as-data tree |
| **Renderer** | **Fixed**: Snabbdom → DOM, baked into the core | **Swappable adapters**: DOM, React Native, native Swift/Compose, canvas, tty |
| **Platforms** | Web / SPA only | Web + mobile + desktop + canvas (native as the upgrade path) |
| **App architecture** | **Mandated** (MVU, no escape hatch) | **Not mandated** — a component substrate; an MVU shell can sit above it |
| **Adoption** | All-or-nothing (or embed via Ports) | Incremental by design (greenfield-first, wrap existing, migrate on touch) |
| **Composition** | Submodels + OutMessage | Typed component tree (props/children) — the "one component set" |
| **Design tokens** | None (raw Tailwind classes) | A single token source both renderers read |
| **Server-driven UI** | No | Designed-for (Hyperview axis), optional |
| **State model** | Single immutable Model, coarse VDUM diffing | Data-driven tree bound to Khala Sync; renderer decides update granularity |
| **Shared foundation** | **Effect + Schema** | **Effect + Schema** |

The rows that matter most: **renderer** and **unit**. Foldkit hardwired the
DOM renderer and made the *application* the unit — which is exactly why the
2026-07-04 ONE-UI decision shelved it: it could not target mobile, it was
all-or-nothing against a codebase we can't rewrite, and it demanded deep
Effect fluency for every contributor. Effect Native's founding commitments
(renderer-agnostic contract; the component as the unit; incremental
adoption) are precisely the axes Foldkit chose the opposite way on. They are
not competitors; they are different layers that happen to share Effect.

## 3. What Effect Native should learn from Foldkit (adopt these)

Foldkit is a decade of Elm wisdom already translated into our stack. The
ideas below are *validated in our own code* — Effect Native should take them
close to verbatim:

1. **Interactions as data, not callbacks.** `h.OnClick(Message)` takes a
   *value*, not a closure. This is the single most important idea to copy:
   Effect Native's behavior algebra must be **named typed intents**, never
   inline functions embedded in the view data. It is what makes the tree
   serializable, loggable, replayable, agent-safe, and (the Hyperview
   dream) server-authorable.
2. **Impossible states unrepresentable.** Foldkit models state as Schema
   tagged unions + `Option` (absence is never `null`; loading/error/loaded
   are distinct variants). Effect Native's component props and view state
   should be typed the same way — a card *cannot* be "loading and loaded."
3. **Exhaustive `Match` over a closed union.** A forgotten case fails to
   compile. Effect Native's intent handling and its closed component
   catalog should get the same compile-time totality.
4. **A named taxonomy of side effects, executed by the runtime.**
   Command / Subscription / Mount / ManagedResource / CustomElement is a
   genuinely good partition of "the ways UI touches the world." Effect
   Native's runtime should adopt an analogous effect taxonomy rather than
   ad-hoc effects — it's what makes time-travel and testing fall out for
   free.
5. **Ports — the Schema-typed embed boundary — is the migration and
   host-interop pattern we need.** Foldkit widgets run inside React hosts
   through a typed Port without the host touching the Model. This is
   *exactly* how Effect Native surfaces should embed into Sarah/Next, into
   the Expo RN app during EN-3 migration, and into any legacy screen: a
   typed boundary, not a leaky integration. Steal the Ports pattern
   wholesale for the renderer/host seam.
6. **Effects-as-data → time-travel, replay, deterministic tests for free.**
   Because Foldkit's effects and messages are values, its DevTools do
   time-travel and its Story/Scene tests are deterministic. Effect Native's
   serializable view tree + typed intents give the same leverage — build the
   DevTools/replay/visual-baseline story on that foundation from the start,
   the way Foldkit did.
7. **"Architecture as a removed decision."** Foldkit's core value is that
   every app looks the same by construction, which makes it readable and
   AI-friendly. Effect Native should make **component authoring** uniform in
   the same spirit: one way to define a component, one way to express an
   intent, one way to bind data — so 1000 agent edits/day stay legible.
8. **No JSX; the view is inspectable data.** Foldkit's typed hyperscript
   proves you don't need JSX to be ergonomic, and that a data view is
   inspectable and safe. Effect Native goes further (a fully serializable
   tree), but the instinct is the same and validated.
9. **Batteries matter.** Foldkit shipped routing, subscriptions, managed
   resources, testing, DevTools, HMR. A component substrate that ships only
   components and punts routing/navigation/testing will stall. Plan the
   navigation-intent, testing, and devtools story early (EN-2), not late.

## 4. Where Effect Native must deliberately differ

Equally important — the places Foldkit's choices are *wrong for our goal*
and Effect Native must diverge:

1. **Renderer abstraction is non-negotiable.** Foldkit's fixed
   Snabbdom/DOM renderer is the exact ceiling that stopped it serving
   mobile. Effect Native's contract (component set + intents) must be
   renderer-agnostic from line one; the renderer is a plugin, and there are
   several.
2. **The unit is the component, not the app.** Foldkit's whole-app MVU
   forces all-or-nothing adoption. Effect Native must be **incrementally
   adoptable** — one component, one screen at a time — because we have 94
   shipping RN files, a live Sarah app, and a web product we migrate on
   touch, never rewrite.
3. **A closed, prop-driven component catalog with design tokens.** Foldkit
   deliberately has neither (Submodels + raw Tailwind). Effect Native's
   entire premise is "one component set" — so a typed catalog with typed
   props and a single token source *is* the product (and the seam where the
   launch-ui look plugs in).
4. **Cross-platform and (optionally) server-driven.** Foldkit is DOM-SPA
   only. Effect Native targets web/mobile/desktop/canvas and designs-for the
   Hyperview server-driven axis, even if v0 doesn't use it.
5. **App architecture stays separable.** Effect Native does not mandate
   MVU; a surface may use an MVU shell (even a Foldkit-style one) above the
   component substrate, or not. The substrate is below the architecture
   decision, not the same as it.

## 5. Could they compose? (Two real integration options)

This is worth naming because it may save us real work and it reframes
"Foldkit vs Effect Native" as "Foldkit *within* Effect Native":

- **Option A — Foldkit as the web renderer + app shell.** Effect Native's
  DOM renderer could emit into Foldkit's mature Snabbdom pipeline, and a
  Foldkit application could *host* Effect Native components through **Ports**
  (the Schema-typed boundary already built for exactly this). We'd reuse
  Foldkit's routing, subscriptions, DevTools, and keyed-VDOM performance
  work instead of rebuilding a web renderer from scratch. Effect Native
  supplies the cross-platform component contract; Foldkit supplies a
  best-in-class web host for it.
- **Option B — shared primitives, separate renderers.** Keep them
  independent but share the foundation they already share (Effect + Schema),
  and lift Foldkit's effect taxonomy, Ports, and testing patterns into
  Effect Native's runtime as libraries rather than as a dependency on
  Foldkit itself.

Option A is the higher-leverage bet *if* the web renderer's fidelity
demands prove heavy — Foldkit is a large amount of solved web-rendering work
that is already ours. Option B keeps Effect Native's renderer surface
uniform and dependency-light. The decision hinges on how much the DOM
renderer actually needs (routing, subscriptions, memoization) versus how
much a thin direct-DOM renderer suffices for the landing + Sarah surfaces.
Recommend prototyping the DOM renderer thin first (v0), and evaluating "host
inside Foldkit via Ports" at EN-1/EN-4 when real web product complexity
arrives.

## 6. Honest take

Effect Native is not a repudiation of Foldkit — it is a **re-scoping of the
same conviction**. Foldkit proved, in our own repo, that UI can live fully
inside the typed Effect world: state as Schema, transitions as exhaustive
matches, effects and interactions as data, correctness by construction. Its
one limiting bet — *own the whole web app and hardwire the DOM renderer* —
is the bet Effect Native must not make, because our need is one component
set across web, mobile, and native, adopted incrementally against shipping
code.

So: **mine Foldkit for everything except its renderer and its all-or-nothing
scope.** Take interactions-as-data, Schema-modeled state, the effect
taxonomy, Ports, and the testing/devtools leverage — these are the crown
jewels and they're already ours. Leave behind the fixed DOM renderer and the
whole-app mandate. And seriously evaluate hosting Effect Native's web surface
*inside* Foldkit (Option A) rather than rebuilding web rendering, so the last
few years of Effect UI work compounds into the new direction instead of
being discarded.

## 7. Open questions

1. Web renderer: thin direct-DOM (dependency-light, uniform) vs. host inside
   Foldkit via Ports (reuse routing/subscriptions/VDOM)? Decide at EN-1 with
   the landing + Sarah surfaces as the test.
2. Do we lift Foldkit's effect taxonomy (Command/Subscription/Mount/
   ManagedResource) into Effect Native's runtime as the standard, or a
   reduced subset for v0?
3. Composition model: Foldkit's Submodel/OutMessage vs. a prop/children
   component tree — which serves "one component set, agent-authored"
   better, or is it per-layer (tree for components, submodel for app shell)?
4. Ports as the universal host boundary — adopt the exact Schema-Port shape
   for the renderer/host and legacy-embed seams?
5. Should the recommended *app shell* above Effect Native be Foldkit-style
   MVU (uniformity, testability) or left open — and does mandating it
   reintroduce the all-or-nothing friction we're trying to avoid?
