# MetaHuman study: what Verse adopts for Draconia

Status: research note, 2026-10-13. Draconia is an original character. Verse
renders her with a forward wgpu PBR renderer, and scripted Blender builds
produce her assets. We take ideas from Unreal's MetaHuman stack and copy none
of its assets. Sources are linked inline. Where a claim comes only from press
or a third party, the text says so.

## 1. Licences

### Summary

| Item | Licence (as of 2026-10) | Use outside UE | Public open-source repo | Port code to Rust |
|---|---|---|---|---|
| MetaHuman characters (meshes, textures, grooms, animation) | Unreal Engine EULA (MetaHuman folded in with UE 5.6, June 2025) | Yes, as "Non-Engine Products", no royalty. A seat licence is needed above $1M/yr revenue | **No.** Only shipped inside a product, never as raw source assets | n/a |
| OpenRigLogic (RigLogic runtime and DNA library), `EpicGames/openriglogic` | **MIT**, (c) 2026 Epic Games, published 2026-06 | Yes | Yes, keeping the MIT notice. Epic trademarks may not be used in names | **Yes**, with the MIT notice |
| MetaHuman DNA Calibration (`EpicGames/MetaHuman-DNA-Calibration`) | Custom "MetaHuman DNA Calibration License Agreement" (2022) | Only to build export/edit tools | **No.** "You may not distribute the Software" | No. Use OpenRigLogic instead |
| RigLogic inside UE source (`Engine/Plugins/Animation/RigLogic`) | UE EULA (source visible only to UE licensees) | n/a | No | No. Use the MIT copy |
| DNA file format | Documented: DNA layer docs plus the RigLogic whitepaper; the reader/writer is open source (MIT) | n/a | Format knowledge is free to use | Yes, via OpenRigLogic |
| MakeHuman / MPFB assets (base mesh, targets, skins, system assets) | **CC0** (the code is GPL/AGPL; the assets are CC0) | Yes | Yes | n/a |
| ICT-FaceKit (`ICT-VGL/ICT-FaceKit`) | MIT | Yes | Yes, with the notice | Yes |
| ARKit 52 blendshape names | Names only, not copyrightable assets | Yes | Yes | n/a |
| Blender, Hair Curves and Geometry Nodes | GPL code; what you make with them is yours | Yes | Yes | n/a |

### MetaHuman characters and assets

