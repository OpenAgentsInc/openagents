# Everglade II on phone-shaped screens: before and after #13071

Measured 2026-11-09 on the Mac (Apple GPU), headless Chrome
(`coax/web/tool.sh destroy-fps`), at 390×844 and 844×390 CSS pixels, DPR 3,
touch and a coarse pointer (`--touch`), the CPU slowed 4 times (`--cpu 4`),
8 s after the page was ready and then 10 s of frames. The page is
`/realms/everglade2/` served statically from each build.

- **Before:** main at 66218652cc (no device facts, a flat 6.4 million pixel
  budget, no knobs).
- **After:** main with phases 0 to 3 (785999f840, 6de04f81c3, 8d375af101,
  d897dd76db).

The script is [`measure.js`](measure.js); the raw lines are
[`runs-auto.txt`](runs-auto.txt) and [`runs-held.txt`](runs-held.txt).
Vertex counts are thousands of vertices a frame (probes 12 to 15), so they are
exact; GPU times are the median frame span from `coaxial.takePassTimes()` and
vary with other work on the shared GPU.

## AUTO (what a phone gets with no pick)

| Shape | Build | FPS | GPU ms | Rung, tier | Grass, scenery, shadow, post, crowd | Bloom passes | View k-verts | Grass k-verts | Tree k-verts | Shadow k-verts | Draws |
| --- | --- | ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 390×844 | before | 60 | 7.22 | none | all 100 % | 5 | 15,570 | 5,355 | 7,788 | 3,932 | 117 |
| 390×844 | after | 60 | 5.85 | 7, MED | 100, 100, 60, 60, 100 | 3 | 15,570 | 5,355 | 7,788 | 50 | 117 |
| 844×390 | before | 60 | 6.92 | none | all 100 % | 5 | 23,253 | 6,902 | 13,179 | 3,932 | 130 |
| 844×390 | after | 60 | 6.43 | 7, MED | 100, 100, 60, 60, 100 | 3 | 23,253 | 6,902 | 13,179 | 3,932 | 130 |

Both shapes start as a phone (class 1, rung 5). The Mac holds 60 FPS, so the
governor climbs a rung every 5 s on time; by the measurement it was at rung 7.
Before #13071 Everglade II had no device start: it drew up to 6.4 million
pixels a frame on any screen, a phone held sideways included.

## Held tiers (`?quality=med`, `?quality=low`)

| Shape | Tier | FPS | GPU ms | Grass, scenery, shadow, post, crowd | Bloom passes | View k-verts | Grass k-verts | Tree k-verts | Draws |
| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 390×844 | MED (rung 8) | 60 | 6.30 | 100, 100, 100, 60, 100 | 3 | 15,570 | 5,355 | 7,788 | 117 |
| 390×844 | LOW (rung 2) | 60 | 6.64 | 45, 80, 30, 20, 25 | 0 | 9,593 | 2,023 | 5,638 | 87 |
| 844×390 | MED (rung 8) | 60 | 6.26 | 100, 100, 100, 60, 100 | 3 | 23,253 | 6,902 | 13,179 | 130 |
| 844×390 | LOW (rung 2) | 60 | 6.05 | 45, 80, 30, 20, 25 | 0 | 13,964 | 2,023 | 9,466 | 87 |

At LOW the frame draws 38 to 40 % fewer vertices than before (the grass 62 to
71 % fewer, the trees 28 % fewer) and 33 fewer draws, with no shadow tiles past
the nearest and no bloom.

## Limits of a headless run

- Headless Chrome draws one device pixel per CSS pixel whatever `--dpr` says,
  so the canvas is 390×844 in every run and the pixel knob (3.0 down to 0.75
  a CSS pixel) shows no change here. On a real DPR 3 phone, rung 5 draws a
  quarter of the pixels HIGH does (1.5² against 3²).
- The Mac's GPU holds 60 FPS at every rung, so the measured start never steps
  down here and the GPU times sit inside their own noise. The vertex counts
  and the knob values are the exact change.
- A real iPhone and Android phone, upright and sideways, are in
  `NEEDS_OWNER.md` (the workspace repo).

## Desktop HIGH

At HIGH every knob is 1 and each shader runs its original code (the
`u[35..37] >= 0.9999` and shadow-row branches). A golden-mode shot at
1280×800 before and after differs in 3.97 % of its pixels, all scattered
single-pixel dither specks; two shots of the before build differ in 47.6 %
(the scene isn't fixed between loads in this tool), so this tool can't prove
pixel identity. Everglade II has no golden in `coax/web/goldens/`;
Everglade's (`everglade.png`) doesn't export the bloom hook and reads the top
rung, whose knobs are all 1.

## Mutation testing

`coax mutate aethyr/quality.coax --library --root-only --jobs 4` made 871
mutants: 867 killed, 1 survived and 3 marked equivalent. The survivor read the
probe's frames from the second one on (`probe_ms[1..<n]`). A check where the
first of eight frames decides the median now kills it: with that mutant
applied, `coax test aethyr/quality.coax --library` fails 1 of 21 tests. That
leaves 0 survivors. The 3 equivalents are marked `// mutant: equivalent` in the
source, each with its reason.
