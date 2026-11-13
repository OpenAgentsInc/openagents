# Portal Lab

An unlisted, owner-local recreation of a baked night scene: a glade with a stone portal, two pole lamps and thirty fireflies. The model and its baked-light texture are third-party course material kept on the owner's machine only (`~/.openagents/verse/private-build/web/portal-lab/`), never in this repository, so this page draws nothing anywhere else.

Coax reads the scene's GLB with the OpenAgents Metaverse Engine's glTF readers and draws it on the shared WebGPU host. The baked meshes show only their texture, the lamps are flat `#ffffe5`, the portal swirls red to blue with 3D Perlin noise, and the fireflies bob as soft additive discs.

- **Drag:** rotate around the portal.
- **Wheel or pinch:** zoom.
- **Right drag or two fingers:** pan.

The camera eases to a stop after you let go, as three.js's orbit controls do with damping.
