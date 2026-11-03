# 12. Materials

> **Status: partly implemented.** The surface shading functions moved from
> `verse/style.coax` into [aethyr/render/material.coax](../render/material.coax),
> with a typed `Material` record, its admission and the CPU form of the
> painterly banded surface. Every realm's WGSL is byte-identical after the
> move. No realm draws from a material table yet; the style presets, the
> outline pass and the texture layers stay in `verse/` (see
> [Still to do](#still-to-do)).

A material says how a surface answers light: its albedo, the texture layer
it reads, the style that draws it, how strongly it asks for an outline, and
what it emits. Realms differ in style (low poly, painterly, photoreal, flat,
anime), not in having separate material systems. Aethyr keeps one record per
surface and one set of shading functions that read it.

## What this chapter teaches

- The shading functions every painterly realm's fragment shader calls, and
  why moving them didn't change a byte of any shader.
- A material as data, and a table that is admitted whole or refused before
  anything draws.
- The banded surface on the CPU, compared bit for bit with a realm's own
  fragment code.

## The shading functions

The painterly style shades a surface in three soft bands. The key light's
`n·l` is wrapped a quarter past the side, so the terminator sits beyond it,
and two smoothed steps place the shadow, mid and lit bands:

<!-- source: aethyr/render/material.coax bands -->
```jai
// smoothstep(e - s, e + s, x): a band edge at e, blended over 2s.
verse_style_soft_step :: (e:f32,s:f32,x:f32)->f32 #export_c {
 t:=st_sat((x-(e-s))/st_max(2.0f32*s,0.000001f32));
 return t*t*(3.0f32-2.0f32*t);
}

// ---- 1. Shading: soft toon bands
// Wrapped Lambert: N.L lifted a quarter, so the terminator sits past the side.
verse_style_wrap :: (ndl:f32)->f32 #export_c {return st_sat((ndl+0.25f32)/1.25f32);}
// Three bands: shadow 0, mid (`mid`, 0.55), lit 1, edges e0 and e1 (0.20, 0.58)
// softened over s (0.06). w is the wrapped term times the shadow map's soft term.
verse_style_bands :: (w:f32,e0:f32,e1:f32,s:f32,mid:f32)->f32 #export_c {
 return mid*verse_style_soft_step(e0,s,w)+(1.0f32-mid)*verse_style_soft_step(e1,s,w);
}
// Two bands, for characters: one edge (0.40, softness 0.08).
verse_style_bands2 :: (w:f32,e:f32,s:f32)->f32 #export_c {return verse_style_soft_step(e,s,w);}
```

These are plain `f32` functions that work on one colour channel at a time,
because the Metal scalar profile has no vectors. A realm's shader is Coax
too (for example, `apps/openagents-web/gpu/fixture.coax`), and `coax wgsl`
emits each function it calls as `coax_p_<name>`. The functions keep their
`verse_style_` names, and `verse/style.coax` loads
`aethyr/render/material.coax`, so every realm's call sites are unchanged.
The check that every committed WGSL module still matches its Coax source
reports no change:

```shell
coax/web/tool.sh coax/web/wgsl-module.coax --site --check
```

```text
wgsl-module: 40 module(s) checked, 0 stale, 0 failed
```

The same move holds on the CPU. Each realm's generated C, before and after,
has the same body for every procedure it had; the C differs only in the
compiler's temporary and source-line numbers, plus the new procedures, which
no realm calls yet.

The module also holds the cool shadow (`verse_style_cool` and
`verse_style_keep_value`: the shadow band takes the tint's hue once more and
is scaled back to its own luminance), the banded mix, the wet-surface dab,
the rim light and `verse_style_luma`.

## The material record

<!-- source: aethyr/render/material.coax record -->
```jai
// The styles a material is drawn in: the values of verse/style-preset.coax's STYLE_*.
MATERIAL_LOWPOLY :: 0;
MATERIAL_PAINTERLY :: 1;
MATERIAL_PHOTOREAL :: 2;
MATERIAL_FLAT :: 3;
MATERIAL_ANIME :: 4;
MATERIAL_STYLES :: 5;
MATERIAL_UNTEXTURED :: -1;   // a material that reads no texture layer
MATERIAL_MAX :: 256;         // materials in one realm's table

// One surface. Colours are linear RGB.
Material :: struct {
  r: f32; g: f32; b: f32;              // albedo, each 0 to 1
  layer: int = MATERIAL_UNTEXTURED;    // the texture layer it reads
  style: int;                          // MATERIAL_*
  outline: f32;                        // how strongly it asks for an outline: 0 none, 1 full
  er: f32; eg: f32; eb: f32;           // the light it emits, 0 or more
}

MATERIAL_OK :: 0;
MATERIAL_ALBEDO :: 1;      // an albedo channel outside 0 to 1
MATERIAL_LAYER :: 2;       // a texture layer the realm doesn't have
MATERIAL_STYLE :: 3;       // a style the realm doesn't draw
MATERIAL_OUTLINE :: 4;     // an outline outside 0 to 1
MATERIAL_EMISSION :: 5;    // an emission below 0
MATERIAL_TABLE :: 6;       // no materials, or more than MATERIAL_MAX

// Why one material can't be drawn by a realm with `layers` texture layers that draws the
// styles whose bits are set in `styles` (bit MATERIAL_PAINTERLY for painterly), or
// MATERIAL_OK. A NaN fails every range, so it is refused too.
material_check :: (m: Material, layers: int, styles: int) -> int {
  if !(m.r >= 0 && m.r <= 1 && m.g >= 0 && m.g <= 1 && m.b >= 0 && m.b <= 1) { return MATERIAL_ALBEDO; }
  if m.layer < MATERIAL_UNTEXTURED || m.layer >= layers { return MATERIAL_LAYER; }
  if m.style < 0 || m.style >= MATERIAL_STYLES || ((styles >> m.style) & 1) == 0 { return MATERIAL_STYLE; }
  if !(m.outline >= 0 && m.outline <= 1) { return MATERIAL_OUTLINE; }
  if !(m.er >= 0 && m.eg >= 0 && m.eb >= 0) { return MATERIAL_EMISSION; }
  return MATERIAL_OK;
}

// The first material a table can't have, by its index, and why. index -1 with MATERIAL_OK:
// the table is admitted whole. index -1 with MATERIAL_TABLE: the table itself is refused.
MaterialRefusal :: struct { index: int = -1; reason: int = MATERIAL_OK; }

// A realm's table, admitted whole or refused at load, before anything draws.
material_admit :: (table: [] Material, layers: int, styles: int) -> MaterialRefusal {
  if table.count == 0 || table.count > MATERIAL_MAX { return MaterialRefusal.{reason = MATERIAL_TABLE}; }
  for i: 0..<table.count {
    why := material_check(table[i], layers, styles);
    if why != MATERIAL_OK { return MaterialRefusal.{index = i, reason = why}; }
  }
  return MaterialRefusal.{};
}

// How strongly the outline pass inks a material: what it asks for times the style's most (the
// preset's outline_max). A material that glows asks for none.
material_ink :: (m: Material, most: f32) -> f32 {
  if m.er > 0 || m.eg > 0 || m.eb > 0 { return 0; }
  return m.outline * most;
}
```

`material_check` refuses a material for the first of these reasons, in this
order:

1. **Albedo.** A channel outside 0 to 1. A NaN fails every range, so it is
   refused too.
2. **Layer.** A texture layer the realm doesn't have. `MATERIAL_UNTEXTURED`
   (-1) reads none.
3. **Style.** A style the realm doesn't draw. `styles` has one bit for each
   style the realm draws.
4. **Outline.** An outline outside 0 to 1.
5. **Emission.** An emitted channel below 0.

`material_admit` takes the whole table or refuses it at its first bad
material, with its index and reason. A table with no materials, or more than
256, is refused itself, with index -1. `material_ink` is how strongly the
outline pass inks a material: what it asks for times the style's most, the
preset's `outline_max`. A material that glows asks for none.

## The banded surface

<!-- source: aethyr/render/material.coax banded -->
```jai
MatRgb :: struct { r: f32; g: f32; b: f32; }
mat_rgb :: (r: f32, g: f32, b: f32) -> MatRgb { return MatRgb.{r = r, g = g, b = b}; }

// Where a surface sits in the three bands: the wrapped key light (n.l) times the shadow map's lit
// share, against the preset's edges e0 and e1, softness and mid band.
material_band :: (ndl: f32, lit: f32, e0: f32, e1: f32, soft: f32, mid: f32) -> f32 {
  return verse_style_bands(verse_style_wrap(ndl) * lit, e0, e1, soft, mid);
}

// One banded surface, the CPU form of a painterly realm's fragment shader (gpu/fixture.coax,
// fixture_banded): the albedo `a` under `dark` (the shadow tint times the ambient), cooled toward
// the tint `t` by `cool` with its luminance kept, and under `bright` (the key plus the ambient),
// mixed by the band.
material_banded :: (a: MatRgb, dark: MatRgb, bright: MatRgb, t: MatRgb, cool: f32, band: f32) -> MatRgb {
  s := mat_rgb(a.r * dark.r, a.g * dark.g, a.b * dark.b);
  tl := verse_style_luma(t.r, t.g, t.b);
  m := mat_rgb(verse_style_cool(s.r, t.r, tl, cool), verse_style_cool(s.g, t.g, tl, cool), verse_style_cool(s.b, t.b, tl, cool));
  y := verse_style_luma(s.r, s.g, s.b);
  ym := verse_style_luma(m.r, m.g, m.b);
  c := mat_rgb(verse_style_keep_value(m.r, y, ym), verse_style_keep_value(m.g, y, ym), verse_style_keep_value(m.b, y, ym));
  return mat_rgb(verse_style_banded(c.r, a.r * bright.r, band), verse_style_banded(c.g, a.g * bright.g, band), verse_style_banded(c.b, a.b * bright.b, band));
}
```

`material_banded` is the CPU form of the fixture realm's `fixture_banded`.
It takes the albedo `a`, the shadow band's light `dark` (the shadow tint times
the hemisphere ambient), the lit band's light `bright` (the key plus the
ambient), the tint `t` and the preset's `cool`. The shadow colour is cooled
toward the tint and rescaled to its luminance, then mixed with the lit colour
by the band.

