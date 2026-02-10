# Integration With Effuse (apps/web)

This doc explains how to use `@openagentsinc/effuse-panes` in the Effuse runtime without
introducing accidental re-mounts, tearing down canvases, or losing pane content.

The key is: **mount once**, then update pane content inside the existing pane DOM.

## Recommended Pattern: Shell That Mounts Once

In `apps/web`, we already do this for the HUD background:

- `apps/web/src/effuse-pages/marketingShell.ts`
- `apps/web/src/effuse-pages/authedShell.ts`

Both shells:

- render the full DOM only once (first call)
- then update only a slot on subsequent calls

Do the same for the pane system:

1. Render a container element once (e.g. `[data-oa-pane-root]`).
2. Call `mountPaneSystemDom(root)` once.
3. Keep a reference to the returned `{ store, render, destroy }`.
4. When you want to change the pane layout, mutate `store` and call `render()`.
5. Render pane content using Effuse's `DomServiceTag.render` into each pane's content node.

## Minimal Controller Sketch

This is intentionally plain JS/TS. Adapt to your controller structure.

```ts
import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";
import {
  mountPaneSystemDom,
  calculateNewPanePosition,
  hotbarSlot,
} from "@openagentsinc/effuse-panes";

let mounted:
  | ReturnType<typeof mountPaneSystemDom>
  | null = null;

export const runPaneShell = (container: Element): Effect.Effect<void> =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag;

    // Render root once.
    if (!container.querySelector("[data-oa-pane-root]")) {
      yield* dom.render(container, html`<div data-oa-pane-root class="h-full w-full"></div>`);
    }

    const root = container.querySelector("[data-oa-pane-root]");
    if (!(root instanceof HTMLElement)) return;

    // Mount once.
    if (!mounted) {
      mounted = mountPaneSystemDom(root, {
        hotbarItems: [
          hotbarSlot(1, "EV", "Events"),
        ],
        onHotbarSlotClick: (slot) => {
          if (!mounted) return;
          if (slot === 1) {
            const screen = { width: root.clientWidth, height: root.clientHeight };
            mounted.store.togglePane("events", screen, (snap) => {
              const rect = snap?.rect ?? calculateNewPanePosition(
                mounted.store.lastPanePosition,
                screen,
                520,
                360,
              );
              return { id: "events", kind: "events", title: "Events", rect, dismissable: true };
            });
            mounted.render();
          }
        },
      });
    }

    // Render content for known panes (host logic).
    const eventsContent = root.querySelector('[data-pane-id="events"] [data-oa-pane-content]');
    if (eventsContent instanceof Element) {
      yield* dom.render(eventsContent, html`<pre class="p-3 text-xs">...events...</pre>`);
    }
  }).pipe(Effect.provide(EffuseLive));
```

## Important: Content Ownership

`mountPaneSystemDom()` creates the pane skeleton:

- `[data-oa-pane]` root
- `[data-oa-pane-title]` chrome
- `[data-oa-pane-content]` content container

It does not know about your pane kinds or what they should display.

As a host, you should:

- define a stable mapping of `pane.id` -> renderer
- render content into `[data-oa-pane-content]` after calling `render()`

## Avoiding Content Reflows During Drag

The DOM adapter calls `render()` on every pointer move during drag/resize/pan.
`render()` reorders and updates pane elements, but it tries to reuse existing nodes by id.

Guidelines:

- Keep pane content in the content node. Do not attach content to the pane root itself.
- Avoid re-rendering pane content on every pointer move. Only re-render when content changes.
- If you need to update a lot of panes at once, batch store changes then call `render()` once.

## Clean-up

If the page is unmounted or replaced, call:

```ts
mounted?.destroy();
mounted = null;
```

This removes event listeners and DOM nodes created by the adapter.

