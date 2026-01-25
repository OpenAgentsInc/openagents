> For connecting to HTML more simply than React does --- we're not doing a virtual DOM --- I think we can learn from HTMX. Flesh out if that's appropriate to borrow patterns from re dynamically updating HTML elements with minimal Javascript.

Yeah — HTMX is *exactly* the right kind of inspiration for what you’re building: **declarative, “hypermedia-driven” DOM updates** with **tiny runtime logic**, *without* needing a VDOM or component tree gymnastics.

The key idea to borrow isn’t “HTMX does partial HTML over HTTP” — it’s:

> **Put intent in HTML attributes, then have a small runtime interpret those attributes into Effects and DOM swaps.**

Effuse already has the right primitives (Effect, Stream, event delegation, `DomService.render()`).

---

## What to borrow from HTMX (pattern-wise)

### 1) Declarative “action attributes”

HTMX uses `hx-get`, `hx-post`, `hx-trigger`, `hx-target`, `hx-swap`.

For Effuse, you can generalize to “run an Effect and swap HTML into a target”:

* `data-ez` (the action name)
* `data-ez-trigger` (when to run)
* `data-ez-target` (where to put result)
* `data-ez-swap` (how to swap)
* `data-ez-vals` / form serialization (inputs/params)

Where “action” is **not necessarily HTTP** — it’s an Effect function you register.

### 2) Targeted DOM updates (avoid re-rendering everything)

Right now Effuse re-renders the component container via `innerHTML`.

HTMX’s biggest win is: you can update *one subsection* without building a whole component boundary.

So you want a first-class concept of:

* **targets** (selector resolution)
* **swap strategies** (inner/outer/before/after/append/prepend)
* **settle** behaviors (focus preservation, scroll, selection)

### 3) Trigger model

HTMX triggers are expressive: `click`, `submit`, `change`, `load`, `revealed`, etc.

For Effuse you can support:

* DOM events: `click`, `submit`, `input`, `change`, `keyup`, …
* lifecycle-ish: `load` (on mount), `visible` (IntersectionObserver), `idle` (requestIdleCallback)
* stream-driven: “when this Stream emits, patch target”

(You can ignore HTMX’s polling triggers if you want to stay aligned with “no polling” culture.)

### 4) Request lifecycle hooks (but local)

HTMX has indicator classes, disable elements, cancel inflight requests.

You want the same **ergonomics**, but for Effects/fibers:

* show loading spinner
* disable triggering element while running
* cancel previous inflight action for same element (switchLatest semantics)

---

## How this fits Effuse without becoming “React-lite”

Effuse has two modes already hiding inside it:

1. **Component re-render loop** (StateCell → render whole container)
2. **Imperative DOM service** (delegate events, render templates)

HTMX-style attributes basically formalize a third mode:

3. **Hypermedia Actions**: DOM contains “wiring” → runtime runs Effect → swaps HTML into target

This is complementary to components, not a replacement.

Use it for:

* forms
* small inline interactions
* “partial refresh” panels
* modal bodies / sidebars
* list rows that update in place
* progressive enhancement (your HTML remains meaningful)

---

## A concrete Effuse “HTMX-inspired” micro-spec

### Attributes

**Action**

* `data-ez="actionName"` — lookup a handler `(ctx) => Effect<TemplateResult | void>`
* Optional: `data-ez-method="invoke|navigate"` (usually just `invoke`)

**Trigger**

* `data-ez-trigger="click|submit|change|input|load|visible"`
* Defaults:

  * buttons/links → `click`
  * forms → `submit`
  * inputs → `change` (or `input` if specified)

**Target**

* `data-ez-target="#selector|closest(.x)|find(.y)|this"`
* Default: `this`

**Swap**

* `data-ez-swap="inner|outer|beforeend|afterbegin|delete|replace"`
* Default: `inner`

**Params**

* `data-ez-vals='{"key":"value"}'` (JSON)
* forms automatically serialize on `submit`
* elements can contribute via `name/value` like normal HTML
* allow `data-ez-include="#otherForm"` like HTMX’s “include” concept

**UX**

* `data-ez-confirm="Are you sure?"`
* `data-ez-indicator="#spinner"` (toggle hidden/class while running)
* `data-ez-disable="true"` (disable triggering element while running)

**Errors**

* `data-ez-error-target="#err"`
* `data-ez-error-swap="inner|outer"` (default inner)
* if action fails, render an error template (or a default)

### Action registry

You register handlers in your Effuse layer:

