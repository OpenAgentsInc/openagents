# Character Create camera and sky

The camera could go underground through negative pitch, a long dolly or negative
vertical pan. Camera construction now samples the authored glade terrain at the
final X/Z and clamps eye height to terrain + 0.28 m, covering all input paths.
The 81-combination check spans minimum/default/maximum distance, pitch and pan.
The physical browser test observes submitted GPU uniforms after 12 extreme
pointer/wheel combinations; the lowest observed Y is 0.2800666 m. Existing
controls, persistence, zoom/pan, male/female and phone journey passes.

The blue bar was the finite reference sky card edge exposing procedural sky.
The sky geometry now covers the viewport at far depth; fragment world rays
preserve the reference projection. A smooth 16% border blends into directional
sky, with clamped sampling to avoid repeat seams. This avoids reflected clouds
and hard edges when looking up. Default and low-angle captures were inspected.
Desktop and phone WebGPU graph checks report zero errors with shadow/post/MRT.
Private and common shader generation passed. Whole-site generation encountered
an unrelated existing valleyworld.coax V4/VcLit type mismatch; only the two
changed shader targets were generated directly. No Valley World files changed.

The rebuilt fivelab WASM is 616,258 bytes. The source pack, saved appearance format,
player lighting and 75% left sidebar are unchanged. Reload the preview at 8817.
