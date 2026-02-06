# Effuse EZ (Hypermedia Actions)

Effuse EZ is an HTMX-inspired runtime for targeted DOM updates. HTML carries the
interaction intent via `data-ez-*` attributes, while actions are Effect programs
that can return a template to swap into the DOM.

## Quick Usage

1. Register actions in an `EzRegistry`.
2. Mount the EZ runtime on a root element.

```ts
import { Effect } from "effect"
import { html, makeEzRegistry, mountEzRuntimeWith } from "./effuse/index.js"

const registry = makeEzRegistry([
  ["counter.inc", ({ el }) =>
    Effect.gen(function* () {
      const next = Number(el.getAttribute("data-count") ?? "0") + 1
      el.setAttribute("data-count", String(next))
      return html`${next}`
    })
  ],
])

Effect.runPromise(
  mountEzRuntimeWith(document.body, registry).pipe(Effect.scoped)
)
```

## Attributes

Implemented:

- `data-ez`: action name
- `data-ez-trigger`: `click | submit | change | input` (defaults based on element type)
- `data-ez-target`: `this`, `closest(...)`, `find(...)`, or a selector
- `data-ez-swap`: `inner | outer | beforeend | afterbegin | delete | replace`
- `data-ez-vals`: JSON to merge into params
- `data-ez-confirm`: confirmation text (uses `window.confirm`)
- `data-ez-disable`: presence-based boolean that disables the element while the action runs

Planned (Phase 2):

- `data-ez-indicator`
- `data-ez-error-target`
- `data-ez-error-swap`
- `data-ez-include`
- `data-ez-trigger`: `load | visible | idle`
- `data-ez-concurrency`: `switch | exhaust | queue`

## Target Resolution

`data-ez-target` controls where the returned template is swapped:

- Missing or `this` -> the action element itself.
- `closest(selector)` -> nearest ancestor matching selector.
- `find(selector)` -> first match within the action element.
- Any other string -> `root.querySelector`, with a fallback to `document.querySelector`.

If the target cannot be resolved, the action still runs but no swap occurs.

## Params Collection

`params` is a flat string map assembled as follows:

- `submit` events serialize the nearest form (target form or closest ancestor).
- `change` / `input` events use the element's `name` and `value` when present.
- `data-ez-vals` JSON is merged last; non-string values are `JSON.stringify`'d.

## Concurrency and Cancellation

EZ applies **switch-latest per element** concurrency:

- A new trigger on the same element interrupts the previous action fiber.
- Different elements can run concurrently.
- `data-ez-disable` temporarily disables the triggering element and restores it
  after completion or interruption.

## Best Practices

- Prefer targeting leaf nodes to avoid wiping large subtrees.
- Use `find(...)` or `closest(...)` over global selectors where possible.
- Keep action HTML small and localized for fast swaps.
- Use components for stateful flows; use EZ for localized interactions.
