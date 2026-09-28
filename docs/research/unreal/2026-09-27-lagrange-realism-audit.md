# Lagrange 1 realism audit and roadmap

**Status:** Audit and roadmap, 2026-09-27; implemented the same day under
tracking issue [#9803](https://github.com/OpenAgentsInc/openagents/issues/9803).
See [Implementation record](#implementation-record) for what landed, what
changed from the plan, and what was not adopted. The sections after it are
the original audit. Studied under
[AGENTS.md](AGENTS.md). The rigid-body solver is covered separately in
[the Chaos physics candidates](2026-09-27-chaos-physics-candidates.md). This
note covers everything else that makes [Lagrange 1](../../verse/lagrange-1.md)
look and behave like the real place: lighting, materials, camera, sky,
precision, and non-rigid effects.

**Source studied:** Unreal Engine 5.8.3 (`release` at `396c9f05`). The
checkout was sparse and included:

- `Engine/Shaders/Private`
- `Engine/Source/Runtime/Renderer`
- the sky, light, post-process, and camera classes
- `Engine/Source/Runtime/Core/Public/Math`
- `CableComponent`, `ChaosCloth`, and Niagara

Unreal paths below are pointers only. This note contains no Unreal code, and
every item names the public paper, dataset, or permissive engine to build from.

**Compared against:** `crates/verse` and `crates/verse-lagrange` at
`f8e5ad73c1`.

## Implementation record

Commits on `main`: data `7694952cea`; physics `718ef7aa9c`, `ecbfe7f6df`,
`3383a1d5eb`, `7ca3498fc6`; renderer and zone `a8ae917044`, `527584b9ee`,
`00186d2efe`. Captures, including before and after views and a DSCOVR EPIC
comparison, are in [`docs/verse/captures/lagrange-1-realism/`](../../verse/captures/lagrange-1-realism/README.md).
The runtime guide is [Lagrange 1](../../verse/lagrange-1.md).

| Candidate | Outcome |
| --- | --- |
| V1 sky at infinity | Landed as a sky pass drawn first at infinity in explicit distance order, plus reversed depth for the whole physical path. Reversed depth was needed anyway: solar cells sit 1 mm proud of their panel. |
| L1 lit path | Landed: forward shading in a separate pipeline; amber zones unchanged. |
| C1–C3 HDR, EV100, tone map | Landed. Tone mapping is Khronos PBR Neutral (Apache-2.0 reference) rather than AgX, for a clearly licensed, hue-preserving curve. |
| C2/L6 physical units | Landed: about 130,000 lux of sunlight, Earthshine as a disc light at its physical ratio (tested at a few millionths), no ambient floor. |
| C8 wide lines | Landed for guides; structural bracing became geometry. |
| M1–M6 materials | Landed, including the material table. MLI crinkle uses smooth wrinkle noise folded into roughness below pixel scale. |
| L3 shadows, L2 | L3 landed (one fitted 2048² map with PCSS from the Sun's radius). L2 was not used: the lattice trusses exceed the analytic-primitive budget. |
| L4 bounce, M7 | Landed: a 3 m irradiance-probe grid baked on a worker thread, rebaked when resting parts change; baked per-vertex occlusion; specular occlusion. |
| V2–V7 sky bodies, M8 | Landed: limb-darkened Sun, textured Earth with haze, glint, and eclipse shadow, Moon with lunar-Lambert and opposition surge, Yale BSC stars and Milky Way. M8 is analytic: the Earth is a second disc light for specular, which is the whole environment worth reflecting in vacuum. |
| C4–C7, C9, C10 camera | Landed: bloom, metered auto exposure that ignores empty space and protects highlights, local exposure, vignette, grain, white balance, diffraction spikes, faint lens ghosts. Lateral chromatic aberration is implemented but off by default: at these contrasts it fringes every glint. |
| E1–E7 effects | Landed: rope tubes, stateless seeded particles, honest plume condensate, XPBD ropes, plume impingement with an `impingement` ledger term, array modes, and vent ice flakes. |
| M9–M12 | Clear coat, thin film, sheen, and anisotropy landed. M9 is the probe radiance sampled in the reflection direction, not a cube capture. |
| E8 rope coupling | Implemented behind `Station::rope_coupling`, off by default: coupled, the arrest force exceeds the 3 kN cap slightly and scripted moves yank the lines. |
| V8 camera-relative positions | Not adopted: the zone spans under 200 m, where single precision holds 20 µm; the sky is direction-only. |
| C11 TAA | Not adopted: with 4× MSAA, specular antialiasing, and wide guide lines, captures show no crawl on the thinnest members (see the truss in `after-wide.jpg`). |
| C12 motion blur | Not adopted: an action camera in full sun at EV 15 and f/2.8 exposes about 1/4,000 s, under 0.1 px of blur at the zone's speeds. |
| C13 HDR display output | Landed under [#9806](https://github.com/OpenAgentsInc/openagents/issues/9806): an RGBA16F extended linear sRGB surface on EDR screens (iOS through the host layer, macOS through wgpu's layer), and a tone-curve shoulder that bends toward the live headroom. `hdr_probe` shows highlights reaching the ceiling with zero change below the shoulder, and the plaza stays in standard range. |
| E9 MLI flaps | Not adopted: station blankets are taped and stitched, and a cloth solver for a cosmetic edge is not worth its cost on mobile. |

**Changed from the plan.** The station now holds a 30° pitch about its truss
axis so the Sun is not exactly along the modules, which left every module side
at zero incidence. Radiators stay edge-on, the fixed arrays keep 87 % of full
sunlight, and the tidal field is evaluated through the attitude. The default
spawn heading is 65° off the Sun line. The helmet camera meters automatically
(EV 10 to 15.5), as small action cameras do; the fixed sunny-16 exposure is the
starting point.

**Validation.** The Earth close-up's mean disc color is within 1 %, 4 %, and
8 % (red, green, blue) of the DSCOVR EPIC frame of 2026-09-25 11:10 UTC,
about 15° of longitude earlier than the render. Tests cover the ephemeris,
catalogue and texture decoding, the ray-cast hierarchy and bakes, the sky in
real units, and Earthshine.

## Where Lagrange stands

The simulation side is careful. It has a real CR3BP orbit in f64, true
angular sizes for the Sun, Earth, and Moon, and a momentum-ledgered rigid-body
step. The rendering side is a placeholder from the stylized amber world:

| Area | Today | File |
| --- | --- | --- |
| Pipeline | One 46-line WGSL shader. The vertex holds only position, linear color, and fog weight. It draws straight to an 8-bit sRGB target with 4× MSAA. | `crates/verse/src/shader.wgsl`, `render.rs`, `mesh.rs` |
| Lighting | CPU-baked per face: `0.1 + 0.9·sun + 0.06·earth + 0.1·|n.y|`. No specular, shadows, or bounce. | `zones/lagrange.rs` `lit()` |
| Materials | Five RGB constants (`WHITE`, `GOLD`, `METAL`, `CELL`, `SAFETY`) used as diffuse colors. | `zones/lagrange.rs` |
| Sun | A disc with color `[3.0, 2.9, 2.6]` that clips, plus 12 "ray" lines. | `zones/lagrange.rs` `sun()` |
| Earth, Moon | Flat discs on a 1.85 km sky shell. Earth has four cloud blobs and a "continent". Neither rotates nor shows phase. | `zones/lagrange.rs` `body_disc` |
| Stars | 900 random triangles at 1.9 km, hand-culled near the Sun. | `zones/lagrange.rs` `stars()` |
| Depth | Standard Z, near 0.1 m, far 2,000 m, `Depth32Float`, `LessEqual`. | `camera.rs`, `render.rs` |
| Tethers, plumes | Tethers are rigid joints drawn only in the debug overlay. Plumes are single fading 1-px lines. | `verse-lagrange/src/station.rs`, `zones/lagrange.rs` |

Targets are desktop, iOS (Metal), and Android, so every item below has a
mobile cost.

## Findings that are bugs today, not polish

1. **The sky's depth layering is unresolvable.** With near 0.1 m and far
   2,000 m, one step of 32-bit depth at the 1,850 m shell is about 2 m.
   `body_disc` layers Earth's clouds 0.5 m and its continent 0.3 m in front
   of the disc, and the Earth and Moon both sit at exactly `SKY`. The layer
   order is decided by rounding and can flicker. A Moon transit across
   Earth, which DSCOVR photographed from L1 in 2015, has no defined order.
2. **Shadowed faces are thousands of times too bright.** Direct sunlight at
   L1 is about 128–133 klx. Earthshine from a 0.49° full Earth with albedo
   ≈ 0.3 is about 0.5–1 lx, a ratio near 5×10⁻⁶. `lit()` gives the Earth
   term 0.06 and every face a 0.1 floor. Real deep-space photos show
   near-black shadows. What light reaches them is bounce from the station's
   own white and gold surfaces, not Earth.
3. **The Sun's glare and the missing stars are faked.** Both follow from
   exposure. At a sunlit exposure (EV100 ≈ 15), a magnitude-0 star
   (≈ 2×10⁻⁶ lx) is far below black. The ray lines and the cull near the Sun
   are special cases that a physical camera removes.
4. **Metals are painted, not metallic.** Aluminium, gold visors, and the
   Kapton-over-aluminium "gold" MLI are almost entirely specular. As diffuse
   colors (`METAL` 0.52, `GOLD` as albedo) they read as chalk.

## What is visible at this scale

These figures, at the default 1-radian vertical field of view on a 1080p
screen, set what is worth building:

| Object | Pixels |
| --- | --- |
| Sun | ≈ 10 |
| Earth | ≈ 9 |
| Moon | 2–3 |
| Stars | sub-pixel |

Earth's atmosphere is about 1.5% of its radius, well under a pixel. So a
full Unreal-style SkyAtmosphere, volumetric clouds, or aerial perspective
would spend a large budget on detail nobody can see. A precomputed,
correctly lit Earth image carries the realism.

Similarly, the Sun's 0.54° width gives a penumbra of about 0.94 cm per metre
between occluder and receiver. Shadows are nearly hard. The problem is
resolving their edge, not softening them.

## Roadmap

The phases are ordered so that each one is visible and testable on its own.
Unreal and public-source pointers are in the candidate lists after the
roadmap.

### R0 — Correct foundations

1. **Draw the sky at infinity with reversed-Z** (V1). This fixes finding 1
   and lifts the 2 km cap on the station.
2. **Split out a Lagrange render path** (L1). Give it its own pipeline and a
   vertex format with normal, tangent, and material id, so that Verse's
   amber zones keep their look.
3. **Add a linear HDR target and a separate output pass** (C1). Apply
   exposure, the AgX tonemap, and sRGB encoding in that pass, and draw the
   UI after it.
4. **Use physical light units and a manual EV100 camera** (C2). Set the Sun
   to about 130 klx and Earthshine to its physical ratio, and remove the
   ambient floor (L6). Add an optional "art" exposure preset for a readable
   stylized mode.
5. **Draw prefiltered wide lines** (C8). This is analytic-coverage quads with
   a width floor, so tethers and truss edges stop crawling. MSAA does not
   antialias hardware lines.
6. **Upgrade the capture tool.** Add fixed-EV `lagrange_capture` shots
   (`sun`, `earth`, `jig`, plus shadow-side views) as the reference set every
   later phase is compared against.

**Accept:**
- The Earth and Moon layers never swap in a scripted transit.
- A sunlit white panel sits near display white at EV 15, and a shadowed face
  is within a few percent of black.
- Stars are invisible in sun-facing views without culling code, and visible
  at a dark exposure.
- The other Verse zones are unchanged.

### R1 — Surfaces that respond to light

1. **Add the GGX BRDF** (M1): Lambert diffuse, height-correlated Smith, and
   Schlick Fresnel, evaluated per pixel against the Sun.
2. **Treat the Sun as a disc light** (M2). The 0.27° angular radius sets the
   specular highlight size.
3. **Add geometric specular antialiasing** (M3).
4. **Replace the color constants with a material table** (M4). Base color,
   metallic, and roughness per real material are in the table below.
5. **Add procedural crinkle normals for MLI** (M5): Voronoi facets a few cm
   across with ±20° tilt, plus variance folded into roughness.
6. **Add multiscatter energy compensation** (M6).

**Accept:**
- Captures show a moving specular glint on the visor and cover glass.
- MLI breaks into facets.
- Bare metal has no diffuse term.
- Rough metals do not lose energy in a furnace test.

### R2 — Shadow and bounce

1. **Add sun shadows with a physically correct penumbra.** Either:
   - **Analytic primitive shadows (L2):** march sun rays through the scene's
     boxes, capsules, and cylinders as analytic distance fields, with the
     penumbra from the true Sun cone. This is exact, has no bias, and is
     cheap at about 64 primitives. Recommended while the scene is built from
     primitives.
   - **One fitted shadow map plus PCSS (L3):** use this once the scene
     outgrows analytic primitives.
2. **Bake station bounce light and AO on the CPU** (L4). Use sun visibility
   by ray-cast, then one or two diffuse bounces weighted by albedo, and rerun
   only when the assembly changes. With Earthshine at its true level, this is
   what lights the dark side.
3. **Add specular occlusion from the baked AO** (M7).

**Accept:**
- Truss members cast crisp shadows whose penumbra widens with distance.
- Gold MLI casts a warm fill onto shadowed neighbours.
- Recessed faces do not mirror the Earth.

### R3 — The sky as it really looks from L1

1. **Use a real star catalogue** (V4). Take the Yale Bright Star Catalogue to
   magnitude 6.5. Convert real positions into the rotating Sun–Earth frame
   from `orbit.rs`, with B–V colors and magnitude-to-illuminance. Draw
   energy-conserving splats of fixed pixel size. Add the Milky Way from NASA
   SVS Deep Star Maps.
2. **Draw the Sun disc in physical units with limb darkening** (V2), and
   delete the ray lines.
3. **Render Earth as a textured, lit impostor** (V3):
   - a ray-cast sphere on a quad with Blue Marble surface and cloud textures;
   - rotation driven by the orbital clock (one turn per 24 s of play);
   - phase from the real Sun–Earth–vehicle angle;
   - a limb band from a small lookup table baked offline from a Bruneton or
     Hillaire atmosphere model.

   This needs a texture path in the renderer, which does not exist today.
4. **Shade the Moon with Hapke or Lommel-Seeliger and an opposition surge**
   (V5). Use albedo 0.12 and the LRO mosaic. From L1 the Moon is always near
   full, with a phase angle under about 20°.
5. **Add ocean sun glint on Earth** (V6).
6. **Reflect the sky in environment specular** (M8). Prefilter a cube map of
   the analytic sky (black, Earth, Moon, stars) with a split-sum lookup table,
   so visors and glass show the Earth.

**Accept:**
- `earth` captures match DSCOVR EPIC natural-color images of the same date
  and phase in disc brightness, color, and terminator position, within a
  stated tolerance.
- Constellations are correct for the scene frame.
- The Moon is visibly darker than Earth.

### R4 — The camera

1. **Add energy-conserving mip-chain bloom with no threshold** (C4). Sun glare
   and specular bloom then come from real energy.
2. **Add camera character** (C6): cos⁴ vignette, shadow-weighted grain, mild
   chromatic aberration, and white balance baked into the tonemap lookup
   table. Include a "helmet cam" preset.
3. **Add clamped auto exposure** (C5), off by default, EV100 12–16, slow. For
   looking into the shadow side or at Earth.
4. **Add local exposure** (C7): keep the lit and shadowed sides both readable,
   as phone and GoPro cameras do. Desktop first.

**Accept:**
- The Sun blooms and glints spread with no geometry hacks.
- A shadow-side view under auto or local exposure shows the bounce light from
  R2 rather than black.

### R5 — Effects and non-rigid accuracy

This phase can start in parallel with R1 because it touches different files.

1. **Render ropes and streaks as ribbons and tubes** (E1), using
   rotation-minimizing frames.
2. **Adopt a stateless, seeded particle contract** (E2). A particle's position
   is a closed-form function of seed, birth tick, and time: a straight line
   plus solar radiation pressure. Replays then reproduce effects with no
   stored state and no compute pass.
3. **Draw cold-gas plumes as they really look** (E3). An N₂ plume is nearly
   invisible, leaving at about 690 m/s. Show a brief glint of sunlit
   condensate at pulse onset, gated by a Sun ray-cast, with an optional HUD
   cone for readability.
4. **Add a visible tether rope** (E4). The rigid joint stays authoritative for
   arrest, tension, and the ledger. A 64–128-particle XPBD rope is pinned to
   the joint anchors and limited from both ends, so it lies straight when
   taut, curves when slack, and carries a whip wave. It lives in
   `crates/physics` so replays reproduce its shape.
5. **Add plume impingement forces** (E5). Treat each firing thruster as a
   free-molecular point source, apply pressure on nearby collider faces, and
   add an `impingement` external term to the ledger (with the matching
   deduction from `exhaust`), so conservation stays exact. This is an accuracy
   item, not polish: real EVA and RCS planning is constrained by it.
6. **Model flex modes of the solar arrays** (E6). Use two or three damped modal
   oscillators per wing at 0.1–1 Hz, excited by impingement and
   station-keeping burns, with exact discrete-time updates and vertex
   displacement in the renderer.
7. **Add fireflies and ice flakes** (E7). These are sunlit specks drifting
   anti-sunward under solar radiation pressure (≈ 10⁻⁴ m/s² for a 100 µm
   flake), vanishing in shadow.

**Accept:**
- A slack tether visibly curves and whips.
- Firing near a free part moves it, and the ledger balances.
- Arrays ring down after a burn.
- Replaying a journal reproduces every rope shape and particle.

### R6 — Optional depth

Take these on only when a capture comparison shows the gap:

- Local station reflections (M9) from a low-rate cube capture.
- Clear coat and thin-film iridescence (M10, M11) for cover glass and anodize.
- Suit sheen and brushed-metal anisotropy (M12).
- A PSF sprite or FFT convolution bloom for diffraction spikes (C9).
- Lens ghosts (C10), TAA (C11), and motion blur (C12).
- HDR display output (C13).
- Two-way rope coupling (E8) and MLI edge flaps (E9).
- Contact shadows or reflective shadow maps (L5).
- The Moon's shadow on Earth during solar eclipses (V7).
- Camera-relative f64 positions (V8), needed only if the station grows past
  a few km or bodies move to true distance.

### Order and dependencies

```
R0 ──► R1 ──► R2 ──► R4
 │      └──────► R3 (environment specular needs R1)
 └─► R3 (stars, Sun, Earth need R0 only)
R5 runs alongside; E5 → E6; E4 → E8
Physics candidates S2 (energy-free correction) should land before E4/E5,
so the energy ledger is trustworthy when new forces arrive.
```

### Decisions for the owner

- **Look.** This roadmap gives Lagrange a physically based path separate from
  the amber style used by the other Verse zones. Should Lagrange default to
  realistic, to the "art" exposure preset, or offer both?
- **Assets.** The zone today generates all geometry in code and fetches no
  asset pack. R3 needs a few public-domain datasets: Blue Marble surface and
  cloud textures (about 2–4 MB at 1024×512 to 2048×1024), the LRO Moon mosaic,
  the Yale BSC (about 0.5 MB), and a low-resolution Milky Way map. Decide
  whether they are committed, generated at build time, or fetched and cached.
- **Mobile tiers.** Items marked desktop-first (local exposure, FFT bloom,
  TAA) need a quality-tier setting, which the renderer does not have yet.

## Candidates

The candidate IDs match the roadmap. Priority: P1 adopt soon, P2 later, P3 note
only. Size: S under a day, M a few days, L a phase of its own.

### Lighting (L)

| ID | Candidate | Unreal pointer | Public reference | P / size |
| --- | --- | --- | --- | --- |
| L1 | Forward shading in a single pass with one directional light, matching Unreal's mobile path. It keeps MSAA working and tile memory cheap. | `Renderer/Private/MobileShadingRenderer.cpp`, `MobileBasePass.cpp`; `Shaders/Private/MobileBasePassPixelShader.usf` | Bevy forward PBR (wgpu, MIT/Apache); Filament docs | P1 / M |
| L2 | Analytic primitive soft shadows: march sun rays through box, capsule, and cylinder distance fields, with penumbra from the Sun cone. | `Shaders/Private/DistanceFieldShadowing.usf`, `CapsuleShadowShaders.usf` | Wright, "Dynamic Occlusion with Signed Distance Fields" (SIGGRAPH 2015); Quilez SDF soft shadows | P1 / M |
| L3 | One shadow map fitted to the station (not the frustum), with texel snapping, normal offset, and PCSS scaled by tan(0.27°). | `ShadowRendering.cpp` (`r.Shadow.CSM*Bias`); `ShadowPercentageCloserFiltering.ush` | Fernando, PCSS (2005); Microsoft "Common Techniques to Improve Shadow Depth Maps" | P2 / M |
| L4 | CPU radiosity bake of station bounce light and AO, rerun when the assembly changes. | Concept only: `IndirectLightingCache.cpp`, `LightmapCommon.ush` | Cohen & Wallace, *Radiosity and Realistic Image Synthesis* | P1 / M |
| L5 | Screen-space contact shadows; reflective shadow maps for bounce from moving bodies. | `ScreenSpaceShadows.usf` (`r.ContactShadows`); Lumen card concepts | Bend Studio screen-space shadows (SIGGRAPH 2023, Apache-2.0 sample); Dachsbacher & Stamminger RSM (2005) | P3 / S–M |
| L6 | Black vacuum sky light. Earthshine becomes a second disc light at its physical ratio, not a fill. | `SkyLightComponent.h` (`bLowerHemisphereIsBlack`) | Lagarde & de Rousiers, "Moving Frostbite to PBR" (2014) | P1 / S |

### Materials (M)

| ID | Candidate | Unreal pointer | Public reference | P / size |
| --- | --- | --- | --- | --- |
| M1 | GGX with height-correlated Smith, Schlick Fresnel, and Lambert diffuse. | `Shaders/Private/BRDF.ush`, `ShadingModels.ush` | Walter 2007; Heitz 2014; Karis, "Real Shading in UE4" (2013); Filament | P1 / S |
| M2 | Sun as a disc light for specular (representative point or roughness widening). | `AreaLightCommon.ush`; `LightSourceAngle` | Karis 2013; Frostbite 2014 §4 | P1 / S |
| M3 | Geometric specular antialiasing from normal derivatives. | Substrate roughness clamping | Kaplanyan et al. 2016; Tokuyoshi & Kaplanyan 2019 | P1 / S |
| M4 | Material table replacing the color constants (see below). | Substrate slab parameters | glTF 2.0 PBR and KHR extensions | P1 / S |
| M5 | Procedural MLI crinkle normals with variance folded into roughness. | `Substrate/Glint/` (true glints: skip) | Worley 1996; Olano & Baker, LEAN (2010) | P1 / M |
| M6 | Multiscatter energy compensation. | `ShadingEnergyConservation.ush` | Kulla & Conty 2017; Turquin 2019; Fdez-Agüera 2019 | P2 / S |
| M7 | Specular occlusion from AO and roughness. | `ReflectionEnvironmentShared.ush` | Frostbite 2014; Jimenez et al. 2016 | P2 / S |
| M8 | Split-sum environment specular from a procedural sky cube. | `BRDF.ush` `EnvBRDF`; `ReflectionEnvironmentCapture.cpp` | Karis 2013; Lazarov 2013 (mobile fit); Filament IBL | P1 / M |
| M9 | Local station reflections from a low-rate parallax-corrected cube. | `ReflectionEnvironmentRealTimeCapture.cpp` | Lagarde & Zanuttini 2012; Ramamoorthi & Hanrahan SH (2001) | P3 / L |
| M10 | Clear coat for cover glass, visor polycarbonate, and Kapton over aluminium. | `ClearCoatCommon.ush` | Filament clear coat; `KHR_materials_clearcoat` | P2 / S |
| M11 | Thin-film iridescence for solar-cell AR coatings. | `ThinFilmBSDF.ush` | Belcour & Barla 2017; `KHR_materials_iridescence` | P2 / M |
| M12 | Charlie sheen for suit fabric; anisotropic GGX for brushed aluminium. | `BRDF.ush` `D_Charlie`, `D_GGXaniso` | Estevez & Kulla 2017; Burley 2012; `KHR_materials_sheen` / `_anisotropy` | P3 / S |

**Proposed material table.** Values come from public measured data. Solar
absorptance α is from Gilmore, *Spacecraft Thermal Control Handbook*, and
conductor F0 from the Filament/Hoffman tables.

| Material | Base / F0 (linear) | Metallic | Roughness | Extras |
| --- | --- | --- | --- | --- |
| Bare aluminium | F0 (0.91, 0.92, 0.92) | 1 | 0.30 | brushed anisotropy 0.5 |
| Aluminized-Kapton MLI ("gold") | aluminium under a coat | 1 | 0.12 per facet | coat transmittance ≈ (0.95, 0.70, 0.30); crinkle 3–8 cm, ±20°; α ≈ 0.35–0.45 |
| White thermal paint (Z-93 / S13G) | (0.82, 0.82, 0.80) | 0 | 0.85 | α ≈ 0.15–0.20 |
| Beta cloth | (0.70, 0.69, 0.66) | 0 | 0.80 | sheen 0.3 |
| Suit outer fabric | (0.75, 0.75, 0.73) | 0 | 0.90 | sheen 0.3, roughness 0.5 |
| Solar cell with cover glass | cell (0.03, 0.04, 0.10) | 0 | coat 0.03 | AR thin film MgF₂ ≈ 110 nm; α ≈ 0.90 |
| Gold visor | F0 (1.00, 0.77, 0.34) | 1 | 0.04 | polycarbonate coat F0 0.05 |
| Silvered-Teflon radiator (option) | F0 (0.97, 0.96, 0.92) | 1 | 0.05 | FEP coat F0 0.04; α ≈ 0.08 |
| Safety yellow paint | (0.80, 0.50, 0.04) | 0 | 0.60 | none |

### Camera and post (C)

| ID | Candidate | Unreal pointer | Public reference | P / size |
| --- | --- | --- | --- | --- |
| C1 | Linear HDR scene target (`Rg11b10Ufloat`, falling back to `Rgba16Float`), MSAA resolved in float, one output pass, UI after it. | `SceneTextures.cpp`; `r.Mobile.TonemapSubpass` | Bevy `Camera { hdr }` and tonemapping node | P1 / M |
| C2 | Physical units and a manual EV100 camera (aperture, shutter, ISO), with pre-exposure. | `Scene.h` `FPostProcessSettings` (`CameraShutterSpeed`, `CameraISO`); `PostProcessEyeAdaptation.cpp` (`r.EyeAdaptation.LensAttenuation`) | Frostbite 2014 §4–5; Filament physical camera | P1 / M |
| C3 | AgX tonemap with white balance and grading baked into a 3D lookup table. | `PostProcessTonemap.usf`, `TonemapCommon.ush`, `PostProcessCombineLUTs.usf` | AgX (Sobotka/Blender); ACES (Academy); Bevy tonemappers | P1 / S–M |
| C4 | Energy-conserving mip-chain bloom with no threshold. | `PostProcessBloom.usf`, `PostProcessDownsample.usf` | Jimenez, "Next Generation Post Processing in CoD:AW" (2014); Bevy `Bloom` | P1 / M |
| C5 | Histogram auto exposure, clamped and slow, off by default. | `PostProcessHistogram.usf`, `PostProcessEyeAdaptation.usf` | Bevy `AutoExposure`; Filament | P2 / M |
| C6 | Vignette, grain, chromatic aberration, white balance; a "helmet cam" preset. | `Scene.h` (`VignetteIntensity`, `FilmGrain*`, `SceneFringeIntensity`, `WhiteTemp`); `CinematicCamera` | Filament color grading; Bevy | P2 / S |
| C7 | Local exposure by exposure fusion or a bilateral grid. | `PostProcessLocalExposure.usf` | Mertens et al. 2007; Chen, Paris & Durand 2007; Wronski 2022 | P2 / M |
| C8 | Prefiltered wide lines with analytic coverage and a sub-pixel width floor. | TSR thin-geometry detection (`TSRDetectThinGeometry.usf`) for contrast | Chan & Durand, "Fast Prefiltered Lines" (GPU Gems 2) | P1 / S |
| C9 | Sun diffraction: a PSF sprite at the Sun's screen position, or FFT convolution bloom (desktop). | `PostProcessFFTBloom.cpp` (`BloomConvolution*`) | Ritschel et al., "Temporal Glare" (2009) | P2 S / P3 L |
| C10 | Lens ghosts from the bloom chain. | `PostProcessLensFlares.usf` | Chapman, "Pseudo Lens Flare" (2013); Hullin et al. 2011 | P3 / S |
| C11 | TAA with variance clipping. | `TemporalAA.usf` | Karis, "High-Quality Temporal Supersampling" (2014); Salvi 2016 | P3 / L |
| C12 | Motion blur from shutter time. | `MotionBlur/*` | McGuire et al. 2012 | P3 / M |
| C13 | HDR display output (Metal EDR first). | `r.HDR.Display.*`, `PostProcessDeviceEncodingOnly.usf` | ITU-R BT.2100; ACES 2.0 output transforms | P3 / M |

### Sky, bodies, and precision (V)

| ID | Candidate | Unreal pointer | Public reference / data | P / size |
| --- | --- | --- | --- | --- |
| V1 | Reversed-Z with an infinite far plane; sky bodies at infinite depth with rotation-only view, layered in explicit order by true f64 distance. | `Core/Public/Math/PerspectiveMatrix.h` | Reed, "Depth Precision Visualized" (2015); Upchurch & Desbrun (2012) | P1 / S |
| V2 | Sun disc: average luminance = illuminance ÷ solid angle (≈ 1.9×10⁹ cd/m²), with limb darkening per channel. Unreal draws a flat disc. | `SkyAtmosphereRendering.cpp`, `SkyAtmosphereCommon.ush` | Neckel & Labs (1994, 2003) | P2 / S |
| V3 | Earth impostor: ray-cast sphere, rotating textures, true phase, and a baked limb band. Unreal ray-marches the atmosphere per pixel from space, which is too costly for a 9-px Earth. | `SkyAtmosphere.usf` (space view, transmittance and multi-scattering lookup tables) | Blue Marble NG (NASA, public domain); Bruneton (BSD-3); Hillaire EGSR 2020 + `sebh/UnrealEngineSkyAtmosphere` (MIT); Bevy `Atmosphere`; validate with DSCOVR EPIC (public domain) | P1 / M |
| V4 | Catalogue stars as energy-conserving pixel splats, plus a Milky Way map. Unreal has no catalogue. | none | Yale BSC 5th ed.; Hipparcos (check terms); NASA SVS Deep Star Maps 2020; Ballesteros 2012 (B–V to temperature); Jensen et al. 2001 | P1 / M |
| V5 | Moon photometry: Hapke or Lommel-Seeliger, albedo 0.12, opposition surge. | none | Hapke (1981, 2012); Kieffer & Stone ROLO (2005); NASA SVS CGI Moon Kit / LRO WAC | P2 / S |
| V6 | Ocean glint on Earth from Cox–Munk slopes. | none | Cox & Munk (1954); Marshak et al., GRL (2017) | P2 / S |
| V7 | Moon umbra and penumbra on Earth during solar eclipses. | none | analytic cones from f64 positions; DSCOVR 2017/2024 images | P3 / S |
| V8 | Camera-relative f64 positions, subtracted on the CPU before conversion to f32. | `LargeWorldCoordinates.ush`, `DoubleFloat.ush` (shader half not needed) | Thorne, "Using Floating-Point Arithmetic in 3D Worlds" | P3 / S |

### Effects and non-rigid (E)

| ID | Candidate | Unreal pointer | Public reference | Home | P / size |
| --- | --- | --- | --- | --- | --- |
| E1 | Ribbon and tube meshes for ropes and streaks. | `CableComponent` proxy; `NiagaraRibbonRendererProperties.h` | Wang et al., rotation-minimizing frames (2008) | renderer | P1 / S |
| E2 | Stateless seeded particles: closed-form position from seed and tick. | `NiagaraEmitter.h` (`bDeterminism`, `RandomSeed`), `NiagaraSimCache.h` | Jarzynski & Olano, "Hash Functions for GPU Rendering" (JCGT 2020) | renderer | P1 / S |
| E3 | Honest cold-gas plume visuals gated by sunlight. | Niagara velocity-aligned sprites | Simons plume model (AIAA J. 1972); Bevy Hanabi | renderer | P1 / S |
| E4 | XPBD rope pinned to the rigid tether joint and limited from both ends, deterministic. Unreal's cable is one-way and variable-step. | `CableComponent.cpp`; `XPBDLongRangeConstraints.h` | Jakobsen 2001; Müller PBD 2007; Macklin XPBD 2016 and "Small Steps" 2019; Kim et al., Long Range Attachments (2012) | `crates/physics` | P1 / M |
| E5 | Free-molecular plume impingement forces with an `impingement` ledger term. Unreal has no plume physics. | none | Simons (1972); Boynton plume model; Bird, DSMC (1994); NASA RCS plume-impingement literature | `crates/physics` | P1 / M |
| E6 | Modal flex of solar arrays and booms. | none (Chaos XPBD bending is overkill) | Likins 1970 (hybrid coordinates); Hughes, *Spacecraft Attitude Dynamics* (1986) | physics + renderer | P2 / S–M |
| E7 | Fireflies and ice flakes under solar radiation pressure. | Niagara concepts | Glenn 1962 observations; E2 contract | renderer | P2 / S |
| E8 | Two-way rope coupling: the rope becomes the tether and conserves momentum. | Chaos cloth attachment | Müller XPBD; our ledger | `crates/physics` | P2 / L |
| E9 | MLI edge flaps as small XPBD patches driven by impingement. | `ChaosCloth`, `PBDConstraintColor.h` | Müller PBD 2007 | renderer-side sim | P3 / M |

## Rejected

| Unreal feature | Why not |
| --- | --- |
| Lumen, virtual shadow maps, cascades beyond one fitted map, MegaLights, global distance fields | Built for kilometre-scale worlds and many lights. One small station, one light, and mobile targets are better served by L2 and L4. |
| Hardware ray-traced shadows | wgpu 29 has no portable ray queries on iOS or Android. L2 gets the same result. |
| Per-frame SkyAtmosphere, VolumetricCloud, height fog, light shafts, cloud shadows | They assume a camera inside an atmosphere. The Earth's limb is sub-pixel from L1, and light shafts in vacuum are wrong. |
| Earth or Moon eclipsing the Sun at the station | Geometrically impossible from L1. |
| Full Substrate multi-slab mixing and G-buffer packing | We are forward-rendered with about ten materials. |
| Subsurface scattering, hair, Burley or Oren-Nayar diffuse, real-time glints | Nothing translucent-skinned, and no visible gain on paint at this scale. Glints are too costly on mobile; M5 gets most of the look. |
| TSR and neural upscaling, depth of field, SMAA | Simple geometry at native resolution; EVA cameras are wide and deep-focus. C8 handles the real aliasing problem. |
| Niagara GPU simulation stages, grids, fluids, depth-buffer collision | The plume is free-molecular with no fluid to solve, and depth-buffer collision is not reproducible across devices. |
| Full Chaos cloth, deformables, self-collision, aerodynamic terms | Nothing is volumetric-soft, and vacuum has no air. |
| Double-float shader math and world tiles | The station is tens of metres. The CPU-side subtraction (V8) is enough. |
| Sun corona and ray lines | Not visible to the eye. The diffraction look comes from C9 after HDR. |

## Documentation to update as phases land

- `docs/verse/lagrange-1.md`: "What you see in the sky", "Approximations",
  and the capture command.
- `crates/verse/src/render.rs` module docs: the pipeline order.
- This note: the status of each candidate.
