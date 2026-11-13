# Character Create lighting after reading Ore

Research input: `/Users/christopherdavid/work/ore`, commit
`f4958def81d544132cf0fa77e7eb5cbb35bb1e4b`. The principal source is
`docs/ocarina-of-time/lighting-reconstruction.md` in that repository, especially
sections 5–9 and 11–12. Related material came from the rendering/material/VFX,
foliage, terrain/streaming, asset-design and engine-architecture reports and the
six-systems implementation record. The WoW creature workflow also describes
separately authored key, rim, fill and an output transform.

Ore carefully distinguishes recovered parameters from unknown rendering
contracts. The useful transfer here is that separation and the rendering
principles, rather than copying Kokiri's EV, intensity or fog numbers into an
unrelated Quaternius scene. No original Ocarina data or shaders are imported.

## Problems found and changes applied

The previous glade sampled an sRGB texture array, converted its already-linear
RGB back to display gamma, and lit those values alongside display-space palette
constants. Although the host allocated HDR targets, that shader did not maintain
an actual linear-light pipeline. Constant ambient and a uniform warm distance
blend further flattened the materials.

`everglade/scene/gpu/character-glade-light.coax` defined an explicit contract, now the shared painterly default
(see "The shared painterly default" below):

- Texture RGB stays in the linear domain supplied by the sRGB GPU sampler.
  Authored display palette values use the standard piecewise sRGB decode once.
- Direct warm sunlight, normal-dependent sky/ground hemisphere fill, and thin
  leaf transmission are separate terms. Geometry visibility masks direct and
  transmitted sunlight. The canopy ambient mask attenuates indirect light.
- Grass has root-to-tip occlusion; autumn leaves receive restrained spatial
  palette variation. The foliage's authored geometry remains unchanged.
- Atmosphere uses four midpoint samples of an exponential height density,
  Beer–Lambert transmittance, and an angular warm-forward/cool-sky source.
  Its near six metres are kept clear after comparison against the supplied
  foreground. It replaces uniform distance-only warm fog. These are authored Metaverse Engine
  approximations, not decoded native volumetric fog or a GI solver.
- Scene radiance, reconstruction and bloom remain linear in rgba16float.
  A peak-channel shoulder applies one RGB gain to preserve hue in highlights;
  the final pass performs the sRGB encode. The already-authored sky is encoded
  without applying the surface shoulder a second time.
- The shared host draws HUD strokes on the final display canvas after the post
  pass. The private HUD wrapper therefore preserves their display colors. The
  frosted 58% sidebar glass (see the reference pass below) is composited over the world by postprocessing before the
  HUD; no opaque background depth mask blocks the world behind it.

The auxiliary target now holds octahedral normal XY, indirect luminance share,
and signed camera depth. Contact shading weights its attenuation by that
indirect share, instead of multiplying the whole HDR result equally. This is a
scalar approximation to RGB indirect occlusion; it does not recreate separate
RGB GI buffers or probes. Negative depth still marks foliage; zero marks sky
and wind traces. The static pack format and all payloads remain unchanged.

## Validation

The GPU readback tool evaluates the actual generated WGSL on Apple Metal-3.
It checks standard gray transfer, midtone preservation, ordered highlights,
retained HDR values above one, height-dependent fog, and independent sun/AO
terms. The shader closure adds highlight hue and normal encoding checks.

Desktop and phone observations require two full-size rgba16float targets,
shadow/post pipelines, the expected texture quality, scene readiness and zero
captured GPU errors. The browser journey checks font loading, appearance changes
and reload, animation, zoom, pan, male preview and phone layout. Offline staging
checks that Character Create keeps the private shader's content-hashed URL.
Common fivelab's generated shader check confirms its artifact is unchanged.

See `docs/verification/character-glade-ore-lighting-2026-11-06/`. Visual captures
show more directional surface shading, brighter transmitted foliage and better
separation of roots, crowns and distant hills. This is an improvement informed
by Ore, with no claim of matching either game's native renderer exactly.

## Reference pass (2026-11-07)

The target was the Quaternius Stylized Nature MegaKit `Preview_1.jpg`: a warm golden sun, a
golden hazy hill, grey mossy rocks, saturated foliage, soft bloom and an olive dirt path. The
pass also used `docs/research/lighting/research.md` and the Metaverse Engine book.

Engine changes (`metaverse-engine/render/`, so every scene can use them):

- `light.coax` 4b adds `metaverse_style_haze_phase`, a Henyey-Greenstein forward-Mie lobe
  normalised so isotropic haze is 1, and `metaverse_style_haze_window`, Ore's near/far fade window.
- `light.coax` 4c adds `metaverse_style_kelvin`, Tanner Helland's black-body fit, and
  `metaverse_style_balanced`, a light seen through a white balance. Ore's noon sun is 4500 K
  under 6500 K.
- `filmic.coax` is self-contained (its own `film_atan2`) so a named scene module can load
  it next to the noise module.

Character Create changes:

- One sun direction and colour, `cg_sun_dir()` and `cg_sun_colour()`: 4500 K balanced at
  6500 K, ×1.85. Every former sun copy in the glade, shadow and creator shaders uses them.
- Atmosphere scatters forward with g = .62, thickening between 12 m and 50 m: haze toward
  the sun is gold and haze away from it stays blue.
- Output goes through the film curve (`film_rec709`) at exposure 1.18, plus a 1.16 saturation
  lift, in place of the peak shoulder.
- Bloom uses the engine soft knee (threshold .62, knee .30) and a stronger glow mix.
- Rocks are neutral grey with moss on faces that point up. The far hill turns gold, the path
  is olive and grass is greener.
