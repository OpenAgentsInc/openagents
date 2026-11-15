# Page inspection menu v1

An optional, local presentation capability, `metaverse.engine.view-menu.v1`.
The producer exports `coaxial_view_menu_v1(state)` and
`coaxial_view_menu_len_v1(state)`: address and byte length of the current UTF-8
JSON recipe. Read length first, then address; copy before calling another
export. Capacity is 32,768 bytes. No exports means no inspection menu.

The one writer is `metaverse-engine/ui/menu-wire.coax`. The object has
`schema`, `title`, and `items`. Each item has `label`, `group`, `key`,
`current`, and `keep_open`. At most 128 items; title/group/label are printable
ASCII, at most 127 bytes (group may be empty). Keys are positive signed
32-bit integers. Duplicate keys refuse. A short output span returns -1.

The host projects these values into its existing accessible menu component
or browser buttons. Groups are submenus. A press calls the existing page
key boundary with that exact key, then releases it. `keep_open` requests that
the host rebuild the same group after the action. The producer remains the
owner of selection, validation and all state changes. The recipe conveys no
account, filesystem, network, world-editing or motor authority. It is neither
a command log nor a saved experiment. It cannot carry arbitrary script.

The native page host and browser consume the same recipe. Menus are refreshed
on opening, rather than holding pointers to a producer's old state. Closing
the page retires the menu and its callbacks.