## A worked example

Take a level patch of grass, albedo (0.21, 0.29, 0.08), under the painterly
preset at noon. The preset's bands have edges 0.20 and 0.58, softness 0.06
and a mid band of 0.55. At noon, `bright` is (1.3025, 1.313, 1.312).

1. **The wrap.** At `n·l` = 0.5, `w` = (0.5 + 0.25) / 1.25 = 0.6, and the
   surface is fully lit (`lit` = 1).
2. **The first edge.** 0.6 is past 0.20 + 0.06, so the first step is 1.
3. **The second edge.** It runs from 0.58 − 0.06 = 0.52 to 0.64. `w` is
   0.08 past its start, so `t` = 0.08 / 0.12 = 2/3, and the smoothed step is
   t²(3 − 2t) = 4/9 · 5/3 = 20/27 = 0.7407.
4. **The band.** 0.55 · 1 + 0.45 · 0.7407 = 0.8833.
5. **The lit colour.** At `n·l` = 1 the band is 1, and the colour is the
   albedo times `bright`: 0.21 · 1.3025 = 0.2735, 0.29 · 1.313 = 0.3808 and
   0.08 · 1.312 = 0.1050.
6. **The mix.** At band 0.8833 the red channel is the shadow's 0.0276 plus
   0.8833 of the way to 0.2735: 0.0276 + 0.8833 · 0.2459 = 0.2448.

