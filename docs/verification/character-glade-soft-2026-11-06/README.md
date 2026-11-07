# Softer Nature glade — 2026-11-06

The user requested closer agreement with `Preview_1.jpg`: softer grass and
foliage, warm sunlight, and a less flat sky/light treatment.

The previous single straight triangles are replaced with 11,863 short curved
tufts. Each ribbon has five shared vertices and three fine shader strands;
actual height ranges from 6 to 16 cm. Two hidden trees behind the left cliff
were omitted to keep this geometry inside the existing 32 MiB pack limit.

All 91,922 leaf vertices now carry normalized crown-volume normals and finite
ambient masks. Warm leaf transmission, cool ambient fill and soft crown/rock
shadow proxies produce depth across the crowns and meadow. Back-facing grass
and leaves retain their volume lighting. The sky has angled cirrus and softer
cloud banks; header darkening is confined to the title area.

| Check | Result |
| --- | --- |
| Focused pack/scene checks | Five pass, including blade size/curvature and near-center normal normalization |
| Shader checks | Sky, bounded soft shadows, strand edges and mobile panel coverage pass |
| Independent geometry oracle | All indices/layers/positions valid; every grass ribbon has five vertices, a curved tip and bounded height; leaf normals and ambient masks valid |
| Original wardrobe | Vertex/index prefixes, catalog declarations and image payloads remain byte-identical |
| Hardware shader | Headless Metal WebGPU compilation passes, including derivative uniformity |
| Physical browser | Wardrobe controls, eye color, animation, persistence, zoom, rotation, panning and phone layout pass |
| Visual review | Desktop and phone captures compared with the reference and the previous glade |

The oracle caught non-unit normals close to crown centers; normalization was
corrected rather than loosening the check. Real WebGPU compilation caught an
edge derivative in material-dependent control flow; it was moved to the
uniform fragment entry. The reusable `glade_shader_check.coax` preserves that
hardware check, which scalar derivative stubs cannot perform.

Current counts, hashes and exact outputs are in `receipt.json`. Prior source
inventory and original scene investigation receipts remain valid at their
recorded commits and were not rerun. This is an Aethyr art-directed recreation,
not a claim of pixel-identical Unreal rendering or shared/native qualification.

With the static dev server on `127.0.0.1:8854`, the shader-only hardware gate is:

```sh
coax/web/tool.sh verse/tools/assets/glade_shader_check.coax
```

After rebasing the visual change at `609e33d26b`, shared movement dependencies
and the optional host texture-size hook had changed. The complete producer
was rebuilt against those inputs: **610,839 bytes**, below its 625,000-byte
budget. The physical browser flow passed again on that rebased build. This
repeat was required by changed dependencies; the unchanged pack/source gates
were not repeated. Exact identity and output are in the receipt's
`post_rebase` record.
