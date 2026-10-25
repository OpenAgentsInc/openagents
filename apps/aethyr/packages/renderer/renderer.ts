import { LocalLightSelection } from "./local-light-selection.ts";
import {
  MMORPG_SUN_DIRECTION,
  MMORPG_SUN_COLOR,
  MMORPG_LIGHT_INTENSITY,
  mmorpgHemisphereSH,
} from "./mmorpg-lighting.ts";
import { planarLightingChanged } from "./planar-lighting-state.ts";
import {
  FULL_RENDERER_DETAIL,
  validateRendererDetail,
  type RendererDetail,
} from "./detail-settings.ts";
import type { Grass, GrassManifest } from "./grass.ts";
import { grassGroundMarker } from "./grass-ground.ts";
import {
  ImpactEventPool,
  DEFAULT_METEOR_IMPACT_RECIPE,
  validateImpactRecipe,
} from "./impact-recipe.ts";
import { sampleWorldWind } from "./shared-wind.ts";
import {
  validateSceneEffects,
  type SceneEffectsSettings,
} from "../runtime/scene-effects.ts";
import { createTightClusteredLightBuilder } from "./clustered-lights.ts";
import {
  stockRasterPipelineKey,
  deriveStockRasterPolicy,
  deriveStockDepthPolicy,
  stockRasterMaterialFlags,
  STOCK_MATERIAL_FEATURE_MASK,
  STOCK_SURFACE_SKY_FLAG,
} from "./stock-raster-policy.ts";
import {
  splitRasterOrientationDraws,
  type RasterOrientation,
} from "./raster-orientation.ts";
import { MAX_CAPTURE_BYTES } from "./film-frame.ts";
import { readAsset } from "../assets/src/asset-download.ts";
import { FrameCompletions } from "./frame-completions.ts";
import { FrameGpuTimer } from "./frame-gpu-timer.ts";
import {
  SourceRenderBundles,
  MAX_BUNDLED_SOURCE_DRAWS,
  type BundledSourceDraw,
} from "./source-render-bundles.ts";
import {
  executeSunShadowSourceBundle,
  isSunShadowBundleView,
  sunShadowPartitionIncludes,
  type SunShadowPartition,
} from "./sun-shadow-source-bundles.ts";
import { stockMaterialConstants } from "./material-specialization.ts";
import { createResidentSourcePreparation } from "./resident-source.ts";
import { RuntimePropTable, type PropPlan } from "../runtime/prop-table.ts";
import type {
  EnemyCharacters,
  EnemyCaptureReference,
} from "./enemy-characters.ts";
import {
  CaptureActorReferences,
  sameCaptureActorReference,
  type ActorCaptureReferenceMetadata,
} from "../runtime/capture-actors.ts";
import type { EnemyActorPose } from "../runtime/enemies.ts";
import type {
  MmorpgCharacters,
  MmorpgActorPose,
  MmorpgClass,
} from "./mmorpg-characters.ts";
import { validateMmorpgActors } from "../assets/src/mmorpg.ts";
import type { MmorpgWorldManifest } from "../assets/src/mmorpg.ts";
import type { MmorpgEffects } from "./mmorpg-effects.ts";
import type { MmorpgNames } from "./mmorpg-names.ts";
import type { MmoEvent } from "../runtime/mmorpg-protocol.ts";
import type { MmorpgTerrain, MmorpgTerrainMetadata } from "./mmorpg-terrain.ts";
import {
  mmorpgSceneryCenter,
  mmorpgSceneryPlacements,
  mmorpgSceneryInstances,
  mmorpgSceneryMetadata,
  mmorpgSceneBatches,
} from "./mmorpg-scenery.ts";
import type {
  CharacterRenderer,
  CharacterFrame,
} from "./character-renderer.ts";
import type { CharacterManifest, CharacterRenderPose } from "./character.ts";
import {
  GameplayEffects,
  gameplayLights,
  gameplayLightRevision,
  maskDestroyedPlacements,
  validateGameplayVisualState,
  type GameplayObject,
  type GameplayVisualState,
} from "./gameplay-effects.ts";
import { DebrisGpuPacking } from "./debris-gpu-packing.ts";
import { SourceDebris, type GameplayTargetGroups } from "./source-debris.ts";
import type { GameplayAimMarker } from "./spell-targeting.ts";
import type { FractureSource } from "./source-fracture.ts";
import { selectedRendererFeatures } from "./features.ts";
import type {
  RendererExperiment,
  RendererGpuTimer,
  RendererResidentFrame,
  RendererOptions,
  RendererFeatures,
} from "./feature-contract.ts";
export type { RendererOptions } from "./feature-contract.ts";
import {
  prepareFilmTransforms,
  validateFilmTransforms,
  type FilmSourceTransform,
} from "./film-transforms.ts";
import {
  savedMaterialDocuments,
  type MaterialDocument,
  type MaterialBundle,
} from "../runtime/material-source.ts";
import type { MaterialEditor, MaterialEditorPlan } from "./material-editor.ts";
import { SurfaceSky, type SurfaceSkyManifest } from "./surface-sky.ts";
import { surfaceSkyDraws } from "./surface-sky-draws.ts";
import {
  SCENE_LINEAR_SCALE,
  OWNED_EXPOSURE,
  GENERATED_SKY_RADIANCE_SCALE,
} from "./radiometry.ts";
import {
  SOURCE_FOG_INSCATTERING_LINEAR,
  SOURCE_FOG_DENSITY_AT_REFERENCE,
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_START_DISTANCE,
  type FogVolumeMedium,
  type FogVolumeRange,
} from "./fog.ts";
import {
  LocalFog,
  sourceLocalFogProfile,
  type LocalFogProfile,
} from "./local-fog.ts";
import {
  FogAmbientVolume,
  sourceFogAmbientSkyProfile,
  type FogAmbientSkyProfile,
} from "./fog-ambient-volume.ts";
import {
  prepareSourceWind,
  sourceWindFlutterMaterial,
  validateSourceWindBuffers,
  usesOwnedWindVertex,
  usesOwnedWindShadow,
  type SourceWindManifest,
  type PreparedSourceWind,
  type WindDraw,
} from "./wind-source.ts";
import { boundedFlutterTime } from "./wind-flutter.ts";
import {
  WindShadowPlanner,
  WIND_SHADOW_LIMITS,
  type WindShadowPlan,
} from "./wind-shadow-plan.ts";
import { decodeRgba8 } from "../assets/src/rgba.ts";
import {
  frustumVisibility,
  compactInstanceDraws,
  type InstanceBounds,
} from "./culling.ts";
import {
  collectVegetationTextures,
  packVegetation,
  VEGETATION_FLOAT_COUNT,
  VEGETATION_SURFACE_SKY_MARKER,
} from "./vegetation.ts";
import {
  LOCAL_POINT_SHADOWS,
  LOCAL_SPOT_SHADOWS,
  SPOT_SHADOW_START,
  LOCAL_SHADOW_VIEWS,
  FRAME_OFFSETS,
  FRAME_FLOAT_COUNT,
} from "./frame.ts";
import type {
  CompiledScene,
  CompiledMaterial,
  CatalogAsset,
  SceneLook,
  SourceLight,
  CompiledEnvironments,
  EnvironmentMap,
} from "../assets/src/scene.ts";
import type { EnvironmentSettings } from "../runtime/environment.ts";
import { buildRendererShaders, POST } from "./shaders.ts";
import { lightIntensity, lightColor } from "./lighting.ts";
import {
  collectLayerTextures,
  packLayers,
  LAYER_FLOAT_COUNT,
} from "./layers.ts";
import { MaterialTexturePools } from "./texture-pool.ts";
import {
  AmbientParticles,
  type AmbientParticleManifest,
} from "./ambient-particles.ts";
import {
  SourceWaterSplashes,
  type SourceWaterSplashesManifest,
} from "./source-water-splashes.ts";
import {
  Creatures,
  type CreaturesManifest,
  type CreatureFrame,
} from "./creatures.ts";
import { Leaves, type LeavesManifest } from "./leaves.ts";
import {
  SourceFogCards,
  type SourceFogCardsManifest,
} from "./source-fog-cards.ts";
import { ContactOcclusion } from "./occlusion.ts";
import { IblTextures, type IblManifest } from "./ibl.ts";
import { ProbeProfiles, type GiProbeProfiles } from "./probe-profiles.ts";
import { BounceTextures, type GiBounceManifests } from "./bounce-textures.ts";
import { probeSettings } from "./probe-textures.ts";
import { ClusteredLighting } from "./clustered-lighting.ts";
import { DisplaySharpen, sourceSharpenStrength } from "./display-sharpen.ts";
import { Bloom } from "./bloom.ts";
import { DisplayLut, displayLutParams } from "./display-lut.ts";
import { normalMapFlags } from "./normals.ts";
import { validateSourceNormalFrames } from "./source-normal-profile.ts";
import { fitDirectionalShadow, SUN_SHADOW_FITS } from "./distant-shadow.ts";
import {
  loadFogAmbientProfiles,
  fogAmbientRadiance,
  type PreparedFogAmbient,
} from "./fog-ambient.ts";
import { cameraVignetteIntensity } from "./vignette.ts";
import type { SourceVignetteOverride } from "../assets/src/scene.ts";
import type { SourceDisplayProfiles } from "./source-display.ts";
import {
  SourceWaterStreams,
  type SourceWaterStreamsManifest,
} from "./source-water-streams.ts";
import {
  FILMIC_FLOAT_COUNT,
  prepareFilmic,
  sourceFilmicControls,
} from "./filmic.ts";
import {
  SurfaceDecalPass,
  type SurfaceDecalManifest,
} from "./surface-decals.ts";
import {
  WaterPass,
  PLANAR_WATER_DISTORTION,
  PLANAR_WATER_TOLERANCE,
  buildWaterBasins,
  type WaterManifest,
} from "./water-pass.ts";
import {
  PlanarReflection,
  type PlanarReflectionResult,
} from "./planar-reflection.ts";
import { collectPaintTextures, packPaint, PAINT_FLOAT_COUNT } from "./paint.ts";
import {
  collectTerrainTextures,
  packTerrain,
  TERRAIN_FLOAT_COUNT,
} from "./terrain.ts";
import { collectGlassTextures, packGlass, GLASS_FLOAT_COUNT } from "./glass.ts";
import { sourceDraws, emissiveAnchors } from "./source.ts";
import {
  add,
  sub,
  scale,
  normalize,
  dot,
  direction,
  lookAt,
  perspective,
  multiply,
  inverse,
  modelMatrix,
  type V3,
} from "./math.ts";

export interface Camera {
  eye: V3;
  yaw: number;
  pitch: number;
  fov?: number;
  near?: number;
  far?: number;
  up?: V3;
  vignette?: SourceVignetteOverride;
  /** Trusted spectator visibility; ordinary player views retain source fog at scale 1. */
  sourceFogScale?: number;
}
export interface RenderedPixelMetadata {
  /** Present only for an explicit capture-only pose reference; live simulation is unchanged. */
  actors?: ActorCaptureReferenceMetadata;
  frameBefore: number;
  frameAfter: number;
  generation: number;
  width: number;
  height: number;
  camera: {
    eye: V3;
    target: V3;
    up: V3;
    fov: number;
    vignette?: SourceVignetteOverride | null;
  };
  time: number;
  exposure: number;
}
type Draw = WindDraw & {
  rasterOrientation?: RasterOrientation;
  sourceDrawIndex?: number;
  originalFirstIndex?: number;
};
function materialTextures(material: CompiledMaterial) {
  return [
    ...collectLayerTextures(material),
    ...collectGlassTextures(material),
    ...collectPaintTextures(material),
    ...collectTerrainTextures(material),
    ...collectVegetationTextures(material),
  ];
}
const SHADER_PIPELINE_FIELDS = [
  "pipeline",
  "blendPipeline",
  "additivePipeline",
  "planarPipeline",
  "planarBlendPipeline",
  "planarAdditivePipeline",
  "shadowPipeline",
  "depthPipeline",
] as const;
type ShaderPipelineFamily = {
  pipelines: GPURenderPipeline[];
  opaqueDepth?: Map<string, GPURenderPipeline>;
};
const IDENTITY_MATERIALS: MaterialEditorPlan = {
  ready: () => true,
  remap: (_entity, material) => material,
  activate() {},
  discard() {},
};
type SourceBatchPlan = import("./compiled-source-batches.ts").CompiledSourceBatchPlan<Draw>;
type SourceBatchGatherer = import("./compiled-source-batches.ts").CompiledBatchGatherer<Draw>;
type SourceSubmission = {
  compiled?: SourceBatchGatherer;
  sourceTable?: readonly Draw[];
  instances: GPUBuffer;
  wind: GPUBuffer;
  draws: Draw[];
  dynamicOnly?: boolean;
  shadowPartition?: SunShadowPartition;
  debris?: "all" | "standing" | "moving" | "none";
  matrix?: Float32Array;
  bundles?: SourceRenderBundles;
  bundleDraws?: Draw[];
  drawPool?: {
    base: Draw[];
    version: number;
    records: Draw[];
    visible: Draw[];
  };
};
const VERTICES: GPUVertexBufferLayout[] = [
  {
    arrayStride: 48,
    attributes: [
      { shaderLocation: 0, offset: 0, format: "float32x3" },
      { shaderLocation: 1, offset: 12, format: "float32x3" },
      { shaderLocation: 2, offset: 24, format: "float32x2" },
      { shaderLocation: 3, offset: 32, format: "float32x4" },
    ],
  },
  {
    arrayStride: 64,
    stepMode: "instance",
    attributes: [0, 1, 2, 3].map((i) => ({
      shaderLocation: 4 + i,
      offset: i * 16,
      format: "float32x4" as const,
    })),
  },
  {
    arrayStride: 4,
    attributes: [{ shaderLocation: 8, offset: 0, format: "unorm8x4" }],
  },
  {
    arrayStride: 8,
    attributes: [{ shaderLocation: 9, offset: 0, format: "float32x2" }],
  },
  {
    arrayStride: 16,
    stepMode: "instance",
    attributes: [{ shaderLocation: 10, offset: 0, format: "float32x4" }],
  },
];