The example [aethyr/examples/shade-a-material.coax](../examples/shade-a-material.coax)
admits a table of three materials, refuses two broken ones, and shades the
grass with the real preset. It also runs the fixture realm's own fragment
code, `fixture_banded` from `apps/openagents-web/gpu/fixture.coax`, on the
CPU, and compares the colours bit for bit:

<!-- source: aethyr/examples/shade-a-material.coax example -->
```jai
// Colours rounded to four places: the cooling takes a pow, which each platform's libm rounds in
// its own last bit.
r4 :: (x: f32) -> f32 { return cast(f32) floor(x * 10000 + 0.5) / 10000; }
LAYERS :: 4;   // the realm's texture layers
STYLES :: 1 << MATERIAL_PAINTERLY;   // it draws only the painterly style

main :: () {
  table := [3] Material.[
    Material.{r = 0.21, g = 0.29, b = 0.08, layer = 0, style = MATERIAL_PAINTERLY, outline = 1},   // grass
    Material.{r = 0.42, g = 0.40, b = 0.36, layer = 1, style = MATERIAL_PAINTERLY, outline = 1},   // stone
    Material.{r = 0.9, g = 0.8, b = 0.5, style = MATERIAL_PAINTERLY, outline = 1, er = 2, eg = 1.6, eb = 0.8}];   // a lantern
  r := material_admit(table[..], LAYERS, STYLES);
  print("table of %: refused %, index %\n", table.count, material_reason_name(r.reason), r.index);
  print("ink: grass %, lantern %\n", material_ink(table[0], 0.8), material_ink(table[2], 0.8));

  roof := table; roof[1].layer = 4;   // a fifth layer the realm doesn't have
  r = material_admit(roof[..], LAYERS, STYLES);
  print("with layer 4 of %: refused %, index %\n", LAYERS, material_reason_name(r.reason), r.index);
  flat := table; flat[2].style = MATERIAL_FLAT;
  r = material_admit(flat[..], LAYERS, STYLES);
  print("with a flat lantern: refused %, index %\n", material_reason_name(r.reason), r.index);

  // The painterly preset at noon, where the fixture's shader reads it: 156 floats in.
  u: [156 + STYLE_FLOATS] f32;
  if !verse_style_at(verse_style_preset(STYLE_PAINTERLY, STYLE_DESKTOP), 0.5, u[156..<156 + STYLE_FLOATS]) { print("no preset\n"); return; }
  s := u[156..<156 + STYLE_FLOATS];
  print("bands: edges % and %, softness %, mid %; cool %\n", s[SU_BANDS], s[SU_BANDS + 1], s[SU_BANDS + 2], s[SU_BANDS + 3], s[SU_SHADOW + 3]);
  // A level patch of grass (its normal straight up) under the noon key.
  ny := 1.0f32;
  amb := mat_rgb(verse_style_ambient(ny, s[SU_SKY_AMB], s[SU_GROUND_AMB], s[SU_SKY_AMB + 3]),
                 verse_style_ambient(ny, s[SU_SKY_AMB + 1], s[SU_GROUND_AMB + 1], s[SU_SKY_AMB + 3]),
                 verse_style_ambient(ny, s[SU_SKY_AMB + 2], s[SU_GROUND_AMB + 2], s[SU_SKY_AMB + 3]));
  tint := mat_rgb(s[SU_TINT], s[SU_TINT + 1], s[SU_TINT + 2]);
  k := s[SU_KEY + 3];
  dark := mat_rgb(tint.r * amb.r, tint.g * amb.g, tint.b * amb.b);
  bright := mat_rgb(s[SU_KEY] * k + amb.r, s[SU_KEY + 1] * k + amb.g, s[SU_KEY + 2] * k + amb.b);
  print("noon: bright % % %, dark % % %\n", bright.r, bright.g, bright.b, dark.r, dark.g, dark.b);
  g := table[0];
  a := mat_rgb(g.r, g.g, g.b);
  same := true;
  for i: 0..<3 {
    lit := ifx i == 2 then 0.0f32 else 1.0f32;   // the last in the key's cast shadow
    ndl := ifx i == 0 then 0.5f32 else 1.0f32;
    band := material_band(ndl, lit, s[SU_BANDS], s[SU_BANDS + 1], s[SU_BANDS + 2], s[SU_BANDS + 3]);
    c := material_banded(a, dark, bright, tint, s[SU_SHADOW + 3], band);
    print("n.l %, lit %: band %, colour % % %\n", ndl, lit, r4(band), r4(c.r), r4(c.g), r4(c.b));
    f := fixture_banded(v3(a.r, a.g, a.b), v3(dark.r, dark.g, dark.b), v3(bright.r, bright.g, bright.b), band, u[..]);
    same = same && f.x == c.r && f.y == c.g && f.z == c.b;
  }
  print("the fixture's fragment code gives the same colours, bit for bit: %\n", same);
}
```

