# coder-wm

Dwindle tiling for Coder surfaces. The layout and Super chords follow
[`os/modules/coderos/desktop.nix`](../../os/modules/coderos/desktop.nix):
screens above nine workspaces, `Super+Return` inserts a tile, arrows
move focus, Super with the mouse moves or resizes a float, and
`Super+Shift+E` is the session exit.

This crate has no GPU and no PTY. It returns rectangles, and
[`coder-compositor`](../coder-compositor/README.md) maps them onto its
windows.

It also answers what the [desk protocol](../coder-desk/README.md) reads and
changes, which the compositor's desk socket serves: `all_windows` reports
every window on every desk, including the ones a fullscreen tile covers,
`desk_of`, `is_floating`, `is_pinned`, and `is_fullscreen` read one
window's state, and `place`, `set_floating`, `set_pinned`, `place_float`,
and `raise` change it. A pinned window floats and follows the desk you
switch to, which is how it shows on every desk here.
