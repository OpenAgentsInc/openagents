# 13. Light and shadow

> **Status: unimplemented — design draft.** Today each realm computes its
> key light, ambient, shadow map and fog in its own WGSL, with shared
> scalar pieces in `verse/style.coax` and `verse/light.coax`. This chapter
> plans one lighting stage for Aethyr: a key light from the realm's clock, a
> stable shadow map, ambient and fog. Nothing here runs yet.

Light is where realms look most different and are most alike underneath:
one key light (sun or moon), a sky and ground ambient, a shadow map around
the player, a little contact darkening and distance haze. Aethyr computes
those once from the realm's clock and hands every material the same inputs.

## What this chapter teaches

- A key light from a clock: direction, colour and strength through the day.
- Hemisphere ambient from the surface normal.
- A shadow map that does not shimmer: snapping its centre to texels and
  filtering with a fixed tap pattern.
- Fog and haze as functions of distance and height.
- Tone mapping and encoding as the last step.

## The method

Ambient blends ground and sky by the normal's up component:

```jai
ambient(n) = ground + (sky − ground) · (n_y · 0.5 + 0.5)
```

The shadow map covers a square of reach `r` metres around the player
(60 m on desktop, 40 m on a phone) along the light. To keep shadows still as
the player walks, its centre snaps to whole texels:

```jai
snap(x) = floor(x / texel) · texel,   texel = 2r / size
shadow  = Σ_k w_k · [depth_k + bias ≥ d]     // 9 taps desktop, 4 phone
```

Height fog falls off exponentially with altitude above a base and is capped,
so a far ridge keeps some contrast. The final colour is tone mapped,
`1 − exp(−k · light)`, then encoded for the display.

## What exists today

- [verse/style.coax](../../verse/style.coax): `verse_style_ambient`,
  `verse_style_shadow_reach`, `verse_style_shadow_snap`,
  `verse_style_shadow_taps`, `verse_style_contact`, `verse_style_haze` and
  `verse_style_fog`; key light and its colour per clock key come from
  [verse/style-preset.coax](../../verse/style-preset.coax) (`SU_KEY`,
  `SU_KEY_DIR`).
- [verse/light.coax](../../verse/light.coax): tone (`verse_tone`), bloom,
  encode and decode, and the lit term for characters; on the GPU it was
  checked against its C build for all 18 procedures over 65,536 rows each on
  an RTX 4080 ([native-gpu.md](../../docs/verse/native-gpu.md), G1).
- [verse/haunt-light.coax](../../verse/haunt-light.coax): the night's moon
  direction, moonlight and shadow reach for the Haunted House.
- Everglade's scene pass draws a depth map and samples it in the next pass;
  `verse/gpu/vk-passes.c` supports depth bias and a `less` comparison
  sampler for it.

## The design in Aethyr

One `Lighting` record per frame, filled from the realm's clock and style
preset: key direction and colour, ambient sky and ground, shadow centre and
reach, fog parameters. The scene pass draws the shadow map first and every
material reads the same record; no realm writes its own light. A realm may
add local lights (lanterns, spells) as a bounded list with attenuation
`max(0, 1 − d/r)²`, counted against chapter 16's budgets.

## What it builds

- `aethyr/render/light.coax`: the lighting record, key light by clock,
  ambient, fog, tone and local lights.
- `aethyr/render/shadow.coax`: the shadow pass, its snapped projection and
  the filter taps.

## Acceptance checks and oracles

- Shadow stability: moving the player by less than one texel leaves the
  shadow map's projection unchanged; across a walk, no shadow edge moves by
  more than one texel between frames except as the light moves.
- At each clock key, the key light equals the preset's value; the sun and
  moon never both lead.
- CPU and GPU shading agree within tolerance over a recorded frame (chapter
  11's pass check).
- The goldens at noon, dusk and night (Everglade's perf walk places, the
  painterly fixture) are unchanged after the move.

## Dependencies

Chapters 11 and 12; chapter 1's clock. From Coax:
[chapter 17](../../coax/book/ch17-gpu.md), R9 GPU interfaces.
