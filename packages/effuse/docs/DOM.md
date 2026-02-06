# Effuse DOM and Swaps

Effuse updates the DOM via explicit swaps, not a virtual DOM. The browser
implementation (`DomServiceLive`) provides swap modes and focus restoration.

## DomService.swap Modes

`DomService.swap(target, content, mode)` supports:

- `inner` (default): `target.innerHTML = html`
- `outer`: `target.outerHTML = html`
- `replace`: alias of `outer` (same behavior)
- `beforeend`: `target.insertAdjacentHTML("beforeend", html)`
- `afterbegin`: `target.insertAdjacentHTML("afterbegin", html)`
- `delete`: `target.remove()`

`DomService.render(container, content)` is shorthand for `swap(..., "inner")`.

## Focus Restoration

`DomServiceLive.swap` captures focus before `inner`, `outer`, and `replace`
swaps and attempts to restore it after the swap.

Restoration rules:

- Only if the active element was inside the swap target.
- Matches by `id` first, then by `name`.
- Restores text selection for `<input>` and `<textarea>` when possible.
- Uses `focus({ preventScroll: true })` to avoid jumping the viewport.

## Limitations

- `outer` / `replace` destroy the original element; any references become stale.
- If the focused element lacks `id` or `name`, focus cannot be restored.
- `beforeend` / `afterbegin` do not attempt focus restoration.
