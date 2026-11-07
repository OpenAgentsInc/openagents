# Ore-informed linear lighting

See [implementation and research notes](../../verse/character-glade-lighting.md).
`research.json` pins the reviewed sources. `before.png` is the scene at
267ffbdc2b; `after.png` uses the same camera, geometry and pack after this change.
The controls captures show the active font, character, pan and phone layout.

The six new shader checks and independent known-value/property GPU readback
exercise transfer functions, HDR highlights, normal encoding, separated light
terms and height fog. Graph observations confirm real full-size HDR targets,
shadow/post passes, expected texture quality and no captured GPU errors.
The appearance/controls journey and offline content-hashed staging pass.
The common shader artifact stays unchanged. Source pack checks and Wasm from
the base commit remain applicable because neither changed.

This records an inspected lighting improvement, not exact image parity or
native Nintendo/Unreal rendering recovery. The principal remaining limitations
are full GI/probes, shadowed volumetrics and the retained legacy character BRDF.
