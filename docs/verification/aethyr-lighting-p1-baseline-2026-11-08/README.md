# Aethyr lighting P1.1 baseline (2026-11-08)

This receipt pins the look of Character Create, Everglade II and Experimental
before any lighting change (issue #13061, umbrella #13068). Later lighting
steps, starting with #13060, compare against it. Character Create's glade must
not drift.

## Command

From the repo root, after `COAX_MUTATION=0 coax/build.sh`:

```sh
coax/web/tool.sh coax/web/lighting-baseline.coax OUT \
  --compare docs/verification/aethyr-lighting-p1-baseline-2026-11-08 --build
```

`--build` rebuilds only the units the pages use (host modules, the
everglade2 page pack, and the everglade2 and fivelab wasm), each through
`~/.openagents/build-slots/heavy.sh`. The tool serves `apps/openagents-web`
on 127.0.0.1:8977 (`--serve PORT` or `--base URL` to change), runs one muted
headless Chrome, writes captures, `probes.tsv`, `run.tsv`, `contact-sheet.jpg`
and `compare.tsv` to `OUT`, and exits non-zero when anything leaves its bounds.
`--only cc,exp-combat` limits the loads. Without `--compare`, it records a new
baseline into `OUT`.

## Source and identities

| Item | Identity |
| --- | --- |
| Source commit | `5c299db9b0` (origin/main) |
| Compiler | coax0 0.52.0, checked build `aa8be04a6d4d`, binary sha256 `7c1e013c1f910f7c…` |
| everglade2.wasm | sha256 `90b43df978390c98…`, 646253 B |
| fivelab.wasm | sha256 `33816148a24c24c6…`, 2582945 B |
| everglade2.vpk3 / everglade2.pack | `929fafc6a3e15e11…` / `731fb6e9917e4d70…` |
| everglade2.wgsl (Character Create, Everglade II) | `73d2db4d1ba0c146…` |
| charactercreator.wgsl (Experimental) | `b4153a17309e55ee…` |
| everglade2-fx.wgsl | `b5263781…` |
| Experimental glade.pack.gz | `b0aa8371d62fc07d…` |
| Character Create glade.pack / fantasy.pack | `075097f03a083cae…` / `887e0cb0bd75a00f…` |

The shader hashes in `run.tsv` are taken from the modules the page actually
compiled, and they match the committed WGSL.

## Host and device

Mac17,6, Apple M5 Max, macOS 26.4 (25E246). Google Chrome 157.0.8103.39,
headless, muted. WebGPU adapter: hardware, Apple, metal-3.

## Pins

- Time: clip mode (`coaxialClip`); frames advance only in fixed 1/30 s steps.
  `Date.now` is frozen at 1794009600000 and the room clock follows clip time
  (`coaxialClipRoom`). The page clock is `fixed:0.5`.
- Quality tier: `verse.quality=high`. Device pixel ratio 1.
- Seed: Experimental's dice seed is fixed in source (5). Everglade II has no
  random state beyond its baked field.
- Frames are read from an offscreen `rgba8unorm` render target
  (`coaxial.captureFrame()`), not from a screenshot.
- Readiness: the scene reports ready, the loading card is gone, and Everglade II
  reports its trees planted, kits and ground tiles published, and its bake live.
  Then three consecutive frame hashes must match.

## Rasters and captures

1920x1080 for every capture except `cc-phone-glade` (390x844).

| Capture | Page | What |
| --- | --- | --- |
| cc-glade | /worlds/character-creator | the glade at rest |
| cc-closeup | same | character close-up (wheel zoom) |
| cc-turn-0…3 | same | four frames of a camera drag turn |
| cc-phone-glade | same, 390x844 | phone layout |
| handover-0…4 | same, then USE IN EVERGLADE | Character Create to Everglade II hand-over, 0.5 s to 23.5 s after the click |
| e2-arrival | /worlds/everglade2?at=1 | glade arrival |
| e2-sunlit-path | /worlds/everglade2?at=60 | open sunlit path |
| e2-deep-canopy | /worlds/everglade2?at=300 | deep canopy path |
| exp-combat | /worlds/everglade-experimental | Fire Bolt cast after a camera orbit |

Captures are JPEG q70 4:4:4 (about 300 KB each). `contact-sheet.jpg` shows all
16. I opened every capture: each scene is fully loaded, with no loading card.

## Probes

GPU probes run on the page's own WebGPU device. The tool keeps the WGSL each
page compiles, appends a compute entry that calls the emitted lighting
functions (`coax_p_cg_*`) at fixed inputs, and reads the result back. It is a
measurement entry; it does not draw. Haze probes use grey 0.18 at the given
distance seen from eye (0, 1.6, 4). Values are linear RGB, except `output.*`,
which is the final display-encoded output for a given linear input.

Character Create (everglade2.wgsl) and Experimental (charactercreator.wgsl)
give identical values today:

| Probe | R | G | B |
| --- | --- | --- | --- |
| key.sun_colour | 2.1500001 | 1.5119573 | 1.1221162 |
| key.sun_dir (xyz) | -0.4262078 | 0.6180012 | -0.6606220 |
| ambient.up | 0.4900000 | 0.5500000 | 0.6200000 |
| ambient.side | 0.4050000 | 0.4450000 | 0.3900000 |
| ambient.down | 0.3200000 | 0.3400000 | 0.1600000 |
| irradiance.up_lit | 1.8187027 | 1.4843915 | 1.3134692 |
| irradiance.up_shaded | 0.2450000 | 0.2750000 | 0.3100000 |
| sky.zenith | 0.2346145 | 0.7112648 | 0.8679722 |
| sky.toward_sun | 0.3971755 | 0.7845046 | 0.8647466 |
| sky.away | 0.3657231 | 0.7744398 | 0.8845987 |
| sky.up45_north | 0.2677773 | 0.7253011 | 0.8495620 |
| haze.10m | 0.1937805 | 0.1952818 | 0.1947712 |
| haze.40m | 0.3015427 | 0.3147850 | 0.3102813 |
| haze.80m | 0.4990202 | 0.5337780 | 0.5219569 |
| haze.toward_sun_60m | 0.5135475 | 0.5173786 | 0.4672491 |
| haze.away_60m | 0.2981699 | 0.3572260 | 0.4065015 |
| cloud.0_0 | 1.0000000 | 1.0000000 | 1.0000000 |
| cloud.12_-20 | 0.4500000 | 0.4500000 | 0.4500000 |
| cloud.-30_10 | 0.9573049 | 0.9573049 | 0.9573049 |
| output.grey18 | 0.5105091 | 0.5105092 | 0.5105093 |
| output.white1 | 0.8872048 | 0.8872049 | 0.8872049 |
| output.hdr_orange | 1.0141181 | 0.9018516 | 0.7175723 |
| output.sky_zenith | 0.6717730 | 0.8465421 | 0.8740784 |
| output.black | 0.0000000 | 0.0000000 | 0.0000000 |

Each capture also has nine pixel probes: a 3x3 grid at 25/50/75% of the frame,
each a 9x9 mean, in sRGB 0–255 and linear. All 192 probe rows are in
`probes.tsv`.

## Bounds

| Check | Bound |
| --- | --- |
| Image block | 8 px blocks; a block moved when a channel mean moved more than 8/255 |
| Image blocks moved | at most 2000 ppm (0.2%) of blocks |
| Image mean shift | at most 1.0/255 averaged over all blocks |
| Pixel probe | at most 3.0/255 per channel |
| GPU probe | at most 1e-4 of the value per channel, never less than 1e-5 |

The image check compares JPEG to JPEG at the same quality, so compression
noise cancels.

## Stability

- Run A recorded this baseline. Run B, on unchanged source, compared against
  it: every capture had 0 of 32400 blocks moved (phone: 0 of 5194), mean shift
  0.0, 192 probes with 0 outside their bounds, and identical frame SHA-256s.
- A separate `--build` run (rebuilding every unit first) compared the same way:
  0 blocks moved everywhere, 0 probes outside.
- Negative control: before the room clock was pinned, handover-3 moved
  24082 of 32400 blocks and the comparison failed. The check catches real
  drift.

## Known limits

- The phone capture is mostly the Character Create panel.
- The hand-over and Experimental frames include gameplay and UI, so
  non-lighting changes can move them. Read their failures with that in mind.
- Everglade II's baked canopy shading covers only the glade; the path has none
  yet (field value 1000 there).
- Bounds are set for this host and device. Another GPU or Chrome version needs
  its own baseline.
