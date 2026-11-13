# Valley Lab: Canyon Pair

An isolated source section: the two Canyon Sandstone instances in Ancient
World's `L0_X0_Y0` cell, component `InstancedStaticMeshComponent_7376`.
Both retain the source asset's 999,995 triangles, original 4K albedo and
roughness maps, UVs, normals, vertex paint and recovered world transforms.
One common origin converts centimetres/Z-up into metres/Y-up. Neither asset
is individually recentered, floored, rotated by eye or replaced by a Fab tier.

Walk with Echo in third person with **WASD**, **Shift** to walk, **Space** to jump, and
arrow keys or a drag to look. Touch: drag left to move, right
to look, and tap **JUMP**. **O** switches to orbit inspection, where drag
orbits and the wheel zooms. **V** changes material/geometry views, **I**
isolates an instance, **R** resets and **1** returns to [Valley Lab](/worlds/valley-lab).
The parent lab's **P** key or `#section` opens this source view.

**C** inspects the full character and **H** inspects her face; **R** returns to walking.

The licensed 668 MB Echo/Canyon source pack stays in the private local Metaverse Engine cache;
it is absent from the public repository. A clean checkout needs that pack to
render this source view. The original primitive stations remain independently
usable without it.

The pair's source geometry and placements are verified. Its environment,
lighting, shadow filtering and tone mapping are the OpenAgents Metaverse Engine's inspection setup.
Full Unreal pixel parity, terrain, surrounding meshes, Lumen/Nanite's runtime
algorithms and the original material graph have not been qualified.

The walking floor and conservative box collision are inspection fixtures.
Original terrain and triangle collision remain separate work. Echo uses the
original mesh, textures, skin weights, animation samples and strand positions.
Shared Metaverse Engine locomotion has an opt-in port of the source's flat-ground velocity
rules. Full source camera/ability graphs, vaulting, facial animation, cloth,
hair simulation and Unreal image parity remain unqualified.
