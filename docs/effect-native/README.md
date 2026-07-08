# Effect Native

**One set of UI components, defined once, that runs everywhere — web,
mobile, desktop, and beyond — without rewriting them for each platform.**

Effect Native is OpenAgents' approach to building user interfaces. Instead
of maintaining separate codebases for the website, the mobile app, and the
desktop app — each with its own framework, its own components, and its own
bugs — we define our UI **once**, as typed data, and render it on any
platform through small, swappable "renderers."

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

## How it works, in three parts

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
move them. The full design, rationale, the reference material that inspired
it, and the phased plan to convert every surface live in:

- [`2026-07-08-effect-native-one-ui-substrate-analysis.md`](./2026-07-08-effect-native-one-ui-substrate-analysis.md)
  — the decision, architecture, and full-conversion roadmap (phases
  EN-0…EN-9).

## The one-sentence version

Effect Native lets us design a component once, as typed data, and render it
faithfully on the web, on phones, and on desktop — so we build our
interfaces in one resilient, typed place instead of duplicating them across
platforms.