- The left panel is frosted glass: a 16-tap blur of the lit scene under a 58% tint and a light
  rim. The "Character Create" title sits inside it above a gold divider, and the labels are
  brighter. The walk to Everglade fades the glass with `u[40]`.

A second pass made the light come from behind, as in the reference:

- The sun is lower (about 36 degrees) and behind the scene, at 2.15 times the balanced colour.
  Shadows now run long toward the camera, and rocks have dark sides.
- `cg_cloud` adds soft cloud shadows: two octaves of value noise scale direct sunlight down
  to .45, so the near meadow and lower path lie in broad shade with a sunlit band beyond.
- The path is darker olive, grass roots are darker (direct .22, ambient .36 at the root), grass
  is greener, near rocks are less white, and forward haze is lighter (gain .40) so the cliff
  reads.

The town at the end of the walk to Everglade is packed on the rock layer with one flat tint a
material, so it drew as blank grey and white. `cg_town_material` recognises those six tints beyond
the ridge (z < -32.5), and `cg_town_albedo` paints them: coursed stone with thin mortar and a value
per block, timber-framed plaster darker at its foot, roof tiles in rows and straight-grained timber.
Steep banks keep more of their olive under the hill's gold, and broad patches vary its value
Inside the ridge cut the gold eases further (`gate-walk.jpg`). `reference-compare.jpg` stacks the reference, the main shot and the walk.

A third pass cleared the cliff: the base air is a third thinner (.0055 a metre) and the forward
haze window opens from 30 m to 65 m, so the cliff reads at its distance while the castle still
hazes. Rocks keep more of their own olive (greyed .35) and moss climbs more faces. `cg_sun_veil`
adds the reference's sunlit upper air, a soft warm lift over the far scene on the sun's side
(`reference-compare-2.jpg`).

Captures are in `docs/verification/character-glade-reference-light-2026-11-07/`: `base.jpg`
before, `p3.jpg` after the scene passes, `p5.jpg` and `p5ui.jpg` on current main with
the walk merged, and `p9.jpg` and `p9ui.jpg` after the backlit pass.

## Player integration (2026-11-07)

The player previously called the common legacy figure shader. Its Studio brightness
override and display-space finish bypassed the glade light and shadow map. The
private character shader now keeps sampled base colour linear, applies display-authored
dye masks before decoding their result, and decodes skin/eye palette choices once.
It uses the same world-space sun and PCF geometry visibility as the meadow, plus
sky/ground hemisphere fill, a soft warm camera-side bounce and a restrained broad
dielectric highlight. No constant brightness override or legacy fog/finish remains.
The indirect luminance share reaches contact shading; the common HDR/post output
handles the character and world together. Shared battle and Everglade shaders are
unchanged. Normal and ORM textures remain authoring-only; full material-specific
BRDFs and GI/probes are still absent.

See `docs/verification/character-player-light-2026-11-07/` for the same-camera
comparison, controls journey, directional/shadow check and desktop/phone GPU graphs.

## Crimson crowns (November 7)

The reference's autumn crowns hold crimson lobes beside orange ones. Our leaf palette used to blend
every red tree up to 70% toward orange, so the crimson never showed. `cg_leaf_palette` now takes
young leaves on a red tree toward a dark crimson and turns only old leaves orange. The crimson is
darker than the orange, because a bright red clips its red channel and reads as orange again. The
foreground plants beside the near rock use deeper red-orange tints. Comparison:
`docs/verification/character-glade-reference-light-2026-11-07/crimson-crowns.png`.

## The shared painterly default (November 8)

The owner's direction: this glade is the most beautiful thing we render, and its lighting is the
default for every painterly realm (lighting plan Phase 1 step 0, GitHub #13060). Its functions and
numbers moved out of the app into the OpenAgents Metaverse Engine:

| What | Now in |
| --- | --- |
| Every number: the key (4500 K under 6500 K, x2.15, direction), hemisphere colours, sky and cloud colours, air, haze window, cloud shade, exposure and saturation | `metaverse-engine/world/environment.coax` `ENV_GLADE_*`, which `env_glade_profile` is built from |
| `cg_linear`, `cg_srgb`, `cg_decode`, `cg_encode`, `cg_shoulder` | `metaverse-engine/render/transfer.coax` (`transfer_*`) |
| `cg_sun_dir`, `cg_sun_colour`, `cg_hemisphere`, `cg_irradiance`, `cg_air_depth`, `cg_phase`, `cg_haze_window`, `cg_atmosphere`, `cg_film`, `cg_output`, `cg_sky`, `cg_cloud`, `cg_cloud_fbm`, `cg_soil`, `cg_noise` | `metaverse-engine/render/painterly.coax` (`paint_*`); the film after exposure is `metaverse-engine/render/filmic.coax` `film_exposed` |
| `cg_sign`, `cg_normal_pack`, `cg_normal_unpack`, the view-point reconstruction and contact taps | `metaverse-engine/render/contact-shade.coax` (`shade_*`) |

The `cg_*` names stay as thin delegates, so Everglade II, Experimental and the lighting probes
compile unchanged until their consumers read the resolver (#13066). `metaverse/character-glade.coax`
holds the glade's composition (terrain, path, planting, town) and no lighting, so nothing moved from
it. `metaverse/style-preset.coax`'s painterly preset names the glade profile as its default environment
(`environment = ENV_GLADE_ID`, `metaverse_style_environment`); its bands, rim, dab and `shadow_cool`
stay as the material response on top, and the other styles name none.

Character Create renders exactly as before: against the P1.1 baseline every one of the 16 captures
moved 0 blocks, every frame SHA-256 and all 192 probes (48 GPU probes) are identical, with the pages
running the regenerated shaders. Receipt: `docs/verification/aethyr-lighting-p1-0-2026-11-08/`.
