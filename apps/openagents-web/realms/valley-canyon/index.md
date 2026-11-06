# Valley Lab: Canyon Pair

An isolated source section: the two Canyon Sandstone instances in Ancient
World's `L0_X0_Y0` cell, component `InstancedStaticMeshComponent_7376`.
Both retain the source asset's 999,995 triangles, original 4K albedo and
roughness maps, UVs, normals, vertex paint and recovered world transforms.
One common origin converts centimetres/Z-up into metres/Y-up. Neither asset
is individually recentered, floored, rotated by eye or replaced by a Fab tier.

Walk in first person with **WASD**, **Shift** to walk, **Space** to jump, and
arrow keys or a drag on the right to look. Touch: drag left to move, right
to look, and tap **JUMP**. **O** switches to orbit inspection, where drag
orbits and the wheel zooms. **V** changes material/geometry views, **I**
isolates an instance, **R** resets and **1** returns to [Valley Lab](/worlds/valley-lab).
The parent lab's **P** key or `#section` opens this source view.

The licensed 71 MB source pack stays in the private local Aethyrforge cache;
it is absent from the public repository. A clean checkout needs that pack to
render this source view. The original primitive stations remain independently
usable without it.

The pair's source geometry and placements are verified. Its environment,
lighting, shadow filtering and tone mapping are Aethyr's inspection setup.
Full Unreal pixel parity, terrain, surrounding meshes, Lumen/Nanite's runtime
algorithms and the original material graph have not been qualified.

The walking floor and conservative box collision are inspection fixtures.
Original terrain, triangle collision and Echo's avatar are not imported here.
