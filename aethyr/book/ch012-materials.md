# 12. Materials

> **Status: unimplemented — design draft.** Today the art styles are scalar
> Coax functions (`verse/style.coax`) driven by presets
> (`verse/style-preset.coax`) and emitted into each realm's WGSL. This
> chapter plans Aethyr's material model: a small typed record per surface,
> evaluated by one shading path for every realm. Nothing here runs yet.

A material says how a surface answers light: its albedo, how its shading
bands, whether it is outlined, how it glows, and which texture layer it
reads. Realms differ in style (low poly, painterly, photoreal, flat), not in
having separate material systems. Aethyr keeps one material record and lets
the style choose the functions that read it.

## What this chapter teaches

- A material as data: albedo, style parameters, texture layer, outline and
  emission, all in linear RGB.
- Presets evaluated against a clock, so a realm never hand-tunes numbers.
- Texture layers as one mipmapped 2D-array texture, decoded and mipmapped
  the same way on every host.
- Why colour functions work one channel at a time today, and what vectors
  change.

## The method

The painterly style's surface shading is wrapped Lambert, banded:

```jai
w     = sat((n·l + 0.25) / 1.25)                 // terminator past the side
band  = mid · step(e0, s, w) + (1 − mid) · step(e1, s, w)
c     = albedo · (shadow_light + (lit_light − shadow_light) · band)
step(e, s, x) = smoothstep(e − s, e + s, x)
```

Shadow-band colours are cooled by multiplying toward the ambient tint's hue
and rescaled to their original luminance, so warm albedos keep colour
instead of passing through grey ([docs/verse/art-styles.md](../../docs/verse/art-styles.md),
section 3). A preset gives a value at four clock keys (midnight 0, dawn
0.25, noon 0.5, dusk 0.75) and is linear between them.

Outlines are ink where depth jumps (its second difference across a pixel),
the normal turns sharply or the material changes, scaled by how strongly the
material asks for one; a glowing material asks for none.

## What exists today

- [verse/style.coax](../../verse/style.coax): the shading functions
  (`verse_style_wrap`, `verse_style_bands`, `verse_style_shade`,
  `verse_style_cool`, rim, haze, sky, clouds, wind and grass bend), plain f32
  so the Metal scalar profile and `coax wgsl` both accept them.
- [verse/style-preset.coax](../../verse/style-preset.coax): `STYLE_LOWPOLY`,
  `STYLE_PAINTERLY`, `STYLE_PHOTOREAL`, `STYLE_FLAT`, desktop and phone
  tiers, and `verse_style_at`, which fills the 128-float `STYLE_FLOATS`
  uniform block.
- [verse/look.coax](../../verse/look.coax): outlines from depth, normal and
  material edges.
- [verse/haunt-light.coax](../../verse/haunt-light.coax): the Haunted House's
  linear-light material functions (attenuation, ambient, fog), emitted as
  Metal.
- [verse/gpu/layers.coax](../../verse/gpu/layers.coax): texture layers decoded
  and mipmapped in Coax into the layout both GPU hosts upload, including the
  browser's premultiplied round trip.
- Asset packs record each layer's licence and enforce the painterly texture
  rules (at most 512 texels, no normal or roughness maps)
  ([docs/verse/realm-host.md](../../docs/verse/realm-host.md), Asset packs).

## The design in Aethyr

```jai
Material :: struct {
  albedo: Rgb;            // linear
  layer: s32;             // texture layer, −1 untextured
  style: Style;           // lowpoly, painterly, photoreal, flat
  outline: f32;           // 0 none, 1 full
  emission: Rgb;
}
```

A realm declares its materials in a table indexed by a material ID carried
per instance. The shading path in chapter 11's scene pass reads the record,
the style preset at the realm's clock and the light from chapter 13. When
Coax gains vector operators, the per-channel functions keep their names and
gain vector bodies; the checks stay.

## What it builds

- `aethyr/render/material.coax`: the record, the table and its admission
  (a material naming a missing layer or a style the realm does not draw is
  refused at load).
- `aethyr/render/style.coax`: the style functions and presets, moved from
  `verse/`.

## Acceptance checks and oracles

- Each style function's CPU results equal its WGSL and SPIR-V results over a
  grid of inputs within one f16 rounding step.
- Preset interpolation: at each clock key the uniform block equals the
  preset's key exactly; between keys it is linear.
- The fixture's painterly goldens at four times of day and on the phone tier
  (`coax/web/goldens/fixture-painterly-*.png`) are unchanged after the move.
- A material referencing a missing texture layer is refused before drawing.

## Dependencies

Chapter 11's passes and binding table. From Coax:
[chapter 17](../../coax/book/ch17-gpu.md), R9 GPU interfaces, R5 layouts for
the material table.