export class Renderer {
  static readonly features = Object.freeze({
    character: !!selectedRendererFeatures.character,
    sky: !!selectedRendererFeatures.sky,
  });
  private features: RendererFeatures = selectedRendererFeatures;
  private shaders = buildRendererShaders(this.features);
  private experiment: RendererExperiment = {
    backend: "baseline",
    profiling: false,
  };
  private frameScenes = new Map<string, RendererResidentFrame>();
  private residentSource = createResidentSourcePreparation();
  private gpuTimer?: RendererGpuTimer;
  private adapterInfo: Record<string, unknown> = {};
  private profiling = false;
  private stockModule?: GPUShaderModule;
  private stockPipelines = new Map<string, GPURenderPipeline>();
  private stockChoiceRevision = 0;
  private stockChoices = new WeakMap<
    GPUBindGroup,
    {
      revision: number;
      features: number;
      pipelines: (GPURenderPipeline | undefined)[];
    }
  >();
  private stockPipelineTasks = new Map<string, Promise<void>>();
  private materialFeatures = new WeakMap<GPUBindGroup, number>();
  private readonly frameCompletions = new FrameCompletions();
  startFrameRecording(capacity = 20_000) {
    this.frameCompletions.start(capacity);
  }
  stopFrameRecording() {
    this.frameCompletions.stop();
  }
  completedFrameSnapshot() {
    return this.frameCompletions.snapshot();
  }
  async drain() {
    await this.device.queue.onSubmittedWorkDone();
  }
  gpuFrameSample(frame: number) {
    return this.gpuTimer?.sampleFrame?.(frame) ?? null;
  }
  private preparedBufferUploadBytes = 0;
  private materialBaseUploadBytes = 0;
  private resourceInitializeMs = 0;
  private initialQueueDrainMs = 0;
  commandEncodeMs: number | null = null;
  cullCpuMs = 0;
  clusterCpuMs = 0;
  setBenchmarkEnabled(enabled: boolean) {
    this.profiling = enabled && this.experiment.profiling;
    this.clusteredLighting?.setObserving(this.profiling);
    if (this.profiling) this.gpuTimer?.clear();
  }
  gpuSample(frame: number) {
    return this.gpuTimer?.sample(frame) ?? null;
  }
  benchmarkInfo() {
    const timingStatus = this.gpuTimer?.status?.();
    const sourceSubmission = {
      enabled: this.sourceBundlesEnabled,
      path: this.sourceBundleExecutions ? "render-bundles-indirect" : "direct",
      views: 0,
      indirectBytes: 0,
      nativeBundleBytes: null,
      instanceBytes: 0,
      windBytes: 0,
      bundles: 0,
      builds: 0,
      reuses: 0,
      uploads: 0,
      uploadBytes: 0,
      executions: this.sourceBundleExecutions,
      tableDraws: 0,
      admittedDraws: 0,
      recordedCommands: 0,
      compiledViews: 0,
      compiledSourceDraws: 0,
      compiledTableDraws: 0,
      compiledMembershipBytes: 0,
      compiledGatherBytes: 0,
      cpuMembershipBytes: 0,
    };
    const compiledPlans = new Set<SourceBatchPlan>();
    for (const source of this.culledSources.values()) {
      if (source.compiled) {
        const plan = source.compiled.plan;
        sourceSubmission.compiledViews++;
        sourceSubmission.compiledSourceDraws += plan.counts.sourceDraws;
        sourceSubmission.compiledTableDraws += plan.counts.groupedDraws;
        if (!compiledPlans.has(plan)) {
          sourceSubmission.compiledMembershipBytes += plan.footprint.planBytes;
          compiledPlans.add(plan);
        }
        sourceSubmission.compiledGatherBytes += source.compiled.memoryBytes;
      }
      if (source.bundles) {
        const stats = source.bundles.snapshot();
        sourceSubmission.views++;
        sourceSubmission.indirectBytes += stats.bytes;
        sourceSubmission.bundles += stats.bundles;
        sourceSubmission.instanceBytes += source.instances.size;
        sourceSubmission.windBytes += source.wind.size;
        sourceSubmission.builds += stats.builds;
        sourceSubmission.reuses += stats.reuses;
        sourceSubmission.uploads += stats.uploads;
        sourceSubmission.uploadBytes += stats.uploadBytes;
        sourceSubmission.tableDraws += stats.draws;
        sourceSubmission.admittedDraws += stats.admittedDraws;
        sourceSubmission.cpuMembershipBytes += stats.cpuMembershipBytes;
        sourceSubmission.recordedCommands += stats.slots.reduce(
          (sum, slot) => sum + slot.commands.draws,
          0,
        );
      }
    }
    return {
      spellSwarm: this.spellSwarmInfo(),
      battle: this.battleInfo(),
      sourceSubmission,
      renderCommands: {
        scope: "last-accepted-frame-source-draw-loop-commands",
        ...this.submittedSourceCommands,
      },
      preparedSurfaceBrdf: this.features?.preparedSurfaceBrdf === true,
      compiledSourceBatches: this.experiment.compiledSourceBatches === true,
      backend: this.experiment.backend,
      renderPreparation:
        this.experiment.framePreparation?.implementation ??
        this.residentSource.implementation,
      width: this.width,
      height: this.height,
      sourceInstances: this.scene.instances.length,
      sourceTriangles: this.triangles,
      frameWasmBytes:
        this.experiment.framePreparation?.implementation === "c"
          ? this.experiment.framePreparation.memoryBytes
          : 0,
      residentFrameBytes:
        this.experiment.framePreparation?.memoryBytes ??
        this.residentSource.memoryBytes,
      gpuAccountedBytes: this.gpuBytes,
      preparedBufferUploadBytes: this.preparedBufferUploadBytes,
      materialBaseUploadBytes: this.materialBaseUploadBytes,
      resourceInitializeMs: this.resourceInitializeMs,
      initialQueueDrainMs: this.initialQueueDrainMs,
      isolatedUploadMs: null,
      layeredUploadBytes: this.layerPools?.uploadBytes ?? 0,
      layeredDecodedBytes: this.layerPools?.decodedBytes ?? 0,
      inFlight: this.inFlight,
      gpuScope: this.gpuTimer ? "main-HDR-pass-only" : "unavailable",
      fullGpuScope:
        timingStatus?.availability === "available"
          ? timingStatus.scope
          : "unavailable",
      gpuTimingStatus: timingStatus ?? null,
      clusteredLights: this.clusteredLighting?.stats("main") ?? null,
      adapter: this.adapterInfo,
    };
  }
  /** Immutable CPU experiment input; helper results never become live render masks. */
  cullProbeSnapshot(camera: Camera, mode: "day" | "night") {
    const bounds = this.variants.get(mode)!.bounds;
    if (bounds.count > 100_000)
      throw new Error(
        "Choose a scene with up to 100,000 source instances for this experiment.",
      );
    const forward = direction(camera.yaw, camera.pitch);
    return {
      generation: this.geometryVersion,
      bounds: {
        count: bounds.count,
        spheres: new Float64Array(bounds.spheres),
        aabbs: new Float64Array(bounds.aabbs),
      },
      matrix: new Float32Array(
        multiply(
          perspective(
            camera.fov ?? 1,
            this.width / this.height,
            camera.near ?? 0.1,
            camera.far,
          ),
          lookAt(camera.eye, add(camera.eye, forward), camera.up ?? [0, 1, 0]),
        ),
      ),
    };
  }
  device!: GPUDevice;
  context!: GPUCanvasContext;
  format!: GPUTextureFormat;
  scene!: CompiledScene;
  canvas!: OffscreenCanvas;
  frameBuffer!: GPUBuffer;
  uniform = new Float32Array(FRAME_FLOAT_COUNT);
  private environmentData?: CompiledEnvironments;
  private lightingEnvironmentData?: CompiledEnvironments;
  private fogAmbientProfiles: Partial<
    Record<"day" | "night", PreparedFogAmbient>
  > = {};
  private iblData?: IblManifest;
  private ibl?: IblTextures;
  private probeData?: GiProbeProfiles;
  private probes?: ProbeProfiles;
  private bounceData?: GiBounceManifests;
  private bounce?: BounceTextures;
  private clusteredLighting?: ClusteredLighting;
  private displaySharpen?: DisplaySharpen;
  private localFog?: LocalFog;
  private fogAmbientVolume?: FogAmbientVolume;
  private fogAmbientInputs = new Map<
    "day" | "night",
    {
      profile: FogAmbientSkyProfile;
      moments: GPUTextureView;
    }
  >();
  private bloom?: Bloom;
  private particles?: AmbientParticles;
  private particleData?: AmbientParticleManifest;
  private sourceWaterStreamData?: SourceWaterStreamsManifest;
  private sourceWaterStreams?: SourceWaterStreams;
  private sourceWaterSplashData?: SourceWaterSplashesManifest;
  private sourceWaterSplashes?: SourceWaterSplashes;
  private creatureData?: CreaturesManifest;
  private creatures?: Creatures;
  private characterData?: CharacterManifest;
  private character?: CharacterRenderer;
  private characterPose?: CharacterRenderPose;
  private mmorpgCharacters?: MmorpgCharacters;
  private mmorpgWorld?: MmorpgWorldManifest;
  private mmorpgEffects?: MmorpgEffects;
  private mmorpgNames?: MmorpgNames;
  private mmorpgSceneryCell?: string;
  private mmorpgTerrain?: MmorpgTerrain;
  private mmorpgActors: readonly MmorpgActorPose[] = [];
  private mmorpgLocalId?: number;
  private mmorpgInteractionPose?: CharacterRenderPose;
  private grassData?: GrassManifest;
  private grass?: Grass;
  private liveEffects?: SceneEffectsSettings;
  private previewImpacts?: ImpactEventPool;
  private previewImpactId = 0;
  get sceneEffectsSettings() {
    return this.liveEffects;
  }
  get effectPreviewStats() {
    return this.previewImpacts
      ? {
          ...this.previewImpacts.stats,
          effectTriangles: this.spellEffects?.triangles ?? 0,
        }
      : undefined;
  }
  /** Parameter edits reuse active buffers and preserve world time and particle identities. */
  setSceneEffects(candidate: unknown) {
    const next = validateSceneEffects(candidate);
    const pool = this.previewImpacts ?? new ImpactEventPool();
    const recipe = { ...DEFAULT_METEOR_IMPACT_RECIPE, ...next.impact };
    // Full validation precedes every mutation; none of these setters allocate GPU resources.
    validateImpactRecipe(recipe);
    this.grass?.setAppearance(next.grass);
    const result = pool.replaceRecipe(recipe);
    if (!result.ok) throw new Error(result.error);
    this.previewImpacts = pool;
    this.liveEffects = next;
    return next;
  }
  previewImpact(position: V3, time: number) {
    if (!this.previewImpacts)
      throw new Error("Open the effects controls first.");
    const id = this.previewImpactId + 1;
    const result = this.previewImpacts.emit(
      {
        id,
        seed: (id * 7919) >>> 0,
        position,
        normal: [0, 1, 0],
        intensity: 1,
        radius: 1.5,
        startTime: time,
      },
      time,
    );
    if (!result.ok) throw new Error(result.error);
    this.previewImpactId = id;
  }
  get groundGrassStats() {
    return this.grass ? { ...this.grass.stats } : undefined;
  }
  private detailSettings: RendererDetail = FULL_RENDERER_DETAIL;
  /** Current Detail switches; `renderScale` is applied by the caller's `resize`. */
  get detail(): Readonly<RendererDetail> {
    return this.detailSettings;
  }
  /** Apply Detail's quality switches (ADR 0007). Takes effect next frame. */
  setDetail(value: RendererDetail) {
    const next = Object.freeze({ ...validateRendererDetail(value) });
    const prior = this.detailSettings;
    this.detailSettings = next;
    if (
      next.shadowRefreshMeters < prior.shadowRefreshMeters ||
      next.shadowRefreshDot > prior.shadowRefreshDot
    )
      this.shadowDirty = true;
    this.grass?.setReach(next.foliageReach);
    this.battleCrowd?.setReach(next.crowdReach);
    this.battleCrowd?.setSilhouettePixels(next.crowdSilhouettePixels);
  }
  private enemyData?: CharacterManifest;
  private enemies?: EnemyCharacters;
  private enemyActors: readonly EnemyActorPose[] = [];
  private captureActorRegistry?: CaptureActorReferences<
    EnemyCaptureReference | undefined
  >;
  private captureActorSourceIdentity?: string;
  private captureActorModelIdentity?: string;
  private lastEnemyFrame = 0;
  private activeCaptureActors?: {
    metadata?: ActorCaptureReferenceMetadata;
    payload: EnemyCaptureReference | undefined;
  };
  private leaves?: Leaves;
  private leavesData?: LeavesManifest;
  private fogCards?: SourceFogCards;
  private fogCardsData?: SourceFogCardsManifest;
  private fogCardDepthView?: GPUTextureView;
  private creatureSH = new Float32Array(36);
  private creatureIdentity = new Float32Array([
    1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]);
  private localFogProfiles = new Map<"day" | "night", LocalFogProfile>();
  private filmicProfiles = new Map<"day" | "night", Float32Array>();
  private displayProfileData?: SourceDisplayProfiles;
  private displayLuts = new Map<"day" | "night", DisplayLut>();
  private displayLutFallback!: GPUTexture;
  private displayLutSampler!: GPUSampler;
  private localFogViews!: {
    scene: GPUTextureView;
    depth: GPUTextureView;
    local: GPUTextureView;
    near: GPUTextureView;
    far: GPUTextureView;
    distant: GPUTextureView;
  };
  private surfaceDecalData?: Partial<
    Record<"day" | "night", SurfaceDecalManifest>
  >;
  private surfaceDecals = new Map<string, SurfaceDecalPass>();
  private environmentTextures = new Map<string, GPUTexture>();
  private environmentGroups = new Map<string, GPUBindGroup>();
  private skyDomes = new Map<"day" | "night", GPUBuffer>();
  private planarGroups = new Map<string, GPUBindGroup>();
  hasSourceSky = false;
  look?: SceneLook;
  postBuffer!: GPUBuffer;
  private skyDisplayBuffer?: GPUBuffer;
  vertices!: GPUBuffer;
  vertexColors!: GPUBuffer;
  vertexUV1!: GPUBuffer;
  private roomUV1?: GPUBuffer;
  private waterData?: WaterManifest;
  private water?: WaterPass;
  private waterHDR?: GPUTexture;
  private streamHDR?: GPUTexture;
  private planar?: PlanarReflection;
  private planarFrameBuffer?: GPUBuffer;
  private waterBounds?: InstanceBounds;
  private waterPlaneHeights: number[] = [];
  private planarLightingValues = new Float32Array(556);
  private planarLightingRevision = 0;
  private planarEnvironmentSignature = "";
  private planarEnvironmentRevision = 0;
  private layerPools?: MaterialTexturePools;
  indices!: GPUBuffer;
  instances!: GPUBuffer;
  frameLayout!: GPUBindGroupLayout;
  materialLayout!: GPUBindGroupLayout;
  layout!: GPUPipelineLayout;
  frameGroup!: GPUBindGroup;
  shadowGroup!: GPUBindGroup;
  farShadowGroup!: GPUBindGroup;
  farFrameBuffer!: GPUBuffer;
  private distantShadowGroup!: GPUBindGroup;
  private distantFrameBuffer!: GPUBuffer;
  materials: GPUBindGroup[] = [];
  private renderMaterialInfo: CompiledMaterial[] = [];
  private surfaceSky?: SurfaceSky;
  private surfaceSkyData?: SurfaceSkyManifest;
  pipeline!: GPURenderPipeline;
  blendPipeline!: GPURenderPipeline;
  additivePipeline!: GPURenderPipeline;
  private planarPipeline!: GPURenderPipeline;
  private planarBlendPipeline!: GPURenderPipeline;
  private planarAdditivePipeline!: GPURenderPipeline;
  shadowPipeline!: GPURenderPipeline;
  depthPipeline!: GPURenderPipeline;
  private opaqueDepthPipelines = new Map<string, GPURenderPipeline>();
  private occlusion?: ContactOcclusion;
  private occlusionFallback!: GPUTexture;
  private comparison!: GPUSampler;
  private skySampler!: GPUSampler;
  skyPipeline?: GPURenderPipeline;
  skyGroup!: GPUBindGroup;
  postPipeline!: GPURenderPipeline;
  private displayLayout!: GPUBindGroupLayout;
  private postGroups = new Map<"day" | "night", GPUBindGroup>();
  private skyDisplayGroups = new Map<"day" | "night", GPUBindGroup>();
  depth!: GPUTexture;
  hdr!: GPUTexture;
  msaa!: GPUTexture;
  shadow!: GPUTexture;
  farShadow!: GPUTexture;
  localShadow!: GPUTexture;
  private localGroups: GPUBindGroup[] = [];
  private localBuffers: GPUBuffer[] = [];
  private localViews: (Float32Array | undefined)[] = [];
  private localSlots = new Map<string, number>();
  private localLightSelection = new LocalLightSelection();
  private localSignatures = new Map<string, string>();
  private localDirtyViews = new Set<number>();
  private geometryVersion = 0;
  sampler!: GPUSampler;
  draws: Draw[] = [];
  textures: GPUTexture[] = [];
  buffers = new Set<GPUBuffer>();
  width = 0;
  height = 0;
  frame = 0;
  triangles = 0;
  gpuBytes = 0;
  lost = false;
  private disposed = false;
  private textureCache = new Map<string, Promise<GPUTexture>>();
  private mipPipelines = new Map<GPUTextureFormat, GPURenderPipeline>();
  private lamps: V3[] = [];
  private shadowDirty = true;
  private gameplaySource?: FractureSource;
  private sourceDebris?: SourceDebris;
  private debrisPacking?: DebrisGpuPacking;
  private spellEffects?: GameplayEffects;
  private gameplayAim?: GameplayAimMarker;
  private gameplayEffects = new Map<string, GameplayVisualState["effects"]>();
  private destroyedPlacements = new Map<string, Set<number>>();
  private gameplayDamageKeys = new Map<string, string>();
  private gameplayAnimationRevision = 0;
  private lastShadowEye: V3 = [1e9, 1e9, 1e9];
  private lastShadowForward: V3 = [0, 0, 0];
  private lastEnvironment = "";
  private shadowMatrix: Float32Array = new Float32Array(16);
  private farShadowMatrix: Float32Array = new Float32Array(16);
  private distantShadowMatrix: Float32Array = new Float32Array(16);
  private lastView?: { camera: Camera; env: EnvironmentSettings; time: number };
  private captureBuffer?: GPUBuffer;
  private captureReadback?: { buffer: GPUBuffer; size: number };
  private captureCanvas?: OffscreenCanvas;
  private capturing = false;
  private variants = new Map<
    string,
    {
      scene: CompiledScene;
      instances: GPUBuffer;
      draws: Draw[];
      bounds: InstanceBounds;
      models: Float32Array;
      wind: PreparedSourceWind;
      windBuffer: GPUBuffer;
      windIndices: Uint32Array;
      windBounds: InstanceBounds;
      lamps: V3[];
      triangles: number;
    }
  >();
  private windData?: Partial<Record<"day" | "night", SourceWindManifest>>;
  private hasFlutter = false;
  private zeroWind!: GPUBuffer;
  private windShadowPlanner = new WindShadowPlanner();
  // Fixed physical view slots never resize or retire while this renderer is alive.
  // Retaining unused slots caps allocated + pending-retirement copies at 136 MiB.
  private windShadowCopies = new Map<string, GPUTexture>();
  private windShadowBytes = 0;
  private cullingEnabled = true;
  private windVertexBounded = true;
  private windShadowBounded = true;
  private sourceBundlesEnabled = false;
  private canonicalDraws?: Map<string, Draw[]>;
  private compiledSourceHost?: import("./compiled-source-host.ts").CompiledSourceHost;
  private sourceBundleExecutions = 0;
  private sourceCommands = {
    draws: 0,
    pipelines: 0,
    materials: 0,
    windBindings: 0,
  };
  private submittedSourceCommands = this.sourceCommands;
  sourceCommandCounts() {
    return { ...this.submittedSourceCommands };
  }
  private countSourceCommands(commands: Readonly<typeof this.sourceCommands>) {
    this.sourceCommands.draws += commands.draws;
    this.sourceCommands.pipelines += commands.pipelines;
    this.sourceCommands.materials += commands.materials;
    this.sourceCommands.windBindings += commands.windBindings;
  }
  private culledSources = new Map<
    string,
    SourceSubmission & { signature: string }
  >();
  private desiredShader = 0;
  private activeShaderSource = this.shaders.MATERIAL;
  get materialShaderSource() {
    return this.activeShaderSource;
  }
  workingShaderSource() {
    return this.activeShaderSource;
  }
  private stockPipelineFamily?: ShaderPipelineFamily;
  private shaderQueue = Promise.resolve();
  private inFlight = 0;
  private spellSwarmHost?: import("./spell-swarm-host.ts").SpellSwarmHost;
  spellSwarmInfo() {
    return this.spellSwarmHost?.info() ?? null;
  }
  async probeSpellSwarm() {
    return this.spellSwarmHost?.probe() ?? null;
  }
  async setSpellSwarm(count: 0 | 10000 | 100000 | 200000) {
    if (!this.spellSwarmHost) {
      const create = this.features.createSpellSwarmHost;
      if (!create)
        throw new Error("Spell swarms are available in the Town editor.");
      this.spellSwarmHost = create({
        device: this.device,
        camera: () => this.lastView?.camera,
        closed: () => this.disposed || this.lost,
        account: (delta) => {
          this.gpuBytes += delta;
        },
      });
    }
    return this.spellSwarmHost.setCount(count);
  }
  private battleCrowd?: import("./battle-crowd.ts").BattleCrowd;
  private battleLoadSerial = 0;
  private battleTime?: number;
  battleInfo() { return this.battleCrowd?.inspect() ?? null; }
  async probeBattle() { return this.battleCrowd?.probe() ?? null; }
  battleParticipantContacts() { return this.battleCrowd?.latestParticipantContacts() ?? null; }
  setBattleParticipant(value: import("./battle-crowd.ts").BattleParticipant) {
    this.battleCrowd?.setParticipant(value);
  }
  resetBattleParticipant() { return this.battleCrowd?.resetParticipant() ?? 0; }
  resetBattle() {
    if (!this.battleCrowd) throw Error("Start a battle first.");
    this.battleCrowd.setCount(this.battleCrowd.inspect().count);
    this.battleTime = undefined;
  }
  /** Battle Lab fixed-tick capture: restart from the seed and stop at exactly `tick`. */
  holdBattle(tick: number) {
    if (!this.battleCrowd) throw Error("Start a battle first.");
    this.battleCrowd.hold(tick);
    this.battleTime = undefined;
  }
  releaseBattle() {
    this.battleCrowd?.release();
    this.battleTime = undefined;
  }
  stopBattle() {
    this.battleLoadSerial++;
    if (this.battleCrowd) {
      this.gpuBytes -= this.battleCrowd.bytes;
      this.battleCrowd.dispose();
      this.battleCrowd = undefined;
    }
    this.battleTime = undefined;
  }
  async startBattle(count: number, seed: number,
    terrain: import("./battle-crowd.ts").BattleCrowdTerrain,
    center: readonly [number, number, number]) {
    const create = this.features.createBattleCrowd;
    const load = this.features.loadBattleCrowdAssets;
    if (!create || !load || !this.scene.battleCrowdURL)
      throw Error("Prepare the battle characters before starting Battle Lab.");
    const ticket = ++this.battleLoadSerial;
    const { classes } = await load(this.scene.battleCrowdURL);
    if (ticket !== this.battleLoadSerial || this.disposed || this.lost)
      throw Error("This battle was cancelled before it opened.");
    const candidate = await create(this.device, classes, terrain, { count, capacity: count, seed, center });
    if (ticket !== this.battleLoadSerial || this.disposed || this.lost) {
      candidate.dispose();
      throw Error("This battle was cancelled before it opened.");
    }
    const previous = this.battleCrowd;
    candidate.setReach(this.detailSettings.crowdReach);
    candidate.setSilhouettePixels(this.detailSettings.crowdSilhouettePixels);
    this.battleCrowd = candidate;
    this.battleTime = undefined;
    this.gpuBytes += candidate.bytes - (previous?.bytes ?? 0);
    previous?.dispose();
  }
  private shaderPending = 0;
  private authoredInstances?: GPUBuffer;
  private authoredDraws: Draw[] = [];
  private readonly propTable = new RuntimePropTable({ initialCapacity: 64 });
  private authoredStructure?: string;
  private authoredEntityDraws = new Map<string, Draw[]>();
  private authoredKinds = new Map<string, string>();
  private authoredFullRebuilds = 0;
  private authoredIncrementalUpdates = 0;
  private authoredBytesUploaded = 0;
  private submittedGeometry = { instances: 0, triangles: 0, drawCalls: 0 };
  runtimeTableInfo() {
    const info = this.propTable.snapshot();
    return {
      ...info,
      bytesUploaded: this.authoredBytesUploaded,
      fullRebuilds: this.authoredFullRebuilds,
      incrementalUpdates: this.authoredIncrementalUpdates,
    };
  }
  runtimeInspection() {
    return {
      generation: this.geometryVersion,
      sourceInstances: this.scene.instances.length,
      sourceTriangles: this.triangles,
      authoredObjects: this.authoredKinds.size,
      authoredDraws: this.authoredDraws.length,
      roomSurfaces: this.roomCount,
      frame: this.frame,
      submittedInstances: this.submittedGeometry.instances,
      submittedTriangles: this.submittedGeometry.triangles,
      submittedDrawCalls: this.submittedGeometry.drawCalls,
    };
  }
  authoredHandle(id: string) {
    return this.propTable.handle(id);
  }
  private roomVertices?: GPUBuffer;
  private roomColors?: GPUBuffer;
  private roomIndices?: GPUBuffer;
  private roomInstances?: GPUBuffer;
  private roomMaterial?: GPUBindGroup;
  private roomCount = 0;
  private report: (message: string) => void = () => {};