```shell
coax/bin/coax run aethyr/examples/shade-a-material.coax
```

```text
table of 3: refused none, index -1
ink: grass 0.8, lantern 0.0
with layer 4 of 4: refused layer, index 1
with a flat lantern: refused style, index 2
bands: edges 0.2 and 0.58, softness 0.06, mid 0.55; cool 1.5
noon: bright 1.3025 1.313 1.312, dark 0.1573 0.21054001 0.39732
n.l 0.5, lit 1.0: band 0.8833, colour 0.2448 0.3434 0.0993
n.l 1.0, lit 1.0: band 1.0, colour 0.2735 0.3808 0.105
n.l 1.0, lit 0.0: band 0.0, colour 0.0276 0.0602 0.0565
the fixture's fragment code gives the same colours, bit for bit: true
```

The two refusals are the broken cases. A stone that names layer 4 of a
realm with four layers is refused at index 1, and a lantern drawn flat in a
realm that draws only painterly is refused at index 2. In both, nothing in
the table is admitted. The colours are printed to four places, because the
cooling takes a `pow` that each platform's maths library rounds in its own
last bit.

## Checks

```shell
coax/bin/coax test aethyr/render/material.coax
coax/bin/coax mutate aethyr/render/material.coax
coax/bin/coax test verse/style.coax
```

