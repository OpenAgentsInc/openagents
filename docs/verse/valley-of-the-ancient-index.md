# Valley of the Ancient: content index

This page indexes Epic's Valley of the Ancient sample as it sits on this Mac:
where it's installed, the content tree with sizes and counts, the asset
categories, the maps and World Partition setup, the Echo character's folder,
the AncientBattle and HoverDrone game features, the C++ source, and the
small plugins. It ends with what each area is worth studying for and the
steps to finish the inventory in the Unreal Editor.

It extends the [UE5 ruins study](ue5-ruins-study.md), whose licensing rule
holds here: Valley is UE-Only content, studied in Reference-only mode.
Nothing from it ships in Verse, no Valley texture enters a pack, and no file
from the project is copied, converted, or committed. This index records
file names, sizes, and package header metadata only. It names the C++
modules and classes and says what they do; it doesn't quote the code.

Status on October 5, 2026, about 20:10 local: the project is installed and
complete. All 19,784 files are present, and nothing has opened it yet.
Opening it needs Unreal Engine 5.7 from the Epic Games Launcher, which isn't
installed. `~/work/UnrealEngine` is a 5.8.3 source checkout with no macOS
build.

## Where it is

| Path | What's there |
| --- | --- |
| `~/Documents/Unreal Projects/ValleyoftheAncient/` | The project: 19,784 files, 92.06 GB (85.74 GiB), `EngineAssociation` `5.7`. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/data/` | The launcher's vault cache: a second full copy of the same 19,784 files, 92.06 GB. See [Disk use and the vault copy](#disk-use-and-the-vault-copy). |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/manifest` | The build manifest the launcher installed from. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/FabLibrary/Valley_of_the_Ancient-0c19880e/unreal-engine/manifest` | The 7.5 MB binary build manifest (Epic BuildPatch format, version 21, zlib-compressed). It lists every file, its size, and its SHA-1. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/FabLibrary/listings_v1.db` | The launcher's Fab library listing cache. |

How it got there, from `~/Library/Logs/Unreal Engine/EpicGamesLauncher/`:

- At 17:33 local, the launcher began building app `AncientGame_5.7` into the
  vault (`VerifyMode` `ShaVerifyAllFiles`).
- At 18:30 it crashed in its embedded browser (`SIGTRAP` in Chromium
  Embedded Framework) with 17,094 files staged, and restarted without
  resuming. `EpicGamesLauncher-backup-2026.10.05-23.30.36.log` holds the
  trace.
- After the owner restarted the download, the vault build finished.
- At 20:05 local, **Create Project** ran `RequestInstallFromCache`: it
  copied all 19,784 files from the vault's `data/` to the project folder
  (`bMove 0`, so a copy rather than a move) and finished at 20:07 with
  `AlertCode=[ok]` and `IncompleteInstall=0`.

The project differs from the vault copy in four places: the launcher renamed
`AncientGame.uproject` and `AncientGame.png` to `ValleyoftheAncient.uproject`
and `ValleyoftheAncient.png`, rewrote `Config/DefaultEngine.ini` (it dropped
comments and added `[URL] GameName=ValleyoftheAncient`), and changed one line
of `Source/AncientGame/AncientGame.cpp`. Every other file has the same size.

### Disk use and the vault copy

The vault copy is a true duplicate, not an APFS clone. For sampled files
(`DerivedDataCache/Compressed.ddp`, an Ancient One texture, `Echo.uasset`,
and a source file), the two copies have different inodes and different
physical block addresses (`F_LOG2PHYS_EXT`), so they share no storage.
Together they hold about 184 GB. The data volume had 367 GB free after the
install, against 483 GB while the download was stalled.

The project doesn't need the vault copy. The launcher keeps it so that a
second **Create Project** for Valley doesn't download again. To reclaim the
92 GB, remove it through the launcher: in **Unreal Engine > Library > Fab
Library**, use the Valley of the Ancient entry's menu to clear its cached
download if the launcher offers that option. Otherwise, quit the launcher
and move `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/`
to the Trash. A later **Create Project** downloads it again. Leave
`FabLibrary/` alone, because it holds the listing cache and manifests for
every Fab item. **Settings > Edit Vault Cache Location** moves future vault
downloads to another disk.

## How this index was made

Everything below comes from three sources, read in place:

1. **The file tree.** A walk of the installed project gave every path and
   size. It matches the build manifest's 19,784 files and 92,062,216,042
   bytes.
2. **Package headers.** For all 5,307 `.uasset` and `.umap` packages outside
   `__ExternalActors__`, a reader took the name table and the asset registry
   tags that each package stores in its header: class, texture
   `Dimensions` and `Format`, static mesh `Triangles` and `NaniteEnabled`,
   skeletal mesh `Bones` and `MorphTargets`, animation `Number of Frames`,
   `Source Frame Rate`, and `bEnableRootMotion`, and Blueprint
   `ParentClass`. It also read a few serialized World Partition properties
   (cell size, loading range) from `AncientWorld.umap`.
