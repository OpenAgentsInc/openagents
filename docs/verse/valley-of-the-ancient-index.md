# Valley of the Ancient: content index

This page indexes Epic's Valley of the Ancient sample as it sits on this Mac:
where it is, what the launcher has finished writing, the content tree with
sizes and counts, the asset categories, the maps and World Partition setup,
and the Echo character's folder. It ends with what each area is worth
studying for and the steps to finish the inventory in the Unreal Editor.

It extends the [UE5 ruins study](ue5-ruins-study.md), whose licensing rule
holds here: Valley is UE-Only content, studied in Reference-only mode.
Nothing from it ships in Verse, no Valley texture enters a pack, and no file
from the project is copied, converted, or committed. This index records
names, sizes, and header metadata only.

Status on October 5, 2026, about 18:45 local: the download is incomplete.
The launcher staged 17,094 of the 19,784 files, then crashed; the project
hasn't been created anywhere.

## Where it is

| Path | What's there |
| --- | --- |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/stage/f/` | 17,095 staged files, 71.0 GB, each named by a 32-character hash instead of its project path. 17,094 are complete. The 17,095th is `DerivedDataCache/Compressed.ddp`, at 12.2 of 16.5 GB. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/data/` | The vault install directory. Empty: the launcher moves staged files here when a build completes. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/stage/m/$resumeData` | The build ID, `VxKi5EJ0TJaKfRpHej4S1A`, for resuming. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/FabLibrary/Valley_of_the_Ancient-0c19880e/unreal-engine/manifest` | The 7.5 MB binary build manifest (Epic BuildPatch format, version 21, zlib-compressed). It lists every file, its size, and its SHA-1. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/FabLibrary/listings_v1.db` | The launcher's Fab library listing cache. |
| `~/Documents/Unreal Projects/ValleyoftheAncient` | The planned project location. It doesn't exist. |

What happened, from `~/Library/Logs/Unreal Engine/EpicGamesLauncher/`:

- At 17:33 local, the launcher began building app `AncientGame_5.7` into the
  vault (`InstallDirectory` `.../AncientGame_5.7/data`, `VerifyMode`
  `ShaVerifyAllFiles`).
- By 18:19 it had completed all of `Content/` except the splash images and
  one localization file.
- At 18:30 it crashed in its embedded browser (`SIGTRAP` in Chromium
  Embedded Framework). The log `EpicGamesLauncher-backup-2026.10.05-23.30.36.log`
  holds the trace.
- At 18:41 it restarted with an empty download queue
  (`Data/DownloadManager/DownloadState_*.json` is `[]`) and didn't resume.

The figure of 85.74 GB in the Fab library is the manifest's installed size
in binary units: 92.06 GB (85.74 GiB) across 19,784 files.

What's staged and what isn't:

| Part | Staged | Missing |
| --- | --- | --- |
| `Content/` (`AncientContent`, `__ExternalActors__`, `Localization`, `Splash`) | 17,067 files, complete except 3 small files | `Content/Splash/Splash.bmp`, `Splash.png`, one `Localization` file |
| `AncientGame.uproject`, `AncientGame.png`, `Config/`, `Binaries/`, `Build/` | Complete | |
| `DerivedDataCache/Compressed.ddp` | 12.2 of 16.5 GB | The rest |
| `Plugins/GameFeatures/AncientBattle/` | Nothing | 2,478 files, 16.75 GB: the boss, destruction, battle map, Echo's combat clips |
| `Plugins/GameFeatures/HoverDrone/`, `Plugins/Systems/`, `Plugins/InstanceLevelCollision/` | Nothing | 151 files, under 0.02 GB |
| `Source/` | Nothing | 56 files |

To finish the install, open the Epic Games Launcher, go to **Unreal
Engine > Library > Fab Library**, find Valley of the Ancient, and choose
**Create Project** with engine 5.7. The launcher verifies the staged files
and downloads the remaining 21 GB. It needs about 95 GB free for the project
beside the vault; the data volume had 483 GB free.

## How this index was made

Everything below comes from three sources, read in place:

1. **The build manifest.** A Python reader decompressed the manifest and
   walked its file list for every path and size. This is the only source for
   the parts that aren't staged.
2. **The launcher log.** Each `Constructing file PATH (HASH)` line maps a
   staged hash name back to its project path.
3. **Package headers.** For staged `.uasset` and `.umap` files, the reader
   took the name table and the asset registry tags that each package stores
   in its header: texture `Dimensions` and `Format`, static mesh
   `Triangles`, `Vertices`, and `NaniteEnabled`, skeletal mesh `Bones` and
   `MorphTargets`, and animation `Number of Frames` and `Source Frame Rate`.
   It also read a few serialized World Partition properties (cell size,
   loading range) from `AncientWorld.umap`.

It decoded no mesh, texture, animation, or audio data, rendered nothing, and
wrote nothing into the vault. The scratch reader isn't committed.

The packages are editor packages: one `.uasset` or `.umap` per asset, with
no `.uexp` or `.ubulk` split. There is no `AssetRegistry.bin`; the editor
builds the registry when the project opens, and a cook writes it.

Two limits on the header counts:

- For a Nanite mesh, the `Triangles` tag counts the fallback mesh (2,000
  triangles for a typical Megascans rock), not the source. File size is the
  better proxy for source density until the editor gives the real number.
- Header tags exist only for staged files, so the AncientBattle plugin has
  names and sizes but no tags.

## The project

| Item | Value |
| --- | --- |
| Fab listing | Valley of the Ancient, `0c19880e-21bd-42ba-8287-1caccc3951b1` |
| App and build | `AncientGame_5.7`, build version `5.7.0-47537391+++UE5+Release-5.7-Mac` |
| Engine | `EngineAssociation` `5.7` in `AncientGame.uproject`. Packages were last saved by builds from `++UE5+Release-5.0-EarlyAccess` to `++UE5+Release-5.3`, and resave on open. |
| Module | `AncientGame` (Runtime), with editor binaries for Mac in `Binaries/Mac/` |
| Target platforms | `PS5`, `Windows`, `WinGDK`, `XSX` |
| Game features | `AncientBattle` (the boss fight) and `HoverDrone` (Echo's flying camera drone) |
| Project plugins | `Uproar`, `Underscore`, `Crossfader` (audio), `ModularGameplayActors`, `InstanceLevelCollision` |
| Engine plugins enabled | `GameplayAbilities`, `GameFeatures`, `ModularGameplay`, `EnhancedInput`, `MotionWarping`, `ControlRig`, `FullBodyIK`, `HairStrands`, `ChaosCaching`, `FieldSystemPlugin`, `Metasound`, `AudioModulation`, `Soundscape`, `NiagaraFluids`, `Volumetrics`, `VirtualHeightfieldMesh`, `ImpostorBaker`, `ModelingToolsEditorMode`, `MovieRenderPipeline` |

## Content tree

Sizes are decimal gigabytes from the manifest.

| Folder | Size | Files | What it holds |
| --- | --- | --- | --- |
| `Content/AncientContent/Megascans/` | 38.81 GB | 1,018 | Scanned rocks, cliffs, plants, and ground surfaces |
| `Content/AncientContent/Geometry/` | 15.94 GB | 549 | Project-made sets: pillars, gates, buttes, spires, stone blocks, ground tiles, MegaAssembly meshes |
| `Plugins/GameFeatures/AncientBattle/Content/Characters/` | 10.44 GB | 674 | The Ancient One (boss) and Echo's combat clips |
| `Plugins/GameFeatures/AncientBattle/Content/Destruction/` | 2.95 GB | 987 | Geometry collections, fracture pieces, fields, ruins |
| `Plugins/GameFeatures/AncientBattle/Content/Maps/` | 2.51 GB | 144 | The battle map and its Chaos caches |
| `Content/AncientContent/Characters/` | 1.61 GB | 183 | Echo |
| `Plugins/GameFeatures/AncientBattle/Content/FX/` | 0.69 GB | 398 | Boss lasers, explosions, dust |
| `Content/AncientContent/Materials/` | 0.72 GB | 91 | Master materials, butte materials, post process |
| `Content/AncientContent/Maps/` | 0.66 GB | 306 | Four top maps and the MegaAssembly levels |
| `Content/__ExternalActors__/` | 0.39 GB | 14,244 | World Partition, one file per actor |
| `Content/AncientContent/Audio/` | 0.31 GB | 415 | MetaSounds, sound waves, Soundscape palettes, submixes |
| `Content/AncientContent/Effects/` | 0.15 GB | 120 | Fog, campfire, portal, footsteps, drone |
| `Plugins/GameFeatures/AncientBattle/Content/Audio/` | 0.14 GB | 189 | Boss and ability sound |
| `Content/AncientContent/Textures/` | 0.13 GB | 5 | Sky and distant cliff images |
| `Content/AncientContent/` other (`Blueprints`, `UI`, `Clouds`, `Lighting`, `Input`, `DataLayers`) | 0.07 GB | 118 | Gameplay, cameras, sky, input |
| `DerivedDataCache/Compressed.ddp` | 16.51 GB | 1 | Prebuilt shader and mesh data, so the first open is faster |
| `Plugins/GameFeatures/HoverDrone/` | 0.01 GB | 36 | The drone pawn, its input, and Echo's release clips |
| `Config/`, `Source/`, `Binaries/`, `Build/`, other plugins | small | 248 | |

## Asset categories

Classified by name prefix and extension across the whole project:

| Category | Files | Size |
| --- | --- | --- |
| Static meshes (`SM_`) | 1,038 | 45.66 GB |
| Textures (`T_`) | 999 | 24.41 GB |
| World Partition actor files (`__ExternalActors__`) | 14,244 | 0.39 GB |
| Fracture pieces (`frac_`) | 716 | 0.07 GB |
| Sounds (`sfx_`, `music_`, `vox_`) | 519 | 0.44 GB |
| Material instances (`MI_`) | 418 | 0.05 GB |
| MegaAssembly assets (`MASS_`: packed-level blueprints and colliders) | 283 | 0.05 GB |
| Materials (`M_`) | 272 | 0.02 GB |
| MegaAssembly levels (`.umap` under `Maps/MASS/`) | 169 | 0.06 GB |
| Character animation assets (sequences, montages, pose assets, blend spaces) | 100 | 0.06 GB |
| Geometry collections (`GC_`) | 69 | 1.53 GB |
| Blueprints (`BP_`) | 63 | 0.02 GB |
| Niagara systems (`NS_`) | 46 | 0.14 GB |
| Material functions (`MF_`) | 33 | small |
| Input actions (`IA_`) | 19 | small |
| Gameplay abilities and effects (`GA_`, `GE_`) | 11 | small |
| Physics assets (`PA_`) | 7 | small |
| Skeletal meshes | 4 (`Echo`, `Echo_Hair`, `SK_AncientOne_Processed`, `SK_ancientOne_skeleton`) | 0.13 GB |
| Top-level maps (`.umap`) | 5 | 0.01 GB |
| Other `.uasset` (control rigs, anim blueprints, sequences, data, Chaos caches, fields) | 521 | 2.59 GB |

Nearly all bytes are in scanned meshes and their 4K and 8K textures. Of the
1,247 staged mesh and texture headers read, 638 textures are 4096 x 4096 and
68 are 8192 x 8192; 341 of the 449 staged static meshes have Nanite on.

## Areas

### Megascans

`Content/AncientContent/Megascans/` holds the Quixel scans, in four groups:

- `3DAssets/` (35.7 GB): `RockSandstone` (55 meshes), `NatureRock2` (63),
  `NatureRock` (12), `QuarryCliff` (21), `LimestoneRocks` (14),
  `IcelandicRockAssembly`, `IcelandicBoulder`, `IcelandicLavaSpire`,
  `IcelandicRock`, `NordicCliff`, `NatureEmbankment`, `CanyonSandstone`,
  `ConcreteRubblePack`, `ConcreteCastleStairs`, `DamagedCastleStairs`,
  `RomanStoneFloor`, `JapaneseShrineFloor`, `BurntFirewoodPack`, and
  `BurntTreeBranch`. `NatureRock2` alone is 11.1 GB.
- `3DPlants/`: `ToadRush`, `GrassClump`, `Deadshrub`, `RiverSaltbush`,
  `BuddingRabbitbrush`, and `Draft` billboards. The plants are the only
  Megascans meshes without Nanite.
- `Surfaces/`: `Rock`, `Sand`, `FlatgroundGravel`, `NaturalGravel`, `Mud`,
  `ErodedGround`, and `DetailTextures`.
- `Decals/Leakage`.

In `AncientWorld`, `RockSandstone` and `NatureRock2` dominate: they're
referenced by about 18,100 and 14,200 actor files, against about 3,200 for
`NatureRock` and under 2,000 for any other set.

### Geometry

`Content/AncientContent/Geometry/` holds sets the project built from scans:

- `PillarCollection/` (7.4 GB): `Pillar`, `PillarChunks`, `PillarBlocks`,
  `Gate`, `GateBase`, `GateBridge`, and `GateChunk`, for example
  `SM_Pillar_M_03_BlockA`. These are the ruin pieces.
- `MASS/` (4.3 GB): 25 large meshes such as `SM_Cliff_L_15_Optimized` and
  `SM_Ground_S_12_Ground`, plus 148 MegaAssembly helper assets.
- `Buttes/`, `BackdropButtes/`, `Backdrop/`: modular and backdrop buttes
  (`SM_ModularButte1`, `SM_ButteMediumDW1`).
- `StoneBlock/` (`SM_StoneBlockA`), `SpireRockCollection/`
  (`SM_SpireRockALow`), `PedestalRock/`.
- `ErosionGround/`, `GroundTiles/`, `Surfaces/` (`DeadRoots`, `DryGround`,
  `IcelandicGravel`, `JaggedRock`, `QuarryGravel`): ground that meets the
  scans.
- `Flag/01` to `03` (cloth banners), `FogCards/`, `LightBlockers/` (meshes
  that shape light without being seen), `SkyDome/`.

### MegaAssemblies

`Content/AncientContent/Maps/MASS/Packed/` holds 165 packed-level maps, each
a group of scans placed as one actor; `Packed/Unique/` holds 7 of them with
their `BP_Collider` blueprints. By name:

| Kind | Maps |
| --- | --- |
| Ground | 30 |
| Scree (including corners and U shapes) | 28 |
| Cliff | 24 |
| Boulders | 16 |
| Butte (including `XXL` cliffs and `Modular_Butte`, `BackdropButte`) | 24 |
| Pile | 14 |
| Hoodoo | 9 |
| Ridge | 6 |
| Hill | 5 |
| Plateau, Flat, Fireplace, Dark World spikes and ground | 9 |

`Maps/MASS/` also holds `MASS_ExampleMap`, `MASS_ScaleReference`,
`Megascans_MASS_Zoo`, and `Megascans_MASS_Zoo_Optimized`.

### Maps and World Partition

| Map | Size | What it is |
| --- | --- | --- |
| `Content/AncientContent/Maps/AncientWorld.umap` | 0.2 MB, plus 13,801 actor files (388 MB) | The playable world |
| `Content/AncientContent/Maps/Startup.umap` | 23 KB | The entry map; it references `AncientWorld` |
| `Content/AncientContent/Maps/Megascans_Asset_Zoo.umap` | 0.3 MB | Every scan laid out for review |
| `Plugins/GameFeatures/AncientBattle/Content/Maps/L_AncientBattleGameplay.umap` | 4.7 MB | The boss arena |
| `.../Destruction/Arena/e_Cache/6_Fall/Robot_Head_Destruction_Cached.umap` | 7 KB | One recorded destruction |

`AncientWorld` is partitioned (`bIsPartitioned`) with a runtime spatial hash
(`WorldPartitionRuntimeSpatialHash`) and one grid, `MainGrid`. Read from the
map's serialized properties, `MainGrid` has a cell size of 6,400 units and a
loading range of 6,400 units, so 64 m cells loaded within 64 m; the editor
hash uses 51,200-unit (512 m) cells. Confirm both in the editor. The map also
references an instanced HLOD layer (`AncientWorldHLODLayer_Instanced`) and
three data layers under `Content/AncientContent/DataLayers/AncientWorld/`:
`Dark_World`, `Campfire_Geometry__`, and `Campfire_Replace`. The level
script hides the Dark World rift until Echo stands up, and switches the
campfire audio off on the transition.

The 13,801 actor files, classified by the class each one imports:

| Actor class | Files |
| --- | --- |
| `PackedLevelActor` (MegaAssembly placements) | 7,334 |
| `StaticMeshActor` | 4,686 |
| `WorldPartitionHLOD` | 879 |
| `DecalActor` | 444 |
| `BlockingVolume` | 139 |
| `BP_CloudMask_Object_C` | 130 |
| Lights (19 point, 12 spot, 3 rect, 2 directional) | 36 |
| `BP_Fogsheet_ProceduralRamp_C` | 12 |
| `PostProcessVolume` | 11 |
| Sky and fog (`SkyAtmosphere`, `SkyLight`, `ExponentialHeightFog`, `VolumetricCloud`) | 7 |
| Other (Echo, the rift, UI volumes, sounds, a sequence, triggers) | about 123 |

There is no landscape actor: the ground is scanned meshes and ground
assemblies. The remaining 443 actor files belong to the MegaAssembly maps.

### Echo

See [Echo's folder](#echos-folder).

### AncientBattle

`Plugins/GameFeatures/AncientBattle/` is the boss fight, loaded as a game
feature. It isn't staged, so these are names and sizes from the manifest:

- `Characters/AncientOne/`: `SK_AncientOne_Processed` (26.9 MB), its
  skeleton and physics assets, 270 static meshes and 256 fracture pieces
  for its armor (10.4 GB), clips such as `Robot_Base_Idle`, `Robot_Charge`,
  `Robot_Fire`, `Robot_Shoulder_Hit`, `Robot_Stomach_Hit`, and
  `Robot_Collapse_Idle`, and the control rigs `AncientOne_Body_CtrlRig`,
  `AncientOne_Mechanics_CtrlRig`, and `CR_AncientOne_ArmAim_CtrlRig`.
- `Characters/Echo/`: Echo's combat clips (see [Echo's folder](#echos-folder)).
- `Destruction/`: `Ruins` (228 meshes), `SandStone`, `SpireRocks`,
  `IcelandicRock`, `HeadStaticMeshes` (460 fracture pieces), field systems
  (`FS_`, `Field_`), anchors, cutters, and physical materials.
- `Maps/`: `L_AncientBattleGameplay`, 58 geometry collections, Chaos cache
  collections such as `CCC_Mound_Back_Pass1_02` and `CCC_LtHand_Final`, and
  pillar and gate collections under `Site01` and `Site02` (for example
  `GC_mid_Pillar`, `GC_Gate_C_SGRes`).
- `FX/`: `Laser`, `Laser_Trail`, `Head_Explosion`, `AncientOneCore`,
  `Rise_Dust`, `Fall_Dust`, `PillarDust`, and 28 Niagara systems.
- `Blueprints/`: `BP_AncientOne`, `BP_AncientOneLaser`, the abilities
  `GA_LightDart`, `GA_Dodge`, and `GA_ForceWalk`, the effects
  `GE_LightDartDamage` and `GE_Levitate`, gameplay cues, targeting, and
  camera modes (`BP_LightDartADSCameraMode`, `BP_DodgeCameraMode`).
- `Sequences/`: `SEQ_Robot_Intro`, `SEQ_Robot_Death`, `LSeq_Pose_Anim`.

### Effects, lighting, and sky

- `Effects/Fog/`: Niagara fog from density maps
  (`NS_FogFromDensityMap`, `NS_FogFromFogSample`, `NS_SandStripes`) and a
  volumetric painting tool (`BP_NiagaraVolumetricPainting`).
- `Effects/Campfire/`, `Effects/Portal/` (`SM_GeoPortal`),
  `Effects/Footsteps/` (`NS_Echo_Footstep_Dust`), `Effects/Drone/`
  (`FX_Echo_Drone`).
- `Lighting/`: fog sheets (`BP_Fogsheet_ProceduralRamp`,
  `SM_FogSheet_Plane`), `Light_Sky_Transitions`, a bokeh shape.
- `Clouds/`: volumetric cloud materials for midday and campfire
  (`M_VolumetricCloud_03_MultipleProfiles_CF_Midday`), sky atmosphere
  material domes, and a cloud-shadow light function
  (`M_Sun_Cloud_LightFunction`).
- `Textures/`: `SkyA_8kx2k_v5_3`, `Darkworld_skyA`, `T_Distant_Cliffs_D`.

### Materials

`Content/AncientContent/Materials/`: butte materials (0.45 GB of textures),
decals, material functions, post-process instances, sky, vegetation, and
`DemoUtility`.

### Audio

`Content/AncientContent/Audio/`: 302 sound waves (238 effects, 52 music,
10 voice), 23 MetaSounds, Soundscape palettes (`DesertPalette`,
`NoonPalette`, `DarkworldPalette`, `DarkworldExploration`), submixes, audio
modulation buses and patches, and `Underscore` cues. Echo's foley covers
cloth, footfalls on rock at walk and run, hand plants, pouch, scuffs, and
exertion breaths.

### Gameplay, input, and UI

- `Blueprints/`: `BP_EchoCharacter`, `BP_VaultingTriggerComponent`,
  `BP_VaultingUtilLib`, the abilities `GA_SitStand`, `GA_Vault`, and
  `GA_TouchInteract`, the effect `GE_Walk`, camera modes
  (`BP_ThirdPersonCameraMode`, `BP_VaultingAreaCameraMode`,
  `BP_SplineKeyedCameraMode`, `BP_CatwalkCameraMode`), and
  `BP_DarkWorldRift`.
- `Input/`: `IA_MoveForward`, `IA_MoveRight`, `IA_Turn`, `IA_LookUp`,
  `IA_Interact`, `IA_SitStand`, and their rate variants, mapped by
  `IM_ThirdPersonControls_InputMapping`.
- `UI/`: tutorial call-outs and prompt textures.

### HoverDrone

`Plugins/GameFeatures/HoverDrone/` is a free-flying camera drone that Echo
releases: `BP_HoverDronePlayerPawn`, `BP_HoverDroneMote`,
`GA_DeployHoverDrone`, drone input (`IA_HoverDrone_Turbo`,
`IA_HoverDrone_SlideUp_World`), and Echo's `Mote_Release` and
`Mote_Release_Sitting` clips. It's a camera, not character flight.

## Echo's folder

Echo's model, rig, and animation sit in
`Content/AncientContent/Characters/Echo/` (183 files, 1.61 GB). Her combat
clips sit in `Plugins/GameFeatures/AncientBattle/Content/Characters/Echo/`.
Her blueprint is `Content/AncientContent/Blueprints/Character/BP_EchoCharacter`,
a `ModularCharacter` from the `ModularGameplayActors` plugin.

### Meshes and skeleton

| Asset | Header tags |
| --- | --- |
| `Meshes/Echo` | Skeletal mesh: 101,672 vertices, 174,003 triangles, 1 LOD, 134 bones, 181 morph targets |
| `Meshes/Echo_Hair` | Skeletal mesh (hair cards): 211,699 vertices, 316,103 triangles, 1 LOD, 200 bones |
| `Meshes/Echo_Skeleton` | The shared skeleton; sync markers `Foot_L` and `Foot_R`; notifies `EndThrow`, `DisableInput`, `ReleaseDart`, `AbilityAnimNotify` |
| `Hair/Hair_S_UpdoBuns`, `Hair/Eyebrows_L_Echo` | Strand grooms with bindings to the LOD0 mesh |

Her skeleton follows the UE5 mannequin: `root`, `pelvis`, `spine_01` to
`spine_05`, `neck_01`, `neck_02`, `head`, clavicles, arms with two twist
bones each, hands with metacarpals and three joints per finger, legs with
two twist bones each, `foot`, `ball`, and the `ik_foot_*` and `ik_hand_*`
bones. On top of that it adds:

- facial joints (`FACIAL_C_Jaw`, eyes, eyelids, pupils, teeth, a four-bone
  tongue);
- dynamics chains for the ponytail (8 bones), scarf (14), skirt, and braids;
- pose-driver corrective joints named by angle (for example
  `upperarm_l_0_n90`).

Her 181 morph targets are the ARKit face set (`eyeBlinkLeft`, `jawOpen`,
`mouthSmileLeft`) plus combination correctives.

### Materials and textures

Materials: `M_Echo_Skin`, `M_Head_Echo`, cloth masters (`M_Echo_ClothSimple`,
`M_Echo_ClothShirtMaterial`, `M_Echo_ClothScarf`, `M_Echo_ClothGold`),
instances for leather, pants, shirt, scarf, canteen, buckle, and accessories,
refractive eyes (`M_EyeRefractive`), lashes, teeth, a groom master, and
subsurface profiles.

Textures, by name and header dimensions:

| Set | Textures | Size |
| --- | --- | --- |
| Costume 1 to 3 | `T_echo_Costume{1,2,3}_D`, `_N`, `_BN` (bent normal), `_ORM`, `_AO`, `_MatID` | 4096 x 4096 |
| Costume 4 and 5 | Same maps | 2048 x 2048 |
| Head | `T_echo_Head_D`, `_BN`, `_ORM`, `_MatID`, brows, `T_echoGirl_Head_N`, `_micro_N` | 4096 x 4096 |
| Head detail | `head_normal_map_003_wm0`, `head_spec_highpass_003`, `head_thickness_map_001_tweaked`, `toksvig_macro` | 8192 x 8192 |
| Eyes | `T_ScleraBase_001`, `T_ScleraVeins_001`, `T_ScleraVessels_002` | 8192 x 8192 |
| Eyes, small | iris, eye normals, wetness, midplane displacement | 512 to 1024 |
| Hair cards | `hair_alpha`, `Depth`, `HairID` at 4096; `Bent_normal`, `Baked_occlusion` at 2048 | |
| Skin roughness masks | `head_SkinRoughness_Mask_001` to `_009` | 2048 x 2048 |

Textures use BC1 (`DXT1`) for color and BC5 for normals, in the character
texture groups. The head's base color has virtual texture streaming on.

### Animations

`Characters/Echo/Animations/`, all at 30 frames per second:

| Clip | Frames | Root motion |
| --- | --- | --- |
| `Idle`, `Idle_Long` | 200, 605 | No |
| `Walk_Fwd`, `Walk_Fwd_to_Idle` | 125, 170 | No |
| `Idle_to_Walk_Fwd`, `_Left_90`, `_Left_180`, `_Right_90`, `_Right_180` | 57 to 71 | No |
| `Jog_Fwd`, `Jog_Fwd_to_Idle` | 41, 104 | No |
| `Idle_To_Jog_Fwd`, `_Left_90`, `_Left_180`, `_Right_90`, `_Right_180` | 60 to 84 | No |
| `Jump_Idle_Fall`, `Jump_Idle_Land` | 9, 60 | No |
| `VaultOver` (with `VaultOver_Montage`) | 61 | Yes |
| `ReachOut_Start`, `ReachOut_Loop`, `ReachOut_End` (with montages) | 40, 200, 59 | Yes |
| `Sitting_Idle`, `StandFromSitting` | 200, 145 | No |

AncientBattle adds `Dodge_To_Idle`, `Dodge_To_Idle_MidAir`, and
`Dodge_To_Run` (with montages), and the light dart set: `LightDart_Charge_1_5s`,
`_Loop`, `_Cancel`, `LightDart_Release`, the pose blend spaces
`LightDart_Charge_Pose_Direction_BS` (center and four directions) and
`LightDart_Charge_Pose_Pitch_BS` and `LightDart_Release_Pose_Pitch_BS` (up,
center, down), and the linked layer `Echo_LightDart_AnimBPLayer`.

### Animation blueprint, rigs, and physics

- `Echo_AnimBP` (parent `AncientGameAnimInstance`) runs a `Locomotion` state
  machine over `Enum_LocomotionState` (`Idle`, `Walk`, `Jog`): idle states
  (`Idle Loop`, `Wide Idle`), walk and jog states with directional starts
  chosen by start angle, stops, an `InAir` state with `Land`, and `Sitting`
  with `StandUp`. Transitions use inertialization. It links
  `Echo_AnimLayerInterface` (`FullBodyLayer`), runs
  `Echo_SlopeWarping_CtrlRig`, can enable full-body IK, and drives the hair
  through `Echo_Hair_AnimBP`.
- Control rigs: `Echo_SlopeWarping_CtrlRig` (feet and pelvis on slopes),
  `Echo_Twist_CtrlRig` (twist bones), `Echo_Helpers_CtrlRig`.
- `Echo_PostProcess_AnimBP` applies RBF pose drivers from the
  `Rig/PoseAssets/` pose assets (`upperarm`, `lowerarm`, `hand`, `calf`,
  `neck_02`) and rigid-body dynamics.
- Physics assets: `PA_Echo` and separate ones for the canister, scarf,
  ponytail, pouch, shoulder pad, and skirt. The scarf and skirt also have
  cloth meshes (`ClothMESH_Scarf`, `ClothMESH_Skirt`).

### Movement she supports

| Move | Supported | How |
| --- | --- | --- |
| Idle, walk, jog | Yes | `Echo_AnimBP` state machine, with turn-angle starts and stops |
| Run, sprint | No | `Jog` is her fastest gait; there is no sprint clip or state |
| Jump, fall, land | Yes | `InAir` and `Land` states |
| Vault | Yes | `GA_Vault`, `BP_VaultingTriggerComponent`, motion warping, `VaultOver` with root motion |
| Sit and stand | Yes | `GA_SitStand` |
| Reach out (touch interact) | Yes | `GA_TouchInteract`, `ReachOut_*` |
| Dodge | Yes, in the battle | `GA_Dodge` |
| Light dart (aimed throw) and levitation while charging | Yes, in the battle | `GA_LightDart`, `GE_Levitate` |
| Slope adaptation | Yes | Slope-warping control rig |
| Climb, slide, mantle, traversal beyond the vault | No | |
| Flight | No | The hover drone flies; Echo doesn't |

What this tells us for Verse: Echo's locomotion is narrow and polished.
Directional starts, stops, slope warping, and corrective poses make a small
clip set read well, which matters more than the clip count.

## What to study each area for

Cross-referenced to [the plan](ue5-ruins-study.md#the-plan) and
[the study list](ue5-ruins-study.md#the-study-list):

| Area | Study it for | Plan item |
| --- | --- | --- |
| `Geometry/PillarCollection`, `StoneBlock`, `SpireRockCollection` | Broken columns, gates, blocks, spires: heights, breaks, how they meet ground | 1, ruins kit |
| `Maps/MASS/` and the 7,334 packed placements | How assemblies interlock and repeat; which kinds carry the world | 6, assemblies |
| Megascans sets and their 4K and 8K maps | Which maps a stone material uses; normal detail against base color | 2, normal maps |
| Nanite meshes' source against fallback counts | Ratios for far levels; the virtual geometry measurement | 3, LODs; 8, virtual geometry |
| `AncientWorld` World Partition: 64 m cells, HLOD actors, data layers | Cell size and loading range at this density; the Dark World swap as a data layer | 7, streaming |
| `LightBlockers`, fog sheets, post-process volumes, sky and cloud materials | Lighting a canyon without bounce from lamps; fog as depth | 5, lighting |
| AncientBattle `Destruction/` and the Chaos caches | Fracture patterns and piece counts for columns and arches | Collapsing pillar and arch in `carve.rs` |
| Ground assemblies, `ErosionGround`, `GroundTiles` | How scans meet the ground without a landscape | 1, debris sets |
| Echo's folder | Locomotion set, starts and stops, slope warping, silhouette | [Female character](female-character.md) |

## Finish the inventory in the editor

A mesh-level inventory needs Unreal Editor 5.7, which isn't installed (the
launcher records no engine, and `~/work/UnrealEngine` is an unbuilt 5.8.3
source tree). The header read above can't give Nanite source triangle
counts, material graphs, placement transforms, or World Partition settings
beyond the serialized defaults. An agent finishes it like this:

1. Install the engine: in the Epic Games Launcher, go to **Unreal Engine >
   Library**, and next to **Engine Versions**, click **+** and choose 5.7.
2. Finish the project: in **Fab Library**, choose **Create Project** for
   Valley of the Ancient, with engine 5.7 and the folder
   `~/Documents/Unreal Projects`. Then check that
   `~/Documents/Unreal Projects/ValleyoftheAncient/AncientGame.uproject`
   exists and that `Plugins/GameFeatures/AncientBattle/` has 2,478 files.
3. Open the project and let shaders compile. Keep it open read-only in
   spirit: don't save, so the packages keep their recorded versions.
4. Export the registry with the editor's Python console (**Tools >
   Execute Python Script**). For each asset under `/Game`, record the path,
   class, and tags; for each static mesh, record the triangle count of LOD
   0, whether Nanite is on, the fallback percentage, and position
   precision; for each texture, its size and compression. Write the result
   as CSV under `bench/verse/<date>/valley-study/`. Check the method names
   against the 5.7 Python API reference before you run the script.
5. For 20 representative Nanite meshes, open the Static Mesh Editor and
   record the Nanite source and fallback triangle counts from its details
   panel.
6. Open `AncientWorld`. In **World Settings > World Partition Setup**,
   record each runtime grid's cell size, loading range, and HLOD layer. Open
   **Window > World Partition > World Partition Editor** and the **Data
   Layers Outliner**, and record the data layers and the world's bounds.
   Use **Tools > Statistics** for actor and triangle counts in one 50 x 50 m
   section.
7. Open `Echo_AnimBP` and record its state machines, transitions, and
   blend times; open the control rigs and physics assets and record their
   bone and body counts. Record names and numbers only.
8. Take screenshots for the study note as reference only. They never enter
   a pack.

Don't use any export command (**Asset Actions > Export**, FBX, glTF, USD, or
image export), and don't open Valley content in Blender. The
[licensing rules](ue5-ruins-study.md#licensing) list what's allowed.