```ts
type EzAction = (args: {
  readonly event: Event
  readonly el: Element
  readonly params: Record<string, string>
  readonly dom: DomService
}) => Effect.Effect<TemplateResult | void, unknown>

const EzRegistryTag = Context.GenericTag<Map<string, EzAction>>("EzRegistry")
```

### Swap engine (no VDOM)

Implement a tiny swapper:

* resolve target element
* compute HTML string from `TemplateResult` (you already have `templateToString`)
* apply swap mode
* optionally run “settle” (focus restore)

---

## What you’d actually add to Effuse (minimal runtime)

### 1) One delegated listener per event type at the root

Instead of every component writing its own `setupEvents`, you can provide a **single** runtime that listens for any `[data-ez]`.

Effuse already has `dom.delegate(container, selector, event, handler)` — perfect.

Mount-time:

* `dom.delegate(document.body, "[data-ez]", "click", ...)`
* `dom.delegate(document.body, "form[data-ez]", "submit", ...)`
* `dom.delegate(document.body, "[data-ez][data-ez-trigger='change']", "change", ...)`

### 2) An “action interpreter”

On trigger:

* find closest element with `data-ez`
* parse attrs (trigger, target, swap, confirm, indicator)
* build params (form serialization + `data-ez-vals`)
* look up action in registry
* run it in a fiber, with cancellation keyed by element (Map<Element, Fiber>)
* if returns TemplateResult, swap into target

### 3) A `dom.swap(...)` helper

Extend `DomService` beyond `render()`:

```ts
swap: (target: Element, content: TemplateResult, mode: SwapMode) => Effect<void, DomError>
```

`render()` can just be `swap(..., "inner")`.

---

## Example: counter without manual event wiring

```ts
const actions = new Map<string, EzAction>()

actions.set("counter.inc", ({ dom, el }) =>
  Effect.gen(function* () {
    const out = el.closest("[data-counter]")!.querySelector("[data-count]")!
    const n = Number(out.textContent ?? "0") + 1
    return html`<span data-count>${n}</span>`
  })
)

// HTML
return html`
  <div data-counter>
    <span data-count>0</span>
    <button data-ez="counter.inc" data-ez-target="find([data-count])" data-ez-swap="outer">
      +
    </button>
  </div>
`
```

No component event union, no `setupEvents`, no queue — just declarative wiring.

(You’d still use full components when you want structured state + subscriptions + test harness.)

---

## Where HTMX patterns *don’t* map 1:1 (and what to do instead)

### “Requests” aren’t necessarily HTTP

HTMX assumes the server returns HTML.

In Tauri, your “server” can be:

* local Effect services
* Tauri commands (Rust) returning HTML/JSON
* filesystem reads
* database queries
* streams

So design “actions” as Effects, and optionally provide a built-in action that does HTTP fetch if you want.

### State consistency / composition

HTMX can become “spaghetti DOM” if everything is ad-hoc swaps.

Effuse can avoid that by:

* encouraging swaps *within a component’s container* (locality)
* providing a “component island” helper: mount a component into a stable node, then use actions for leaf updates inside it
* using action namespaced by component id (`autopilot.task.toggle` etc.)

---

## Practical recommendation: add this as an optional Effuse module

**Make it a small package layer:**

* `EffuseEzLive` (or `HypermediaLive`)
* depends on `DomService` + an `EzRegistry`
* one `mountEzRuntime(root: Element)` Effect that installs delegates and observers

So teams can choose:

* pure components
* pure actions
* mixed

---

## The one big gotcha (you should fix anyway)

Your current render strategy is `innerHTML = ...` which **replaces nodes**, so:

* direct event listeners die (you already use delegation, good)
* input cursor/selection resets (bad for forms)
* focus jumps
* scroll can jump

HTMX solves this with “settle” and careful swap strategies.

Even without a DOM diff, you can dramatically improve UX by:

* defaulting swaps for inputs/forms to **targeted** replacements (not whole container)
* adding a small “focus preservation” utility:

  * capture `document.activeElement` + selection range if it’s an input
  * after swap, try to restore if an element with same `id`/`name` exists

This keeps Effuse feeling “native” even with string-based renders.

---

If you want a north-star sentence for the docs:

**Effuse Components** are for durable state + subscriptions.
**Effuse Hypermedia Actions** are for declarative DOM wiring + partial swaps — inspired by HTMX, powered by Effects.

If you want, I can sketch the exact `EzRuntime` module surface (types + `mountEzRuntime` + `dom.swap` modes) in a way that drops cleanly into your existing `src/effuse/` layout.
