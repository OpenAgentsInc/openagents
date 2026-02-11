# Troubleshooting

This is the "gotchas" doc for integrating the DOM adapter into a real app.

## Root Container Must Have Stable Size

`mountPaneSystemDom(root)` assumes `root` is a fixed-size viewport.

If `root` has `height: auto` or is inside a collapsing flex container, you will see:

- panes rendered off-screen
- `clientWidth/clientHeight` returning 0
- weird drag behavior (because pointer coords are relative to root bounds)

Fix:

- ensure root is sized: `position: fixed; inset: 0;` or `h-full w-full` inside a known-height parent.

## Keyboard Shortcuts Do Not Work

The adapter attaches `keydown` to the root element and sets `root.tabIndex = 0`.

Keyboard shortcuts only fire if the root is focused.

The adapter tries to `root.focus()` on pointer down, but if your app prevents focus or moves focus elsewhere,
you may need to explicitly focus:

```ts
root.focus();
```

or attach keydown at a higher level (host responsibility).

## Dragging Starts When Clicking Inside Content

By default, dragging should only start when the pointer down target is within `[data-oa-pane-title]`.

If your pane content overlaps the title bar (e.g. negative margins), the closest() checks may match
unexpectedly.

Fix:

- keep pane content inside `[data-oa-pane-content]` without overlapping the title bar
- avoid CSS tricks that move content into the title area

## Clicking Close Button Starts a Drag

The adapter explicitly excludes `[data-oa-pane-close]` from drag start.

If you override pane chrome markup, ensure your close control preserves:

- attribute `data-oa-pane-close="1"`

## Resize Hit Test Feels "Off"

Resize hit testing is based on:

- the pane bounds (x/y/width/height)
- `paneResizeHandle` (default 10px)

If you apply large borders/padding that change visual edges, the hit area might not match visuals.

Fix:

- keep pane border widths small
- or increase `paneResizeHandle`

## Content Rendering Disappears After Drag/Resize

The adapter reorders pane nodes using `layer.replaceChildren(...)`.
Nodes are reused by `data-pane-id`, so their children should survive reordering.

If you are observing disappearing content, it usually means:

- your host re-renders the whole pane root element (not just content)
- you attached content outside `[data-oa-pane-content]` and the adapter overwrote it

Fix:

- render only inside `[data-oa-pane-content]`
- avoid replacing the pane node from the host side

## Closing/Unmounting Leaves Ghost Listeners

If your host removes the pane root DOM without calling `destroy()`, global pointer/keyboard behavior
can become inconsistent on the next mount.

Fix:

- always call `destroy()` (or an Effect-scoped `release`) before removing the root container
- centralize mount/unmount in one controller/service so cleanup cannot be skipped

## Hotbar Flash Callback Fires After Unmount

The adapter schedules a short callback after hotbar key flashes. This callback is cleared by
`destroy()`.

If you still see post-unmount work:

- verify cleanup runs in all close paths
- verify cleanup runs only after the final pane close/overlay close decision