`aethyr/render/material.coax` has 17 checks. The nine shading checks moved
with their functions from `verse/style.coax`: the band edges, bands that
never step back as the light grows, the wrap, the characters' two bands, the
shadow band under the tinted ambient, the cool shadow that keeps its value,
the dab, and the rim's angles and its width on a figure. The new checks
cover:

- each range in the record, at and just past its edges, and the order of
  the refusals;
- a table admitted whole, refused at its first or a later material, and
  refused itself when empty or past 256;
- the ink rule and the reasons' names;
- the band worked by hand above, and `material_banded` against plain
  arithmetic in the lit band, the shadow band, uncooled and halfway;
- `st_min` and `st_max` on two zeros of opposite sign, which tells the
  comparison apart from its `<=` mutant.

`coax mutate` kills all 140 mutants. A golden walk on Linux hardware
(`coax/web/golden-check.sh --commit`, headless Chrome with the GPU's WebGPU
adapter) checks the pages the move can reach against their goldens.

## Still to do

- **A material table in each realm.** The record and its admission exist,
  but realms still pass albedo and style through their own vertex words. A
  realm declaring its table, with a material ID per instance read by the
  scene pass, is the next step.
- **Presets and outlines.** The style presets (`verse/style-preset.coax`),
  the outline pass (`verse/look.coax`), the flat and anime functions and the
  sky, cloud, grass and foliage functions stay in `verse/style.coax`.
- **Texture layers.** [verse/gpu/layers.coax](../../verse/gpu/layers.coax)
  decodes and mipmaps texture layers into the layout both GPU hosts upload.
  Admission checks a layer index against the realm's count; moving the
  decoder and the asset packs' texture rules into Aethyr remains.
- **GPU agreement per function.** Each shading function's CPU result equal
  to its WGSL and SPIR-V results over a grid of inputs is not recorded yet.
  Today the agreement is whole-frame: chapter 11's Grid profile.

## Dependencies

Chapter [11](ch011-the-wgsl-renderer.md)'s passes and binding table. From
Coax: [chapter 17](../../coax/book/ch17-gpu.md), GPU interfaces.
