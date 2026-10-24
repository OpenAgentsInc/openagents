# Grass reference study

Status: reference study and Open Ground iteration, 2026-10-23. This study
guides a visual iteration; it does not establish reference parity or sustained
60 FPS. The baseline engine revision is `cb87808cd2b96bd866e286675f73cf2411e4d792`.

## References and evidence

- **Primary:** [Procedural Grass in 'Ghost of Tsushima'](https://www.youtube.com/watch?v=Ibe1JBF5i5Y), Eric Wohllaib, Sucker Punch Productions, GDC 2021; official upload dated 2022-07-14.
- **Secondary:** [SimonDev's explanation](https://www.youtube.com/watch?v=bp7REZBV4P4) and [Quick_Grass source](https://github.com/simondevyoutube/Quick_Grass/tree/6b56164272f213e353a4045d1439d071d5a968cd). Its [MIT license](https://github.com/simondevyoutube/Quick_Grass/blob/6b56164272f213e353a4045d1439d071d5a968cd/LICENSE) covers that implementation, not the GDC presentation or game artwork.

The primary video was downloaded in full: 26:09, 130,614,841 bytes. Its complete
4,667-word automatic English transcript was read after removing caption overlap.
Twenty representative 1920 × 1080 frames and their contact sheet were inspected.
Private evidence lives under `grass-video-study/priority-video/`: metadata,
captions, working transcript, `frame-manifest.json`, and `study-proof.json`.
Media and transcripts stay outside Git.

## Reference findings

The primary talk describes a field whose neighboring blades share growth,
direction, and color. These relationships guide the desired appearance.

| Primary timestamp | Technique |
| --- | --- |
| 02:43–04:39 | Terrain tiles supply height and type; jittered placement and filtering bound visible blades. |
| 05:46–07:03 | Voronoi clumps share height, facing, attraction, and outward orientation. |
| 07:04–08:06 | Compute prepares instances and indirect arguments in a bounded tile pool. |
| 08:08–10:14 | Shape and density morph across detail levels; short grass can split into two blades. |
| 10:16–13:40 | Cubic curves provide positions and normals; shared wind drives constrained animation. |
| 13:42–15:07 | Rounded normals and view-space widening fill the field; distant normal smoothing and reduced gloss limit shimmer. |
| 15:14–17:46 | Clumps control palette variation; analytic root shading avoids unstable temporal AO. |
| 17:51–20:13 | Instanced accent plants and distant terrain substitutes preserve field identity. |
| 21:04–22:29 | Terrain shadow proxies and short screen-space shadows bound shadow cost. |

Representative checks are 06:09 for coherent growth, 14:51 for distant highlights,
15:47 for palette organization, and 22:27 for shadow density. The secondary demo
provides accessible shader examples. Its terrain changes brightness over 25–50 m
and color over 50–100 m; its terrain image controls height, not albedo. On Open
Ground's two-triangle plane, distance-dependent color must run per fragment.

## Current iteration scope

The iteration keeps the continuous 2 km plane, source sky and Druid, movement,
wind, player bending, and independent frame loop. World-space Voronoi clumps
organize growth, facing, and tint, with individual variation inside each clump.
Curved silhouettes have matching derivatives and rounded edge normals. Distant
normals converge within each clump, and a restrained highlight fades with distance.

One compute pass, with at most two dispatches, prepares a fixed blade cache;
two indexed instanced draws read it. Static clump data rebuilds when patch slots
change. Animation refreshes from the supplied world time. The maximum is 262,144
submitted blades. Grass GPU buffers grow from 6,080 to 12,589,100 bytes, including
the 12 MiB blade cache. This is an explicit quality/memory tradeoff.

Far patches increase from 128 to 512 blades, retain three curved segments, and
share a stable root subset with nearby patches. Extra near blades retire over
staggered coverage fades. The ground blends brightness and green-to-olive color
per fragment. The baseline's shared shrinking ring and fixed dark soil are removed.
This slice has no indirect compaction, authored density maps, or grass shadows.

## Measured baseline

Evidence: `grass-video-study/baseline-browser/ground-browser.json`, isolated
Chrome `155.0.8059.85`, 1280 × 720. Rates count WebGPU submissions; GPU execution
and display presentation were not measured.

| Sample | Observed result |
| --- | --- |
| Stationary, 5.058 s | 294 submissions; 58.13 submissions/s; HUD median/minimum 60/60. |
| Walking and mouse look, 10.669 s | 640 submissions; 59.99 submissions/s; HUD median/minimum 60/57. |
| Moving sample responsiveness | UI animation-frame p95 16.8 ms; zero intervals over 50 ms; 67.19 m traveled. |
| Initial grass workload | 40,448 submitted blades, 357,376 triangles, two draws, zero dropped patches. |
| Functional result | Nine browser checks passed; no recorded errors. |

## Fixed visual comparison

`baseline-browser/fixed-capture-recipe.json` records poses and image hashes.
Use 1280 × 720, vertical field of view 1 radian, up `[0,1,0]`, day mode, exposure 1,
sun azimuth/elevation 225°/48°, and player feet `[6,0,20]`.

| Shot | Eye → target | World time |
| --- | --- | --- |
| Third person | `[6,2.4,26]` → `[6,0.9,18]` | 4 s, including repeat. |
| Low grass | `[10,0.7,24]` → `[10,0.7,14]` | 4 s and 5 s. |
| First person | `[6,1.62,20]` → `[6,1.55,10]` | 4 s. |

The repeated third-person capture has zero changed grass pixels in the measured
region after excluding the independently animated avatar. Repeat the same recipe
after changes; inspect motion as well as stills.

## Final comparison

Evidence: `grass-video-study/integrated-browser/ground-browser.json`, its matching
`fixed-capture-recipe.json`, and `grass-video-study/comparison-integrated.json`.
The browser, viewport, lens, environment, player position, and capture times
match the baseline. The final shader hash is
`4ccb516d` (prefix); source hashes and complete private artifact identities are
recorded with the captures. The original Quick_Grass repository remains unchanged.

| Sample | Baseline → final |
| --- | --- |
| Stationary | 58.13 → 60.98 submissions/s. |
| Walking and mouse look | 59.99 → 60.55 submissions/s over 10.69 s. |
| Moving HUD median/minimum | 60/57 → 60/59. |
| Moving UI animation-frame p95 | 16.8 → 16.7 ms; neither run has intervals over 50 ms. |
| Same initial pose | 40,448 → 78,848 submitted blades; 357,376 → 638,976 submitted triangles. |
| Final GPU work | One compute pass, two dispatches, two indexed draws; zero dropped patches. |
| Movement and input | 67.19 m traveled, grounded throughout, with 14.1° of mouse look. |

Nine browser checks pass, including first/third person, walking, running,
jumping, wind motion, and rejected abilities. Five fixed captures pass, and the
repeat has zero changed grass pixels outside the avatar. There are no recorded
page, network, or graphics errors. TypeScript checking, 54 focused tests, six
ground compiler tests, the web build, and specification checks pass.

The integration includes other rendering and frame-scheduling changes from
`main`; the performance difference cannot be attributed solely to grass. A run
before that integration averaged 58.30 submissions/s with a minimum HUD sample
of 48, preserved in `final-browser/` and `comparison-final.json`. All five final
grass captures match the integrated images exactly outside the avatar.

The matching images show a continuous far field in place of the baseline's
sparse band, narrower curved silhouettes, and related growth with individual
variation. Close low-angle shots still expose ribbon faces, dark roots, and
strong directional swaths. This improves the prototype; it does not reproduce
the game's complete terrain, lighting, shadows, or vegetation. These short local
samples do not qualify sustained 60 FPS or measure GPU execution/presentation.

## Later work

- GPU culling, compaction, and indirect draws beyond the implemented fixed cache; measure WebGPU scheduling rather than assuming the original hardware's overlap.
- Authored density/type maps and growth surfaces, with clump-correlated material profiles.
- Authored far-field terrain substitutes and instanced accent plants beyond the current distance-based ground color.
- Received shadows, density proxies, and contact shadows with separate budgets.

These remain proposals. The primary talk's hardware-specific timing is not an
Aethyrforge benchmark. Increasing blade counts alone does not qualify the result.