- **The licence changed with UE 5.6 (State of Unreal, 2025-06-03/04).**
  - MetaHuman tooling now falls under the standard UE EULA, free under $1M a
    year in revenue
    ([CG Channel, 2025-06-04](https://www.cgchannel.com/2025/06/you-can-now-sell-metahumans-or-use-them-in-unity-or-godot/)).
  - The characters may be used in other engines and DCC apps (Unity, Godot and
    so on), and sold on marketplaces.
- **No royalty outside Unreal.** Epic's licence page classes MetaHuman
  characters and animation as "Non-Engine Products" under the UE EULA. Using
  them as linear media, or at runtime in another engine, triggers no royalty
  ([metahuman.com/license](https://www.metahuman.com/license); it needs a
  login and was quoted through search results).
- **What the EULA says about Non-Engine Products.** Products with no Engine
  Code that "do not include Starter Content in source format" owe no royalty.
  Asset files stay Non-Engine Products even inside products that use other
  engines ([UE EULA](https://www.unrealengine.com/eula/unreal); excerpt via
  [ConductAtlas](https://conductatlas.com/platform/unreal-engine/unreal-engine-eula/royalty-free-non-engine-product-distribution/)).
- **Seat licence above $1M a year.** If you earn more than $1M in annual gross
  revenue and render MetaHumans outside Unreal, you need UE seat licences.
  CG Channel gives the price as $1,850 per seat per year in 2025. This comes
  from secondary sources; Epic's own page sits behind a login.
- **AI restriction.** MetaHumans may be used in AI-assisted workflows, but not
  to train or improve AI models (CG Channel, and the
  [EULA change log](https://www.unrealengine.com/eula-change-log/unreal)).
  This matters to us because we train models.
- **Redistribution: not in our repo.**
  - The EULA allows content to be distributed inside a Product, not as
    standalone source-format assets.
  - It restricts public distribution of Engine Tools to Fab or a fork of
    Epic's GitHub.
  - It forbids combining Licensed Technology with copyleft licences.
  - A DCC Export (head DNA, body DNA and textures) is therefore not ours to
    commit. The OpenRigLogic sample content is under the Fab Standard Licence
    and is not redistributable either.
- **Conclusion:** no MetaHuman meshes, textures, DNA files or grooms go in the
  repo, in scripts, or in training data. Our assumption holds.
- **Side note:** the MetaHuman Creator web app is reported to shut down on
  2026-11-05 as creation moves into UE 5.7
  ([StraySpark](https://www.strayspark.studio/blog/metahuman-2026-web-app-shutdown-migration-guide);
  third-party source).

### DNA Calibration (licence file dated 2022-10-31; repo last pushed 2025-06-20)

- **The grant.** "worldwide, nonexclusive, royalty-free, non-sublicensable
  license to reproduce and prepare derivative works", solely to build tools
  that export and edit MetaHuman characters.
- **The limits.**
  - "You may not distribute the Software."
  - Characters are limited to UE-based products. This clause predates 5.6.
  - Epic can terminate for convenience, after which you must delete all copies
    and derivatives.
  - Source:
    [LICENSE](https://github.com/EpicGames/MetaHuman-DNA-Calibration/blob/main/LICENSE).
- **The repo is stale.** Its README says it has not been updated for UE 5.6
  characters.
- **RigLogic ships only as binaries.** RigLogic appears there only as a binary
  Maya plugin (`lib/*/embeddedRL4.mll`).
- **Verdict:** do not vendor, port or link this code.

### RigLogic and DNA: now MIT (OpenRigLogic)

- **What it is.**
  [EpicGames/openriglogic](https://github.com/EpicGames/openriglogic) was
  created 2026-06-05 under an MIT licence. It holds:
  - the DNA library (binary and JSON reader/writer);
  - the RigLogic evaluator (controls, PSD net, joints, blend shapes, animated
    maps, RBF, ML behaviour, conditional tables);
  - design docs (`docs/design/src.md`, "RigLogic 4").
- **Branches:** a stable `5.8` branch and an experimental `main`.
- **Porting.** We may port algorithms or code to Rust if we keep the MIT notice
  in the ported files.
- **Trademarks.**
  [TRADEMARKS.md](https://github.com/EpicGames/openriglogic/blob/main/TRADEMARKS.md)
  forbids "MetaHuman", "RigLogic" or "OpenRigLogic" in our product or crate
  names. "based on OpenRigLogic" is allowed as attribution.
- **Recommendation:** adopt the ideas and a much smaller data model (section 3).
  Port specific kernels with attribution only if we need DNA compatibility. We
  don't need it: we never load MetaHuman DNA files.

### Format documentation

- **DNA layer docs:**
  [docs/dna.md](https://github.com/EpicGames/MetaHuman-DNA-Calibration/blob/main/docs/dna.md).
- **The RigLogic whitepaper** says: "the format specification, as well as the
  code that reads it, is open source"
  ([PDF](https://cdn2.unrealengine.com/rig-logic-whitepaper-v2-5c9f23f7e210.pdf)).
- **Field-level detail** lives in the OpenRigLogic headers
  (`include/dna/*Reader.h`).

### CC0 and permissive alternatives

- **MakeHuman system assets: CC0.**
  - Covers the base mesh (hm08, about 13k quads, with eyes, teeth, tongue and
    eyebrow proxies), the modelling targets and the skins.
  - MPFB code is GPL; the assets are CC0. MPFB's FAQ confirms the base mesh,
    targets and skins are CC0.
  - Sources:
    [licence](https://static.makehumancommunity.org/about/license.html),
    [MPFB FAQ](https://static.makehumancommunity.org/mpfb/faq/use_in_closed_source.html).
  - Download the "MakeHuman system assets" pack from
    [asset packs](https://static.makehumancommunity.org/assets/assetpacks/index.html).
    Install it in Blender through MPFB > Library settings > Install asset pack.
  - MPFB's face expressions are pose units on its "default" rig. Check each
    pack's licence line; the community packs vary (CC0 or CC-BY).
- **ICT-FaceKit (MIT).**
  - A morphable face model with an ARKit-compatible expression blendshape set
    ([repo](https://github.com/ICT-VGL/ICT-FaceKit)).
  - Usable with attribution. Read the README for any data caveats before
    shipping its meshes.
- **ARKit 52 names.** Names like `jawOpen` or `eyeBlinkLeft` are plain
  vocabulary
  ([Apple docs](https://developer.apple.com/documentation/arkit/arfaceanchor/blendshapelocation)).
  Use them as our control names, so Live Link-style capture maps straight in.
  We sculpt the shapes ourselves.
- **Blender hair.** Hair Curves, Geometry Nodes and the hair node assets are
  GPL software. Grooms and cards we author with them belong to us.

## 2. Architecture

### DNA layers (from the whitepaper and dna.md)

| Layer | Contents |
|---|---|
| Descriptor | Name, age, archetype, key/value metadata, compatibility params |
| Definition | Names of GUI/raw controls, joints, blend shape channels, animated maps and meshes. LOD membership lists, joint hierarchy, bind-pose transforms |
| Behavior | GUI-to-raw mapping, PSD (corrective) definitions, joint delta matrices per joint group, blend shape channel mapping, animated-map conditional tables |
| Geometry | Per-mesh vertices, UVs, normals and faces (OBJ-like), skin weights, blend shape target deltas |

Behavior and Geometry are independent of each other. A runtime that only
drives a rig loads Behavior and leaves Geometry unloaded.

### RigLogic evaluation (whitepaper; OpenRigLogic `docs/design`)

1. **GUI to raw controls.** Each mapping is clamped piecewise-linear:
   `raw = clamp(slope*gui + cut, from, to)`. One GUI slider (a 2D faceboard
   control) can drive several raw FACS controls, for example a split left/right
   or a positive/negative axis. Raw controls lie in [0, 1], ideally one per
   FACS Action Unit.
2. **PSD correctives.** Combination inputs are appended to the input vector.
   Each PSD value is the product of its source raw controls, clamped to [0, 1].
   They correct "multiplied" deformations when basic expressions overlap.
   Correctives are authored only for the combinations that matter, since their
   number explodes combinatorially.
3. **Joints.** Output = `K * x`: per-joint translate, rotate and scale deltas,
   the old driven keys reduced to `y = k*x`. The sparse CRS matrix was pruned
   into dense **joint groups**, one per face region (forehead, jaw and so on),
   with a fixed control-to-group map. Groups are evaluated block-vectorised.
4. **Blend shapes.** A simple injective remap from inputs to channel weights.
   The target deltas live in Geometry.
5. **Animated maps.** Conditional tables, piecewise slope/intercept over input
   ranges, produce shader multipliers for wrinkle and blood-flow masks. They
   are under 3% of the data.
6. **LOD.** Eight built-in LODs. The joints, blend shapes and maps used at
   LOD n+1 are a strict subset of LOD n, and the data is ordered by LOD, so
   switching LOD is just a slice. Clients can set a max LOD at load time so
   unused data is never loaded.
7. **Scale.** A state-of-the-art face rig has about 800 joints, more than 200
   controls and more than 1,000 correctives. MetaHuman faces run about 250 GUI
   controls driving about 800 raw and corrective expressions. Treat these as
   order-of-magnitude figures.

### Face and body

- **Two DNA files.** The DCC Export ships a head DNA and a body DNA (OpenRigLogic
  README).
- **The neck seam.** The head is a separate skeletal mesh that shares the neck
  joints with the body skeleton. In UE the head follows the body through a
  copy-pose / leader-pose arrangement. The seam vertices are matched across
  LODs and weighted to the same neck joints, so no crack opens.
- **Body corrective joints.** The body uses RBF and twist/swing helpers; see
  `src/riglogic/rbf`.

### Skin (UE Subsurface Profile)

[UE docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/subsurface-profile-shading-model-in-unreal-engine)

- **Scattering.** Screen-space SSS with either Burley normalized diffusion (the
  preferred option; needs TAA) or the older separable SSS.
- **Profile parameters:** mean free path colour and distance, boundary colour
  bleed, and transmission (extinction, normal scale, IOR, tint).
- **Dual specular.** Roughness 0 is the softer lobe and Roughness 1 the tighter
  one; Lobe Mix blends them.
- **MetaHuman skin materials** add:
  - a cavity map that occludes specular in pores;
  - a tiled micro-normal over the baked normal;
  - roughness variation by region;
  - three wrinkle-map sets (albedo and normal), blended per region through
    masks weighted by RigLogic animated-map outputs, which also drive the
    blood-flow tint.

### Eyes

- **The eye model.** The UE Eye shading model handles a refractive cornea over
  an iris with depth. Its parameters are IrisMask, IrisDistance and the iris
  normal, plus a parallax/refraction offset of iris UVs and a limbal ring
  darkening at the iris edge. Sclera vein textures sit under the sclera tint.
- **Mesh slots.** MetaHuman uses separate eye mesh slots: EyeLeft, EyeRight,
  EyeShell, LacrimalFluid and EyeEdge
  ([API enum](https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/MetaHumanCharacter/EMetaHumanCharacterTeethAndEyesS-)).
  - **EyeShell** is the eye occlusion shell: a translucent shadow and AO band
    where the lids meet the eye.
  - **LacrimalFluid** is the tear line, a thin wet strip along the lower lid
    edge.
  - The caruncle is part of the head mesh, with its own wet, high-spec region.

### Hair

- **Strands.** Groom assets are rendered as strands with a Marschner-based BSDF
  (R, TT and TRT lobes; longitudinal Gaussian, azimuthal terms), plus dual
  scattering for multiple scattering
  ([Karis, SIGGRAPH 2016](https://blog.selfshadow.com/publications/s2016-shading-course/karis/s2016_pbs_epic_hair.pdf);
  [UE hair docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/hair-rendering-and-simulation-in-unreal-engine)).
- **Cards for lower LODs.** The Hair Card Generator (Dataflow) bakes per-atlas
  textures: Depth, Coverage, Tangent, Attributes (root UV, U coordinate along
  the strand, seed, group ID) and Material
  ([docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/hair-card-generator-for-grooms-in-unreal-engine)).
- **Shading.** The Hair shading model then shades the cards. Colour runs
  root-to-tip from the U coordinate, and the seed adds per-strand variation.

### Fabric

- **The Cloth shading model.** Base GGX plus a sheen lobe set by Fuzz Colour
  and Cloth (amount)
  ([shading models](https://dev.epicgames.com/documentation/en-us/unreal-engine/shading-models-in-unreal-engine)).

## 3. What Verse adopts

| Idea | Verse implementation (forward wgpu plus Blender scripts) |
|---|---|
| Shading model per material | Blender writes `extras.verse_shading = "skin" \| "eye" \| "cornea" \| "hair_card" \| "cloth" \| "default"` to glTF material extras. The loader maps it to a `u32` model id in the material uniform, and the fragment shader branches on it (one pipeline, uniform branch per draw) |
| Skin SSS | Pre-integrated skin: a 2D LUT of (N·L, curvature) to diffuse RGB, with curvature baked to a vertex attribute or computed from `fwidth`. Plus a wrap term for terminator softening, and red-shifted thickness-based transmission for ears and nostrils (thickness map). No screen-space pass in v1 |
| Dual specular | Two GGX lobes, `r0 ≈ 0.75*r` and `r1 ≈ 1.3*r` (clamped), mixed at about 0.15/0.85, sharing one Fresnel (F0 = 0.028) |
| Cavity, micro detail | A cavity map multiplies specular. A tiled micro-normal (about 16-32x tiling) is reoriented with RNM onto the base normal and fades with view distance. A roughness map is baked per region (T-zone glossier) |
| Wrinkle maps | Two extra normal maps (compress and stretch) and an RGBA region-mask texture. Four region weights per frame from the rig's `animated_maps` output drive `n = mix(base, wrinkle, mask·w)` |
| Eyes | Two meshes. **Eyeball** (opaque): sclera with veins; the iris is sampled at `uv + parallax(view_ts, iris_depth·mask)` with refraction approximated by Snell on the cornea normal; a limbal ring term `smoothstep` at the iris radius. **Cornea shell** (alpha-blended, GGX at about 0.05 roughness, IOR 1.376 Fresnel only). Also an **occlusion shell** (multiply-blend AO gradient) and a **tear strip** (additive spec, roughness about 0.02). The caruncle gets a wet roughness region on the head |
| Hair cards | Blender builds cards from Hair Curves (Geometry Nodes, curve-to-ribbon) and bakes Coverage, Depth, Root-UV and U/seed to an atlas in Cycles. Shader: Kajiya-Kay/Scheuermann, two tangent-shifted lobes (primary white `shift -0.1`, secondary tinted `shift +0.1`, broader exponent) as a cheap Marschner R/TRT approximation. Root-to-tip colour lerps on U, the seed jitters hue and shift, alpha-to-coverage with MSAA, sorted pre-pass for the depth-tested opaque core |
| Cloth | A Charlie sheen lobe (Estevez-Kulla), with the glTF `KHR_materials_sheen` colour/roughness read straight from the glTF. Ashikhmin visibility, plus the base GGX lobe scaled by `1 - sheen_albedo` |
| Rig format | `draconia.rig.json` (a DNA-lite we design ourselves, no Epic data). `controls[]`: name (ARKit vocabulary where it fits), `gui -> raw` maps `{gui, raw, slope, cut, from, to}`. `correctives[]`: `{inputs:[raw...], out}` as the product of inputs. `blendshapes[]`: `{name, input, lod_max}`. `joints[]` grouped into regions: dense `K` blocks `{group, inputs[], joints[], deltas[rows][cols]}`. `animated_maps[]`: `{name, input, ranges:[{from,to,slope,cut}]}`. Each array is ordered by LOD with `lod_counts[]`, so a LOD is a prefix slice |
| Rig runtime | A Rust crate (`verse_face_rig` or similar; Epic marks stay out of the name). Steps: evaluate GUI to raw, correctives, `K·x` per group, blend weights, map weights. CPU evaluation is enough at our control count (about 60-120 raw). Blend deltas upload to a storage buffer for GPU morphing; joints go to the existing skinning palette |
| LOD | Face LOD0 and LOD2 to start (MakeHuman-based mesh plus a decimated mesh). Rig data is a LOD prefix, and the seam vertices are locked during decimation |
| Face/body | One body armature. The head shares the `neck_01/02` and `head` joints, and seam vertices carry identical weights. One glTF, with the head as a separate primitive so it can have its own material and morph targets |
| Base assets | MakeHuman CC0 base mesh and targets via MPFB in the scripted build (`scripts/blender/`), sculpted into an original Draconia identity. Expressions are authored by us (FACS AU list plus about 30 correctives for jaw/lips/brow combinations). ICT-FaceKit (MIT) is an option for an ARKit-shaped starting set, with attribution |

Not adopted in v1:

- screen-space Burley SSS;
- strand rendering and dual scattering;
- RBF body correctives;
- RigLogic ML behaviour;
- DNA file compatibility.

Each can come later without changing the rig JSON or the material id scheme.