3. **Plugin descriptors, configuration, and C++ declarations.** The
   `.uproject` and `.uplugin` files, the gameplay tag lists under
   `Config/Tags/`, and the class declarations and Blueprint-callable
   function names in the C++ headers.

It decoded no mesh, texture, animation, or audio data, rendered nothing, and
wrote nothing into the project or the vault. The scratch reader isn't
committed.

The packages are editor packages: one `.uasset` or `.umap` per asset, with
no `.uexp` or `.ubulk` split. There is no `AssetRegistry.bin`; the editor
builds the registry when the project opens, and a cook writes it.

For a Nanite mesh, the `Triangles` tag counts the fallback mesh (2,000
triangles for a typical Megascans rock), not the source. File size is the
better proxy for source density until the editor gives the real number.

The earlier, download-time version of this index read headers from staged
files only. Checked against the installed tree, its folder sizes, file
counts, prefix counts, and Echo's tags all hold. Four numbers changed: the
small plugins hold 152 files, not 151; `Maps/` in AncientBattle holds 50
geometry collections, not 58; the `GC_` prefix row mixed in gameplay cue
Blueprints and textures; and the full tree has more 4K and 8K textures and
Nanite meshes than the staged subset showed (see [Asset
categories](#asset-categories)).

## The project

| Item | Value |
| --- | --- |
| Fab listing | Valley of the Ancient, `0c19880e-21bd-42ba-8287-1caccc3951b1` |
| App and build | `AncientGame_5.7`, build version `5.7.0-47537391+++UE5+Release-5.7-Mac` |
| Project file | `ValleyoftheAncient.uproject`, `EngineAssociation` `5.7`, category `Samples` |
| Saved by | 4,854 packages by `++UE5+Release-5.0-EarlyAccess`; 351 by `++UE5+Release-5.7` (every sound wave and MetaSound, resaved for this release); 50 by Epic's internal `Main` branches; 52 by 5.0 to 5.3 releases. Older packages resave on open. |
| Module | `AncientGame` (Runtime), with the editor library `UnrealEditor-AncientGame.dylib` prebuilt for Mac in `Binaries/Mac/` |
| Target platforms | `PS5`, `Windows`, `WinGDK`, `XSX` |
| Game features | `AncientBattle` (the boss fight, content only) and `HoverDrone` (Echo's flying camera drone, with a C++ module) |
| Project plugins | `ModularGameplayActors`, `Underscore`, `Crossfader`, `Uproar` (under `Plugins/Systems/`), and `InstanceLevelCollision` |
| Engine plugins enabled | `GameplayAbilities`, `GameFeatures`, `ModularGameplay`, `EnhancedInput`, `MotionWarping`, `ControlRig`, `FullBodyIK`, `HairStrands`, `ChaosCaching`, `FieldSystemPlugin`, `Metasound`, `AudioModulation`, `Soundscape`, `NiagaraFluids`, `Volumetrics`, `VirtualHeightfieldMesh`, `ImpostorBaker`, `ModelingToolsEditorMode`, `MovieRenderPipeline` |
| Maps | `GameDefaultMap` `AncientWorld`, `EditorStartupMap` `Startup` |
| Rendering | Lumen for global illumination and reflections (`r.DynamicGlobalIlluminationMethod=1`, `r.ReflectionMethod=1`), Nanite tuned with `r.Nanite.MaxPixelsPerEdge=2` |

## Content tree

Sizes are decimal gigabytes from the installed tree.

| Folder | Size | Files | What it holds |
| --- | --- | --- | --- |
| `Content/AncientContent/Megascans/` | 38.81 GB | 1,018 | Scanned rocks, cliffs, plants, and ground surfaces |
| `Content/AncientContent/Geometry/` | 15.94 GB | 549 | Project-made sets: pillars, gates, buttes, spires, stone blocks, ground tiles, MegaAssembly meshes |
| `Plugins/GameFeatures/AncientBattle/Content/Characters/` | 10.44 GB | 674 | The Ancient One (boss) and Echo's combat clips |
| `Plugins/GameFeatures/AncientBattle/Content/Destruction/` | 2.95 GB | 987 | Fracture source meshes, fracture pieces, fields, textures |
| `Plugins/GameFeatures/AncientBattle/Content/Maps/` | 2.51 GB | 144 | The battle map, its geometry collections, and Chaos caches |
| `Content/AncientContent/Characters/` | 1.61 GB | 183 | Echo |
| `Content/AncientContent/Materials/` | 0.72 GB | 91 | Master materials, butte materials, post process |
| `Plugins/GameFeatures/AncientBattle/Content/FX/` | 0.69 GB | 398 | Boss lasers, explosions, dust |
| `Content/AncientContent/Maps/` | 0.66 GB | 306 | Four top maps and the MegaAssembly levels |
| `Content/__ExternalActors__/` | 0.39 GB | 14,244 | World Partition, one file per actor |
| `Content/AncientContent/Audio/` | 0.31 GB | 415 | MetaSounds, sound waves, Soundscape palettes, submixes |
| `Content/AncientContent/Effects/` | 0.15 GB | 120 | Fog, campfire, portal, footsteps, drone |
| `Plugins/GameFeatures/AncientBattle/Content/Audio/` | 0.14 GB | 189 | Boss and ability sound |
| `Content/AncientContent/Textures/` | 0.13 GB | 5 | Sky and distant cliff images |
| `Content/AncientContent/` other (`UI`, `Blueprints`, `Lighting`, `Clouds`, `Input`, `DataLayers`) | 0.04 GB | 118 | Gameplay, cameras, sky, input |
| `Plugins/GameFeatures/AncientBattle/` other (`Blueprints`, `Feedbacks`, `UI`, `Input`, `Sequences`, `Materials`, `Meshes`, `Config`, descriptor) | 0.02 GB | 86 | Abilities, targeting, camera shakes, cinematics |
| `DerivedDataCache/Compressed.ddp` | 16.51 GB | 1 | Prebuilt shader and mesh data, so the first open is faster |
| `Plugins/GameFeatures/HoverDrone/` | 0.01 GB | 36 | The drone pawn and its C++, input, and Echo's release clips |
| `Plugins/Systems/`, `Plugins/InstanceLevelCollision/` | small | 116 | Audio systems, modular actors, a collision tool |
| `Source/` | 0.2 MB | 56 | The `AncientGame` module |
| `Config/`, `Binaries/`, `Build/`, `Content/Localization/`, `Content/Splash/`, project file and icon | small | 48 | |

## Asset categories

Classified by name prefix and extension across the whole project; the
counts match a case-insensitive match on each prefix:

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
| `GC_` prefix (42 geometry collections, 19 textures, 8 gameplay cue Blueprints) | 69 | 1.53 GB |
| Blueprints (`BP_`) | 63 | 0.02 GB |
| Niagara systems (`NS_`) | 46 | 0.14 GB |
| Material functions (`MF_`) | 33 | small |
| Input actions (`IA_`) | 19 | small |
| Gameplay abilities and effects (`GA_`, `GE_`) | 11 | small |
| Physics assets (`PA_`) | 7 | small |
| Skeletal meshes | 4 (`Echo`, `Echo_Hair`, `SK_AncientOne_Processed`, `SK_ancientOne_skeleton`) | 0.13 GB |
| Top-level maps (`.umap`) | 5 | 0.01 GB |

By the class in each package header, the 5,307 packages are mostly 1,900
static meshes (45.78 GB; the count includes fracture pieces and armor parts
that lack the `SM_` prefix), 1,116 textures (26.11 GB), 537 material
instances, 457 sound waves, 268 Blueprints, 201 materials, 174 worlds, 69
animation sequences, 53 geometry collections (1.77 GB), 50 Niagara systems,
49 MetaSounds, 25 field systems, 19 level sequences, and 9 Chaos cache
collections (0.31 GB).

Nearly all bytes are in scanned meshes and their 4K and 8K textures. Under
`Content/`, 645 of the textures are 4096 x 4096 and 72 are 8192 x 8192, and
343 of the 581 static meshes have Nanite on. Across the whole project, 525
of the 1,900 static meshes have Nanite on; most of the rest are fracture
pieces and the boss's armor parts.

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
| `Plugins/GameFeatures/AncientBattle/Content/Maps/L_AncientBattleGameplay.umap` | 4.7 MB | The boss arena, added to `AncientWorld` as a level instance |
| `.../Maps/Destruction/Arena/e_Cache/6_Fall/Robot_Head_Destruction_Cached.umap` | 7 KB | One recorded destruction |

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

- `Blueprints/`: `BP_EchoCharacter` (parent `ModularCharacter`),
  `BP_VaultingTriggerComponent` (parent `AncientGameBoxComponentBase`),
  `BP_VaultingUtilLib`, the abilities `GA_SitStand`, `GA_Vault`, and
  `GA_TouchInteract`, the effect `GE_Walk`, `BP_GameplayEffectVolume`,
  `BP_AbilityEventNotify`, the interfaces `BPI_Interactable` and
  `BPI_InteractorController`, `BP_LinkAnimLayersComponent`, camera modes
  (`BP_ThirdPersonCameraMode` from the C++ `SpringArmCameraMode`, and
  `BP_VaultingAreaCameraMode`, `BP_SplineKeyedCameraMode`,
  `BP_CatwalkCameraMode`, `BP_StaticFramingCamera` from it),
  `BP_AncientGameMode`, `BP_AncientGameInstance`, and `BP_DarkWorldRift`.
- `Input/`: `IA_MoveForward`, `IA_MoveRight`, `IA_Turn`, `IA_LookUp`,
  `IA_Interact`, `IA_SitStand`, and their rate variants, mapped by
  `IM_ThirdPersonControls_InputMapping`.
- `UI/`: tutorial call-outs and prompt textures.
- `Config/Tags/BaseCharacterAbilityTags.ini` and `DefaultGameplayTags.ini`:
  the tags the abilities trade in, such as `Ability.LocomotionControl`,
  `Ability.TerrainInteraction`, `AbilityEvent.Character.Locomotion.Vault`,
  `Status.InCinematic`, and `Status.Buff.Speed.Override`.

## AncientBattle

`Plugins/GameFeatures/AncientBattle/` is the boss fight: 2,478 files,
16.75 GB, loaded as a game feature (`ExplicitlyLoaded`). It has no C++
module; it's Blueprints and content over the `AncientGame` module's ability
system and game feature actions (see [Source](#source)). By header class,
its 2,474 packages are 1,319 static meshes, 287 textures (11.78 GB), 186
material instances, 153 sound waves, 120 materials, 71 Blueprints, 53
geometry collections (1.77 GB), 40 Niagara systems, 34 animation sequences,
26 MetaSounds, 25 field systems, 16 level sequences, 9 Chaos cache
collections, and 2 worlds.

### How the feature attaches

Its `GameFeatureData` asset, `AncientBattle`, uses five actions. Which
asset each action carries is read from the names the asset references:

- `GameFeatureAction_AddLevelInstances` puts `L_AncientBattleGameplay` into
  `AncientWorld`.
- `GameFeatureAction_AddAbilities` grants `GA_LightDart`, `GA_Dodge`, and
  `GA_ForceWalk` to `BP_EchoCharacter`.
- `GameFeatureAction_AddInputContextMapping` adds
  `IM_LightDartInputMappings` (`IA_Fire`, `IA_JogOverride`).
- `GameFeatureAction_AddComponents` adds components, among them
  `BP_LinkAncientBattleAnimLayersComponent`, which links Echo's light dart
  animation layer, and `BP_TargetableRegistrarComponent`.
- `GameFeatureAction_AddWorldSystem` starts `BP_WorldTargetableManager`, a
  `GameFeatureWorldSystem` that tracks what Echo can target.

The game mode exposes `ToggleGameFeaturePlugin` beside its Dark World
functions, which suggests the feature loads as the world turns dark;
confirm it in `BP_AncientGameMode`.

### The Ancient One

`Characters/AncientOne/` (648 files, 10.43 GB) is the boss, a stone and
metal golem the audio and music tags call `Golem`:

- `SK_AncientOne_Processed`: a skeletal mesh with 141,963 vertices, 150,571
  triangles, 1 LOD, 143 bones, and no morph targets.
  `SK_ancientOne_skeleton` is a 12-triangle placeholder on the same
  143-bone skeleton.
- 529 static meshes (161 with Nanite) for rigid armor parts, grouped by
  body part: `arm_l`, `arm_r`, `clavicles`, `head`, `legs`, `pelvis`,
  `spine02`, `spine03`, plus `CollisionPartsNew2` (114 files) and
  `Fracture` (258 files, mostly `frac_` pieces of the head).
- 48 textures, 8.96 GB, the largest in the project. They're streamed as
  virtual textures, with header dimensions up to 16,384 x 12,288, though
  their names say 4K (`T_LegL_4K_D`), which suggests 4 x 3 UDIM tiles of 4K
  each. Each part has base color (`_D`), normal (`_N`), and
  metal-roughness-occlusion (`_MRAO`) maps.
- 13 clips at 30 frames per second, each with a source level sequence
  (`SourceSequences/SEQ_Robot_*`): `Robot_Base_Idle` (195 frames),
  `Robot_Fidget_Idle_A` and `_B` (195), `Robot_Charge` (59),
  `Robot_Fire` and `Robot_Fire_Idle` (56), `Robot_Shoulder_Hit` (99),
  `Robot_Stomach_Hit` (79), `Robot_Collapse_Idle` (79), three blends, and
  `Robot_Infinite_Dab`. `FixedAnimation/ancientOne_FirstFrame` runs 718
  frames.
- Control rigs `AncientOne_Body_CtrlRig` (8.7 MB), `AncientOne_Mechanics_CtrlRig`,
  and `CR_AncientOne_ArmAim_CtrlRig`; `AncientOne_AnimBP` and
  `AncientOne_PostProcess_AnimBP`; control rig poses `Attack_R`, `Fist_L`,
  and `Fist_R`; physics assets `AncientOne_Armor_Physics` and
  `ancientOne_skeleton_PhysicsAsset`.
- `Blueprints/AncientOne/`: `BP_AncientOne` (2.8 MB, the fight's main
  actor), `BP_AncientOneLaser`, `Enum_AncientOneBehaviorStates`, and
  `MPC_AncientOneControls`.

### The fight's structure

The names lay out the fight:

- **States.** Gameplay cues `ReadyCharge`, `ChargeShoot`, `LaserDone`,
  `Downed`, and `Dead`, and music states `Ambient`, `GolemAwaken`,
  `GolemFiring`, `GolemDown`, and `Active` with three battle intensities
  (`Config/Tags/MusicGameplayTags.ini`).
- **Weak points.** Physical materials `PhysMat_AncientOne_HotSpot`,
  `PhysMat_AncientOne_Head`, and `PhysMat_AncientDestructible`, with
  impact effects `NS_HotSpotImpact`, `NS_WeakPointImpact`, and
  `NS_DestructibleImpact`. `BP_Targetable` (implementing `BPI_Targetable`),
  `BP_Targetable_Chaos`, and `BP_TargetableRegistrarComponent` mark what
  the light dart can lock on to.
- **Arena destruction in body-named zones.** Under `Maps/Destruction/Arena/`,
  each zone has intact geometry (`b_PreDestruction`), its geometry
  collections (`c_Destruction`), and a recorded Chaos cache (`e_Cache`):
  `1_RtHand`, `2_Back`, `3_LtHand`, `4_RtFoot`, and `6_Fall` (the head).
  The map's names group them under `ElectricDesert/Destructable Zones`.
  `Site01` and `Site02` hold the gate and pillar collections
  (`GC_Gate_C_SGRes`, `GC_mid_Pillar`, `GC_Pillar_M_03_BlockA`) with their
  own caches.
- **Cinematics.** `SEQ_Robot_Intro`, `SEQ_Robot_Death`, and
  `LSeq_Pose_Anim`; the battle map binds them as `OpeningSequence` and
  `EndingSequence`.
- **Feedback.** Camera shakes (`camShake_Impact_01` to `_05`,
  `Shake_LaserCharge`, `Shake_LaserBlast`) and force feedback effects
  (`FF_LaserCharge`, `FF_LightDartThrow`, `FF_TargetableHit`).
- **Music.** `BP_MusicEventSubscriberComponent` and
  `BPI_MusicEventSubscriptionService` drive `Underscore` from the fight's
  state.

### Destruction and caches

- `Maps/Destruction/` holds 50 geometry collections (1.60 GB), 35 source
  meshes (4.9 million triangles, 6 with Nanite), and 9 Chaos cache
  collections (0.31 GB), the largest `CCC_Mound_Back_Pass1_02` (92 MB) and
  `cache_Fall_RobotHead_C` (82 MB). The battle map plays the caches
  (`ECacheMode::Play`) through a `ChaosCacheManager`, so the collapse is a
  recording, not a live simulation.
- `Destruction/Ruins/`: fracture sources for `Gate_C` (557,823 triangles),
  `GateChunk_C` (223 meshes), `Pillar_M_01_Base`, `Pillar_M_01_Shaft`
  (527,869), `Pillar_M_02`, and `Pillar_M_03_BlockA`, without Nanite, and
  2 geometry collections.
- `Destruction/SandStone/`: Nanite boulders, large boulders, and slabs
  (`Boulder01` to `05`, `LBoulder01` to `05`, `Slab01` to `04`).
- `Destruction/SpireRocks/` (`SpireRock_C`, `_D`), `IcelandicRock/`, and
  `HeadStaticMeshes/` (460 `frac_head_bb_piece` meshes, 264,804 triangles
  in all).
- Field systems: `BP_genericForceField`, `BP_genericForceField_Projectile`,
  `BP_kinField`, and `BP_triggeredForce` (`FieldSystemActor`), over 94
  field node assets grouped as `Anchor`, `Decay`, `Disable`, `Generic`,
  `Kill`, `Motions`, `Sleep`, and `Strain`. `Cutters/Rock` holds a
  sandstone cutter material.
- `Destruction/Systems/`: `BPFX_Sandstone_01` and `BPFX_Darkstone_01`, with
  breaking, collision, and trailing dust Niagara systems for each stone.
- `Destruction/Textures/FRACTURES/`: 8K interior textures for broken rock
  and rough concrete (1.34 GB).

### Effects

`FX/` holds 30 Niagara systems: the laser (`NS_Robot_Laser`,
`NS_Robot_Laser_Hand_Charge`, `NS_Robot_Laser_Blast_Hit`), the light dart
(`NS_LightdartCore2`, `NS_LightdartEdge2`, `NS_LightdartHandCharge`,
`NS_LightDartCoreChargeTrail`), hits (`NS_HotSpotImpact`,
`NS_WeakPointImpact`, `NS_DestructibleImpact`, `NS_GroundFX`), the head
(`NS_Robot_Head_Explode`, `NS_Robot_Head_Impact`), the chest and shoulder,
rise and fall dust (`NS_robot_emission`, `NS_robot_falling`,
`NS_robot_stomach_hit_ground_pound`), pillar dust, and two `NiagaraFluids`
gas simulations for the boss's core (`Grid3D_Gas_Robot_Head_Core`,
`Grid3D_Gas_Robot_Head_Core_Gameplay`).

### Echo's combat

- `Blueprints/Abilities/`: `GA_LightDart` and `GA_Dodge` (parent
  `GameplayAbility`), `GA_ForceWalk`, the effects `GE_LightDartDamage`,
  `GE_Levitate`, and `GE_ForceWalk`, and `BP_LightDartTargetingActor`
  (parent `AncientGameAbilityTargetActor`).
- `Blueprints/LightDart/`: `BP_LightDartProjectile`,
  `BP_LightDartSmootherComponent`, and the impact effect table
  `DT_LightDartImpactFXInfo`.
- Camera modes `BP_LightDartADSCameraMode` (aim down sights),
  `BP_DodgeCameraMode`, and `BP_TemporaryFramingCameraMode`, all from
  `BP_ThirdPersonCameraMode`.
- UI: `W_LightDart_HUD`, `W_LightDart_Reticle`, and
  `W_LightDart_Reticle_Chevron`.
- Gameplay cues `LightDart.Throw`, `LightDart.Levitating`,
  `LightDart.Impact`, and `LightDart.Charging`, and the animation events
  `LightDart.BeginLevitateTurn` and `LightDart.Release`.
- Her combat clips: see [Animations](#animations).

### Audio

`Audio/` (189 files): 93 sound waves (`Feet`, `Movement` for the golem's
cogs, creaks, gears, motor, screech, and slide, `Rocks`, `Vox`, `Weapon`),
19 MetaSounds such as `sfx_Golem_CuttingBeam_meta` and
`vox_Golem_Roar_nl_meta`, and the `Intro_Sequence` and `Death_Sequence`
sets.

## HoverDrone

`Plugins/GameFeatures/HoverDrone/` (36 files) is a free-flying camera drone
that Echo releases. It's active by default (`BuiltInInitialFeatureState`
`Active`). Its `GameFeatureData` uses `GameFeatureAction_AddAbilities`
(`GA_DeployHoverDrone` on `BP_EchoCharacter`),
`GameFeatureAction_AddInputContextMapping`, and
`GameFeatureAction_AddSpawnedActors`. Its referenced names suggest the last
one spawns `BP_HoverDroneMote_AutoOff` and
`BP_SpecializedCampfireDroneTransitionCamera` in `AncientWorld`.

- C++ module `HoverDrone` (Runtime, prebuilt for Mac): `AHoverDronePawn`
  (from `AModularPawn`), `UHoverDroneMovementComponent` (from
  `UFloatingPawnMovement`), and `UHoverDroneControlsComponent` (from the
  project's `UPlayerControlsComponent`).
- Blueprints: `BP_HoverDronePlayerPawn` (from `AHoverDronePawn`),
  `BP_HoverDroneMote`, `BP_HoverDroneMote_AutoOff`, and
  `BP_DroneTransitionCameraMode`.
- Input: two mapping contexts and `IA_DeployDrone`, `IA_AbandonDrone`,
  `IA_HoverDrone_MoveForward`, `_MoveRight`, `_Turn`, `_LookUp`,
  `_SlideUp_World`, `_Turbo`, and `_LowSpeed`.
- Echo's `Mote_Release` and `Mote_Release_Sitting` clips (75 frames each,
  with montages), activate and deactivate sounds, and the animation events
  `HoverDrone.MoteEquip` and `HoverDrone.MoteRelease`.

It's a camera, not character flight.

## Source

`Source/` (56 files, 0.2 MB) holds one C++ module, `AncientGame` (Runtime),
with game and editor targets. It depends on `GameplayAbilities`,
`GameplayTasks`, `GameplayTags`, `EnhancedInput`, `GameFeatures`,
`ModularGameplay`, and the project's `ModularGameplayActors`. It's small:
the project's gameplay lives in Blueprints, and the C++ supplies the bases
they build on. By folder:

| Folder | Classes | What they do |
| --- | --- | --- |
| `Framework/` | `AAncientGameModeBase` (from `AModularGameModeBase`), `UAncientGameInstance`, `ULoadingUtilLibrary`, `UCurveUtilLibrary` | The game mode exposes `PrefetchDarkWorld`, `BeginDarkWorldTransition`, and `ToggleGameFeaturePlugin`: the Dark World swap and the switch that loads AncientBattle. The loading library sets streaming priority (default, streaming, highest, custom), flushes level streaming, and forces garbage collection around that swap. The curve library evaluates runtime curves and cubic interpolation for Blueprints. |
| `Character/` | `AAncientGamePlayerController` (from `AModularPlayerController`), `UMovementAttributeSet`, `UAncientGameBoxComponentBase` | The attribute set holds one attribute, `MoveSpeed`, which `GE_Walk` and `GE_ForceWalk` change. The box component is the base of `BP_VaultingTriggerComponent`. |
| `Input/` | `UPlayerControlsComponent` (from `UPawnComponent`) | Adds and removes a pawn's Enhanced Input mapping context and bindings when the pawn restarts or its controller changes. The ability input binder and the drone controls derive from it. |
| `AbilitySystem/` | `UAncientGameAbilitySystemComponent`, `UAncientGameAbilityAttributeSet`, `UAbilityInputBindingComponent`, `AAncientGameAbilityTargetActor` | The ability system component grants default abilities and default attribute sets and can grant an ability by type. The binder maps input actions to granted abilities. The target actor gives Blueprints hooks to start targeting, allow confirmation, and build target data; the light dart's targeting actor derives from it. |
| `Animation/` | `UAncientGameAnimInstance` | Mirrors gameplay tags from the ability system into animation variables through a tag-to-property map. `Echo_AnimBP` and `Echo_LightDart_AnimBPLayer` derive from it. |
| `Camera/` | `UAncientGameCameraComponent`, `UAncientGameCameraMode`, the camera mode stack and view, `USpringArmCameraMode`, `USpringArmBlueprintLibrary`, interpolators | A stack of camera modes that blend by weight with selectable blend functions, a spring-arm mode with critically damped spring and acceleration interpolators for position and rotation, and a pivot, location, rotation, and control rotation view. Every Blueprint camera mode derives from `USpringArmCameraMode`. |
| `GameFeatures/` | `UGameFeatureAction_WorldActionBase`; actions `AddAbilities`, `AddInputContextMapping`, `AddLevelInstances`, `AddSpawnedActors`, and `AddWorldSystem`; `UGameFeatureWorldSystem` and `UGameFeatureWorldSystemManager` (a world subsystem) | Game feature actions that apply per world: grant abilities and attribute sets to an actor class, add input mappings, stream level instances into a world, spawn actors, and run a world system object for the feature's lifetime. AncientBattle and HoverDrone are built from them. |

What the source does and doesn't implement:

- **Traversal.** No C++ traversal code. The vault is `GA_Vault`,
  `BP_VaultingTriggerComponent`, `BP_VaultingUtilLib`, and motion warping,
  all in Blueprints; C++ supplies the trigger base, the ability system, and
  the `MoveSpeed` attribute.
- **Abilities.** The ability system component, input binding, targeting
  base, and attribute sets. Each ability is a Blueprint.
- **The boss fight.** None in C++. AncientBattle uses the game feature
  actions, the world system manager, and the targeting base.
- **Dark World.** The game mode's prefetch, transition, and feature toggle,
  with the loading library, over the `Dark_World` data layer and
  `BP_DarkWorldRift`.
- **Camera.** All the camera framework is C++.

`Source/ThirdParty/MapZen/` holds only a license (CC BY 4.0) and an
attribution list for Mapzen terrain tiles, which suggests elevation data
went into the terrain work.

## Small plugins

| Plugin | Modules | Classes and purpose |
| --- | --- | --- |
| `ModularGameplayActors` (19 files) | Runtime | `AModularCharacter`, `AModularPawn`, `AModularPlayerController`, `AModularPlayerState`, `AModularGameMode`, `AModularGameModeBase`, `AModularGameState`, `AModularGameStateBase`, and `AModularAIController`: engine actor bases that register with the game framework component manager, so a game feature can add components to them. |
| `Underscore` (26 files) | Runtime, Editor | Interactive music: `UUnderscoreSubsystem` (a game instance subsystem), `UUnderscoreCue`, `UUnderscoreSection` (with stinger quantization), and `UUnderscoreCueBehavior`, plus asset factories. |
| `Crossfader` (19 files) | Runtime, Editor | Gameplay mix states over Audio Modulation: `UCrossfaderSubsystem`, `UCrossfaderSettings`, and the `UMixStateBank` asset. |
| `Uproar` (39 files) | Runtime, Editor | Sound for Chaos physics, quantized in space and time: `UUproarSubsystem` (a tickable world subsystem), `UUproarChaosListenerComponent` (from `UChaosEventListenerComponent`), `UUproarStaticMeshListenerComponent`, settings assets for Chaos break, Chaos collision, and static mesh hit events, and the `FUproarSoundDefinition` table row, keyed by event type, magnitude, speed, and spatial grid size. |
| `InstanceLevelCollision` (13 files) | Editor | By Marien El Alaoui: builds a collision mesh from a level instance or static mesh actor (`UInstanceLevelCollisionBPLibrary`, `ECollisionMaxSlice`, and the `Widget_ColliderMaker` editor widget). It likely made the MegaAssemblies' `BP_Collider` meshes. |

## Echo's folder

Echo's model, rig, and animation sit in
`Content/AncientContent/Characters/Echo/` (183 files, 1.61 GB). Her combat
clips sit in `Plugins/GameFeatures/AncientBattle/Content/Characters/Echo/`
(26 files). Her blueprint is
`Content/AncientContent/Blueprints/Character/BP_EchoCharacter`, a
`ModularCharacter` from the `ModularGameplayActors` plugin.

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

AncientBattle adds her combat clips, also at 30 frames per second:

| Clip | Frames | Root motion |
| --- | --- | --- |
| `Dodge_To_Idle`, `Dodge_To_Idle_MidAir` (with montages) | 106, 106 | Yes |
| `Dodge_To_Run` | 49 | No |
| `LightDart_Charge_1_5s`, `LightDart_Charge_1_5s_Loop` | 85, 69 | No |
| `LightDart_Charge_Cancel` (with montage) | 22 | No |
| `LightDart_Release` (with montage) | 42 | No |
| Charge and release poses (`_Direction_C`, `_N`, `_E`, `_S`, `_W`; `_Pitch_Up`, `_Center`, `_Down`) | 1 each | No |

The poses feed the blend spaces `LightDart_Charge_Pose_Direction_BS`,
`LightDart_Charge_Pose_Pitch_BS`, and `LightDart_Release_Pose_Pitch_BS`,
which aim her charge and throw. The linked layer `Echo_LightDart_AnimBPLayer`
(parent `AncientGameAnimInstance`) plays them. HoverDrone adds
`Mote_Release` and `Mote_Release_Sitting` (75 frames each).

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
| Run, sprint | No | `Jog` is her fastest gait; there is no sprint clip or state. In the battle, `IA_JogOverride` and `GA_ForceWalk` switch between walk and jog. |
| Jump, fall, land | Yes | `InAir` and `Land` states |
| Vault | Yes | `GA_Vault`, `BP_VaultingTriggerComponent`, motion warping, `VaultOver` with root motion |
| Sit and stand | Yes | `GA_SitStand` |
| Reach out (touch interact) | Yes | `GA_TouchInteract`, `ReachOut_*` |
| Dodge | Yes, in the battle | `GA_Dodge`, root-motion `Dodge_To_Idle` on the ground and in the air, `Dodge_To_Run` |
| Light dart (aimed throw) and levitation while charging | Yes, in the battle | `GA_LightDart`, `GE_Levitate`, pose blend spaces for aim |
| Slope adaptation | Yes | Slope-warping control rig |
| Climb, slide, mantle, traversal beyond the vault | No | |
| Flight | No | The hover drone flies; Echo doesn't |

What this tells us for Verse: Echo's locomotion is narrow and polished.
Directional starts, stops, slope warping, and corrective poses make a small
clip set read well, which matters more than the clip count. Her combat set
is small too: two dodges and one aimed throw built from single-frame poses
in blend spaces.

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
| AncientBattle `Destruction/Ruins`, the arena zones, and the Chaos caches | Fracture patterns and piece counts for columns, gates, and slabs; how zones stage a collapse | Collapsing pillar and arch in `carve.rs` |
| Field node groups (`Anchor`, `Strain`, `Decay`, `Sleep`) | Which controls a scripted collapse needs | Demolition support graph |
| `Uproar` | Mapping break and collision events to sound by magnitude and speed | Demolition audio |
| Ground assemblies, `ErosionGround`, `GroundTiles` | How scans meet the ground without a landscape | 1, debris sets |
| Echo's folder and her combat clips | Locomotion set, starts and stops, slope warping, aimed poses, silhouette | [Female character](female-character.md) |
| `Source/Camera/` and the camera mode Blueprints | Blended camera modes for vaults, catwalks, aim, and dodge | Verse follow camera |

## Finish the inventory in the editor

A mesh-level inventory needs Unreal Editor 5.7, which isn't installed (the
launcher records no engine, and `~/work/UnrealEngine` is an unbuilt 5.8.3
source tree). The header read above can't give Nanite source triangle
counts, material graphs, Blueprint graphs, placement transforms, or World
Partition settings beyond the serialized defaults. An agent finishes it
like this:

1. Install the engine: in the Epic Games Launcher, go to **Unreal Engine >
   Library**, and next to **Engine Versions**, click **+** and choose 5.7.
2. Check the project: `~/Documents/Unreal Projects/ValleyoftheAncient/ValleyoftheAncient.uproject`
   exists, and the tree has 19,784 files. This holds on October 5, 2026.
3. Open the project and let shaders compile. Keep it open read-only in
   spirit: don't save, so the packages keep their recorded versions.
4. Export the registry with the editor's Python console (**Tools >
   Execute Python Script**). For each asset under `/Game` and the two game
   feature mounts (`/AncientBattle`, `/HoverDrone`), record the path,
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
   bone and body counts. Open `BP_AncientOne` and record its behavior
   states and the order of the arena zones. Record names and numbers only.
8. Take screenshots for the study note as reference only. They never enter
   a pack.

Don't use any export command (**Asset Actions > Export**, FBX, glTF, USD, or
image export), and don't open Valley content in Blender. The
[licensing rules](ue5-ruins-study.md#licensing) list what's allowed.