  static async create(
    canvas: OffscreenCanvas,
    scene: CompiledScene,
    report: (s: string) => void,
    nightScene?: CompiledScene,
    look?: SceneLook,
    environments?: CompiledEnvironments,
    ibl?: IblManifest,
    water?: WaterManifest,
    probes?: GiProbeProfiles,
    surfaceDecals?: Partial<Record<"day" | "night", SurfaceDecalManifest>>,
    lightingEnvironments?: CompiledEnvironments,
    bounceManifests?: GiBounceManifests,
    particles?: AmbientParticleManifest,
    sourceWaterStreams?: SourceWaterStreamsManifest,
    displayProfiles?: SourceDisplayProfiles,
    sourceWaterSplashes?: SourceWaterSplashesManifest,
    creatures?: CreaturesManifest,
    wind?: Partial<Record<"day" | "night", SourceWindManifest>>,
    leaves?: LeavesManifest,
    surfaceSky?: SurfaceSkyManifest,
    fogCards?: SourceFogCardsManifest,
    character?: CharacterManifest,
    options: RendererOptions = {},
  ): Promise<Renderer> {
    const features = options.features ?? selectedRendererFeatures;
    if (
      (features.character && !selectedRendererFeatures.character) ||
      (features.sky && !selectedRendererFeatures.sky)
    )
      throw new Error(
        "This player does not include the requested graphics features.",
      );
    if (
      !features.character &&
      (character ||
        options.enemyCharacter ||
        scene.characterURL ||
        nightScene?.characterURL ||
        scene.enemyCharacterURL ||
        nightScene?.enemyCharacterURL)
    )
      throw new Error("This player does not include character rendering.");
    if (
      !features.sky &&
      (environments ||
        lightingEnvironments ||
        scene.environmentURL ||
        nightScene?.environmentURL)
    )
      throw new Error("This player does not include sky rendering.");
    const r = new Renderer();
    r.features = features;
    r.shaders = buildRendererShaders(features);
    r.activeShaderSource = r.shaders.MATERIAL;
    r.experiment =
      options.experiment ??
      (await selectedRendererFeatures.createExperiment?.()) ??
      r.experiment;
    if (options.diagnostics)
      r.experiment = {
        ...r.experiment,
        profiling: true,
        createGpuTimer:
          r.experiment.createGpuTimer ??
          ((device) => new FrameGpuTimer(device)),
      };
    if (r.experiment.compiledSourceBatches)
      r.compiledSourceHost = r.features.createCompiledSourceHost?.();
    r.canvas = canvas;
    r.scene = scene;
    r.look = look;
    r.environmentData = environments;
    r.lightingEnvironmentData = lightingEnvironments;
    r.iblData = ibl;
    r.waterData = water;
    r.probeData = probes;
    r.bounceData = bounceManifests;
    r.particleData = particles;
    r.sourceWaterStreamData = sourceWaterStreams;
    r.sourceWaterSplashData = sourceWaterSplashes;
    r.creatureData = creatures;
    r.characterData = character;
    r.grassData = options.grass;
    r.enemyData = options.enemyCharacter;
    r.windData = wind;
    r.leavesData = leaves;
    r.surfaceSkyData = surfaceSky;
    r.fogCardsData = fogCards;
    r.displayProfileData = displayProfiles;
    r.surfaceDecalData = surfaceDecals;
    r.report = (message) => {
      if (!r.disposed) report(message);
    };
    if (!navigator.gpu)
      throw new Error(
        "This browser does not offer WebGPU. Open Aethyrforge in current Chrome or Edge.",
      );
    const adapter = await navigator.gpu.requestAdapter({
      powerPreference: "high-performance",
    });
    if (!adapter) throw new Error("A WebGPU graphics device is unavailable.");
    r.adapterInfo = {
      vendor: adapter.info?.vendor ?? "",
      architecture: adapter.info?.architecture ?? "",
      device: adapter.info?.device ?? "",
      description: adapter.info?.description ?? "",
    };
    const timestamps =
      r.experiment.profiling &&
      !!r.experiment.createGpuTimer &&
      adapter.features.has("timestamp-query");
    const indirect = adapter.features.has("indirect-first-instance");
    const requiredFeatures: GPUFeatureName[] = [];
    if (timestamps) requiredFeatures.push("timestamp-query");
    if (indirect) requiredFeatures.push("indirect-first-instance");
    r.device = await adapter.requestDevice({ requiredFeatures });
    r.sourceBundlesEnabled = indirect;
    if (timestamps) r.gpuTimer = r.experiment.createGpuTimer!(r.device);
    r.device.addEventListener("uncapturederror", (e) => {
      if (r.disposed) return;
      r.planar?.invalidate();
      r.lost = true;
      r.report("Graphics: " + e.error.message);
    });
    r.device.lost.then((info) => {
      if (r.disposed) return;
      r.planar?.invalidate();
      r.lost = true;
      r.report("Graphics device stopped: " + info.message);
    });
    try {
      r.context = canvas.getContext("webgpu")!;
      r.format = navigator.gpu.getPreferredCanvasFormat();
      r.context.configure({
        device: r.device,
        format: r.format,
        alphaMode: "opaque",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      const initializeStarted = performance.now();
      await r.initialize(nightScene);
      r.resourceInitializeMs = performance.now() - initializeStarted;
      const drainStarted = performance.now();
      await r.device.queue.onSubmittedWorkDone();
      r.initialQueueDrainMs = performance.now() - drainStarted;
      return r;
    } catch (error) {
      try {
        r.dispose();
      } catch {
        // Preserve the startup failure if a partially created resource also fails cleanup.
      }
      throw error;
    }
  }
  private buffer(
    data: ArrayBuffer | Float32Array,
    usage: GPUBufferUsageFlags,
    label: string,
  ): GPUBuffer {
    const bytes =
      data instanceof ArrayBuffer
        ? new Uint8Array(data)
        : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    const b = this.device.createBuffer({
      size: Math.max(4, Math.ceil(bytes.byteLength / 4) * 4),
      usage: usage | GPUBufferUsage.COPY_DST,
      label,
    });
    this.device.queue.writeBuffer(b, 0, bytes);
    this.preparedBufferUploadBytes += bytes.byteLength;
    this.buffers.add(b);
    this.gpuBytes += b.size;
    return b;
  }
  private releaseBuffer(buffer?: GPUBuffer) {
    if (!buffer) return;
    this.buffers.delete(buffer);
    this.gpuBytes -= buffer.size;
    buffer.destroy();
  }
  private async initialize(nightScene?: CompiledScene) {
    this.debrisPacking = await DebrisGpuPacking.create(this.device, (delta) => {
      this.gpuBytes += delta;
    });
    const d = this.device;
    this.surfaceSky = await SurfaceSky.create(d, this.surfaceSkyData);
    this.gpuBytes += this.surfaceSky.bytes;
    const fogProfiles = await loadFogAmbientProfiles(
      this.lightingEnvironmentData,
      this.look,
    );
    this.fogAmbientProfiles = fogProfiles.profiles;
    for (const diagnostic of fogProfiles.diagnostics) console.warn(diagnostic);
    try {
      for (const mode of ["day", "night"] as const) {
        const profile = this.displayProfileData?.[mode];
        if (profile) {
          const lut = await DisplayLut.create(
            d,
            profile.recipe.lut,
            profile.bytes,
          );
          this.displayLuts.set(mode, lut);
          this.gpuBytes += lut.bytes;
        }
      }
    } catch (error) {
      for (const lut of this.displayLuts.values()) lut.dispose();
      this.displayLuts.clear();
      throw error;
    }
    this.displayLutFallback = d.createTexture({
      label: "Inactive display lookup",
      dimension: "3d",
      size: [1, 1, 1],
      format: "rgba16float",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    d.queue.writeTexture(
      { texture: this.displayLutFallback },
      new Uint16Array([0, 0, 0, 0x3c00]),
      { bytesPerRow: 8 },
      [1, 1, 1],
    );
    this.textures.push(this.displayLutFallback);
    this.displayLutSampler = d.createSampler({
      minFilter: "linear",
      magFilter: "linear",
    });
    const frameCore = this.experiment.framePreparation;
    this.clusteredLighting = new ClusteredLighting(
      d,
      frameCore
        ? (input) => frameCore.buildClusters(input)
        : createTightClusteredLightBuilder(),
      this.experiment.profiling
        ? (milliseconds) => {
            if (this.profiling) this.clusterCpuMs += milliseconds;
          }
        : undefined,
    );
    this.clusteredLighting.setObserving(this.profiling);
    this.report("Loading the full Medieval Town geometry…");
    const [vertices, indices, colors] = await Promise.all([
      this.fetchBuffer(this.scene.vertices),
      this.fetchBuffer(this.scene.indices),
      this.fetchBuffer(this.scene.vertexColors),
    ]);
    for (const scene of [this.scene, ...(nightScene ? [nightScene] : [])]) {
      await validateSourceNormalFrames(scene, vertices, indices, (url) =>
        this.fetchBuffer(url),
      );
    }
    await validateSourceWindBuffers(this.windData, {
      vertices,
      indices,
      vertexColors: colors,
    });
    if (colors.byteLength !== this.scene.vertexCount * 4)
      throw new Error("The prepared vertex colors are incomplete.");
    const uv1 = this.scene.vertexUV1
      ? await this.fetchBuffer(this.scene.vertexUV1)
      : new ArrayBuffer(this.scene.vertexCount * 8);
    if (uv1.byteLength !== this.scene.vertexCount * 8)
      throw new Error("The prepared secondary UV coordinates are incomplete.");
    for (const mode of ["day", "night"] as const) {
      const source = this.environmentData?.scenarios[mode]?.sourceMaterial;
      const dome =
        typeof source === "string"
          ? this.features.sky?.sourceDome(
              mode === "night" ? (nightScene ?? this.scene) : this.scene,
              source,
              {
                vertices: new Float32Array(vertices),
                indices: new Uint32Array(indices),
              },
            )
          : undefined;
      this.skyDomes.set(
        mode,
        this.buffer(
          new Float32Array(
            dome ? [...dome.center, dome.radiusMeters] : [0, 0, 0, 0],
          ),
          GPUBufferUsage.UNIFORM,
          "Source sky dome: " + mode,
        ),
      );
    }
    this.vertexUV1 = this.buffer(
      uv1,
      GPUBufferUsage.VERTEX,
      "Source secondary UV coordinates",
    );
    this.vertexColors = this.buffer(
      colors,
      GPUBufferUsage.VERTEX,
      "Source vertex colors",
    );
    this.vertices = this.buffer(
      vertices,
      GPUBufferUsage.VERTEX |
        (this.scene.mmorpgURL ? GPUBufferUsage.STORAGE : 0),
      "Town geometry",
    );
    this.meshSourceIndices = new Uint32Array(indices);
    this.gameplaySource = {
      vertices: new Float32Array(vertices),
      indices: this.meshSourceIndices,
      colors: new Uint8Array(colors),
      uv1: new Float32Array(uv1),
    };
    this.indices = this.buffer(indices, GPUBufferUsage.INDEX, "Town indices");
    this.zeroWind = this.buffer(
      new Float32Array(65536 * 4),
      GPUBufferUsage.VERTEX,
      "Fixed geometry wind inputs",
    );
    this.instances = this.buffer(
      new Float32Array(this.scene.instances.flatMap((x) => x.model)),
      GPUBufferUsage.VERTEX,
      "Town instances",
    );
    this.frameBuffer = this.buffer(
      this.uniform,
      GPUBufferUsage.UNIFORM,
      "Camera and environment",
    );
    this.farFrameBuffer = this.buffer(
      this.uniform,
      GPUBufferUsage.UNIFORM,
      "Far shadow camera",
    );
    this.distantFrameBuffer = this.buffer(
      this.uniform,
      GPUBufferUsage.UNIFORM,
      "Distant shadow camera",
    );
    this.planarFrameBuffer = this.buffer(
      this.uniform,
      GPUBufferUsage.UNIFORM,
      "Planar reflection camera",
    );
    this.postBuffer = this.buffer(
      new Float32Array(12 + FILMIC_FLOAT_COUNT),
      GPUBufferUsage.UNIFORM,
      "Display transform",
    );
    if (this.features.sky)
      this.skyDisplayBuffer = this.buffer(
        new Float32Array(12 + FILMIC_FLOAT_COUNT),
        GPUBufferUsage.UNIFORM,
        "Retained sky display transform",
      );
    this.sampler = d.createSampler({
      minFilter: "linear",
      magFilter: "linear",
      mipmapFilter: "linear",
      addressModeU: "repeat",
      addressModeV: "repeat",
      maxAnisotropy: 8,
    });
    this.report("Preparing world lighting…");
    this.frameLayout = d.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
        {
          binding: 1,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "depth" },
        },
        {
          binding: 2,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "comparison" },
        },
        {
          binding: 3,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "depth", viewDimension: "2d-array" },
        },
        {
          binding: 4,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "depth", viewDimension: "2d-array" },
        },
        {
          binding: 5,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        { binding: 6, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
        {
          binding: 7,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 8,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        {
          binding: 9,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 10,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        {
          binding: 11,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 12,
          visibility: GPUShaderStage.FRAGMENT,
          buffer: { type: "read-only-storage" },
        },
        {
          binding: 13,
          visibility: GPUShaderStage.FRAGMENT,
          buffer: { type: "read-only-storage" },
        },
        {
          binding: 14,
          visibility: GPUShaderStage.FRAGMENT,
          buffer: { type: "read-only-storage" },
        },
        {
          binding: 15,
          visibility: GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
        {
          binding: 16,
          visibility: GPUShaderStage.VERTEX,
          buffer: { type: "read-only-storage" },
        },
      ],
    });
    this.materialLayout = d.createBindGroupLayout({
      entries: [
        ...[0, 1, 2].map((binding) => ({
          binding,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" as const },
        })),
        {
          binding: 3,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          sampler: {},
        },
        {
          binding: 4,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
        {
          binding: 5,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        ...[6, 7, 8].map((binding) => ({
          binding,
          visibility:
            GPUShaderStage.FRAGMENT |
            (binding === 6 ? GPUShaderStage.VERTEX : 0),
          texture: {
            sampleType: "float" as const,
            viewDimension: "2d-array" as const,
          },
        })),
      ],
    });
    this.layout = d.createPipelineLayout({
      bindGroupLayouts: [this.frameLayout, this.materialLayout],
    });
    this.shadow = d.createTexture({
      label: "Sun shadow",
      size: [4096, 4096],
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT |
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.COPY_DST,
    });
    this.gpuBytes += 4096 * 4096 * 4;
    this.farShadow = d.createTexture({
      label: "Far and distant sun shadows",
      size: [2048, 2048, 2],
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT |
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.COPY_DST,
    });
    this.gpuBytes += 2048 * 2048 * 2 * 4;
    this.localShadow = d.createTexture({
      label: "Local light shadows",
      size: [512, 512, LOCAL_SHADOW_VIEWS],
      format: "depth32float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT |
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.COPY_DST,
    });
    this.gpuBytes += 512 * 512 * LOCAL_SHADOW_VIEWS * 4;
    const single = d.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
        {
          binding: 16,
          visibility: GPUShaderStage.VERTEX,
          buffer: { type: "read-only-storage" },
        },
      ],
    });
    this.shadowGroup = d.createBindGroup({
      layout: single,
      entries: [
        { binding: 0, resource: { buffer: this.frameBuffer } },
        { binding: 16, resource: { buffer: this.surfaceSky!.fallback } },
      ],
    });
    this.farShadowGroup = d.createBindGroup({
      layout: single,
      entries: [
        { binding: 0, resource: { buffer: this.farFrameBuffer } },
        { binding: 16, resource: { buffer: this.surfaceSky!.fallback } },
      ],
    });
    this.distantShadowGroup = d.createBindGroup({
      layout: single,
      entries: [
        { binding: 0, resource: { buffer: this.distantFrameBuffer } },
        { binding: 16, resource: { buffer: this.surfaceSky!.fallback } },
      ],
    });
    for (let i = 0; i < LOCAL_SHADOW_VIEWS; i++) {
      const buffer = this.buffer(
        this.uniform,
        GPUBufferUsage.UNIFORM,
        "Local shadow camera",
      );
      this.localBuffers.push(buffer);
      this.localGroups.push(
        d.createBindGroup({
          layout: single,
          entries: [
            { binding: 0, resource: { buffer } },
            { binding: 16, resource: { buffer: this.surfaceSky!.fallback } },
          ],
        }),
      );
    }
    this.skyGroup = this.shadowGroup;
    const fallbackSky = d.createTexture({
      label: "Fallback sky",
      size: [1, 1],
      format: "rgba16float",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    d.queue.writeTexture(
      { texture: fallbackSky },
      new Uint16Array(4),
      { bytesPerRow: 8 },
      [1, 1],
    );
    this.textures.push(fallbackSky);
    for (const [mode, description] of Object.entries(
      this.environmentData?.scenarios ?? {},
    )) {
      if (!description) continue;
      const { width, height } = description;
      if (
        !Number.isInteger(width) ||
        !Number.isInteger(height) ||
        width < 1 ||
        width > 4096 ||
        height < 1 ||
        height > 2048
      )
        throw new Error("Prepare a supported HDR sky size.");
      const data = await this.fetchBuffer(description.url);
      if (data.byteLength !== width * height * 8)
        throw new Error("The HDR sky data is incomplete.");
      const levels = Math.floor(Math.log2(Math.max(width, height))) + 1;
      const texture = d.createTexture({
        label: mode + " source HDR sky",
        size: [width, height],
        mipLevelCount: levels,
        format: "rgba16float",
        usage:
          GPUTextureUsage.TEXTURE_BINDING |
          GPUTextureUsage.COPY_DST |
          GPUTextureUsage.RENDER_ATTACHMENT,
      });
      d.queue.writeTexture({ texture }, data, { bytesPerRow: width * 8 }, [
        width,
        height,
      ]);
      this.mipmaps(texture, "rgba16float", width, height, levels);
      this.environmentTextures.set(mode, texture);
      this.textures.push(texture);
      this.gpuBytes += Math.ceil((width * height * 8 * 4) / 3);
    }
    this.hasSourceSky = this.environmentTextures.size > 0;
    this.report("Preparing surface shading…");
    this.occlusion = await ContactOcclusion.create(d);
    this.occlusionFallback = d.createTexture({
      size: [1, 1],
      format: "r8unorm",
      usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
    });
    d.queue.writeTexture(
      { texture: this.occlusionFallback },
      new Uint8Array([255]),
      { bytesPerRow: 1 },
      [1, 1],
    );
    this.textures.push(this.occlusionFallback);
    this.comparison = d.createSampler({
      compare: "less-equal",
      minFilter: "linear",
      magFilter: "linear",
    });
    this.skySampler = d.createSampler({
      addressModeU: "repeat",
      addressModeV: "clamp-to-edge",
      minFilter: "linear",
      magFilter: "linear",
      mipmapFilter: "linear",
    });
    if (this.waterData) {
      this.water = await WaterPass.create(d);
      this.water.setSources(
        this.sourceWaterStreamData
          ? { ...this.waterData, jets: [] }
          : this.waterData,
      );
      this.planar = new PlanarReflection(d);
      const count = this.waterData.basins.length;
      this.waterBounds = {
        count,
        spheres: new Float64Array(count * 4),
        aabbs: new Float64Array(count * 6),
      };
      this.waterData.basins.forEach((basin, i) => {
        const vertices = buildWaterBasins([basin]).vertices;
        const min = [Infinity, Infinity, Infinity],
          max = [-Infinity, -Infinity, -Infinity];
        for (let v = 0; v < vertices.length; v += 12)
          for (let axis = 0; axis < 3; axis++) {
            min[axis] = Math.min(min[axis], vertices[v + axis]);
            max[axis] = Math.max(max[axis], vertices[v + axis]);
          }
        const center = min.map((n, axis) => (n + max[axis]) * 0.5);
        this.waterPlaneHeights.push(center[1]);
        min[1] -= 0.02;
        max[1] += 0.02;
        this.waterBounds!.aabbs.set([...min, ...max], i * 6);
        this.waterBounds!.spheres.set(
          [...center, Math.hypot(...max.map((n, axis) => n - center[axis]))],
          i * 4,
        );
      });
    }
    this.report("Opening sky lighting…");
    this.ibl = await IblTextures.create(d, this.iblData);
    this.gpuBytes += this.ibl.bytes;
    this.report("Opening town lighting…");
    this.probes = await ProbeProfiles.create(d, this.probeData);
    this.gpuBytes += this.probes.bytes;
    this.report("Opening bounce lighting…");
    this.bounce = await BounceTextures.create(
      d,
      this.probeData,
      this.bounceData,
      {
        day: this.look?.scenarios.day.lights ?? [],
        night: this.look?.scenarios.night.lights ?? [],
      },
    );
    this.gpuBytes += this.bounce.bytes;
    this.report("Preparing town fog…");
    if (this.fogCardsData) {
      this.fogCards = await SourceFogCards.create(d, this.fogCardsData);
      this.gpuBytes += this.fogCards.bytes;
    }
    this.localFog = await LocalFog.create(d);
    this.fogAmbientVolume = await FogAmbientVolume.create(d);
    this.gpuBytes += this.fogAmbientVolume.bytes;
    for (const mode of ["day", "night"] as const) {
      const probes = this.probes.get(mode);
      this.fogAmbientInputs.set(mode, {
        profile: sourceFogAmbientSkyProfile(
          this.look?.scenarios[mode].environment,
        ),
        moments: (
          probes.scenarioMoments.get(mode) ?? probes.moments
        ).createView(),
      });
    }
    this.report("Preparing fountain water…");
    if (this.sourceWaterStreamData) {
      this.sourceWaterStreams = await SourceWaterStreams.create(
        d,
        this.sourceWaterStreamData,
      );
      this.gpuBytes += this.sourceWaterStreams.bytes;
    }
    this.report("Preparing moving details…");
    if (this.sourceWaterSplashData) {
      this.sourceWaterSplashes = await SourceWaterSplashes.create(
        d,
        this.sourceWaterSplashData,
      );
      this.gpuBytes += this.sourceWaterSplashes.bytes;
    }
    if (this.creatureData) {
      this.creatures = await Creatures.create(d, this.creatureData);
      this.gpuBytes += this.creatures.bytes;
    }
    if (this.characterData && this.features.character) {
      this.character = await this.features.character!.create(
        d,
        this.characterData,
        undefined,
        1,
        { ibl: this.ibl },
      );
      this.gpuBytes += this.character.bytes;
    }
    if (this.enemyData && this.features.character) {
      if (!this.features.character.createEnemies)
        throw new Error("This player does not include knight rendering.");
      this.enemies = await this.features.character.createEnemies(
        d,
        this.enemyData,
        undefined,
        this.enemyData.animationProfile === "quick-mmorpg-zombie-v1"
          ? { ibl: this.ibl }
          : undefined,
      );
      this.gpuBytes += this.enemies.bytes;
    }
    if (this.grassData) {
      if (!this.features.createGrass)
        throw new Error("This player does not include grass rendering.");
      this.report("Preparing the grass…");
      this.grass = await this.features.createGrass(d, this.grassData, {
        format: "rgba16float",
        depthFormat: "depth24plus",
        sampleCount: 4,
      });
      this.gpuBytes += this.grass.accountedBytes;
      this.grass.setReach(this.detailSettings.foliageReach);
    }
    this.spellEffects = await GameplayEffects.create(d);
    this.gpuBytes += this.spellEffects.bytes;
    if (this.leavesData) {
      this.leaves = await Leaves.create(d, this.leavesData);
      this.gpuBytes += this.leaves.bytes;
    }
    this.bloom = await Bloom.create(d);
    if (this.particleData)
      this.particles = await AmbientParticles.create(d, this.particleData);
    for (const mode of ["day", "night"] as const)
      this.localFogProfiles.set(
        mode,
        sourceLocalFogProfile(this.look?.scenarios[mode].environment ?? []),
      );
    for (const mode of ["day", "night"] as const)
      this.filmicProfiles.set(
        mode,
        prepareFilmic(sourceFilmicControls(this.look, mode).controls).packed,
      );
    for (const mode of ["day", "night"] as const)
      this.surfaceDecals.set(
        mode,
        await SurfaceDecalPass.create(
          d,
          this.surfaceDecalData?.[mode] ?? {
            schema: "aethyrforge.surface-decals.v1",
            materials: [],
            instances: [],
          },
        ),
      );
    this.report("Preparing world shading…");
    this.rebuildFrameGroups();
    const material = d.createShaderModule({ code: this.shaders.MATERIAL });
    this.stockModule = material;
    this.pipeline = await d.createRenderPipelineAsync(
      this.pipelineDescriptor(material),
    );
    this.blendPipeline = await d.createRenderPipelineAsync(
      this.pipelineDescriptor(material, true),
    );
    this.additivePipeline = await d.createRenderPipelineAsync(
      this.pipelineDescriptor(material, true, true),
    );
    [
      this.planarPipeline,
      this.planarBlendPipeline,
      this.planarAdditivePipeline,
    ] = await Promise.all([
      d.createRenderPipelineAsync(
        this.pipelineDescriptor(material, false, false, "cw"),
      ),
      d.createRenderPipelineAsync(
        this.pipelineDescriptor(material, true, false, "cw"),
      ),
      d.createRenderPipelineAsync(
        this.pipelineDescriptor(material, true, true, "cw"),
      ),
    ]);
    this.shadowPipeline = await d.createRenderPipelineAsync({
      layout: d.createPipelineLayout({
        bindGroupLayouts: [single, this.materialLayout],
      }),
      vertex: { module: material, entryPoint: "shadowVS", buffers: VERTICES },
      fragment: { module: material, entryPoint: "shadowFS", targets: [] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth32float",
        depthWriteEnabled: true,
        depthCompare: "less",
        depthBias: 2,
        depthBiasSlopeScale: 2,
      },
    });
    this.depthPipeline = await d.createRenderPipelineAsync(
      this.depthDescriptor(material, single),
    );
    await Promise.all(
      (
        [
          ["none", "ccw"],
          ["back", "ccw"],
          ["back", "cw"],
        ] as const
      ).map(async ([cullMode, frontFace]) => {
        const descriptor = this.depthDescriptor(material, single);
        delete descriptor.fragment;
        descriptor.primitive = {
          topology: "triangle-list",
          cullMode,
          frontFace,
        };
        const pipeline = await d.createRenderPipelineAsync(descriptor);
        this.opaqueDepthPipelines.set(`${cullMode}:${frontFace}`, pipeline);
      }),
    );
    const post = d.createShaderModule({ code: POST });
    this.displayLayout = d.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 1,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "filtering" },
        },
        {
          binding: 2,
          visibility: GPUShaderStage.FRAGMENT,
          buffer: {
            type: "uniform",
            minBindingSize: (12 + FILMIC_FLOAT_COUNT) * 4,
          },
        },
        {
          binding: 3,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float" },
        },
        {
          binding: 4,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "3d" },
        },
        {
          binding: 5,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "filtering" },
        },
      ],
    });
    this.postPipeline = await d.createRenderPipelineAsync({
      layout: d.createPipelineLayout({
        bindGroupLayouts: [this.displayLayout],
      }),
      vertex: { module: post, entryPoint: "vs" },
      fragment: {
        module: post,
        entryPoint: "fs",
        targets: [{ format: "rgba16float" }],
      },
    });
    if (this.features.sky) {
      const sky = d.createShaderModule({ code: this.shaders.SKY });
      this.skyPipeline = await d.createRenderPipelineAsync({
        layout: d.createPipelineLayout({
          bindGroupLayouts: [this.frameLayout, this.displayLayout],
        }),
        vertex: { module: sky, entryPoint: "vs" },
        fragment: {
          module: sky,
          entryPoint: "fs",
          targets: [{ format: "rgba16float" }],
        },
        multisample: { count: 4 },
        depthStencil: {
          format: "depth24plus",
          depthWriteEnabled: false,
          depthCompare: "less-equal",
        },
      });
    }
    this.displaySharpen = await DisplaySharpen.create(d, this.format);
    this.report("Preparing layered surface maps…");
    this.layerPools = await MaterialTexturePools.create(
      d,
      new Map(
        this.scene.materials.map((material) => [
          material.id,
          materialTextures(material),
        ]),
      ),
    );
    this.gpuBytes += this.layerPools.bytes;
    this.renderMaterialInfo = [...this.scene.materials];
    const refinedMaterials = new Map<
      string,
      { index: number; original: number; selection: number }
    >();
    let done = 0;
    const total = this.scene.materials.length;
    const queue = [...this.scene.materials.entries()];
    const loadedMaterials = await Promise.allSettled(
      Array.from({ length: 6 }, async () => {
        for (;;) {
          const item = queue.shift();
          if (!item) return;
          const [i, m] = item;
          this.materials[i] = await this.makeMaterial(m);
          done++;
          this.report(`Loading materials ${done}/${total}…`);
        }
      }),
    );
    const failedMaterial = loadedMaterials.find(
      (item) => item.status === "rejected",
    );
    if (failedMaterial?.status === "rejected") throw failedMaterial.reason;
    for (const scene of [this.scene, ...(nightScene ? [nightScene] : [])]) {
      const draws = sourceDraws(scene).filter(
        (draw) =>
          !this.waterData?.basins.some(
            (basin) =>
              basin.source &&
              scene.meshes[scene.instances[draw.firstInstance].mesh].id ===
                basin.source.meshId &&
              scene.materials[draw.material].id === basin.source.materialId,
          ),
      );
      const instances =
        scene === this.scene
          ? this.instances
          : this.buffer(
              new Float32Array(scene.instances.flatMap((x) => x.model)),
              GPUBufferUsage.VERTEX,
              "Night town instances",
            );
      const wind = prepareSourceWind(
        scene,
        draws,
        new Float32Array(vertices),
        new Uint32Array(indices),
        new Uint8Array(colors),
        this.windData?.[scene.scenario],
      );
      const windIndices = Uint32Array.from(
        Array.from(wind.moving).flatMap((moving, i) => (moving ? [i] : [])),
      );
      const windBounds: InstanceBounds = {
        count: windIndices.length,
        spheres: new Float64Array(windIndices.length * 4),
        aabbs: new Float64Array(windIndices.length * 6),
      };
      windIndices.forEach((source, i) => {
        windBounds.spheres.set(
          wind.bounds.spheres.subarray(source * 4, source * 4 + 4),
          i * 4,
        );
        windBounds.aabbs.set(
          wind.bounds.aabbs.subarray(source * 6, source * 6 + 6),
          i * 6,
        );
      });
      this.variants.set(scene.scenario, {
        scene,
        instances,
        draws: splitRasterOrientationDraws(
          new Float32Array(scene.instances.flatMap((x) => x.model)),
          surfaceSkyDraws(
            scene,
            wind.draws,
            this.surfaceSkyData,
            (original, selection) => {
              const key = original + ":" + selection;
              let entry = refinedMaterials.get(key);
              if (!entry) {
                entry = {
                  index: this.renderMaterialInfo.length,
                  original,
                  selection,
                };
                refinedMaterials.set(key, entry);
                this.renderMaterialInfo.push(this.scene.materials[original]);
              }
              return entry.index;
            },
          ),
        ).map((draw, sourceDrawIndex) => ({ ...draw, sourceDrawIndex })),
        bounds: wind.bounds,
        wind,
        windIndices,
        windBounds,
        windBuffer: this.buffer(
          wind.parameters,
          GPUBufferUsage.VERTEX,
          "Source wind inputs: " + scene.scenario,
        ),
        models: new Float32Array(
          scene.instances.flatMap((instance) => instance.model),
        ),
        lamps: emissiveAnchors(
          scene,
          new Float32Array(vertices),
          new Uint32Array(indices),
        ),
        triangles: draws.reduce((n, d) => n + (d.indexCount / 3) * d.count, 0),
      });
    }
    for (const { index, original, selection } of refinedMaterials.values())
      this.materials[index] = await this.makeMaterial(
        this.scene.materials[original],
        selection,
      );
    const prepared = this.scene.mmorpgURL
      ? undefined
      : this.features.prepareCanonicalSourceIndices?.(
          new Uint32Array(indices),
          new Map(
            [...this.variants].map(([mode, variant]) => [mode, variant.draws]),
          ),
          (index) => {
            const material = this.renderMaterialInfo[index];
            return material.flame ||
              material.glass ||
              material.alphaMode === "BLEND"
              ? undefined
              : this.materialFeatures.get(this.materials[index]);
          },
        );
    if (prepared) {
      const replacement = this.buffer(
        prepared.indices.buffer as ArrayBuffer,
        GPUBufferUsage.INDEX,
        "Stock source indices",
      );
      this.releaseBuffer(this.indices);
      this.indices = replacement;
      this.canonicalDraws = prepared.variants;
    }
    const initial = this.variants.get(this.scene.scenario)!;
    this.draws = initial.draws;
    this.lamps = initial.lamps;
    this.triangles = initial.triangles;
    this.report(
      `Town ready · ${this.scene.instances.length.toLocaleString()} instances`,
    );
  }
  private createFrameGroup(
    mode: "day" | "night",
    frameBuffer: GPUBuffer,
    passMode: "main" | "planar" = "main",
    extraEntries: readonly GPUBindGroupEntry[] = [],
  ) {
    return this.device.createBindGroup({
      label: `${passMode} frame: ${mode}`,
      layout: this.frameLayout,
      entries: [
        { binding: 0, resource: { buffer: frameBuffer } },
        { binding: 1, resource: this.shadow.createView() },
        { binding: 2, resource: this.comparison },
        {
          binding: 3,
          resource: this.farShadow.createView({ dimension: "2d-array" }),
        },
        {
          binding: 4,
          resource: this.localShadow.createView({ dimension: "2d-array" }),
        },
        {
          binding: 5,
          resource: (
            this.environmentTextures.get(mode) ??
            this.textures.find((t) => t.label === "Fallback sky")!
          ).createView(),
        },
        { binding: 6, resource: this.skySampler },
        {
          binding: 7,
          resource:
            passMode === "main" && this.width
              ? this.occlusion!.view
              : this.occlusionFallback.createView(),
        },
        {
          binding: 8,
          resource: (
            this.ibl!.scenarios.get(mode)?.texture ?? this.ibl!.fallback
          ).createView({ dimension: "2d-array" }),
        },
        { binding: 9, resource: this.ibl!.lut.createView() },
        { binding: 10, resource: this.surfaceDecals.get(mode)!.view },
        {
          binding: 11,
          resource: (
            this.probes!.get(mode).scenarioMoments.get(mode) ??
            this.probes!.get(mode).moments
          ).createView(),
        },
        {
          binding: 12,
          resource: {
            buffer:
              this.probes!.get(mode).scenarios.get(mode) ??
              this.probes!.get(mode).fallback,
          },
        },
        {
          binding: 13,
          resource: { buffer: this.clusteredLighting!.buffer(passMode) },
        },
        { binding: 14, resource: { buffer: this.bounce!.get(mode) } },
        { binding: 15, resource: { buffer: this.skyDomes.get(mode)! } },
        { binding: 16, resource: { buffer: this.surfaceSky!.get(mode) } },
        ...extraEntries,
      ],
    });
  }
  private rebuildFrameGroups() {
    for (const mode of ["day", "night"] as const) {
      this.environmentGroups.set(
        mode,
        this.createFrameGroup(mode, this.frameBuffer),
      );
      this.planarGroups.set(
        mode,
        this.createFrameGroup(mode, this.planarFrameBuffer!, "planar"),
      );
    }
    this.frameGroup = this.environmentGroups.get("day")!;
    this.skyGroup = this.frameGroup;
  }
  private depthDescriptor(
    module: GPUShaderModule,
    frameLayout: GPUBindGroupLayout,
    stock = true,
  ): GPURenderPipelineDescriptor {
    return {
      layout: this.device.createPipelineLayout({
        bindGroupLayouts: [frameLayout, this.materialLayout],
      }),
      vertex: {
        module,
        entryPoint: stock ? "depthVS" : "vs",
        buffers: VERTICES,
      },
      fragment: { module, entryPoint: "depthFS", targets: [] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "less",
      },
      multisample: { count: 4 },
    };
  }
  private pipelineDescriptor(
    module: GPUShaderModule,
    blend = false,
    additive = false,
    frontFace: GPUFrontFace = "ccw",
    features?: number,
    orientation: RasterOrientation = "degenerate",
  ): GPURenderPipelineDescriptor {
    const raster = deriveStockRasterPolicy(
      features,
      blend,
      frontFace,
      orientation,
    );
    const plain =
      features !== undefined && (features & STOCK_SURFACE_SKY_FLAG) === 0;
    return {
      label: blend ? "Town transparent" : "Town PBR",
      layout: this.layout,
      vertex: {
        module,
        entryPoint: plain ? "vsPlain" : "vs",
        buffers: VERTICES,
        ...(features === undefined
          ? {}
          : {
              constants: {
                AF_SURFACE_SKY: Number(
                  (features & STOCK_SURFACE_SKY_FLAG) !== 0,
                ),
              },
            }),
      },
      fragment: {
        module,
        entryPoint: plain ? "fsPlain" : "fs",
        ...(features === undefined
          ? {}
          : {
              constants: {
                AF_MATERIAL_FEATURES: features & STOCK_MATERIAL_FEATURE_MASK,
                AF_RASTER_FLAGS: raster.rasterFlags,
                AF_SURFACE_SKY: Number(
                  (features & STOCK_SURFACE_SKY_FLAG) !== 0,
                ),
              },
            }),
        targets: [
          {
            format: "rgba16float",
            ...(blend
              ? {
                  blend: {
                    color: {
                      srcFactor: "src-alpha" as const,
                      dstFactor: additive
                        ? ("one" as const)
                        : ("one-minus-src-alpha" as const),
                      operation: "add" as const,
                    },
                    alpha: {
                      srcFactor: "one" as const,
                      dstFactor: "one-minus-src-alpha" as const,
                      operation: "add" as const,
                    },
                  },
                }
              : {}),
          },
        ],
      },
      primitive: {
        topology: "triangle-list",
        cullMode: raster.cullMode,
        frontFace: raster.frontFace,
      },
      depthStencil: {
        format: "depth24plus",
        // Main stock opaque surfaces already wrote the same depth in the prepass.
        depthWriteEnabled: raster.depthWriteEnabled,
        depthCompare: "less-equal",
      },
      multisample: {
        count: 4,
        alphaToCoverageEnabled: raster.alphaToCoverageEnabled,
      },
    };
  }
  private stockPipelineKey(
    features: number,
    blend: boolean,
    additive: boolean,
    front: GPUFrontFace,
    orientation: RasterOrientation,
  ) {
    return stockRasterPipelineKey(
      features,
      blend,
      additive,
      front,
      orientation,
    );
  }
  private async prepareStockPipelines(
    features: number,
    blend: boolean,
    additive: boolean,
  ) {
    if (!this.stockModule) return;
    await Promise.all(
      (["ccw", "cw"] as const).flatMap((front) =>
        (["positive", "negative", "degenerate"] as const).map((orientation) => {
          const key = this.stockPipelineKey(
            features,
            blend,
            additive,
            front,
            orientation,
          );
          let pending = this.stockPipelineTasks.get(key);
          if (!pending) {
            pending = this.device
              .createRenderPipelineAsync(
                this.pipelineDescriptor(
                  this.stockModule!,
                  blend,
                  additive,
                  front,
                  features,
                  orientation,
                ),
              )
              .then((pipeline) => {
                this.stockPipelines.set(key, pipeline);
                this.stockChoiceRevision++;
              });
            this.stockPipelineTasks.set(key, pending);
          }
          return pending;
        }),
      ),
    );
  }
  private materialPipeline(
    group: GPUBindGroup,
    blend: boolean,
    additive: boolean,
    kind: "main" | "planar" | "depth",
    orientation: RasterOrientation = "degenerate",
  ) {
    if (this.activeShaderSource !== this.shaders.MATERIAL)
      return this.resolveMaterialPipeline(
        group,
        blend,
        additive,
        kind,
        orientation,
      );
    const direction =
      orientation === "positive"
        ? 0
        : orientation === "negative"
          ? 1
          : orientation === "degenerate"
            ? 2
            : -1;
    const pass =
      kind === "depth" ? 0 : kind === "main" ? 1 : kind === "planar" ? 2 : -1;
    const features = this.materialFeatures.get(group);
    if (
      features === undefined ||
      typeof blend !== "boolean" ||
      typeof additive !== "boolean" ||
      direction < 0 ||
      pass < 0 ||
      !group ||
      (typeof group !== "object" && typeof group !== "function")
    )
      return this.resolveMaterialPipeline(
        group,
        blend,
        additive,
        kind,
        orientation,
      );
    let choice = this.stockChoices.get(group);
    if (!choice) {
      choice = { revision: this.stockChoiceRevision, features, pipelines: [] };
      this.stockChoices.set(group, choice);
    } else if (
      choice.revision !== this.stockChoiceRevision ||
      choice.features !== features
    ) {
      choice.revision = this.stockChoiceRevision;
      choice.features = features;
      choice.pipelines.length = 0;
    }
    const slot =
      pass * 12 + Number(blend) * 6 + Number(additive) * 3 + direction;
    return (choice.pipelines[slot] ??= this.resolveMaterialPipeline(
      group,
      blend,
      additive,
      kind,
      orientation,
    ));
  }
  private resolveMaterialPipeline(
    group: GPUBindGroup,
    blend: boolean,
    additive: boolean,
    kind: "main" | "planar" | "depth",
    orientation: RasterOrientation = "degenerate",
  ) {
    if (kind === "depth") {
      if (this.activeShaderSource !== this.shaders.MATERIAL)
        return this.depthPipeline;
      const policy = deriveStockDepthPolicy(
        this.materialFeatures.get(group),
        orientation,
      );
      return policy.fragmentRequired
        ? this.depthPipeline
        : (this.opaqueDepthPipelines.get(
            `${policy.cullMode}:${policy.frontFace}`,
          ) ?? this.depthPipeline);
    }
    const front = kind === "planar" ? "cw" : "ccw";
    const features = this.materialFeatures.get(group);
    if (
      this.activeShaderSource === this.shaders.MATERIAL &&
      features !== undefined
    ) {
      const specialized = this.stockPipelines.get(
        this.stockPipelineKey(features, blend, additive, front, orientation),
      );
      if (specialized) return specialized;
    }
    return kind === "main"
      ? additive
        ? this.additivePipeline
        : blend
          ? this.blendPipeline
          : this.pipeline
      : additive
        ? this.planarAdditivePipeline
        : blend
          ? this.planarBlendPipeline
          : this.planarPipeline;
  }
  private async fetchBuffer(url: string) {
    return readAsset(url, async (r) => {
      if (!r.ok)
        throw new Error(
          "Prepare the private town assets before opening the world.",
        );
      return r.arrayBuffer();
    });
  }
  private async texture(
    url: string | undefined,
    kind: "color" | "normal" | "mra",
  ): Promise<GPUTexture> {
    const key = (url ?? kind) + ":" + kind;
    const cached = this.textureCache.get(key);
    if (cached) return cached;
    const pending = (async () => {
      const d = this.device;
      const format: GPUTextureFormat =
        kind === "color" ? "rgba8unorm-srgb" : "rgba8unorm";
      let bitmap: ImageBitmap | undefined;
      let raw: ReturnType<typeof decodeRgba8> | undefined;
      if (url) {
        const data = await readAsset(url, async (response) => {
          if (!response.ok)
            throw new Error(
              "A prepared material map could not open. Prepare the private assets and reopen the world.",
            );
          return new URL(url, location.href).pathname.endsWith(".bin")
            ? response.arrayBuffer()
            : response.blob();
        });
        if (data instanceof ArrayBuffer) raw = decodeRgba8(data);
        else
          bitmap = await createImageBitmap(data, {
            colorSpaceConversion: "none",
            premultiplyAlpha: "none",
          });
      }
      const width = raw?.width ?? bitmap?.width ?? 1,
        height = raw?.height ?? bitmap?.height ?? 1,
        levels = Math.floor(Math.log2(Math.max(width, height))) + 1;
      const texture = d.createTexture({
        label: url ?? kind,
        size: [width, height],
        mipLevelCount: levels,
        format,
        usage:
          GPUTextureUsage.TEXTURE_BINDING |
          GPUTextureUsage.COPY_DST |
          GPUTextureUsage.RENDER_ATTACHMENT,
      });
      this.textures.push(texture);
      this.materialBaseUploadBytes += width * height * 4;
      this.gpuBytes += Math.ceil((width * height * 4 * 4) / 3);
      if (raw) {
        d.queue.writeTexture(
          { texture },
          raw.pixels,
          { bytesPerRow: width * 4 },
          [width, height],
        );
        this.mipmaps(texture, format, width, height, levels);
      } else if (bitmap) {
        d.queue.copyExternalImageToTexture({ source: bitmap }, { texture }, [
          width,
          height,
        ]);
        bitmap.close();
        this.mipmaps(texture, format, width, height, levels);
      } else {
        const color =
          kind === "normal"
            ? [128, 128, 255, 255]
            : kind === "mra"
              ? [0, 255, 255, 255]
              : [255, 255, 255, 255];
        d.queue.writeTexture(
          { texture },
          new Uint8Array(color),
          { bytesPerRow: 4 },
          [1, 1],
        );
      }
      return texture;
    })();
    this.textureCache.set(key, pending);
    return pending;
  }
  private mipmaps(
    texture: GPUTexture,
    format: GPUTextureFormat,
    width: number,
    height: number,
    levels: number,
  ) {
    let pipeline = this.mipPipelines.get(format);
    const d = this.device;
    if (!pipeline) {
      const module = d.createShaderModule({
        code: `@group(0)@binding(0)var t:texture_2d<f32>;@group(0)@binding(1)var s:sampler;struct O{@builtin(position)p:vec4<f32>,@location(0)uv:vec2<f32>};@vertex fn v(@builtin(vertex_index)i:u32)->O{let q=vec2(f32((i<<1u)&2u),f32(i&2u));var o:O;o.p=vec4(q*2.0-1.0,0.0,1.0);o.uv=vec2(q.x,1.0-q.y);return o;}@fragment fn f(i:O)->@location(0)vec4<f32>{return textureSample(t,s,i.uv);}`,
      });
      pipeline = d.createRenderPipeline({
        layout: "auto",
        vertex: { module, entryPoint: "v" },
        fragment: { module, entryPoint: "f", targets: [{ format }] },
      });
      this.mipPipelines.set(format, pipeline);
    }
    const encoder = d.createCommandEncoder();
    const sampler = d.createSampler({
      minFilter: "linear",
      magFilter: "linear",
    });
    for (let i = 1; i < levels; i++) {
      const group = d.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          {
            binding: 0,
            resource: texture.createView({
              baseMipLevel: i - 1,
              mipLevelCount: 1,
            }),
          },
          { binding: 1, resource: sampler },
        ],
      });
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: texture.createView({ baseMipLevel: i, mipLevelCount: 1 }),
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.draw(3);
      pass.end();
    }
    d.queue.submit([encoder.finish()]);
  }
  private materialEditor?: MaterialEditor;
  private editMaterials() {
    if (this.disposed) return;
    return (this.materialEditor ??= this.features.createMaterialEditor?.({
      scene: this.scene,
      device: this.device,
      images: (document) => this.creatorImageProvider?.(document),
      count: () => this.materials.length,
      material: (source, texture, uniform) =>
        this.makeMaterial(source, 0, { texture, uniform }),
      install: (start, groups, sources) => {
        this.materials.splice(start, this.materials.length - start, ...groups);
        this.renderMaterialInfo.splice(
          start,
          this.renderMaterialInfo.length - start,
          ...sources,
        );
      },
      release: (buffer) => this.releaseBuffer(buffer),
    }));
  }
  private async makeMaterial(
    m: CompiledMaterial,
    surfaceSkyMarker = 0,
    creator?: {
      texture: GPUTexture;
      uniform: (buffer: GPUBuffer, original: Float32Array) => void;
    },
  ) {
    const grassGround = grassGroundMarker(m);
    if (
      m.normalConvention !== undefined &&
      m.normalConvention !== "opengl" &&
      m.normalConvention !== "directx"
    )
      throw new Error(
        "Prepare a supported normal map orientation before opening the town.",
      );
    const pools = this.layerPools!.forMaterial(m.id, materialTextures(m));
    const profile = Boolean(
      m.layers || m.glass || m.flame || m.paint || m.terrain || m.vegetation,
    );
    const [base, normal, mra, emission] = await Promise.all([
      this.texture(profile ? undefined : m.baseColorUrl, "color"),
      this.texture(profile ? undefined : m.normalUrl, "normal"),
      this.texture(profile ? undefined : m.mraUrl, "mra"),
      this.texture(profile ? undefined : m.emissiveUrl, "color"),
    ]);
    const u = new Float32Array(
      20 +
        LAYER_FLOAT_COUNT +
        GLASS_FLOAT_COUNT +
        PAINT_FLOAT_COUNT +
        TERRAIN_FLOAT_COUNT +
        VEGETATION_FLOAT_COUNT +
        4,
    );
    u.set(m.baseColorFactor, 0);
    u.set(m.emissiveFactor, 4);
    u.set(
      [
        m.roughness,
        m.metallic,
        m.normalScale,
        m.alphaMode === "MASK" ? 1 : m.alphaMode === "BLEND" ? 2 : 0,
      ],
      8,
    );
    u.set([...m.uvScale, m.alphaCutoff, m.doubleSided ? 1 : 0], 12);
    u.set(
      [
        m.normalUrl ? 1 : 0,
        m.mraUrl ? 1 : 0,
        m.vertexOpacity ? 1 : 0,
        normalMapFlags(m),
      ],
      16,
    );
    u.set(
      packLayers(m, (url, role) => pools[role].index(url)),
      20,
    );
    u.set(
      packGlass(m, (url, role) => pools[role].index(url)),
      20 + LAYER_FLOAT_COUNT,
    );
    u.set(
      packPaint(m, (url, role) => pools[role].index(url)),
      20 + LAYER_FLOAT_COUNT + GLASS_FLOAT_COUNT,
    );
    u.set(
      packTerrain(m, (url, role) => pools[role].index(url)),
      20 + LAYER_FLOAT_COUNT + GLASS_FLOAT_COUNT + PAINT_FLOAT_COUNT,
    );
    const flutter = sourceWindFlutterMaterial(this.windData, m);
    this.hasFlutter ||= Boolean(flutter);
    u.set(
      packVegetation(m, (url, role) => pools[role].index(url), flutter),
      20 +
        LAYER_FLOAT_COUNT +
        GLASS_FLOAT_COUNT +
        PAINT_FLOAT_COUNT +
        TERRAIN_FLOAT_COUNT,
    );
    u[
      20 +
        LAYER_FLOAT_COUNT +
        GLASS_FLOAT_COUNT +
        PAINT_FLOAT_COUNT +
        TERRAIN_FLOAT_COUNT +
        VEGETATION_SURFACE_SKY_MARKER
    ] = surfaceSkyMarker;
    u[u.length - 4] = creator ? 1 : 0;
    u[u.length - 3] = grassGround;
    const uniform = this.buffer(
      u,
      GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      m.name,
    );
    if (creator) creator.uniform(uniform, u.slice());
    else if (surfaceSkyMarker === 0)
      this.editMaterials()?.track(m.id, uniform, u);
    const group = this.device.createBindGroup({
      layout: this.materialLayout,
      entries: [
        { binding: 0, resource: base.createView() },
        { binding: 1, resource: normal.createView() },
        { binding: 2, resource: mra.createView() },
        { binding: 3, resource: this.sampler },
        {
          binding: 4,
          resource: { buffer: uniform },
        },
        { binding: 5, resource: emission.createView() },
        ...(["color", "normal", "data"] as const).map((role, i) => ({
          binding: 6 + i,
          resource: (role === "color" && creator
            ? creator.texture
            : pools[role].texture
          ).createView({ dimension: "2d-array" }),
        })),
      ],
    });
    const glassOffset = 20 + LAYER_FLOAT_COUNT;
    const paintOffset = glassOffset + GLASS_FLOAT_COUNT;
    const terrainOffset = paintOffset + PAINT_FLOAT_COUNT;
    const vegetationOffset = terrainOffset + TERRAIN_FLOAT_COUNT;
    const features =
      stockMaterialConstants({
        layered: u[20] > 0.5,
        glass: u[glassOffset] > 0.5,
        flame: u[glassOffset + 1] > 0.5,
        paint: u[paintOffset] > 0.5,
        terrain: u[terrainOffset] > 0.5,
        vegetation: u[vegetationOffset] > 0.5,
        normal: u[16] > 0.5,
        mra: u[17] > 0.5,
        creator: u[u.length - 4] > 0.5,
      }).AF_MATERIAL_FEATURES |
      stockRasterMaterialFlags(u[11] > 0.5 && u[11] < 1.5, u[15] >= 0.5) |
      (surfaceSkyMarker > 0 ? STOCK_SURFACE_SKY_FLAG : 0);
    this.materialFeatures.set(group, features);
    await this.prepareStockPipelines(
      features,
      !!m.flame || !!m.glass || m.alphaMode === "BLEND",
      !!m.flame,
    );
    return group;
  }
  resize(width: number, height: number) {
    if (this.disposed) return;
    if (width === this.width && height === this.height) return;
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.canvas.width = this.width;
    this.canvas.height = this.height;
    this.displaySharpen!.resize(this.width, this.height);
    this.localFog!.resize(this.width, this.height);
    this.bloom!.resize(this.width, this.height);
    for (const pass of this.surfaceDecals.values())
      pass.resize(this.width, this.height);
    this.depth?.destroy();
    this.hdr?.destroy();
    this.msaa?.destroy();
    this.waterHDR?.destroy();
    this.streamHDR?.destroy();
    this.waterHDR = undefined;
    this.streamHDR = undefined;
    this.depth = this.device.createTexture({
      size: [this.width, this.height],
      format: "depth24plus",
      sampleCount: 4,
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.occlusion!.resize(this.width, this.height, this.depth.createView());
    this.fogCardDepthView = this.depth.createView();
    this.rebuildFrameGroups();
    this.hdr = this.device.createTexture({
      size: [this.width, this.height],
      format: "rgba16float",
      usage:
        GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.msaa = this.device.createTexture({
      size: [this.width, this.height],
      format: "rgba16float",
      sampleCount: 4,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    if (this.water?.hasContent)
      this.waterHDR = this.device.createTexture({
        label: "Water HDR output",
        size: [this.width, this.height],
        format: "rgba16float",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
    if (this.sourceWaterStreams)
      this.streamHDR = this.device.createTexture({
        label: "Source stream refraction output",
        size: [this.width, this.height],
        format: "rgba16float",
        usage:
          GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
    this.localFogViews = {
      scene: (this.streamHDR ?? this.waterHDR ?? this.hdr).createView(),
      depth: this.depth.createView(),
      local: this.localShadow.createView({ dimension: "2d-array" }),
      near: this.shadow.createView(),
      far: this.farShadow.createView({
        dimension: "2d",
        baseArrayLayer: 0,
        arrayLayerCount: 1,
      }),
      distant: this.farShadow.createView({
        dimension: "2d",
        baseArrayLayer: 1,
        arrayLayerCount: 1,
      }),
    };
    this.postGroups.clear();
    this.skyDisplayGroups.clear();
    for (const mode of ["day", "night"] as const) {
      const lut = this.displayLuts.get(mode);
      this.postGroups.set(
        mode,
        this.device.createBindGroup({
          layout: this.displayLayout,
          entries: [
            { binding: 0, resource: this.localFog!.view },
            {
              binding: 1,
              resource: this.device.createSampler({
                minFilter: "linear",
                magFilter: "linear",
              }),
            },
            { binding: 2, resource: { buffer: this.postBuffer } },
            { binding: 3, resource: this.bloom!.view },
            {
              binding: 4,
              resource:
                lut?.view ??
                this.displayLutFallback.createView({ dimension: "3d" }),
            },
            { binding: 5, resource: lut?.sampler ?? this.displayLutSampler },
          ],
        }),
      );
      if (this.skyDisplayBuffer)
        this.skyDisplayGroups.set(
          mode,
          this.device.createBindGroup({
            layout: this.displayLayout,
            entries: [
              { binding: 0, resource: this.localFog!.view },
              { binding: 1, resource: this.sampler },
              { binding: 2, resource: { buffer: this.skyDisplayBuffer } },
              { binding: 3, resource: this.bloom!.view },
              {
                binding: 4,
                resource:
                  lut?.view ??
                  this.displayLutFallback.createView({ dimension: "3d" }),
              },
              { binding: 5, resource: lut?.sampler ?? this.displayLutSampler },
            ],
          }),
        );
    }
  }
  get hasCharacter() {
    return !!this.character;
  }
  async loadMmorpgTerrain(metadata: MmorpgTerrainMetadata) {
    const create = this.features.character?.createMmorpgTerrain;
    if (!create) throw Error("This view does not include the shared world.");
    const candidate = await create(this.device, this.vertices, metadata);
    if (this.disposed) {
      candidate.dispose();
      throw Error("The world was closed before its ground loaded.");
    }
    this.mmorpgTerrain?.dispose();
    this.mmorpgTerrain = candidate;
  }
  async loadMmorpgEffects(world: MmorpgWorldManifest) {
    const create = this.features.character?.createMmorpgEffects;
    if (!create) throw Error("This view does not include the shared world.");
    const candidate = await create(
      this.device,
      world.particleTextures,
      world.worldScale,
      undefined,
      { depthFormat: "depth24plus", sampleCount: 4 },
    );
    if (this.disposed) {
      candidate.dispose();
      throw Error("The world was closed before its effects loaded.");
    }
    this.gpuBytes += candidate.bytes - (this.mmorpgEffects?.bytes ?? 0);
    this.mmorpgEffects?.dispose();
    this.mmorpgEffects = candidate;
  }
  get mmorpgEffectStats() {
    return this.mmorpgEffects?.stats;
  }
  loadMmorpgScenery(world: MmorpgWorldManifest) {
    this.mmorpgWorld = world;
    this.mmorpgSceneryCell = undefined;
  }
  setMmorpgScenery(center: readonly number[]) {
    const world = this.mmorpgWorld;
    if (!world) return false;
    const cell = mmorpgSceneryCenter(center).join(":");
    if (cell === this.mmorpgSceneryCell) return false;
    const placements = mmorpgSceneryPlacements(world, center);
    for (const variant of this.variants.values()) {
      const scene = variant.scene,
        metadata = mmorpgSceneryMetadata(scene);
      if (!metadata) throw Error("Prepare the source scenery ranges.");
      scene.instances = [
        ...scene.instances.slice(0, metadata.firstInstance),
        ...mmorpgSceneryInstances(world, placements, metadata.firstMesh),
      ];
      scene.batches = mmorpgSceneBatches(scene.instances);
      const models = new Float32Array(
        scene.instances.flatMap((instance) => instance.model),
      );
      if (variant.instances.size < models.byteLength) {
        this.releaseBuffer(variant.instances);
        variant.instances = this.buffer(
          new Float32Array(512 * 16),
          GPUBufferUsage.VERTEX,
          "Moving source scenery",
        );
      }
      this.device.queue.writeBuffer(variant.instances, 0, models);
      const source = this.gameplaySource!;
      const wind = prepareSourceWind(
        scene,
        sourceDraws(scene),
        source.vertices,
        source.indices,
        source.colors,
      );
      if (variant.windBuffer.size < wind.parameters.byteLength) {
        this.releaseBuffer(variant.windBuffer);
        variant.windBuffer = this.buffer(
          new Float32Array(512 * 4),
          GPUBufferUsage.VERTEX,
          "Moving scenery wind data",
        );
      }
      this.device.queue.writeBuffer(variant.windBuffer, 0, wind.parameters);
      variant.models = models;
      variant.wind = wind;
      variant.bounds = wind.bounds;
      variant.draws = splitRasterOrientationDraws(models, wind.draws).map(
        (draw, sourceDrawIndex) => ({ ...draw, sourceDrawIndex }),
      );
      variant.triangles = variant.draws.reduce(
        (n, d) => n + (d.indexCount / 3) * d.count,
        0,
      );
      variant.windIndices = new Uint32Array();
      variant.windBounds = {
        count: 0,
        spheres: new Float64Array(),
        aabbs: new Float64Array(),
      };
    }
    const active = this.variants.get(
      this.lastView?.env.mode ?? this.scene.scenario,
    )!;
    this.instances = active.instances;
    this.draws = active.draws;
    this.triangles = active.triangles;
    this.mmorpgSceneryCell = cell;
    // A streamed window can change both instance and draw counts. Resident
    // handles have fixed capacities, so rebuild their storage on the next view.
    for (const resident of this.frameScenes.values()) resident.dispose?.();
    this.frameScenes.clear();
    this.geometryVersion++;
    this.shadowDirty = true;
    this.windShadowPlanner.reset();
    this.planar?.invalidate();
    return true;
  }
  get mmorpgTerrainStats() {
    return this.mmorpgTerrain?.stats;
  }
  async loadMmorpgCharacters(
    manifests: Readonly<Record<MmorpgClass, CharacterManifest>>,
  ) {
    const create = this.features.character?.createMmorpg;
    const createNames = this.features.character?.createMmorpgNames;
    if (!create || !createNames)
      throw Error("This view does not include the shared world.");
    const candidate = await create(this.device, manifests, undefined, {
      ibl: this.ibl,
      shadow: { texture: this.shadow, sampler: this.comparison },
    });
    let names: MmorpgNames;
    try {
      names = await createNames(this.device);
    } catch (error) {
      candidate.dispose();
      throw error;
    }
    if (this.disposed) {
      candidate.dispose();
      names.dispose();
      throw Error("The world was closed before its characters loaded.");
    }
    this.gpuBytes +=
      candidate.bytes +
      names.bytes -
      (this.mmorpgCharacters?.bytes ?? 0) -
      (this.mmorpgNames?.bytes ?? 0);
    this.mmorpgCharacters?.dispose();
    this.mmorpgNames?.dispose();
    this.mmorpgCharacters = candidate;
    this.mmorpgNames = names;
  }
  setMmorpgActors(
    actors: readonly MmorpgActorPose[],
    localActorId?: number,
    events: readonly MmoEvent[] = [],
    tick?: number,
    hideLocal = false,
  ) {
    validateMmorpgActors(actors);
    this.mmorpgEffects?.setMmorpgActors(actors, events, tick);
    this.mmorpgActors = hideLocal ? actors.filter((actor) => actor.id !== localActorId) : actors;
    this.mmorpgLocalId = localActorId;
    const local = actors.find((actor) => actor.id === localActorId);
    if (local) this.setMmorpgScenery(local.position);
    this.mmorpgInteractionPose = local
      ? {
          position: local.position,
          yaw: local.yaw,
          animation: local.state,
          time: local.actionTime,
          visible: true,
        }
      : undefined;
  }
  get mmorpgStats() {
    return {
      ...(this.mmorpgCharacters?.stats ?? {
        modelLoaded: false,
        actors: 0,
        triangles: 0,
        healthBars: 0,
        healthBarTriangles: 0,
        classes: 0,
        geometryBytes: 0,
      }),
      ...(this.mmorpgNames?.stats ?? {
        nameLabels: 0,
        nameTriangles: 0,
        nameDrawCalls: 0,
        nameTextureBytes: 0,
        nameTextureLayers: 0,
        nameUploads: 0,
      }),
    };
  }
  setCharacter(pose: CharacterRenderPose | undefined) {
    this.characterPose = pose;
  }
  setEnemies(actors: readonly EnemyActorPose[]) {
    this.enemyActors = actors;
  }
  get enemyStats() {
    return {
      modelLoaded: !!this.enemies,
      submittedActors: this.enemies?.stats.submittedActors ?? 0,
      submittedTriangles: this.enemies?.stats.submittedTriangles ?? 0,
      submittedHealthBars: this.enemies?.stats.submittedHealthBars ?? 0,
      submittedHealthBarTriangles:
        this.enemies?.stats.submittedHealthBarTriangles ?? 0,
      geometryBytes: this.enemies?.bytes ?? 0,
    };
  }
  /** Pin the last submitted native actor poses and palettes, never a future live state. */
  async pinCaptureActors(
    sourceIdentity: string,
    scenario: "day" | "night",
    nativeStart = false,
  ) {
    if (
      this.disposed ||
      this.capturing ||
      !this.lastView ||
      this.lastView.env.mode !== scenario
    )
      throw new Error(
        "Wait for this world and lighting to render before pinning its actors.",
      );
    if (
      this.captureActorSourceIdentity &&
      this.captureActorSourceIdentity !== sourceIdentity
    )
      throw new Error("Pin actors from the current world source.");
    let sourceFrame = this.enemies ? this.lastEnemyFrame : this.frame;
    const payload = nativeStart
      ? this.enemies?.pinNativeStartReference(this.enemyActors)
      : this.enemies?.pinCaptureReference();
    const identity = payload
      ? await this.enemies!.captureReferenceIdentity(payload)
      : await (async () => {
          const bytes = new Uint8Array(
            await crypto.subtle.digest(
              "SHA-256",
              new TextEncoder().encode("aethyrforge.no-enemies.v1"),
            ),
          );
          const hash = [...bytes]
            .map((value) => value.toString(16).padStart(2, "0"))
            .join("");
          return { modelIdentity: hash, poseSha256: hash };
        })();
    if (
      this.disposed ||
      this.capturing ||
      this.lastView.env.mode !== scenario ||
      (this.captureActorSourceIdentity !== undefined &&
        this.captureActorSourceIdentity !== sourceIdentity) ||
      (payload &&
        (payload.actorIds.length !== this.enemyActors.length ||
          payload.actorIds.some(
            (id, index) => id !== this.enemyActors[index].id,
          )))
    )
      throw new Error(
        "The world's actors changed while their capture reference was prepared.",
      );
    if (nativeStart && payload)
      await this.captureFrame(
        0,
        this.lastView,
        "pixels",
        undefined,
        (frame) => {
          sourceFrame = frame.frameAfter;
        },
        payload,
      );
    if (this.disposed || this.lastView.env.mode !== scenario)
      throw new Error(
        "The world changed before its actor capture reference was ready.",
      );
    this.captureActorRegistry ??= new CaptureActorReferences({
      sourceIdentity,
      incarnation: {},
    });
    this.captureActorSourceIdentity = sourceIdentity;
    this.captureActorModelIdentity = identity.modelIdentity;
    return this.captureActorRegistry.pin({
      scenario,
      ...identity,
      sourceFrame,
      actorIds: payload ? [...payload.actorIds] : [],
      payload,
      ...(nativeStart ? { basis: "native-start" as const } : {}),
    });
  }
  setGameplayObjects(
    mode: "day" | "night",
    objects: readonly GameplayObject[],
    targetGroups?: GameplayTargetGroups,
  ): void {
    if (!this.sourceDebris) {
      this.sourceDebris = new SourceDebris(
        this.device,
        new Map(
          Array.from(this.variants, ([key, value]) => [key, value.scene]),
        ),
        this.gameplaySource!,
        this.debrisPacking,
      );
      this.gpuBytes += this.sourceDebris.bytes;
    }
    this.sourceDebris.setObjects(mode, objects, targetGroups);
    this.destroyedPlacements.delete(mode);
    this.gameplayDamageKeys.delete(mode);
    this.gameplayEffects.delete(mode);
    this.geometryVersion++;
    this.shadowDirty = true;
  }
  /** Apply an accepted native reset while retaining the loaded town topology. */
  resetGameplay(mode: "day" | "night"): void {
    const changed = this.sourceDebris?.resetPlayState(mode) ?? false;
    const masked = (this.destroyedPlacements.get(mode)?.size ?? 0) > 0;
    const effects = (this.gameplayEffects.get(mode)?.length ?? 0) > 0;
    this.destroyedPlacements.delete(mode);
    this.gameplayDamageKeys.set(mode, ":");
    this.gameplayEffects.delete(mode);
    if (changed || masked) {
      this.geometryVersion++;
      this.shadowDirty = true;
      this.planar?.invalidate();
    }
    if (changed || masked || effects) this.gameplayAnimationRevision++;
  }
  prepareGameplayDestruction(
    mode: "day" | "night",
    prospectiveDestroyed: readonly number[],
  ): boolean {
    return (
      this.sourceDebris?.prepare(mode, prospectiveDestroyed) ??
      prospectiveDestroyed.length === 0
    );
  }
  async prepareGameplayDestructionAsync(
    mode: "day" | "night",
    prospectiveDestroyed: readonly number[],
    cancelled: () => boolean = () => false,
  ): Promise<boolean> {
    return (
      this.sourceDebris?.prepareAsync(mode, prospectiveDestroyed, cancelled) ??
      prospectiveDestroyed.length === 0
    );
  }
  castleFragmentMasks(mode: "day" | "night"): [number, number][] {
    return this.sourceDebris?.castleFragmentMasks(mode) ?? [];
  }
  prepareCastleDestruction(
    mode: "day" | "night",
    ids: readonly number[],
    cancelled: () => boolean = () => false,
  ) {
    return this.prepareGameplayDestructionAsync(mode, ids, cancelled);
  }
  gameplayFragmentMasks(mode: "day" | "night", ids: readonly number[]) {
    return this.sourceDebris?.fragmentMasks(mode, ids) ?? [];
  }
  get gameplayLimitReason(): string {
    return this.sourceDebris?.limitReason ?? "";
  }
  get gameplayStats() {
    return {
      ...(this.sourceDebris?.statsSnapshot ?? {
        activeFracturedSources: 0,
        standingCells: 0,
        standingTriangles: 0,
        renderedDebris: 0,
        debrisTriangles: 0,
        maskedNodes: 0,
        rubbleCount: 0,
        rubbleTriangles: 0,
        geometryBytes: 0,
        limitReason: "",
      }),
      effectTriangles: this.spellEffects?.triangles ?? 0,
      aimTriangles: this.spellEffects?.aimTriangles ?? 0,
    };
  }
  setGameplayAim(marker?: GameplayAimMarker): void {
    this.gameplayAim = marker;
  }
  setGameplayState(mode: "day" | "night", state: GameplayVisualState): void {
    validateGameplayVisualState(state);
    const staticRevision = this.sourceDebris?.staticRevision;
    const sources =
      this.sourceDebris?.update(
        mode,
        state.destroyed,
        state.debris,
        state.rubble ?? state.castle?.rubble,
      ) ?? new Set<number>();
    this.destroyedPlacements.set(mode, sources);
    this.gameplayEffects.set(mode, state.effects);
    const key =
      state.destroyed.join(",") +
      ":" +
      ((state.rubble ?? state.castle?.rubble)
        ?.map((body) => body.id)
        .join(",") ?? "");
    if (
      this.gameplayDamageKeys.get(mode) !== key ||
      this.sourceDebris?.staticRevision !== staticRevision
    ) {
      this.gameplayDamageKeys.set(mode, key);
      this.geometryVersion++;
      this.shadowDirty = true;
      this.planar?.invalidate();
    }
    this.gameplayAnimationRevision++;
  }
  render(
    camera: Camera,
    env: EnvironmentSettings,
    time: number,
    captureFrame = false,
  ) {
    this.cullCpuMs = 0;
    this.clusterCpuMs = 0;
    this.commandEncodeMs = null;
    if (this.disposed) return false;
    if (this.capturing && !captureFrame) return false;
    if (this.lost)
      throw new Error(
        "The graphics device stopped. Reopen the world to continue.",
      );
    if (this.inFlight >= 2) return false;
    this.sourceCommands = {
      draws: 0,
      pipelines: 0,
      materials: 0,
      windBindings: 0,
    };
    this.lastView = {
      camera: { ...camera, eye: [...camera.eye] },
      env: { ...env },
      time,
    };
    const variant = this.variants.get(env.mode);
    if (variant) {
      if (this.instances !== variant.instances) {
        this.shadowDirty = true;
        this.geometryVersion++;
      }
      this.instances = variant.instances;
      this.draws = variant.draws;
      this.lamps = variant.lamps;
      this.triangles = variant.triangles;
    }
    const fwd = direction(camera.yaw, camera.pitch);
    const view = multiply(
      perspective(
        camera.fov ?? 1.0,
        this.width / this.height,
        camera.near ?? 0.1,
        camera.far,
      ),
      lookAt(camera.eye, add(camera.eye, fwd), camera.up ?? [0, 1, 0]),
    );
    const night = env.mode === "night";
    this.frameGroup = this.environmentGroups.get(env.mode)!;
    this.skyGroup = this.frameGroup;
    const az = (env.sunAzimuth * Math.PI) / 180,
      el = (env.sunElevation * Math.PI) / 180;
    const sun: V3 = this.mmorpgWorld
      ? MMORPG_SUN_DIRECTION
      : normalize([
          Math.sin(az) * Math.cos(el),
          Math.sin(el),
          Math.cos(az) * Math.cos(el),
        ]);
    const key = `${env.sunAzimuth}:${env.sunElevation}:${env.mode}`;
    if (key !== this.lastEnvironment) {
      this.shadowDirty = true;
      this.lastEnvironment = key;
    }
    const horizontalForward: V3 =
      Math.hypot(fwd[0], fwd[2]) > 1e-10
        ? normalize([fwd[0], 0, fwd[2]])
        : [0, 0, -1];
    if (
      Math.hypot(...sub(camera.eye, this.lastShadowEye)) >
        this.detailSettings.shadowRefreshMeters ||
      dot(horizontalForward, this.lastShadowForward) <
        this.detailSettings.shadowRefreshDot
    )
      this.shadowDirty = true;
    if (this.shadowDirty) {
      this.shadowMatrix = fitDirectionalShadow(
        camera.eye,
        fwd,
        sun,
        SUN_SHADOW_FITS.near,
      ).matrix;
      this.farShadowMatrix = fitDirectionalShadow(
        camera.eye,
        fwd,
        sun,
        SUN_SHADOW_FITS.far,
      ).matrix;
      this.distantShadowMatrix = fitDirectionalShadow(
        camera.eye,
        fwd,
        sun,
        SUN_SHADOW_FITS.distant,
      ).matrix;
      this.lastShadowEye = [...camera.eye];
      this.lastShadowForward = horizontalForward;
    }
    const u = this.uniform;
    u.set([0, 0, 0, 0], FRAME_OFFSETS.clipPlane);
    u.set(
      [0, 1, 0, this.hasFlutter ? boundedFlutterTime(time) : 0],
      FRAME_OFFSETS.passSettings,
    );
    const skyDescription = this.environmentData?.scenarios[env.mode];
    const lightingDescription =
      this.lightingEnvironmentData?.scenarios[env.mode] ?? skyDescription;
    const sourceSunLux = lightingDescription?.sourceSunIlluminanceLux;
    if (
      sourceSunLux !== undefined &&
      (!Number.isFinite(sourceSunLux) || sourceSunLux <= 0)
    )
      throw new Error(
        "Prepare the lighting map with a positive source sun intensity.",
      );
    const skyLightingSunRatio =
      sourceSunLux === undefined
        ? 1
        : (night ? env.moonIntensity : env.sunIntensity) / sourceSunLux;
    u.set(
      [
        ...(lightingDescription?.tint ?? [1, 1, 1]),
        (lightingDescription?.brightness ?? 1) * SCENE_LINEAR_SCALE,
      ],
      FRAME_OFFSETS.iblGain,
    );
    const useSky =
      env.skySource === "kit" &&
      !!skyDescription &&
      this.environmentTextures.has(env.mode);
    u[FRAME_OFFSETS.skyEnabled] = useSky ? 1 : 0;
    const lighting = useSky ? this.ibl?.scenarios.get(env.mode) : undefined;
    const fractured = (this.destroyedPlacements.get(env.mode)?.size ?? 0) > 0;
    u.set(
      probeSettings(
        this.probes?.manifest(env.mode),
        !!lighting &&
          !!this.probes?.get(env.mode).scenarios.has(env.mode) &&
          this.cullingEnabled &&
          this.authoredDraws.length === 0 &&
          this.roomCount === 0,
        // Gameplay topology uses the live direct/IBL preview until its bake is rebuilt.
        env.mode,
      ),
      FRAME_OFFSETS.probes,
    );
    if (fractured) u.fill(0, FRAME_OFFSETS.probes, FRAME_OFFSETS.probes + 16);
    u[FRAME_OFFSETS.passSettings + 2] = Number(
      !!lighting &&
        !!this.surfaceSky?.has(env.mode) &&
        this.cullingEnabled &&
        this.authoredDraws.length === 0 &&
        this.roomCount === 0,
    );
    if (fractured) u[FRAME_OFFSETS.passSettings + 2] = 0;
    u.set(this.bounce!.settings(env.mode, sun), FRAME_OFFSETS.bounceSettings);
    if (fractured)
      u.fill(0, FRAME_OFFSETS.bounceSettings, FRAME_OFFSETS.bounceSettings + 4);
    u.set(
      [lighting ? 1 : 0, lighting?.mipMax ?? 0, env.ambientIntensity, 0],
      FRAME_OFFSETS.iblSettings,
    );
    u.set(
      [
        skyLightingSunRatio,
        Number(!!lighting?.hasConstantFog),
        Number(!!this.probes?.get(env.mode).hasConstantFog(env.mode)),
        Number(!!this.bounce?.hasConstantFog(env.mode)),
      ],
      FRAME_OFFSETS.skyLayers,
    );
    u.fill(0, FRAME_OFFSETS.iblConstantSH, FRAME_OFFSETS.iblConstantSH + 36);
    if (lighting) {
      u.set(lighting.sh, FRAME_OFFSETS.iblSH);
      u.set(lighting.constantSH, FRAME_OFFSETS.iblConstantSH);
      u.set(lighting.rotation, FRAME_OFFSETS.iblRotation);
    }
    if (skyDescription) {
      u.set(skyDescription.rotation, FRAME_OFFSETS.skyRotation);
      u.set(
        [
          skyDescription.uSign,
          skyDescription.uOffset,
          skyDescription.vSign,
          skyDescription.vOffset,
        ],
        FRAME_OFFSETS.skyUV,
      );
      u.set(
        [
          ...skyDescription.tint,
          skyDescription.brightness *
            SCENE_LINEAR_SCALE *
            (env.skyBackgroundIntensity ?? 1),
        ],
        FRAME_OFFSETS.skyTint,
      );
    }

    u.set(view, 0);
    u.set(inverse(view), 16);
    u.set(this.shadowMatrix, 32);
    u.set(this.farShadowMatrix, 336);
    u.set(this.distantShadowMatrix, FRAME_OFFSETS.distantShadow);
    u.set([...camera.eye, time], 48);
    u.set(
      [
        ...sun,
        (night ? env.moonIntensity : env.sunIntensity) * SCENE_LINEAR_SCALE,
      ],
      52,
    );
    u.set(night ? [0.22, 0.38, 0.75, 0] : [1, 0.94, 0.84, 0], 56);
    u.set(
      night
        ? [
            0.055 * env.ambientIntensity * SCENE_LINEAR_SCALE,
            0.09 * env.ambientIntensity * SCENE_LINEAR_SCALE,
            0.18 * env.ambientIntensity * SCENE_LINEAR_SCALE,
            0,
          ]
        : [
            0.65 * env.ambientIntensity * SCENE_LINEAR_SCALE,
            0.76 * env.ambientIntensity * SCENE_LINEAR_SCALE,
            0.94 * env.ambientIntensity * SCENE_LINEAR_SCALE,
            0,
          ],
      60,
    );
    const fogSun = this.look?.scenarios[env.mode].sun;
    const fogAmbient = fogSun
      ? fogAmbientRadiance(this.fogAmbientProfiles[env.mode], {
          skySource: env.skySource,
          sun: {
            direction: sun,
            color: lightColor(fogSun),
            illuminanceLux: night ? env.moonIntensity : env.sunIntensity,
          },
        })
      : [0, 0, 0];
    u.set(
      [
        ...SOURCE_FOG_INSCATTERING_LINEAR.map(
          (c, i) => (c + fogAmbient[i]) * SCENE_LINEAR_SCALE,
        ),
        env.fogDensity,
      ],
      64,
    );
    u.set(
      [env.exposure, env.saturation, env.roughness, SCENE_LINEAR_SCALE],
      68,
    );
    const volumeProfile = this.localFogProfiles.get(env.mode)!;
    u.set(
      [
        volumeProfile.extinctionAtReference / SOURCE_FOG_DENSITY_AT_REFERENCE,
        volumeProfile.heightRate,
        volumeProfile.referenceHeight,
        volumeProfile.startDistance,
      ],
      FRAME_OFFSETS.volumeMedium,
    );
    u.set(
      [
        volumeProfile.maxDistance,
        Number(volumeProfile.enabled && env.skySource === "kit"),
        0,
        0,
      ],
      FRAME_OFFSETS.volumeRange,
    );
    const fogTransport = {
      volumeMedium: Array.from(
        u.subarray(FRAME_OFFSETS.volumeMedium, FRAME_OFFSETS.volumeMedium + 4),
      ) as unknown as FogVolumeMedium,
      volumeRange: Array.from(
        u.subarray(FRAME_OFFSETS.volumeRange, FRAME_OFFSETS.volumeRange + 2),
      ) as unknown as FogVolumeRange,
    };
    u.set([env.cloudCoverage, env.cloudSpeed, env.skyIntensity, 0], 72);
    u.set(
      [night ? 1 : 0, env.moonIntensity, env.bloomStrength, env.normalStrength],
      76,
    );
    const effects = this.previewImpacts
      ? [
          ...(this.gameplayEffects.get(env.mode) ?? []),
          ...this.previewImpacts.snapshot(time, captureFrame),
        ]
      : (this.gameplayEffects.get(env.mode) ?? []);
    const transientLights = gameplayLights(effects);
    const sourceLights = transientLights.length
      ? [...(this.look?.scenarios[env.mode].lights ?? []), ...transientLights]
      : this.look?.scenarios[env.mode].lights;
    const lightRevision = gameplayLightRevision(env.mode, transientLights);
    const selectedLights = this.localLightSelection.select(
      sourceLights,
      camera.eye,
    );
    if (sourceLights) {
      u[63] = selectedLights.length;
      selectedLights.forEach((light, i) => {
        u.set([...light.position, light.range ?? 1], 80 + i * 4);
        u.set(
          [...lightColor(light), lightIntensity(light) * SCENE_LINEAR_SCALE],
          208 + i * 4,
        );
        u.set([...light.direction, light.kind === "spot" ? 1 : 0], 352 + i * 4);
        u.set(
          [
            Math.cos(light.innerConeAngleRadians ?? 0),
            Math.cos(light.outerConeAngleRadians ?? Math.PI / 4),
            -1,
            light.inverseSquaredFalloff === false
              ? (light.falloffExponent ?? 8)
              : -1,
          ],
          480 + i * 4,
        );
      });
    } else if (night) {
      const lamps = this.lamps
        .map((p) => ({ p, d: Math.hypot(...sub(p, camera.eye)) }))
        .sort((a, b) => a.d - b.d)
        .slice(0, 32);
      u[63] = lamps.length;
      lamps.forEach(({ p }, i) => {
        u.set([...p, 12], 80 + i * 4);
        u.set([1, 0.48, 0.12, 9 * SCENE_LINEAR_SCALE], 208 + i * 4);
        u.set([0, 0, 0, 0], 352 + i * 4);
        u.set([1, 0, -1, -1], 480 + i * 4);
      });
    } else u[63] = 0;
    this.prepareLocalShadows(selectedLights);
    for (let i = 0; i < selectedLights.length; i++)
      u[480 + i * 4 + 2] = this.localSlots.get(selectedLights[i].id) ?? -1;
    const shadowIndices = new Map(
      selectedLights.map((light, index) => [light.id, index]),
    );
    const clusters = this.clusteredLighting!.update(
      "main",
      {
        viewProjection: view,
        eye: camera.eye,
        forward: fwd,
        width: this.width,
        height: this.height,
        lights: sourceLights ?? [],
        shadowIndices,
      },
      lightRevision,
    );
    if (clusters.bindingChanged) this.rebuildFrameGroups();
    this.localViews.forEach((matrix, i) => {
      if (matrix) u.set(matrix, 608 + i * 16);
    });
    const authoredSun = this.look?.scenarios[env.mode].sun;
    if (authoredSun) u.set([...lightColor(authoredSun), 0], 56);
    if (this.mmorpgWorld) {
      u.set([...MMORPG_SUN_COLOR, 0], 56);
      u[55] = Math.PI * MMORPG_LIGHT_INTENSITY * SCENE_LINEAR_SCALE;
      u[FRAME_OFFSETS.skyLayers + 3] = camera.sourceFogScale ?? 1;
    } else u[FRAME_OFFSETS.skyLayers + 3] = 0;
    this.device.queue.writeBuffer(this.frameBuffer, 0, u);
    {
      const far = u.slice();
      far.set(this.farShadowMatrix, 32);
      this.device.queue.writeBuffer(this.farFrameBuffer, 0, far);
      const distant = u.slice();
      distant.set(this.distantShadowMatrix, 32);
      this.device.queue.writeBuffer(this.distantFrameBuffer, 0, distant);
    }
    this.device.queue.writeBuffer(
      this.postBuffer,
      0,
      new Float32Array([
        (this.mmorpgWorld ? 1 : env.exposure * OWNED_EXPOSURE[env.mode]) /
          SCENE_LINEAR_SCALE,
        this.mmorpgWorld ? 1 : env.saturation,
        this.mmorpgWorld ? 0 : env.bloomStrength,
        this.mmorpgWorld ? -1 : Number(env.skySource === "kit"),
        ...this.filmicProfiles.get(env.mode)!,
        ...displayLutParams(
          this.displayLuts.get(env.mode)?.descriptor,
          env.skySource === "kit",
        ),
        env.skySource === "kit" ? cameraVignetteIntensity(camera.vignette) : 0,
        this.width / this.height,
        0,
        0,
      ]),
    );
    if (this.skyDisplayBuffer)
      this.device.queue.writeBuffer(
        this.skyDisplayBuffer,
        0,
        new Float32Array([
          (env.exposure * OWNED_EXPOSURE[env.mode]) / SCENE_LINEAR_SCALE,
          env.saturation,
          0,
          Number(env.skySource === "kit"),
          ...this.filmicProfiles.get(env.mode)!,
          ...displayLutParams(
            this.displayLuts.get(env.mode)?.descriptor,
            env.skySource === "kit",
          ),
          env.skySource === "kit"
            ? cameraVignetteIntensity(camera.vignette)
            : 0,
          this.width / this.height,
          0,
          0,
        ]),
      );
    const diffuseGain =
      (lightingDescription?.brightness ?? 1) * env.ambientIntensity * u[74];
    for (let i = 0; i < 36; i++) {
      const tint = lightingDescription?.tint[i % 4] ?? 1;
      this.creatureSH[i] = lighting
        ? (lighting.sh[i] * skyLightingSunRatio + lighting.constantSH[i]) *
          diffuseGain *
          tint
        : 0;
    }
    if (this.mmorpgWorld) this.creatureSH.set(mmorpgHemisphereSH());
    let movingFrame: CharacterFrame | undefined;
    if (
      this.creatures ||
      this.leaves ||
      this.character ||
      this.enemies ||
      this.grass ||
      this.mmorpgCharacters
    ) {
      movingFrame = {
        sharedWind: this.liveEffects?.wind,
        mode: env.mode,
        time,
        viewProjection: u.subarray(0, 16),
        eye: camera.eye,
        sunDirection: sun,
        sunIlluminance: [u[56] * u[55], u[57] * u[55], u[58] * u[55]].map(
          (c) => c / SCENE_LINEAR_SCALE,
        ) as V3,
        sourceLighting: !!this.mmorpgWorld,
        shadowMatrix: this.shadowMatrix,
        diffuseRotation: this.mmorpgWorld
          ? this.creatureIdentity
          : (lighting?.rotation ?? this.creatureIdentity),
        diffuseSH: this.creatureSH,
        ...(lighting
          ? {
              specularEnvironment: {
                gain: [0, 1, 2].map(
                  (i) => diffuseGain * (lightingDescription?.tint[i] ?? 1),
                ),
                primaryScale: this.mmorpgWorld ? 0 : skyLightingSunRatio,
                constantScale: this.mmorpgWorld
                  ? 0
                  : lighting.hasConstantFog
                    ? 1
                    : 0,
              },
            }
          : {}),
        ambient: lighting
          ? [0, 0, 0]
          : ([u[60], u[61], u[62]].map((c) => c / SCENE_LINEAR_SCALE) as V3),
        fog: {
          ...(this.mmorpgWorld ? { profile: "quick-mmorpg-v1" as const, sourceScale: camera.sourceFogScale ?? 1 } : {}),
          density: env.fogDensity,
          color: [u[64], u[65], u[66]].map((c) => c / SCENE_LINEAR_SCALE) as V3,
          ...fogTransport,
        },
      };
    }
    if (movingFrame) {
      this.mmorpgCharacters?.prepare(movingFrame, this.mmorpgActors);
      this.mmorpgEffects?.prepare(
        movingFrame,
        this.mmorpgCharacters?.attachments ?? new Map(),
      );
    }
    const encodeStarted = this.profiling ? performance.now() : 0;
    let encoder = this.device.createCommandEncoder();
    const gpuSlot = this.profiling ? this.gpuTimer?.acquire() : undefined;
    encoder = this.gpuTimer?.instrumentEncoder?.(encoder, gpuSlot) ?? encoder;
    this.gpuTimer?.beginFrame?.(encoder, gpuSlot);
    if (movingFrame)
      this.mmorpgNames?.prepare(
        encoder,
        movingFrame,
        this.mmorpgActors,
        this.battleCrowd?.inspect().participant.active ? undefined : this.mmorpgLocalId,
      );
    this.mmorpgTerrain?.prepare(
      encoder,
      this.battleCrowd?.inspect().center ?? this.mmorpgInteractionPose?.position ?? camera.eye,
    );
    this.spellSwarmHost?.prepareRender(encoder, time, captureFrame);
    if (this.battleCrowd) {
      const elapsed = this.battleTime === undefined ? 1 / 60 : Math.max(0, time - this.battleTime);
      this.battleTime = time;
      this.battleCrowd.prepare(encoder, {
        viewProjection: view, eye: camera.eye, time, viewportHeight: this.height,
        sunDirection: movingFrame?.sunDirection,
        sunIlluminance: movingFrame?.sunIlluminance,
        ambient: [0.45, 0.52, 0.65],
        sourceFogScale: camera.sourceFogScale ?? 1,
      }, elapsed);
    }
    this.sourceDebris?.encodePacking(encoder);
    const windShadowPlan = this.renderWindShadows(encoder, env.mode, u, time);
    this.localDirtyViews.clear();
    this.shadowDirty = false;
    let planarReflection: PlanarReflectionResult | undefined;
    const planeY = this.primaryWaterPlane(view, camera.eye);
    if (this.planar && planeY !== undefined) {
      if (
        planarLightingChanged(this.planarLightingValues, u) ||
        this.planarLightingRevision === 0
      )
        this.planarLightingRevision++;
      const environmentSignature = JSON.stringify([
        env,
        useSky || env.cloudSpeed === 0 ? 0 : Math.floor(time * 4),
        variant?.wind.sourceCount ? time : 0,
        this.sourceDebris?.hasMoving ? this.gameplayAnimationRevision : 0,
      ]);
      if (environmentSignature !== this.planarEnvironmentSignature) {
        this.planarEnvironmentSignature = environmentSignature;
        this.planarEnvironmentRevision++;
      }
      planarReflection = this.planar.render(
        encoder,
        {
          viewProjection: view,
          eye: camera.eye,
          planeY,
          width: this.width,
          height: this.height,
          sceneRevision: this.geometryVersion,
          lightingRevision: this.planarLightingRevision,
          environmentRevision: this.planarEnvironmentRevision,
          // Custom WGSL can animate arbitrary geometry without declaring its bounds or clock use.
          cache: this.cullingEnabled,
        },
        (pass, context) => {
          const scissor =
            this.cullingEnabled &&
            this.features.planarScissor?.(
              view,
              context.viewProjection,
              this.waterBounds!.aabbs,
              this.waterPlaneHeights,
              context.planeY,
              context.width,
              context.height,
              PLANAR_WATER_DISTORTION,
              PLANAR_WATER_TOLERANCE,
            );
          if (scissor) pass.setScissorRect(...scissor);
          const mirrored = u.slice();
          mirrored.set(context.viewProjection, 0);
          mirrored.set(context.inverseViewProjection, 16);
          mirrored.set([...context.eye, time], 48);
          mirrored.set(context.clipPlane, FRAME_OFFSETS.clipPlane);
          mirrored.set(
            [
              1,
              0,
              u[FRAME_OFFSETS.passSettings + 2],
              u[FRAME_OFFSETS.passSettings + 3],
            ],
            FRAME_OFFSETS.passSettings,
          );
          this.device.queue.writeBuffer(this.planarFrameBuffer!, 0, mirrored);
          const clusters = this.clusteredLighting!.update(
            "planar",
            {
              viewProjection: context.viewProjection,
              eye: context.eye,
              forward: [fwd[0], -fwd[1], fwd[2]],
              width: context.width,
              height: context.height,
              lights: sourceLights ?? [],
              shadowIndices,
            },
            lightRevision,
          );
          if (clusters.bindingChanged) this.rebuildFrameGroups();
          const group = this.planarGroups.get(env.mode)!;
          const source = this.sourceSubmission(
            context.viewProjection,
            "planar",
            env.mode,
          );
          if (this.skyPipeline) {
            pass.setPipeline(this.skyPipeline);
            pass.setBindGroup(0, group);
            pass.setBindGroup(1, this.skyDisplayGroups.get(env.mode)!);
            pass.draw(3);
          }
          pass.setPipeline(this.planarPipeline);
          pass.setBindGroup(0, group);
          this.draw(pass, false, false, source, false, false, "planar", group);
          pass.setPipeline(this.planarBlendPipeline);
          this.draw(pass, true, false, source, false, false, "planar", group);
          pass.setPipeline(this.planarAdditivePipeline);
          this.draw(pass, true, true, source, false, false, "planar", group);
        },
      );
    }
    const cameraSource = this.sourceSubmission(view, "camera", env.mode);
    const depthPass = encoder.beginRenderPass({
      label: "Opaque depth prepass",
      colorAttachments: [],
      depthStencilAttachment: {
        view: this.depth.createView(),
        depthClearValue: 1,
        depthLoadOp: "clear",
        depthStoreOp: "store",
      },
    });
    depthPass.setPipeline(this.depthPipeline);
    depthPass.setBindGroup(0, this.shadowGroup);
    this.draw(
      depthPass,
      false,
      false,
      cameraSource,
      false,
      false,
      "depth",
      this.shadowGroup,
    );
    depthPass.end();
    this.surfaceDecals.get(env.mode)!.render(encoder, {
      width: this.width,
      height: this.height,
      depthView: this.depth.createView(),
      viewProjection: u.subarray(0, 16),
      inverseViewProjection: u.subarray(16, 32),
    });
    this.occlusion!.render(
      encoder,
      u.slice(0, 16),
      u.slice(16, 32),
      camera.eye,
    );
    if (movingFrame)
      this.grass?.prepare(
        encoder,
        movingFrame,
        this.mmorpgInteractionPose ?? this.characterPose,
      );
    const p = encoder.beginRenderPass({
      label: "Main HDR color",
      timestampWrites: this.gpuTimer?.writes(gpuSlot),
      colorAttachments: [
        {
          view: this.msaa.createView(),
          resolveTarget: this.hdr.createView(),
          loadOp: "clear",
          storeOp:
            this.water?.hasContent || this.sourceWaterStreams
              ? "store"
              : "discard",
          clearValue: [0, 0, 0, 1],
        },
      ],
      depthStencilAttachment: {
        view: this.depth.createView(),
        depthLoadOp: "load",
        depthStoreOp: "store",
      },
    });
    this.frameGroup = this.environmentGroups.get(env.mode)!;
    this.skyGroup = this.frameGroup;
    if (this.skyPipeline) {
      p.setPipeline(this.skyPipeline);
      p.setBindGroup(0, this.skyGroup);
      p.setBindGroup(1, this.skyDisplayGroups.get(env.mode)!);
      p.draw(3);
    }
    p.setPipeline(this.pipeline);
    p.setBindGroup(0, this.frameGroup);
    this.submittedGeometry = { instances: 0, triangles: 0, drawCalls: 0 };
    this.draw(
      p,
      false,
      false,
      cameraSource,
      false,
      true,
      "main",
      this.frameGroup,
    );
    if (this.battleCrowd) {
      this.battleCrowd.render(p);
      const battle = this.battleCrowd.inspect();
      this.submittedGeometry.drawCalls += battle.frameDraws;
      this.submittedGeometry.instances += battle.gpu?.visible ?? 0;
      this.submittedGeometry.triangles += battle.gpu?.triangles ?? 0;
      p.setBindGroup(0, this.frameGroup);
    }
    if (movingFrame) {
      this.creatures?.render(p, movingFrame);
      this.leaves?.render(p, movingFrame);
      this.grass?.render(
        p,
        movingFrame,
        this.mmorpgInteractionPose ?? this.characterPose,
      );
      this.mmorpgCharacters?.render(
        p,
        movingFrame,
        this.mmorpgActors,
        this.mmorpgLocalId,
      );
      if (this.characterPose)
        this.character?.render(p, movingFrame, this.characterPose);
      if (captureFrame && this.activeCaptureActors?.payload)
        this.enemies?.renderCapture(
          p,
          movingFrame,
          this.activeCaptureActors.payload,
        );
      else {
        this.enemies?.render(p, movingFrame, this.enemyActors, !captureFrame);
        if (!captureFrame && this.enemies) this.lastEnemyFrame = this.frame + 1;
      }
      this.mmorpgEffects?.render(p);
      this.mmorpgNames?.render(p);
      p.setBindGroup(0, this.frameGroup);
    }
    if (
      this.spellSwarmHost?.render(
        p,
        view,
        camera.eye,
        time,
        this.submittedGeometry,
      )
    )
      p.setBindGroup(0, this.frameGroup);
    p.setPipeline(this.blendPipeline);
    this.draw(
      p,
      true,
      false,
      cameraSource,
      false,
      true,
      "main",
      this.frameGroup,
    );
    p.setPipeline(this.additivePipeline);
    this.draw(
      p,
      true,
      true,
      cameraSource,
      false,
      true,
      "main",
      this.frameGroup,
    );
    if (this.meshEnabled && this.meshPipeline) {
      p.setPipeline(this.meshPipeline);
      this.draw(p, false, false, cameraSource, true);
      this.draw(p, true, false, cameraSource, true);
      this.draw(p, true, true, cameraSource, true);
    }
    this.spellEffects?.render(
      p,
      effects,
      view,
      camera.eye,
      fwd,
      this.gameplayAim,
      this.liveEffects
        ? (position, at) =>
            sampleWorldWind(position, at, this.liveEffects!.wind) as V3
        : undefined,
    );
    p.end();
    this.fogCards?.render(encoder, {
      mode: env.mode,
      time,
      viewProjection: u.subarray(0, 16),
      inverseViewProjection: u.subarray(16, 32),
      eye: camera.eye,
      forward: fwd,
      width: this.width,
      height: this.height,
      color: this.msaa.createView(),
      resolve: this.hdr.createView(),
      depth: this.fogCardDepthView!,
      fogMedium: [
        env.fogDensity,
        FOG_HEIGHT_RATE,
        FOG_REFERENCE_HEIGHT,
        FOG_START_DISTANCE,
      ],
      ...fogTransport,
    });
    if (this.water?.hasContent && this.waterHDR)
      this.water.render(encoder, {
        width: this.width,
        height: this.height,
        colorView: this.msaa.createView(),
        resolveTarget: this.waterHDR.createView(),
        depthView: this.depth.createView(),
        sceneColor: this.hdr.createView(),
        viewProjection: u.subarray(0, 16),
        inverseViewProjection: u.subarray(16, 32),
        eye: camera.eye,
        time,
        planar: planarReflection,
        sunDirection: sun,
        sunColor: [u[56] * u[55], u[57] * u[55], u[58] * u[55]],
        ambient: [u[60], u[61], u[62]],
        skyZenith: (night ? [0.003, 0.012, 0.036] : [0.12, 0.33, 0.65]).map(
          (c) => c * SCENE_LINEAR_SCALE * GENERATED_SKY_RADIANCE_SCALE,
        ) as V3,
        skyHorizon: (night ? [0.026, 0.055, 0.11] : [0.64, 0.78, 0.91]).map(
          (c) => c * SCENE_LINEAR_SCALE * GENERATED_SKY_RADIANCE_SCALE,
        ) as V3,
        fog: {
          color: [u[64], u[65], u[66]],
          density: env.fogDensity,
          ...fogTransport,
        },
        environment:
          lighting && lightingDescription
            ? {
                view: lighting.texture.createView({ dimension: "2d-array" }),
                worldToEnvironment: lighting.rotation,
                mipMax: lighting.mipMax,
                solarRatio: skyLightingSunRatio,
                hasConstantFog: lighting.hasConstantFog,
                gain: lightingDescription.tint.map(
                  (c) =>
                    c *
                    lightingDescription.brightness *
                    SCENE_LINEAR_SCALE *
                    env.skyIntensity *
                    env.ambientIntensity,
                ) as V3,
              }
            : undefined,
        lights: selectedLights
          .filter((light) => light.kind === "point")
          .slice(0, 16)
          .map((light) => ({
            position: light.position as V3,
            color: lightColor(light).map(
              (c) => c * lightIntensity(light) * SCENE_LINEAR_SCALE,
            ) as V3,
            range: light.range ?? 1,
            falloffExponent:
              light.inverseSquaredFalloff === false
                ? (light.falloffExponent ?? 8)
                : undefined,
          })),
      });
    this.sourceWaterStreams?.render(encoder, {
      mode: env.mode,
      time: ((time % 86400) + 86400) % 86400,
      width: this.width,
      height: this.height,
      colorView: this.msaa.createView(),
      refraction: {
        source: this.water?.hasContent ? this.waterHDR! : this.hdr,
        resolve: this.streamHDR!,
      },
      depthView: this.localFogViews.depth,
      viewProjection: u.subarray(0, 16),
      inverseViewProjection: u.subarray(16, 32),
      eye: camera.eye,
      effectiveExposure: env.exposure * OWNED_EXPOSURE[env.mode],
      lighting: {
        sunDirectionTowardLight: sun,
        sunRadianceRGB: [u[56] * u[55], u[57] * u[55], u[58] * u[55]].map(
          (c) => c / SCENE_LINEAR_SCALE,
        ),
        localLights: sourceLights,
        diffuseSH: this.creatureSH,
        diffuseRotation: lighting?.rotation ?? this.creatureIdentity,
      },
      environment:
        lighting && lightingDescription
          ? {
              view: lighting.texture.createView({ dimension: "2d-array" }),
              worldToEnvironment: lighting.rotation,
              mipMax: lighting.mipMax,
              solarRatio: skyLightingSunRatio,
              hasConstantFog: lighting.hasConstantFog,
              gain: lightingDescription.tint.map(
                (c) =>
                  c *
                  lightingDescription.brightness *
                  SCENE_LINEAR_SCALE *
                  env.skyIntensity *
                  env.ambientIntensity,
              ) as V3,
            }
          : undefined,
      fog: {
        density: env.fogDensity,
        color: [u[64], u[65], u[66]],
        ...fogTransport,
      },
    });
    const present = this.context.getCurrentTexture();
    const inheritedFog = this.localFogProfiles.get(env.mode)!;
    const fogProfile = this.mmorpgWorld
      ? { ...inheritedFog, enabled: false }
      : inheritedFog;
    const ambientInput = this.fogAmbientInputs.get(env.mode)!;
    const ambientEnabled =
      fogProfile.enabled &&
      ambientInput.profile.enabled &&
      useSky &&
      u[FRAME_OFFSETS.probes + 3] > 0.5;
    this.fogAmbientVolume!.render(encoder, {
      frameBuffer: this.frameBuffer,
      probeRecords:
        this.probes!.get(env.mode).scenarios.get(env.mode) ??
        this.probes!.get(env.mode).fallback,
      probeMoments: ambientInput.moments,
      enabled: ambientEnabled,
      phaseG: fogProfile.phaseG,
      scatteringIntensity: ambientInput.profile.scatteringIntensity,
      startDistance: fogProfile.startDistance,
      maxDistance: fogProfile.maxDistance,
    });
    this.localFog!.setLightScattering(
      sourceLights ?? [],
      clusters.data.sourceIndices,
      env.mode,
    );
    this.localFog!.render(encoder, {
      width: this.width,
      height: this.height,
      sceneColor: this.localFogViews.scene,
      depthView: this.localFogViews.depth,
      frameBuffer: this.frameBuffer,
      clusterBuffer: clusters.buffer,
      localShadowView: this.localFogViews.local,
      shadowSampler: this.comparison,
      profile: fogProfile,
      ambient: {
        view: this.fogAmbientVolume!.view,
        sampler: this.fogAmbientVolume!.sampler,
        enabled: ambientEnabled,
      },
      extinctionAtReference:
        (fogProfile.extinctionAtReference * env.fogDensity) /
        SOURCE_FOG_DENSITY_AT_REFERENCE,
      directional: {
        scatteringIntensity:
          typeof authoredSun?.raw.volumetric_scattering_intensity === "number"
            ? authoredSun.raw.volumetric_scattering_intensity
            : 1,
        nearShadowView: this.localFogViews.near,
        farShadowView: this.localFogViews.far,
        distantShadowView: this.localFogViews.distant,
      },
    });
    this.sourceWaterSplashes?.render(encoder, {
      mode: env.mode,
      time,
      width: this.width,
      height: this.height,
      colorView: this.localFog!.view,
      depthView: this.localFogViews.depth,
      viewProjection: u.subarray(0, 16),
      inverseViewProjection: u.subarray(16, 32),
      eye: camera.eye,
      effectiveExposure: env.exposure * OWNED_EXPOSURE[env.mode],
      fog: {
        density: env.fogDensity,
        color: [u[64], u[65], u[66]],
        ...fogTransport,
      },
    });
    this.particles?.render(encoder, {
      sharedWind: this.liveEffects?.wind,
      mode: env.mode,
      time,
      width: this.width,
      height: this.height,
      colorView: this.localFog!.view,
      depthView: this.localFogViews.depth,
      viewProjection: u.subarray(0, 16),
      inverseViewProjection: u.subarray(16, 32),
      eye: camera.eye,
      effectiveExposure: env.exposure * OWNED_EXPOSURE[env.mode],
      fog: {
        density: env.fogDensity,
        color: [u[64], u[65], u[66]],
        ...fogTransport,
      },
    });
    this.bloom!.render(encoder, {
      width: this.width,
      height: this.height,
      source: this.localFog!.view,
      enabled: env.bloomStrength > 0,
    });
    const output = encoder.beginRenderPass({
      label: "Display tone mapping",
      colorAttachments: [
        { view: this.displaySharpen!.view, loadOp: "clear", storeOp: "store" },
      ],
    });
    output.setPipeline(this.postPipeline);
    output.setBindGroup(0, this.postGroups.get(env.mode)!);
    output.draw(3);
    output.end();
    this.displaySharpen!.render(
      encoder,
      present.createView(),
      sourceSharpenStrength(this.look, env.mode),
    );
    if (this.captureBuffer) {
      encoder.copyTextureToBuffer(
        { texture: present },
        {
          buffer: this.captureBuffer,
          bytesPerRow: Math.ceil((this.width * 4) / 256) * 256,
        },
        [this.width, this.height],
      );
      this.captureBuffer = undefined;
    }
    this.gpuTimer?.endFrame?.(encoder, gpuSlot);
    this.gpuTimer?.resolve(encoder, gpuSlot);
    const observeCompletion = this.frameCompletions.recording;
    const submittedAt = observeCompletion ? performance.now() : 0;
    this.device.queue.submit([encoder.finish()]);
    this.submittedSourceCommands = this.sourceCommands;
    const submitReturnedAt = observeCompletion ? performance.now() : undefined;
    this.inFlight++;
    // Register completion before profiler mapAsync calls can delay capacity release.
    const workDone = this.device.queue.onSubmittedWorkDone();
    this.spellSwarmHost?.afterSubmit();
    this.battleCrowd?.afterSubmit();
    this.sourceDebris?.packingSubmitted();
    const completed = observeCompletion
      ? this.frameCompletions.submitted(
          this.frame + 1,
          submittedAt,
          submitReturnedAt,
        )
      : undefined;
    if (this.profiling)
      this.commandEncodeMs = performance.now() - encodeStarted;
    this.gpuTimer?.submitted(this.frame + 1, gpuSlot);
    this.windShadowPlanner.commit(windShadowPlan);
    planarReflection?.markSubmitted();
    void workDone.then(
      () => {
        this.inFlight--;
        completed?.(performance.now());
      },
      () => {
        this.inFlight--;
        completed?.(performance.now(), false);
        if (this.disposed) return;
        this.lost = true;
        this.planar?.invalidate();
      },
    );
    this.frame++;
    return true;
  }
  /** Static depth copies have fixed physical slots; no replacement can overlap pending GPU work. */
  private renderWindShadows(
    encoder: GPUCommandEncoder,
    mode: "day" | "night",
    uniform: Float32Array,
    time: number,
  ): WindShadowPlan {
    const views: Array<{
      id: string;
      texture: GPUTexture;
      layer: number;
      size: number;
      matrix: Float32Array;
      group: GPUBindGroup;
    }> = [
      {
        id: "nearSun",
        texture: this.shadow,
        layer: 0,
        size: 4096,
        matrix: this.shadowMatrix,
        group: this.shadowGroup,
      },
      {
        id: "farSun",
        texture: this.farShadow,
        layer: 0,
        size: 2048,
        matrix: this.farShadowMatrix,
        group: this.farShadowGroup,
      },
      {
        id: "distantSun",
        texture: this.farShadow,
        layer: 1,
        size: 2048,
        matrix: this.distantShadowMatrix,
        group: this.distantShadowGroup,
      },
    ];
    this.localViews.forEach((matrix, layer) => {
      if (!matrix) return;
      const values = uniform.slice();
      values.set(matrix, 32);
      this.device.queue.writeBuffer(this.localBuffers[layer], 0, values);
      views.push({
        id: "local:" + layer,
        texture: this.localShadow,
        layer,
        size: 512,
        matrix,
        group: this.localGroups[layer],
      });
    });
    if (this.mmorpgCharacters) {
      for (const view of views) {
        const pass = encoder.beginRenderPass({
          label: "MMO sunlight: " + view.id,
          colorAttachments: [],
          depthStencilAttachment: {
            view: view.texture.createView({
              dimension: "2d",
              baseArrayLayer: view.layer,
              arrayLayerCount: 1,
            }),
            depthLoadOp: "clear",
            depthStoreOp: "store",
            depthClearValue: 1,
          },
        });
        pass.setPipeline(this.shadowPipeline);
        pass.setBindGroup(0, view.group);
        this.draw(
          pass,
          false,
          false,
          this.sourceSubmission(view.matrix, view.id, mode),
          false,
          false,
          undefined,
          view.group,
        );
        this.mmorpgCharacters.renderShadow(pass, view.matrix, view.id);
        pass.end();
      }
      return this.windShadowPlanner.plan([]);
    }
    const variant = this.variants.get(mode)!;
    const inputs = views.map((view) => {
      const visible = frustumVisibility(variant.windBounds, view.matrix),
        casters: number[] = [];
      if (this.windVertexBounded)
        visible.forEach((v, i) => {
          if (
            v &&
            !this.destroyedPlacements.get(mode)?.has(variant.windIndices[i])
          )
            casters.push(variant.windIndices[i]);
        });
      const movingSources = this.windVertexBounded
        ? this.sourceDebris?.visibleMovingSources(view.matrix)
        : this.sourceDebris?.movingSourceIds;
      for (const index of movingSources ?? [])
        if (!casters.includes(index)) casters.push(index);
      casters.sort((a, b) => a - b);
      return {
        id: view.id,
        width: view.size,
        height: view.size,
        projection: view.matrix,
        // Arbitrary WGSL cannot claim the owned static/deformation contract.
        staticRevision: JSON.stringify([
          mode,
          this.geometryVersion,
          this.windShadowBounded ? 0 : time,
        ]),
        animationRevision: time + ":" + this.gameplayAnimationRevision,
        dynamicCasters: new Uint32Array(casters),
      };
    });
    const plan = this.windShadowPlanner.plan(inputs);
    for (const allocation of plan.allocations) {
      const previous = this.windShadowCopies.get(allocation.id);
      if (previous) {
        if (
          previous.width !== allocation.width ||
          previous.height !== allocation.height
        )
          throw Error(
            "Wind shadow slots cannot resize during a renderer lifetime.",
          );
        continue;
      }
      if (
        this.windShadowBytes + allocation.bytes >
          WIND_SHADOW_LIMITS.staticBytes ||
        this.windShadowCopies.size >= WIND_SHADOW_LIMITS.views
      )
        throw Error("Wind shadow copies exceed the 136 MiB lifetime budget.");
      const texture = this.device.createTexture({
        label: "Static wind shadow: " + allocation.id,
        size: [allocation.width, allocation.height],
        format: "depth32float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      this.windShadowCopies.set(allocation.id, texture);
      this.windShadowBytes += allocation.bytes;
      this.gpuBytes += allocation.bytes;
    }
    // Retain removed fixed slots for reuse. This avoids deferred-retirement overlap across mode/light edits.
    for (const work of plan.views) {
      const view = views.find((v) => v.id === work.id)!;
      if (
        !work.renderStatic &&
        !work.restoreStatic &&
        !work.dynamicCasterIndices.length
      )
        continue;
      const source = this.sourceSubmission(view.matrix, view.id, mode);
      if (work.renderStatic) {
        const target =
          work.renderStatic === "cache"
            ? this.windShadowCopies.get(view.id)!
            : view.texture;
        const pass = encoder.beginRenderPass({
          label: "Static shadow: " + view.id,
          colorAttachments: [],
          depthStencilAttachment: {
            view: target.createView({
              dimension: "2d",
              baseArrayLayer: work.renderStatic === "cache" ? 0 : view.layer,
              arrayLayerCount: 1,
            }),
            depthLoadOp: "clear",
            depthStoreOp: "store",
            depthClearValue: 1,
          },
        });
        pass.setPipeline(this.shadowPipeline);
        pass.setBindGroup(0, view.group);
        this.draw(
          pass,
          false,
          false,
          {
            ...source,
            shadowPartition: work.renderStatic === "cache" ? "static" : "all",
            debris: "standing",
          },
          false,
          false,
          undefined,
          view.group,
        );
        pass.end();
      }
      if (work.restoreStatic)
        encoder.copyTextureToTexture(
          {
            texture: this.windShadowCopies.get(view.id)!,
            aspect: "depth-only",
          },
          {
            texture: view.texture,
            origin: [0, 0, view.layer],
            aspect: "depth-only",
          },
          [view.size, view.size, 1],
        );
      if (work.dynamicCasterIndices.length) {
        const pass = encoder.beginRenderPass({
          label: "Moving shadow: " + view.id,
          colorAttachments: [],
          depthStencilAttachment: {
            view: view.texture.createView({
              dimension: "2d",
              baseArrayLayer: view.layer,
              arrayLayerCount: 1,
            }),
            depthLoadOp: "load",
            depthStoreOp: "store",
          },
        });
        pass.setPipeline(this.shadowPipeline);
        pass.setBindGroup(0, view.group);
        this.draw(
          pass,
          false,
          false,
          {
            ...source,
            shadowPartition: "moving",
            dynamicOnly: true,
            debris: "moving",
          },
          false,
          false,
          undefined,
          view.group,
        );
        pass.end();
      }
    }
    return plan;
  }
  private sourceSubmission(
    matrix: Float32Array,
    key: string,
    mode: string,
  ): SourceSubmission {
    const start = this.profiling ? performance.now() : 0;
    try {
      return this.prepareSourceSubmission(matrix, key, mode);
    } finally {
      if (this.profiling) this.cullCpuMs += performance.now() - start;
    }
  }
  private prepareSourceSubmission(
    matrix: Float32Array,
    key: string,
    mode: string,
  ): SourceSubmission {
    const variant = this.variants.get(mode)!;
    const variantDraws =
      this.canonicalDraws &&
      (key === "camera" || key === "planar") &&
      this.activeShaderSource === this.shaders.MATERIAL
        ? (this.canonicalDraws?.get(mode) ?? variant.draws)
        : variant.draws;
    const canonical = variantDraws !== variant.draws;
    const damaged = this.destroyedPlacements.get(mode);
    const viewKey = mode + ":" + key;
    const previous = this.culledSources.get(viewKey);
    const compiled =
      this.compiledSourceHost &&
      this.cullingEnabled &&
      (key === "camera" || key === "planar") &&
      !variant.scene.mmorpgURL &&
      this.activeShaderSource === this.shaders.MATERIAL
        ? this.compiledSourceHost.prepare(
            variantDraws,
            variant.bounds.count,
            this.renderMaterialInfo,
            variant.scene.materials.length,
            Math.min(262_144, Math.floor(this.zeroWind.size / 16)),
            previous?.compiled,
            this.geometryVersion,
          )
        : undefined;
    const plan = compiled?.plan;
    const tableDraws = plan?.baseDraws ?? variantDraws;
    if (!this.cullingEnabled && !damaged?.size)
      return {
        instances: this.instances,
        wind: variant.windBuffer,
        draws: canonical ? variantDraws : this.draws,
        matrix,
      };
    const signature =
      mode +
      (canonical ? ":canonical:" : ":") +
      (plan ? "compiled:" : "source:") +
      this.geometryVersion +
      ":" +
      (this.cullingEnabled ? matrix.join(",") : "all");
    const bundled =
      this.sourceBundlesEnabled &&
      (key === "camera" ||
        key === "planar" ||
        isSunShadowBundleView(
          key,
          Boolean(
            this.shaders && this.activeShaderSource === this.shaders.MATERIAL,
          ),
        )) &&
      tableDraws.length <= MAX_BUNDLED_SOURCE_DRAWS;
    if (
      previous?.signature === signature &&
      previous.sourceTable === tableDraws &&
      previous.compiled === compiled
    ) {
      previous.matrix = matrix;
      return previous;
    }
    if (previous) previous.signature = "";
    let compacted: { models: Float32Array; draws: Draw[] },
      parameters: Float32Array,
      drawPool: SourceSubmission["drawPool"];
    const gather = (visibility: Uint8Array) => {
      const result = compiled!.gather({
        models: variant.models,
        wind: variant.wind.parameters,
        visibility,
        excluded: damaged,
        sourceVersion: this.geometryVersion,
      });
      return {
        compacted: { models: result.models, draws: result.visibleDraws },
        parameters: result.wind,
      };
    };
    const compactVisible = (visibility: Uint8Array) => {
      if (damaged) maskDestroyedPlacements(visibility, damaged);
      const result = compactInstanceDraws(
        variant.models,
        variantDraws,
        visibility,
      );
      const wind = new Float32Array(result.models.length / 4);
      let rank = 0;
      visibility.forEach((visible, i) => {
        if (visible)
          wind.set(
            variant.wind.parameters.subarray(i * 4, i * 4 + 4),
            rank++ * 4,
          );
      });
      return { compacted: result, parameters: wind };
    };
    const core = this.experiment.framePreparation ?? this.residentSource;
    if (core && this.cullingEnabled) {
      let resident = this.frameScenes.get(mode);
      if (!resident) {
        const source = () => ({
          bounds: variant.bounds,
          models: variant.models,
          draws: variant.draws,
          wind: variant.wind.parameters,
        });
        resident = core.prepare(source, this.geometryVersion);
        this.frameScenes.set(mode, resident);
      } else if (resident.version !== this.geometryVersion) {
        resident.update();
        resident.version = this.geometryVersion;
      }
      const maskedResident = resident as typeof resident & {
        cullMasked?: (
          matrix: Float32Array,
          excluded: ReadonlySet<number>,
        ) => ReturnType<NonNullable<typeof resident>["cull"]>;
      };
      const result =
        damaged?.size && maskedResident.cullMasked
          ? maskedResident.cullMasked(matrix, damaged)
          : resident.cull(matrix);
      if (compiled) {
        ({ compacted, parameters } = gather(result.visibility));
      } else if (damaged?.size && !maskedResident.cullMasked) {
        // Backends without masked compaction retain the established fallback.
        ({ compacted, parameters } = compactVisible(result.visibility));
      } else {
        if (key === "camera" || key === "planar") {
          drawPool = previous?.drawPool;
          if (
            drawPool?.base !== variantDraws ||
            drawPool.version !== this.geometryVersion
          )
            drawPool = {
              base: variantDraws,
              version: this.geometryVersion,
              records: [],
              visible: [],
            };
        }
        const draws = drawPool?.visible ?? [];
        draws.length = 0;
        variantDraws.forEach((draw, i) => {
          const count = result.drawRanges[i * 2 + 1];
          if (count) {
            const record = drawPool
              ? (drawPool.records[i] ??= { ...draw })
              : { ...draw };
            record.firstInstance = result.drawRanges[i * 2];
            record.count = count;
            draws.push(record);
          }
        });
        compacted = { models: result.models, draws };
        parameters = result.wind;
      }
    } else {
      const visibility = this.cullingEnabled
        ? frustumVisibility(variant.bounds, matrix)
        : new Uint8Array(variant.bounds.count).fill(1);
      ({ compacted, parameters } = compiled
        ? gather(visibility)
        : compactVisible(visibility));
    }
    // Bundle vertex bindings remain stable as visibility grows and shrinks.
    const sourceBuffer = (
      field: "instances" | "wind",
      data: Float32Array,
      capacity: number,
      label: string,
    ) => {
      const required = Math.max(4, bundled ? capacity : data.byteLength);
      let buffer = previous?.[field];
      if (!buffer || buffer.size < required) {
        const replacement = this.device.createBuffer({
          label,
          size: required,
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
        this.releaseBuffer(buffer);
        buffer = replacement;
        if (previous) previous[field] = buffer;
        this.buffers.add(buffer);
        this.gpuBytes += buffer.size;
        this.preparedBufferUploadBytes += data.byteLength;
      }
      if (data.byteLength) this.device.queue.writeBuffer(buffer, 0, data);
      return buffer;
    };
    const instances = sourceBuffer(
      "instances",
      compacted.models,
      bundled
        ? (plan?.capacity ?? variant.bounds.count) * 16 * 4
        : compacted.models.byteLength,
      "Visible instances: " + viewKey,
    );
    const wind = sourceBuffer(
      "wind",
      parameters,
      bundled
        ? (plan?.capacity ?? variant.bounds.count) * 4 * 4
        : parameters.byteLength,
      "Visible wind inputs: " + viewKey,
    );
    let bundles = previous?.bundles;
    if (!bundled || bundles?.draws !== tableDraws) {
      bundles?.dispose();
      bundles = bundled
        ? new SourceRenderBundles(
            this.device,
            tableDraws as readonly BundledSourceDraw[],
            (delta) => {
              this.gpuBytes += delta;
            },
          )
        : undefined;
    }
    bundles?.update(compacted.draws as BundledSourceDraw[]);
    const result = {
      signature,
      sourceTable: tableDraws,
      compiled,
      instances,
      wind,
      draws: compacted.draws,
      matrix,
      bundles,
      bundleDraws: compacted.draws,
      drawPool,
    };
    if (previous?.compiled && previous.compiled !== compiled)
      previous.compiled.dispose();
    this.culledSources.set(viewKey, result);
    return result;
  }
  /** Frustum overlap is conservative; this does not claim occlusion-tested basin selection. */
  private primaryWaterPlane(
    viewProjection: Float32Array,
    eye: V3,
  ): number | undefined {
    if (!this.waterBounds?.count) return undefined;
    const visible = frustumVisibility(this.waterBounds, viewProjection);
    let best = Infinity,
      selected: number | undefined;
    for (let i = 0; i < visible.length; i++) {
      const y = this.waterPlaneHeights[i];
      if (!visible[i] || eye[1] <= y + 0.01) continue;
      const b = this.waterBounds.aabbs,
        offset = i * 6;
      const distance = Math.hypot(
        Math.max(b[offset] - eye[0], 0, eye[0] - b[offset + 3]),
        eye[1] - y,
        Math.max(b[offset + 2] - eye[2], 0, eye[2] - b[offset + 5]),
      );
      if (distance < best) {
        best = distance;
        selected = y;
      }
    }
    return selected;
  }
  private prepareLocalShadows(lights: SourceLight[]) {
    const points = lights
      .filter(
        (light) =>
          light.kind === "point" &&
          light.castShadows &&
          (light.range ?? 0) > 0.1,
      )
      .slice(0, LOCAL_POINT_SHADOWS);
    const spots = lights
      .filter((light) => light.kind === "spot" && light.castShadows)
      .slice(0, LOCAL_SPOT_SHADOWS);
    const chosen = [...points, ...spots];
    const wanted = new Map(chosen.map((light) => [light.id, light]));
    for (const [id, slot] of this.localSlots) {
      const light = wanted.get(id);
      if (
        light &&
        (light.kind === "point"
          ? slot < SPOT_SHADOW_START && slot % 6 === 0
          : slot >= SPOT_SHADOW_START)
      )
        continue;
      this.localSlots.delete(id);
      this.localSignatures.delete(id);
      for (
        let layer = slot;
        layer < slot + (slot < SPOT_SHADOW_START ? 6 : 1);
        layer++
      ) {
        this.localViews[layer] = undefined;
        this.localDirtyViews.delete(layer);
      }
    }
    const occupied = new Set(this.localSlots.values());
    const faces: [V3, V3][] = [
      [
        [1, 0, 0],
        [0, -1, 0],
      ],
      [
        [-1, 0, 0],
        [0, -1, 0],
      ],
      [
        [0, 1, 0],
        [0, 0, 1],
      ],
      [
        [0, -1, 0],
        [0, 0, -1],
      ],
      [
        [0, 0, 1],
        [0, -1, 0],
      ],
      [
        [0, 0, -1],
        [0, -1, 0],
      ],
    ];
    for (const light of chosen) {
      let slot = this.localSlots.get(light.id);
      if (slot === undefined) {
        slot = light.kind === "point" ? 0 : SPOT_SHADOW_START;
        while (occupied.has(slot)) slot += light.kind === "point" ? 6 : 1;
        this.localSlots.set(light.id, slot);
        occupied.add(slot);
      }
      const signature = JSON.stringify([
        this.geometryVersion,
        light.kind,
        light.position,
        light.range,
        ...(light.kind === "spot"
          ? [light.direction, light.outerConeAngleRadians]
          : []),
      ]);
      if (signature === this.localSignatures.get(light.id)) continue;
      this.localSignatures.set(light.id, signature);
      const position = light.position as V3,
        far = Math.max(0.2, light.range ?? 10);
      if (light.kind === "point") {
        faces.forEach(([direction, up], face) => {
          this.localViews[slot! + face] = multiply(
            perspective(Math.PI / 2, 1, 0.05, far),
            lookAt(position, add(position, direction), up),
          );
          this.localDirtyViews.add(slot! + face);
        });
      } else {
        const direction = normalize(light.direction as V3);
        this.localViews[slot] = multiply(
          perspective(
            Math.min(
              3,
              Math.max(0.1, 2 * (light.outerConeAngleRadians ?? Math.PI / 4)),
            ),
            1,
            0.05,
            far,
          ),
          lookAt(
            position,
            add(position, direction),
            Math.abs(direction[1]) > 0.98 ? [0, 0, 1] : [0, 1, 0],
          ),
        );
        this.localDirtyViews.add(slot);
      }
    }
  }
  private draw(
    pass: GPURenderPassEncoder,
    transparent: boolean,
    additive = false,
    source?: SourceSubmission,
    wire = false,
    countSubmission = false,
    specialized?: "main" | "planar" | "depth",
    frameGroup?: GPUBindGroup,
  ) {
    this.sourceCommands ??= {
      draws: 0,
      pipelines: 0,
      materials: 0,
      windBindings: 0,
    };
    pass.setVertexBuffer(0, this.vertices);
    pass.setVertexBuffer(1, source?.instances ?? this.instances);
    pass.setVertexBuffer(2, this.vertexColors);
    pass.setVertexBuffer(3, this.vertexUV1);
    pass.setIndexBuffer(wire ? this.meshIndices! : this.indices, "uint32");
    const include = (draw: { material: number; wind?: boolean }) => {
      const material = this.renderMaterialInfo[draw.material];
      return (
        Boolean(material.flame) === additive &&
        (additive ||
          (Boolean(material.glass) || material.alphaMode === "BLEND") ===
            transparent) &&
        sunShadowPartitionIncludes(draw, source?.shadowPartition)
      );
    };
    const shadowBundle =
      !wire &&
      !specialized &&
      source?.shadowPartition &&
      this.sourceBundlesEnabled
        ? executeSunShadowSourceBundle(pass, source, {
            enabled: this.sourceBundlesEnabled,
            stockShader: this.activeShaderSource === this.shaders.MATERIAL,
            frameGroup,
            pipeline: this.shadowPipeline,
            materials: this.materials,
            vertices: this.vertices,
            colors: this.vertexColors,
            uv1: this.vertexUV1,
            zeroWind: this.zeroWind,
            indices: this.indices,
            include,
          })
        : undefined;
    if (shadowBundle !== undefined) {
      this.sourceBundleExecutions += Number(shadowBundle);
      if (shadowBundle)
        this.countSourceCommands(source!.bundles!.commandCounts());
    } else if (
      !wire &&
      specialized &&
      frameGroup &&
      source?.bundles &&
      source.bundleDraws === source.draws &&
      !source.dynamicOnly
    ) {
      const bundle = source.bundles.get({
        key: specialized + ":" + Number(transparent) + ":" + Number(additive),
        descriptor: {
          colorFormats: specialized === "depth" ? [] : ["rgba16float"],
          depthStencilFormat: "depth24plus",
          sampleCount: 4,
        },
        frameGroup,
        shader: this.pipeline,
        pipelineVersion: this.stockPipelines.size,
        materials: this.materials,
        vertices: this.vertices,
        instances: source.instances,
        colors: this.vertexColors,
        uv1: this.vertexUV1,
        wind: source.wind,
        zeroWind: this.zeroWind,
        indices: this.indices,
        include,
        material: (draw) => this.materials[draw.material],
        pipeline: (draw) =>
          this.materialPipeline(
            this.materials[draw.material],
            transparent,
            additive,
            specialized,
            draw.rasterOrientation,
          ),
      });
      if (bundle.length) {
        pass.executeBundles(bundle);
        this.sourceBundleExecutions++;
        this.countSourceCommands(source.bundles.commandCounts());
      }
      // executeBundles clears pipeline, groups, and vertex/index bindings.
      pass.setBindGroup(0, frameGroup);
      pass.setVertexBuffer(0, this.vertices);
      pass.setVertexBuffer(1, source.instances);
      pass.setVertexBuffer(2, this.vertexColors);
      pass.setVertexBuffer(3, this.vertexUV1);
      pass.setIndexBuffer(this.indices, "uint32");
      if (countSubmission)
        for (const draw of source.draws)
          if (include(draw)) {
            this.submittedGeometry.instances += draw.count;
            this.submittedGeometry.triangles +=
              (draw.indexCount * draw.count) / 3;
            this.submittedGeometry.drawCalls++;
          }
    } else {
      let previous = -1;
      let previousOrientation: RasterOrientation | undefined;
      let previousWind: GPUBuffer | undefined;
      let previousPipeline: GPURenderPipeline | undefined;
      let previousMaterial: GPUBindGroup | undefined;
      for (const draw of source?.draws ?? this.draws) {
        if (!include(draw)) continue;
        const wind = draw.wind
          ? (source?.wind ?? this.zeroWind)
          : this.zeroWind;
        if (wind !== previousWind) {
          pass.setVertexBuffer(4, wind);
          this.sourceCommands.windBindings++;
          previousWind = wind;
        }
        if (
          draw.material !== previous ||
          draw.rasterOrientation !== previousOrientation
        ) {
          const material = this.materials[draw.material];
          if (specialized) {
            const pipeline = this.materialPipeline(
              material,
              transparent,
              additive,
              specialized,
              draw.rasterOrientation,
            );
            if (!previousPipeline || pipeline !== previousPipeline) {
              pass.setPipeline(pipeline);
              this.sourceCommands.pipelines++;
              previousPipeline = pipeline;
            }
          }
          if (!previousMaterial || material !== previousMaterial) {
            pass.setBindGroup(1, material);
            this.sourceCommands.materials++;
            previousMaterial = material;
          }
          previous = draw.material;
          previousOrientation = draw.rasterOrientation;
        }
        this.sourceCommands.draws++;
        pass.drawIndexed(
          draw.indexCount * (wire ? 2 : 1),
          draw.count,
          (wire
            ? (draw.originalFirstIndex ?? draw.firstIndex)
            : draw.firstIndex) * (wire ? 2 : 1),
          draw.baseVertex,
          draw.firstInstance,
        );
        if (countSubmission) {
          this.submittedGeometry.instances += draw.count;
          this.submittedGeometry.triangles +=
            (draw.indexCount * draw.count) / 3;
          this.submittedGeometry.drawCalls++;
        }
      }
    }
    if (this.authoredInstances && !source?.dynamicOnly) {
      pass.setVertexBuffer(4, this.zeroWind);
      pass.setVertexBuffer(1, this.authoredInstances);
      let previousPipeline: GPURenderPipeline | undefined;
      let previousMaterial: GPUBindGroup | undefined;
      for (const draw of this.authoredDraws) {
        if (
          Boolean(this.renderMaterialInfo[draw.material].flame) !== additive ||
          (!additive &&
            (Boolean(this.renderMaterialInfo[draw.material].glass) ||
              this.renderMaterialInfo[draw.material].alphaMode === "BLEND") !==
              transparent)
        )
          continue;
        const material = this.materials[draw.material];
        if (specialized) {
          const pipeline = this.materialPipeline(
            material,
            transparent,
            additive,
            specialized,
            draw.rasterOrientation,
          );
          if (!previousPipeline || pipeline !== previousPipeline) {
            pass.setPipeline(pipeline);
            previousPipeline = pipeline;
          }
        }
        if (!previousMaterial || material !== previousMaterial) {
          pass.setBindGroup(1, material);
          previousMaterial = material;
        }
        pass.drawIndexed(
          draw.indexCount * (wire ? 2 : 1),
          draw.count,
          (wire
            ? (draw.originalFirstIndex ?? draw.firstIndex)
            : draw.firstIndex) * (wire ? 2 : 1),
          draw.baseVertex,
          draw.firstInstance,
        );
        if (countSubmission) {
          this.submittedGeometry.instances += draw.count;
          this.submittedGeometry.triangles +=
            (draw.indexCount * draw.count) / 3;
          this.submittedGeometry.drawCalls++;
        }
      }
    }
    if (
      !transparent &&
      !additive &&
      !source?.dynamicOnly &&
      this.roomCount &&
      this.roomVertices &&
      this.roomIndices &&
      this.roomInstances &&
      this.roomMaterial
    ) {
      pass.setVertexBuffer(4, this.zeroWind);
      pass.setVertexBuffer(0, this.roomVertices);
      pass.setVertexBuffer(1, this.roomInstances);
      pass.setVertexBuffer(2, this.roomColors!);
      pass.setVertexBuffer(3, this.roomUV1!);
      pass.setIndexBuffer(
        wire ? this.meshRoomIndices! : this.roomIndices,
        "uint32",
      );
      pass.setBindGroup(1, this.roomMaterial);
      if (specialized)
        pass.setPipeline(
          this.materialPipeline(this.roomMaterial, false, false, specialized),
        );
      pass.drawIndexed(wire ? 72 : 36, this.roomCount);
      if (countSubmission) {
        this.submittedGeometry.instances += this.roomCount;
        this.submittedGeometry.triangles += this.roomCount * 12;
        this.submittedGeometry.drawCalls++;
      }
    }
    if (!wire && this.sourceDebris && source?.debris !== "none") {
      if (specialized)
        pass.setPipeline(
          specialized === "depth"
            ? this.depthPipeline
            : specialized === "main"
              ? additive
                ? this.additivePipeline
                : transparent
                  ? this.blendPipeline
                  : this.pipeline
              : additive
                ? this.planarAdditivePipeline
                : transparent
                  ? this.planarBlendPipeline
                  : this.planarPipeline,
        );
      const include = (index: number) =>
        Boolean(this.renderMaterialInfo[index].flame) === additive &&
        (additive ||
          (Boolean(this.renderMaterialInfo[index].glass) ||
            this.renderMaterialInfo[index].alphaMode === "BLEND") ===
            transparent);
      if (source?.debris !== "moving")
        this.sourceDebris.drawStanding(
          pass,
          this.materials,
          this.zeroWind,
          include,
          this.cullingEnabled ? source?.matrix : undefined,
          specialized
            ? (index) =>
                this.materialPipeline(
                  this.materials[index],
                  transparent,
                  additive,
                  specialized,
                  "degenerate",
                )
            : undefined,
        );
      if (source?.debris !== "standing")
        this.sourceDebris.drawMoving(
          pass,
          this.materials,
          this.zeroWind,
          include,
          this.cullingEnabled ? source?.matrix : undefined,
          specialized
            ? (index) =>
                this.materialPipeline(
                  this.materials[index],
                  transparent,
                  additive,
                  specialized,
                  "degenerate",
                )
            : undefined,
        );
    }
  }
  private async prepareCreatorMaterials(
    snapshot: any,
  ): Promise<MaterialEditorPlan> {
    if (this.disposed) throw new Error("The world closed.");
    const editor = this.editMaterials();
    if (editor) return editor.prepare(snapshot);
    if (savedMaterialDocuments(snapshot).length)
      throw new Error(
        "Open this world in the editor to use its face materials.",
      );
    return IDENTITY_MATERIALS;
  }
  creatorImageProvider?: (document: MaterialDocument) => MaterialBundle;
  private structureKey(snapshot: any) {
    return JSON.stringify([
      snapshot.rooms,
      snapshot.buildings,
      snapshot.wallColliders,
      snapshot.supports,
      snapshot.ramps,
      snapshot.removedWalls,
      (snapshot.entities ?? []).filter((e: any) => e.buildingPlan),
      savedMaterialDocuments(snapshot),
    ]);
  }
  private async buildIncrementalAuthored(snapshot: any, plan: PropPlan) {
    const entities = new Map<string, any>(
      (snapshot.entities ?? []).map((e: any) => [e.id, e]),
    );
    const draws = new Map(this.authoredEntityDraws);
    const kinds = new Map(this.authoredKinds);
    for (const removed of plan.removed) {
      draws.delete(removed.id);
      kinds.delete(removed.id);
    }
    for (const entity of entities.values()) {
      if (kinds.get(entity.id) === entity.kind) continue;
      // Geometry and face bindings remain resident; only this object's draw references change.
      const mesh =
        this.scene.meshes[this.authoredMeshIndices!.get(entity.kind)!];
      const remap = (material: number) =>
        this.materialEditor?.remap(entity.id, material) ?? material;
      draws.set(
        entity.id,
        mesh.primitives.map((p) => ({
          firstIndex: p.firstIndex,
          indexCount: p.indexCount,
          baseVertex: p.baseVertex,
          firstInstance: plan.handle(entity.id)!.slot,
          count: 1,
          material: remap(p.material),
        })),
      );
      kinds.set(entity.id, entity.kind);
    }
    const growth =
      !this.authoredInstances ||
      this.authoredInstances.size < plan.capacity * 64;
    const changedMatrixCount = plan.writes.length;
    const replacement = growth
      ? this.buffer(
          plan.matrixRecords(),
          GPUBufferUsage.VERTEX,
          "Authored object instances",
        )
      : undefined;
    let used = false;
    return {
      ready: () => !used && plan.ready(),
      activate: () => {
        if (used || !plan.ready())
          throw new Error("A newer edit replaced this update.");
        used = true;
        if (replacement) {
          this.releaseBuffer(this.authoredInstances);
          this.authoredInstances = replacement;
        } else {
          for (const write of plan.writes)
            this.device.queue.writeBuffer(
              this.authoredInstances!,
              write.slot * 64,
              write.matrix,
            );
        }
        plan.activate();
        this.authoredBytesUploaded += replacement
          ? replacement.size
          : changedMatrixCount * 64;
        this.authoredIncrementalUpdates++;
        this.authoredEntityDraws = draws;
        this.authoredKinds = kinds;
        this.authoredDraws = [...entities.keys()].flatMap(
          (id) => draws.get(id) ?? [],
        );
        this.geometryVersion++;
        this.shadowDirty = true;
      },
      discard: () => {
        if (used) return;
        used = true;
        this.releaseBuffer(replacement);
        plan.discard();
      },
    };
  }
  private authoredMeshIndices?: Map<string, number>;
  async prepareAuthored(
    snapshot: any,
    assetMeshes: Map<string, number>,
    catalog = new Map<string, CatalogAsset>(),
  ) {
    this.device.pushErrorScope("out-of-memory");
    this.device.pushErrorScope("validation");
    let candidate: Awaited<ReturnType<Renderer["buildAuthored"]>> | undefined;
    let plan: PropPlan | undefined;
    let failure: unknown;
    try {
      plan = this.propTable.prepare(
        (snapshot.entities ?? []).map((entity: any) =>
          entity.buildingPlan ? { ...entity, collidable: false } : entity,
        ),
      ); // Building collision comes from its rooms and openings, not a solid prop box.
      this.authoredMeshIndices = assetMeshes;
      const key = this.structureKey(snapshot);
      candidate =
        import.meta.env?.VITE_PROP_UPDATES !== "full" &&
        this.authoredStructure === key
          ? await this.buildIncrementalAuthored(snapshot, plan)
          : await this.buildAuthored(snapshot, assetMeshes, catalog, plan, key);
    } catch (error) {
      failure = error;
    }
    const validation = await this.device.popErrorScope();
    const memory = await this.device.popErrorScope();
    if (failure || validation || memory) {
      candidate?.discard();
      plan?.discard();
      throw failure ?? new Error((validation ?? memory)!.message);
    }
    return candidate!;
  }
  private async buildAuthored(
    snapshot: any,
    assetMeshes: Map<string, number>,
    catalog = new Map<string, CatalogAsset>(),
    plan: PropPlan,
    structureKey: string,
  ) {
    const entities = snapshot.entities ?? [];
    const removed = new Set<string>(snapshot.removedWalls ?? []);
    const models = plan.matrixRecords();
    const draws: Draw[] = [];
    const creatorOwners: string[] = [];
    const mappedWalls = new Set<string>();
    for (const entity of entities) {
      const meshIndex = assetMeshes.get(entity.kind);
      if (meshIndex === undefined) continue;
      const first = plan.handle(entity.id)!.slot;
      creatorOwners[first] = entity.id;
      const suppressed = new Set<number>();
      for (const part of catalog.get(entity.kind)?.wallParts ?? []) {
        const wallId = entity.id + ":" + part.wallId;
        mappedWalls.add(wallId);
        if (removed.has(wallId))
          for (const i of part.primitiveIndices) suppressed.add(i);
      }
      this.scene.meshes[meshIndex].primitives.forEach((p, i) => {
        if (!suppressed.has(i))
          draws.push({
            firstIndex: p.firstIndex,
            indexCount: p.indexCount,
            baseVertex: p.baseVertex,
            firstInstance: first,
            count: 1,
            material: p.material,
          });
      });
    }
    if (!this.roomVertices) {
      const v: number[] = [];
      const faces: [V3, V3, V3, V3, V3][] = [
        [
          [1, 0, 0],
          [0.5, -0.5, -0.5],
          [0.5, 0.5, -0.5],
          [0.5, 0.5, 0.5],
          [0.5, -0.5, 0.5],
        ],
        [
          [-1, 0, 0],
          [-0.5, -0.5, 0.5],
          [-0.5, 0.5, 0.5],
          [-0.5, 0.5, -0.5],
          [-0.5, -0.5, -0.5],
        ],
        [
          [0, 1, 0],
          [-0.5, 0.5, -0.5],
          [-0.5, 0.5, 0.5],
          [0.5, 0.5, 0.5],
          [0.5, 0.5, -0.5],
        ],
        [
          [0, -1, 0],
          [-0.5, -0.5, 0.5],
          [-0.5, -0.5, -0.5],
          [0.5, -0.5, -0.5],
          [0.5, -0.5, 0.5],
        ],
        [
          [0, 0, 1],
          [0.5, -0.5, 0.5],
          [0.5, 0.5, 0.5],
          [-0.5, 0.5, 0.5],
          [-0.5, -0.5, 0.5],
        ],
        [
          [0, 0, -1],
          [-0.5, -0.5, -0.5],
          [-0.5, 0.5, -0.5],
          [0.5, 0.5, -0.5],
          [0.5, -0.5, -0.5],
        ],
      ];
      for (const [n, ...points] of faces)
        for (const i of [0, 1, 2, 0, 2, 3])
          v.push(
            ...points[i],
            ...n,
            ...[
              [0, 0],
              [0, 1],
              [1, 1],
              [1, 0],
            ][i],
            1,
            0,
            0,
            1,
          );
      this.roomVertices = this.buffer(
        new Float32Array(v),
        GPUBufferUsage.VERTEX,
        "Owned room cube",
      );
      this.roomColors = this.buffer(
        new Uint8Array(36 * 4).fill(255).buffer,
        GPUBufferUsage.VERTEX,
        "Room vertex colors",
      );
      this.roomUV1 = this.buffer(
        new Float32Array(36 * 2),
        GPUBufferUsage.VERTEX,
        "Room secondary UV coordinates",
      );
      this.roomIndices = this.buffer(
        new Uint32Array(Array.from({ length: 36 }, (_, i) => i)).buffer,
        GPUBufferUsage.INDEX,
        "Room indices",
      );
      this.roomMaterial = await this.makeMaterial({
        id: "owned-plaster",
        name: "Owned plaster",
        baseColorFactor: [0.72, 0.66, 0.52, 1],
        emissiveFactor: [0, 0, 0],
        roughness: 0.9,
        metallic: 0,
        normalScale: 1,
        uvScale: [1, 1],
        alphaMode: "OPAQUE",
        alphaCutoff: 0.5,
        doubleSided: true,
        packedChannels: "metallic-r-roughness-g-occlusion-b",
      });
    }
    const cubes: number[] = [];
    const append = (
      local: number[],
      transform?: { position: V3; yaw: number; scale: number },
    ) =>
      cubes.push(
        ...(transform
          ? multiply(
              modelMatrix(transform.position, transform.yaw, transform.scale),
              local,
            )
          : local),
      );
    const box = (
      min: V3,
      max: V3,
      transform?: { position: V3; yaw: number; scale: number },
    ) => {
      const w = max.map((x, i) => Math.max(0.01, x - min[i]));
      append(
        [
          w[0],
          0,
          0,
          0,
          0,
          w[1],
          0,
          0,
          0,
          0,
          w[2],
          0,
          (min[0] + max[0]) / 2,
          (min[1] + max[1]) / 2,
          (min[2] + max[2]) / 2,
          1,
        ],
        transform,
      );
    };
    for (const room of snapshot.rooms ?? []) {
      const [x, z] = room.center,
        [w, d] = room.size,
        y = room.floorY ?? 1.55;
      box([x - w / 2, y - 0.14, z - d / 2], [x + w / 2, y, z + d / 2]);
    }
    for (const wall of snapshot.wallColliders ?? []) {
      if (mappedWalls.has(wall.id)) continue;
      box(wall.localMin ?? wall.min, wall.localMax ?? wall.max, wall.transform);
    }
    for (const support of snapshot.supports ?? []) {
      const [x, z] = support.center,
        [w, d] = support.size,
        y = support.floorY;
      box(
        [x - w / 2, y - 0.14, z - d / 2],
        [x + w / 2, y, z + d / 2],
        support.transform,
      );
    }
    for (const ramp of snapshot.ramps ?? []) {
      const a: V3 = ramp.from,
        b: V3 = ramp.to;
      const dx = b[0] - a[0],
        dy = b[1] - a[1],
        dz = b[2] - a[2],
        length = Math.hypot(dx, dz);
      append(
        [
          (dz / length) * ramp.width,
          0,
          (-dx / length) * ramp.width,
          0,
          0,
          0.14,
          0,
          0,
          dx,
          dy,
          dz,
          0,
          (a[0] + b[0]) / 2,
          (a[1] + b[1]) / 2 - 0.07,
          (a[2] + b[2]) / 2,
          1,
        ],
        ramp.transform,
      );
    }
    const instances = models.length
      ? this.buffer(models, GPUBufferUsage.VERTEX, "Authored object instances")
      : undefined;
    const roomInstances = cubes.length
      ? this.buffer(
          new Float32Array(cubes),
          GPUBufferUsage.VERTEX,
          "Authored room surfaces",
        )
      : undefined;
    let creator: Awaited<ReturnType<Renderer["prepareCreatorMaterials"]>>;
    try {
      creator = await this.prepareCreatorMaterials(snapshot);
    } catch (error) {
      this.releaseBuffer(instances);
      this.releaseBuffer(roomInstances);
      throw error;
    }
    for (const draw of draws)
      draw.material = creator.remap(
        creatorOwners[draw.firstInstance],
        draw.material,
      );
    let used = false;
    return {
      ready: () => !used && plan.ready() && creator.ready(),
      activate: () => {
        if (used || !plan.ready() || !creator.ready())
          throw new Error("A newer edit replaced this update.");
        used = true;
        plan.activate();
        this.authoredStructure = structureKey;
        this.authoredFullRebuilds++;
        this.authoredBytesUploaded += models.byteLength;
        this.authoredKinds = new Map(entities.map((e: any) => [e.id, e.kind]));
        this.authoredEntityDraws = new Map();
        for (const draw of draws) {
          const id = creatorOwners[draw.firstInstance];
          const group = this.authoredEntityDraws.get(id) ?? [];
          group.push(draw);
          this.authoredEntityDraws.set(id, group);
        }
        this.releaseBuffer(this.authoredInstances);
        this.releaseBuffer(this.roomInstances);
        this.authoredInstances = instances;
        this.authoredDraws = draws;
        creator.activate();
        this.roomInstances = roomInstances;
        this.roomCount = cubes.length / 16;
        this.geometryVersion++;
        this.shadowDirty = true;
      },
      discard: () => {
        if (used) return;
        used = true;
        this.releaseBuffer(instances);
        this.releaseBuffer(roomInstances);
        creator.discard();
        plan.discard();
      },
    };
  }
  async updateAuthored(
    snapshot: any,
    assetMeshes: Map<string, number>,
    catalog = new Map<string, CatalogAsset>(),
  ) {
    (await this.prepareAuthored(snapshot, assetMeshes, catalog)).activate();
  }
  private meshEnabled = false;
  private meshSourceIndices?: Uint32Array;
  private meshIndices?: GPUBuffer;
  private meshRoomIndices?: GPUBuffer;
  private meshPipeline?: GPURenderPipeline;
  async meshView(enabled: boolean) {
    if (enabled && !this.meshPipeline) {
      const create = this.features.createMeshInspection;
      if (!create)
        throw new Error("Mesh inspection is not available in this player.");
      const mesh = await create(
        this.device,
        this.shaders.MATERIAL,
        this.meshSourceIndices!,
        (module) => this.pipelineDescriptor(module),
        (data, label) => this.buffer(data, GPUBufferUsage.INDEX, label),
        (buffer) => this.releaseBuffer(buffer),
        () => !this.disposed,
      );
      if (this.disposed) {
        this.releaseBuffer(mesh.indices);
        this.releaseBuffer(mesh.rooms);
        throw new Error("The world closed before mesh inspection was ready.");
      }
      this.meshIndices = mesh.indices;
      this.meshRoomIndices = mesh.rooms;
      this.meshPipeline = mesh.pipeline;
    }
    this.meshEnabled = enabled;
  }
  materialBindings(snapshot: any, meshes: Map<string, number>) {
    return this.editMaterials()?.bindings(snapshot, meshes) ?? [];
  }
  applyMaterialValues(values: Record<string, Record<string, number>>) {
    const editor = this.editMaterials();
    if (!editor && Object.keys(values).length)
      throw new Error("Open the editor to change materials.");
    editor?.applyValues(values);
    this.shadowDirty = true;
  }
  private retainStockPipelineFamily() {
    if (
      this.stockPipelineFamily ||
      this.activeShaderSource !== this.shaders.MATERIAL
    )
      return;
    const pipelines = SHADER_PIPELINE_FIELDS.map((field) => this[field]);
    // Retain only a fully initialized owned stock family.
    if (pipelines.every(Boolean))
      this.stockPipelineFamily = {
        pipelines,
        opaqueDepth: this.opaqueDepthPipelines,
      };
  }
  private shaderActivation(
    source: string,
    revision: number,
    current: () => boolean,
    family: ShaderPipelineFamily,
  ) {
    const ready = () =>
      !this.disposed && revision === this.desiredShader && current();
    return {
      ready,
      activate: () => {
        if (!ready()) return;
        this.cullingEnabled = source === this.shaders.MATERIAL;
        this.windVertexBounded = usesOwnedWindVertex(
          source,
          this.shaders.MATERIAL,
        );
        this.windShadowBounded = usesOwnedWindShadow(
          source,
          this.shaders.MATERIAL,
        );
        SHADER_PIPELINE_FIELDS.forEach((field, index) => {
          this[field] = family.pipelines[index];
        });
        this.activeShaderSource = source;
        this.stockChoiceRevision++;
        if (family.opaqueDepth) this.opaqueDepthPipelines = family.opaqueDepth;
        this.geometryVersion++;
        this.shadowDirty = true;
        this.windShadowPlanner.reset();
      },
    };
  }
  prepareShader(source: string, current: () => boolean = () => true) {
    if (this.disposed) return Promise.resolve(undefined);
    this.retainStockPipelineFamily();
    if (this.shaderPending >= 3)
      return Promise.reject(
        new Error("Let the current shader edits finish first."),
      );
    this.shaderPending++;
    const revision = ++this.desiredShader;
    const task = this.shaderQueue.then(() =>
      this.compileShader(source, revision, current),
    );
    this.shaderQueue = task.then(
      () => {},
      () => {},
    );
    return task.finally(() => {
      this.shaderPending--;
    });
  }
  async replaceShader(
    source: string,
    current: () => boolean = () => true,
  ): Promise<boolean> {
    const candidate = await this.prepareShader(source, current);
    if (!candidate || !candidate.ready()) return false;
    candidate.activate();
    return true;
  }
  private async compileShader(
    source: string,
    revision: number,
    current: () => boolean,
  ) {
    if (new TextEncoder().encode(source).length > 131072)
      throw new Error("Keep shader edits below 128 KiB.");
    if (revision !== this.desiredShader || !current() || this.disposed)
      return undefined;
    if (source === this.shaders.MATERIAL && this.stockPipelineFamily)
      return this.shaderActivation(
        source,
        revision,
        current,
        this.stockPipelineFamily,
      );
    this.device.pushErrorScope("validation");
    const pipelines: GPURenderPipeline[] = [];
    let failure: unknown;
    try {
      const module = this.device.createShaderModule({ code: source });
      const info = await module.getCompilationInfo();
      const errors = info.messages.filter((x) => x.type === "error");
      if (errors.length)
        throw new Error(
          errors.map((x) => `Line ${x.lineNum}: ${x.message}`).join("\n"),
        );
      const color = (variant: number, frontFace: GPUFrontFace) =>
        this.device.createRenderPipelineAsync(
          this.pipelineDescriptor(
            module,
            variant > 0,
            variant === 2,
            frontFace,
          ),
        );
      for (let variant = 0; variant < 3; variant++)
        pipelines.push(await color(variant, "ccw"));
      pipelines.push(
        ...(await Promise.all(
          [0, 1, 2].map((variant) => color(variant, "cw")),
        )),
      );
      const depth = await this.device.createRenderPipelineAsync(
        this.depthDescriptor(
          module,
          this.shadowPipeline.getBindGroupLayout(0),
          source === this.shaders.MATERIAL,
        ),
      );
      const shadow = await this.device.createRenderPipelineAsync({
        layout: this.device.createPipelineLayout({
          bindGroupLayouts: [
            this.shadowPipeline.getBindGroupLayout(0),
            this.materialLayout,
          ],
        }),
        vertex: { module, entryPoint: "shadowVS", buffers: VERTICES },
        fragment: { module, entryPoint: "shadowFS", targets: [] },
        primitive: { topology: "triangle-list", cullMode: "none" },
        depthStencil: {
          format: "depth32float",
          depthWriteEnabled: true,
          depthCompare: "less",
          depthBias: 2,
          depthBiasSlopeScale: 2,
        },
      });
      pipelines.push(shadow, depth);
    } catch (e) {
      failure = e;
    }
    const error = await this.device.popErrorScope();
    if (failure || error) throw failure ?? new Error(error!.message);
    if (revision !== this.desiredShader || !current() || this.disposed)
      return undefined;
    return this.shaderActivation(source, revision, current, { pipelines });
  }

  private invalidateCaptureViews() {
    // Live shadow fitting has camera hysteresis. A fixed-time capture must fit
    // its own camera instead of inheriting a nearby live frame's shadow anchor.
    this.shadowDirty = true;
    this.localSignatures.clear();
    this.windShadowPlanner.reset();
    this.planar?.invalidate();
  }
  /** Rigid film poses share every geometry pass. Original baked GI occlusion remains an approximation. */
  private async applyFilmTransforms(
    transforms: FilmSourceTransform[],
    mode: "day" | "night",
  ): Promise<() => void> {
    const variant = this.variants.get(mode);
    if (!variant)
      throw Error("Open the selected world before filming its objects.");
    const prepared = await prepareFilmTransforms(
      variant.scene,
      transforms,
      mode,
    );
    if (
      prepared.some(
        (item) =>
          !variant.draws.some(
            (draw) =>
              item.instanceIndex >= draw.firstInstance &&
              item.instanceIndex < draw.firstInstance + draw.count,
          ),
      )
    )
      throw Error("Choose a source object that is drawn in this world.");
    if (prepared.some((item) => variant.wind.moving[item.instanceIndex]))
      throw Error("Choose a fixed source mesh for a rigid film rotation.");
    const saved = prepared.map((item) => ({
      index: item.instanceIndex,
      model: variant.models.slice(
        item.instanceIndex * 16,
        item.instanceIndex * 16 + 16,
      ),
      sphere: variant.bounds.spheres.slice(
        item.instanceIndex * 4,
        item.instanceIndex * 4 + 4,
      ),
      aabb: variant.bounds.aabbs.slice(
        item.instanceIndex * 6,
        item.instanceIndex * 6 + 6,
      ),
    }));
    const version = this.geometryVersion,
      selected = {
        instances: this.instances,
        draws: this.draws,
        lamps: this.lamps,
        triangles: this.triangles,
      };
    const invalidate = () => {
      // Capture mode deliberately pays for fresh static and moving shadows, including local views.
      this.invalidateCaptureViews();
      // A restored film version can be reused for a different pose on the next
      // capture. Force a resident refresh even when those version numbers match.
      const resident = this.frameScenes.get(mode);
      if (resident) resident.version = -1;
      for (const cached of this.culledSources.values()) cached.signature = "";
    };
    let restored = false;
    const restore = () => {
      if (restored) return;
      restored = true;
      let failed: unknown;
      for (const item of saved) {
        variant.models.set(item.model, item.index * 16);
        variant.bounds.spheres.set(item.sphere, item.index * 4);
        variant.bounds.aabbs.set(item.aabb, item.index * 6);
        try {
          this.device.queue.writeBuffer(
            variant.instances,
            item.index * 64,
            item.model,
          );
        } catch (error) {
          failed ??= error;
        }
      }
      Object.assign(this, selected);
      this.geometryVersion = version;
      invalidate();
      if (failed) throw failed;
    };
    try {
      for (const item of prepared) {
        variant.models.set(item.model, item.instanceIndex * 16);
        variant.bounds.spheres.set(item.bounds.spheres, item.instanceIndex * 4);
        variant.bounds.aabbs.set(item.bounds.aabbs, item.instanceIndex * 6);
        this.device.queue.writeBuffer(
          variant.instances,
          item.instanceIndex * 64,
          item.model,
        );
      }
      this.geometryVersion = version + 1;
      invalidate();
      return restore;
    } catch (error) {
      try {
        restore();
      } catch {
        /* Keep the original failure if the device also stopped during cleanup. */
      }
      throw error;
    }
  }
  async capture(
    animationTime?: number,
    view?: {
      camera: Camera;
      env: EnvironmentSettings;
      actorReference?: ActorCaptureReferenceMetadata;
    },
    format: "png" | "rgba" = "png",
    transforms?: FilmSourceTransform[],
  ): Promise<Blob> {
    if (format !== "png" && format !== "rgba")
      throw new Error("Choose PNG or raw RGBA capture.");
    return this.captureFrame(
      animationTime,
      view,
      format,
      transforms,
    ) as Promise<Blob>;
  }
  /** Tightly packed RGBA8 without a Blob or image-encoding allocation; ownership may be transferred. */
  async capturePixels(
    animationTime?: number,
    view?: {
      camera: Camera;
      env: EnvironmentSettings;
      actorReference?: ActorCaptureReferenceMetadata;
    },
    transforms?: FilmSourceTransform[],
  ): Promise<ArrayBuffer> {
    return this.captureFrame(
      animationTime,
      view,
      "pixels",
      transforms,
    ) as Promise<ArrayBuffer>;
  }
  async capturePixelsDetailed(
    animationTime: number,
    view: {
      camera: Camera;
      env: EnvironmentSettings;
      actorReference?: ActorCaptureReferenceMetadata;
    },
  ): Promise<RenderedPixelMetadata & { pixels: ArrayBuffer }> {
    let metadata: RenderedPixelMetadata | undefined;
    const pixels = (await this.captureFrame(
      animationTime,
      view,
      "pixels",
      undefined,
      (rendered) => {
        metadata = rendered;
      },
    )) as ArrayBuffer;
    if (!metadata) throw new Error("The screenshot did not submit a frame.");
    return { ...metadata, pixels };
  }
  private async captureFrame(
    animationTime: number | undefined,
    view:
      | {
          camera: Camera;
          env: EnvironmentSettings;
          actorReference?: ActorCaptureReferenceMetadata;
        }
      | undefined,
    format: "png" | "rgba" | "pixels",
    transforms?: FilmSourceTransform[],
    rendered?: (metadata: RenderedPixelMetadata) => void,
    nativeStartPayload?: EnemyCaptureReference,
  ): Promise<Blob | ArrayBuffer> {
    if (transforms !== undefined) validateFilmTransforms(transforms);
    if (
      animationTime !== undefined &&
      (!Number.isFinite(animationTime) ||
        animationTime < 0 ||
        animationTime > 86400)
    )
      throw new Error("Choose a screenshot time between zero and 24 hours.");
    if (this.capturing)
      throw new Error("Let the current screenshot finish first.");
    if (!this.lastView) throw new Error("Wait for the world to open.");
    const liveView = this.lastView;
    const selected = view ?? this.lastView;
    const camera = {
      ...selected.camera,
      eye: [...selected.camera.eye] as V3,
      up: selected.camera.up ? ([...selected.camera.up] as V3) : undefined,
    };
    const env = { ...selected.env };
    if (nativeStartPayload)
      this.activeCaptureActors = { payload: nativeStartPayload };
    if (view?.actorReference) {
      if (!this.captureActorRegistry || !this.captureActorModelIdentity)
        throw new Error(
          "Pin this world's actors before requesting their capture reference.",
        );
      const reference = this.captureActorRegistry.resolve(
        view.actorReference.id,
        {
          scenario: env.mode,
          modelIdentity: this.captureActorModelIdentity,
          actorIds: this.enemyActors.map((actor) => actor.id),
        },
      );
      if (!sameCaptureActorReference(reference.metadata, view.actorReference))
        throw new Error(
          "The actor capture reference metadata does not match this world.",
        );
      this.activeCaptureActors = reference;
    }
    const time = animationTime ?? this.lastView.time;
    this.capturing = true;
    let buffer: GPUBuffer | undefined;
    let restoreTransforms: (() => void) | undefined;
    let failed = false;
    try {
      await this.device.queue.onSubmittedWorkDone();
      if (transforms?.length)
        restoreTransforms = await this.applyFilmTransforms(
          transforms,
          env.mode,
        );
      else this.invalidateCaptureViews();
      const width = this.width,
        height = this.height,
        stride = Math.ceil((width * 4) / 256) * 256;
      const size = stride * height;
      if (size > MAX_CAPTURE_BYTES || width * height * 4 > MAX_CAPTURE_BYTES)
        throw new Error("Choose a smaller frame to capture.");
      if (this.captureReadback?.size !== size) {
        this.captureReadback?.buffer.destroy();
        this.captureReadback = {
          size,
          buffer: this.device.createBuffer({
            label: "Screenshot readback",
            size,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
          }),
        };
      }
      buffer = this.captureReadback.buffer;
      this.captureBuffer = buffer;
      const frameBefore = this.frame;
      if (!this.render(camera, env, time, true))
        throw new Error("The screenshot could not start. Try again.");
      rendered?.({
        ...(this.activeCaptureActors?.metadata
          ? { actors: structuredClone(this.activeCaptureActors.metadata) }
          : {}),
        frameBefore,
        frameAfter: this.frame,
        generation: this.geometryVersion,
        width,
        height,
        time,
        exposure: env.exposure,
        camera: {
          eye: [...camera.eye],
          target: add(camera.eye, direction(camera.yaw, camera.pitch)),
          up: camera.up ? [...camera.up] : [0, 1, 0],
          fov: camera.fov ?? 1,
          vignette: camera.vignette ? { ...camera.vignette } : null,
        },
      });
      await buffer.mapAsync(GPUMapMode.READ);
      const source = new Uint8Array(buffer.getMappedRange()),
        pixels = new Uint8ClampedArray(width * height * 4);
      const bgra = this.format.startsWith("bgra");
      for (let y = 0; y < height; y++)
        for (let x = 0; x < width; x++) {
          const a = y * stride + x * 4,
            b = (y * width + x) * 4;
          pixels[b] = source[a + (bgra ? 2 : 0)];
          pixels[b + 1] = source[a + 1];
          pixels[b + 2] = source[a + (bgra ? 0 : 2)];
          pixels[b + 3] = 255;
        }
      buffer.unmap();
      if (format === "pixels") return pixels.buffer;
      if (format === "rgba")
        return new Blob([pixels.buffer], { type: "application/octet-stream" });
      if (
        !this.captureCanvas ||
        this.captureCanvas.width !== width ||
        this.captureCanvas.height !== height
      )
        this.captureCanvas = new OffscreenCanvas(width, height);
      this.captureCanvas
        .getContext("2d")!
        .putImageData(new ImageData(pixels, width, height), 0, 0);
      return await this.captureCanvas.convertToBlob({ type: "image/png" });
    } catch (error) {
      failed = true;
      throw error;
    } finally {
      let restoreFailure: unknown;
      try {
        if (restoreTransforms) restoreTransforms();
        else this.invalidateCaptureViews();
      } catch (error) {
        restoreFailure = error;
      }
      this.lastView = liveView;
      if (this.captureBuffer === buffer) this.captureBuffer = undefined;
      if (buffer?.mapState === "mapped") buffer.unmap();
      this.capturing = false;
      this.activeCaptureActors = undefined;
      if (!failed && restoreFailure) throw restoreFailure;
    }
  }
  /** Bind a fully prepared candidate to the live surface at the activation point. */
  presentTo(canvas: OffscreenCanvas) {
    const context = canvas.getContext("webgpu");
    if (!context || this.disposed || this.lost)
      throw new Error("The prepared world cannot use this display.");
    context.configure({
      device: this.device,
      format: this.format,
      alphaMode: "opaque",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    if (this.context && this.context !== context) this.context.unconfigure();
    this.canvas = canvas;
    this.context = context;
  }
  dispose(preserveContext = false) {
    if (this.disposed) return;
    this.disposed = true;
    this.captureActorRegistry?.dispose();
    this.activeCaptureActors = undefined;
    this.desiredShader++;
    this.stockPipelineFamily = undefined;
    this.frameCompletions.stop();
    this.gpuTimer?.dispose();
    for (const scene of this.frameScenes.values()) scene.dispose?.();
    this.frameScenes.clear();
    for (const source of this.culledSources.values()) {
      source.bundles?.dispose();
      source.compiled?.dispose();
    }
    this.compiledSourceHost?.dispose();
    this.compiledSourceHost = undefined;
    this.culledSources.clear();
    this.canonicalDraws = undefined;
    this.residentSource.dispose();
    this.experiment.framePreparation?.dispose?.();
    this.experiment.framePreparation = undefined;
    try {
      this.captureReadback?.buffer.destroy();
      this.captureReadback = undefined;
      this.captureCanvas = undefined;
      this.stockChoices = new WeakMap();
      this.materialEditor?.dispose();
      this.materialEditor = undefined;
      for (const b of this.buffers) b.destroy();
      for (const t of this.textures) t.destroy();
      this.depth?.destroy();
      this.occlusion?.dispose();
      this.ibl?.dispose();
      this.probes?.dispose();
      this.surfaceSky?.dispose();
      this.bounce?.dispose();
      this.clusteredLighting?.dispose();
      this.displaySharpen?.dispose();
      for (const lut of this.displayLuts.values()) lut.dispose();
      this.displayLuts.clear();
      this.localFog?.dispose();
      this.fogAmbientVolume?.dispose();
      this.fogAmbientInputs.clear();
      this.bloom?.dispose();
      this.particles?.dispose();
      this.sourceWaterStreams?.dispose();
      this.sourceWaterSplashes?.dispose();
      this.creatures?.dispose();
      this.character?.dispose();
      this.mmorpgCharacters?.dispose();
      this.mmorpgEffects?.dispose();
      this.mmorpgNames?.dispose();
      this.mmorpgTerrain?.dispose();
      this.grass?.dispose();
      this.enemies?.dispose();
      this.spellEffects?.dispose();
      this.previewImpacts?.clear();
      this.previewImpacts = undefined;
      this.liveEffects = undefined;
      this.stopBattle();
      this.spellSwarmHost?.dispose();
      this.spellSwarmHost = undefined;
      this.sourceDebris?.dispose();
      this.debrisPacking?.dispose();
      this.leaves?.dispose();
      this.fogCards?.dispose();
      this.layerPools?.dispose();
      for (const pass of this.surfaceDecals.values()) pass.destroy();
      this.water?.dispose();
      this.planar?.dispose();
      this.waterHDR?.destroy();
      this.streamHDR?.destroy();
      this.hdr?.destroy();
      this.msaa?.destroy();
      this.shadow?.destroy();
      this.farShadow?.destroy();
      this.localShadow?.destroy();
      for (const texture of this.windShadowCopies.values()) texture.destroy();
      this.windShadowCopies.clear();
      this.windShadowPlanner.reset();
    } finally {
      try {
        if (!preserveContext) this.context?.unconfigure();
      } finally {
        this.device?.destroy();
      }
    }
  }
}
