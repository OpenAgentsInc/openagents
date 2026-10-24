/** Private compiled scene contract. Vendor data never belongs in source control. */
export interface CompiledMaterial {
  id: string;
  name: string;
  baseColorUrl?: string;
  normalUrl?: string;
  /** Tangent-space Y convention after graph evaluation; omitted means OpenGL. */
  normalConvention?: "opengl" | "directx";
  /** Decode each vector texture before graph operations; omitted means RGB. */
  normalEncoding?: "rgb" | "xy-positive-z";
  /** Optional recovered mesh basis; the default uses world UV gradients. */
  normalFrameProfile?: SourceNormalFrameProfile;
  mraUrl?: string;
  emissiveUrl?: string;
  baseColorFactor: [number, number, number, number];
  baseColorBake?: {
    key: string;
    model: "mm-props-mask-v1";
    url: string;
    sourceSha256: string;
    maxDimension: number;
    recipe: {
      tint: number[];
      overlay: number[];
      intensity: number;
      maskContrast: number;
      sourceColorSpace: "srgb" | "linear";
    };
  };
  emissiveFactor: [number, number, number];
  roughness: number;
  metallic: number;
  normalScale: number;
  uvScale: [number, number];
  alphaMode: "OPAQUE" | "MASK" | "BLEND";
  alphaCutoff: number;
  doubleSided: boolean;
  /** Multiply source vertex alpha into coverage, independently of paint masks. */
  vertexOpacity?: boolean;
  /** MM_props is source-confirmed; other master families remain provisional. */
  packedChannels: "metallic-r-roughness-g-occlusion-b";
  layers?: TwoLayerMaterial;
  glass?: GlassMaterial;
  flame?: FlameMaterial;
  paint?: PaintLayerMaterial;
  terrain?: TerrainLayerMaterial;
  vegetation?: VegetationMaterial;
  /** Camera-distance soil color for the owned continuous grass plane. */
  grassGround?: { profile: "quick-grass-v1" };
}

export interface SourceNormalFrameProfile {
  model: "source-mesh-noscale-v1" | "source-vegetation-noscale-v1";
  evidence: {
    sourceFactsSha256: string;
    sourceMeshId: string;
    verticesSha256: string;
    indicesSha256: string;
    normalTextureSha256: string;
    /** Vegetation profiles clone this otherwise unchanged source recipe. */
    sourceMaterialId?: string;
    sourceMaterialsSha256?: string;
    sourceNormalMetadataSha256?: string;
  };
}

/** Owned bounded angular transport; no measured thickness or source-model equivalence. */
export interface VegetationTransmission {
  model: "bounded-forward-sheet-v1";
  anisotropy: number;
}

export interface VegetationMaterial {
  model: "ue-mm-vegetation-1-v1";
  baseColorUrl: string;
  normalUrl: string;
  mraUrl: string;
  mask: {
    url: string;
    sourceColorSpace: "srgb" | "linear";
    enabled: boolean;
    color: [number, number, number];
  };
  tint: [number, number, number];
  color: {
    luminance: [number, number, number];
    desaturation: number;
    contrast: number;
    intensity: number;
    overlayColor: [number, number, number];
    overlayStrength: number;
  };
  gradient: { axis: "u" | "v"; intensity: number };
  aoAffectsColorIntensity: number;
  normalFlatness: number;
  roughnessIntensity: number;
  specularAmount: number;
  fuzz: { coreDarkness: number; power: number; edgeBrightness: number };
  subsurface: { color: [number, number, number]; intensity: number };
  /** Missing preserves the existing direct backlight arithmetic. */
  transmission?: VegetationTransmission;
  alphaCutoff: number;
  twoSided: boolean;
  lightingModel: "two-sided-foliage";
  /** Source switch; an optional sway-only sidecar selects qualified primitive uses. */
  sourceWindEnabled: boolean;
  source: SourceMaterialRecipe;
}

export interface TerrainLayerMaterial {
  model: "ue-mm-blend-3-v1";
  slots: [PaintLayerSlot, PaintLayerSlot, PaintLayerSlot];
  uv: { mode: "mesh" | "objectXY" | "objectYZ"; scale: [number, number] };
  height: {
    url: string;
    sourceColorSpace: "srgb" | "linear";
    uvScale: [number, number];
    contrast: number;
    useCustom: boolean;
    customIntensity: number;
  };
  wetness: {
    url: string;
    sourceColorSpace: "srgb" | "linear";
    colorIntensity: number;
    roughnessIntensity: number;
  };
  parallax: {
    enabled: boolean;
    url: string;
    sourceColorSpace: "srgb" | "linear";
    referencePlane: number;
    heightRatio: number;
    uvSet: 0;
  };
  baseRoughnessIntensity: number;
  source: SourceMaterialRecipe;
}

export interface PaintLayerMaterial {
  model: "ue-mm-blend-1-v1";
  slots: [PaintLayerSlot, PaintLayerSlot];
  uv: { mode: "mesh" | "objectXY" | "objectYZ"; scale: [number, number] };
  height: {
    url: string;
    sourceColorSpace: "srgb" | "linear";
    uvScale: [number, number];
    contrast: number;
  };
  overlay: {
    url: string;
    sourceColorSpace: "srgb" | "linear";
    uvSet: 1;
    useAlpha: boolean;
    colorIntensity: number;
    roughnessIntensity: number;
  };
  baseRoughnessIntensity: number;
  source: SourceMaterialRecipe;
}

export interface PaintLayerSlot extends LayerMaterialSlot {
  emissiveUrl?: string;
  emissiveIntensity: number;
  emissiveColorSpace: "srgb" | "linear";
}

export interface SourceMaterialRecipe {
  master: string;
  graphSha256: string;
  parameters: Record<string, unknown>;
}

export interface GlassMaterial {
  model: "ue-mm-glass-v1";
  color: [number, number, number];
  dirtColor: [number, number, number];
  colorAmount: number;
  metallic: number;
  uvScale: [number, number];
  opacity: {
    base: number;
    side1: number;
    side2: number;
    fresnelPower: number;
    useTexture: boolean;
    textureValue: number;
  };
  roughness: {
    constant: number;
    main: number;
    intensity: number;
    power: number;
    useDirt: boolean;
  };
  /** The constant is already tangent-space. Flatness 1 removes sampled detail. */
  normal: {
    enabled: boolean;
    url?: string;
    flatness: number;
    constant: [number, number, number];
  };
  dirt: {
    url: string;
    sourceColorSpace: "srgb" | "linear";
    world: {
      url: string;
      sourceColorSpace: "srgb" | "linear";
      sizeMeters: number;
      power: number;
      channel: "g";
      projection: "ue-world-aligned-v1";
    };
    useForColor: boolean;
  };
  textureColor: [number, number, number];
  emissiveColor: [number, number, number];
  emissiveIntensity: number;
  source: SourceMaterialRecipe;
}

export interface FlameMaterial {
  model: "ue-mm-flame-v1";
  colorUrl: string;
  sourceColorSpace: "srgb";
  tint: [number, number, number];
  emissiveIntensity: number;
  opacityChannel: "a";
  blend: "additive";
  shading: "unlit";
  sourceUnits: "unreal-material-emissive";
  source: SourceMaterialRecipe;
}

export interface TwoLayerMaterial {
  model: "ue-mm-blend-2-v1";
  slots: [LayerMaterialSlot, LayerMaterialSlot];
  uv: { mode: "mesh" | "objectXY" | "objectYZ"; scale: [number, number] };
  blend: {
    vertexChannel: "r";
    phase: number;
    contrast: number;
    height: {
      mode: "layer2Alpha" | "maskRed";
      url?: string;
      customIntensity: number;
      sourceColorSpace: "srgb" | "linear";
    };
    worldSubtract: {
      url: string;
      sourceColorSpace: "srgb" | "linear";
      sizeMeters: number;
      intensity: number;
      power: number;
      channel: "r";
      projection: "ue-world-aligned-v1";
    };
  };
  detailNormal: { url: string; uvScale: [number, number]; flatness: number };
  colorIntensity: number;
  source: {
    master: string;
    graphSha256: string;
    parameters: Record<string, unknown>;
  };
}

export interface LayerMaterialSlot {
  /** RGB is sRGB; alpha retains source height data and does not mean opacity. */
  baseColorUrl: string;
  normalUrl: string;
  mraUrl: string;
  colorFactor: [number, number, number, number];
}

export interface CompiledPrimitive {
  firstIndex: number;
  indexCount: number;
  baseVertex: number;
  material: number;
  materialSlot: number;
}

export interface CompiledMesh {
  id: string;
  name: string;
  primitives: CompiledPrimitive[];
  bounds: { min: number[]; max: number[] };
}

export interface CompiledInstance {
  mesh: number;
  /** Column-major glTF-style world matrix, meters, Y up. */
  model: number[];
  materialOverrides: number[];
  actor: string;
  component: string;
  sourceClass: string;
}

export interface CompiledBatch {
  mesh: number;
  materialOverrides: number[];
  firstInstance: number;
  instanceCount: number;
}

export interface CompiledScene {
  schema: "aethyrforge.private-scene.v1";
  scenario: "day" | "night";
  vertices: string;
  indices: string;
  vertexColors: string;
  /** Optional float32x2 source UV channel 1, separate from the 48-byte geometry. */
  vertexUV1?: string;
  vertexUV1Validity?: {
    sourceVerticesWithUV1: number;
    sourceVerticesMissingUV1: number;
    sourcePrimitivesMissingUV1: number;
    otherVerticesZeroFilled: number;
  };
  vertexStride: 48;
  /** position3, normal3, uv2, tangent4; float32 little-endian. */
  vertexCount: number;
  indexCount: number;
  materials: CompiledMaterial[];
  meshes: CompiledMesh[];
  instances: CompiledInstance[];
  batches: CompiledBatch[];
  bounds: { min: number[]; max: number[] };
  spawn: { position: number[]; target: number[]; source: string };
  source: {
    map: string;
    placements: number;
    actorCensus: Record<string, number>;
  };
  diagnostics: { warnings: string[]; skippedPlacements: number };
  catalog?: string;
  lookURL?: string;
  environmentURL?: string;
  lightingEnvironmentURL?: string;
  particlesURL?: string;
  sourceWaterStreamsURL?: string;
  sourceWaterSplashesURL?: string;
  creaturesURL?: string;
  /** Prepared static walking surfaces; never distributed without the source asset rights. */
  collisionURL?: string;
  gameplayURL?: string;
  /** Optional bounded procedural grass settings; geometry stays in the owned renderer. */
  grassURL?: string;
  /** Source MMO classes, scenery, and presentation assets. */
  mmorpgURL?: string;
  /** Optional private, editable dressing for the Open Ground creator. */
  openGroundPropsURL?: string;
  /** Optional private animation data for the local GPU battle experiment. */
  battleCrowdURL?: string;
  /** Optional prepared Everglade player rig, textures, and movement clips. */
  characterURL?: string;
  /** Optional Knight rig and bounded runtime patrols, admitted together. */
  enemyCharacterURL?: string;
  enemyPatrolsURL?: string;
  leavesURL?: string;
  fogCardsURL?: string;
  /** Optional static per-surface direct-sky refinement, separate from the probe grid. */
  surfaceSkyURL?: string;
  /** Optional source-selected ordinary sway; the independent flutter branch is excluded. */
  windURL?: string;
  displayLutURL?: string;
  waterURL?: string;
  probesURL?: string;
  surfaceDecalsURL?: string;
  decals?: {
    sourceComponents: number;
    selectedComponents: number;
    generatedComponents: number;
    generatedTriangles: number;
    receiverPolicy: "opaque-static-source-geometry";
    normalOffsetMeters: number;
    depthFade: "source-piecewise-linear-unorm8";
  };
}

export interface EnvironmentMap {
  /** Optional source sky surface identity, used to recover its finite fog distance. */
  sourceMaterial?: string;
  url: string;
  width: number;
  height: number;
  rotation: number[];
  uSign: number;
  uOffset: number;
  vSign: number;
  vOffset: number;
  tint: number[];
  brightness: number;
  /** Only for a lighting map baked from a particular source sun intensity. */
  sourceSunIlluminanceLux?: number;
  /** Raw constant fog radiance; parent dimensions/orientation apply, without sun scaling. */
  constantFog?: {
    url: string;
    sha256: string;
    bytes: number;
    format: "rgba16float";
  };
}

export interface CompiledEnvironments {
  schema: "aethyrforge.private-environments.v1";
  scenarios: Partial<Record<"day" | "night", EnvironmentMap>>;
  lightingURL?: string;
  /** Source atmosphere inputs retained for optional measured-lighting checks. */
  atmosphere?: Partial<Record<"day" | "night", unknown>>;
  /** Separate from map identities: this does not alter IBL or baked transport. */
  fogAmbient?: Partial<Record<"day" | "night", MeasuredFogAmbientRecipe>>;
}

export interface MeasuredFogAmbientRecipe {
  schema: "aethyrforge.measured-fog-ambient.v1";
  scenario: "day" | "night";
  controls: Record<string, unknown>;
  controlsJson: string;
  controlsSha256: string;
  bindingSha256: string;
  ambientRGB: [number, number, number];
  space: "scene-linear-rec709";
  includes: {
    skyLightIntensity: false;
    sceneStorageScale: false;
    exposure: false;
    sourceAmbientContributionScale: false;
  };
  calibration: Record<string, unknown>;
}

export interface SourceCamera {
  id: string;
  label: string;
  position: number[];
  target: number[];
  up: number[];
  verticalFovDegrees: number;
  horizontalFovDegrees: number;
  aspectRatio: number;
  source: string;
  /** Explicit camera override only; stored values with disabled overrides are omitted. */
  vignetteOverride?: SourceVignetteOverride;
  raw: Record<string, unknown>;
}

export interface SourceVignetteOverride {
  intensity: number;
  blendWeight: number;
  shape: "cosine-fourth";
}

export interface SourceLight {
  id: string;
  label: string;
  kind: "point" | "spot" | "directional";
  position: number[];
  /** Emitted ray direction; negate it for a surface-to-light shading vector. */
  direction: number[];
  color: number[];
  intensity: number;
  intensityUnits: string;
  /** Local lights can use either physical inverse-square or authored legacy falloff. */
  inverseSquaredFalloff?: boolean;
  falloffExponent?: number;
  sourceRadiusMeters?: number;
  range?: number;
  innerConeAngleRadians?: number;
  outerConeAngleRadians?: number;
  castShadows: boolean;
  enabled: boolean;
  temperatureKelvin: number;
  useTemperature: boolean;
  /** Omitted records retain the legacy temperature approximation. */
  temperatureModel?: "cie-planckian-rec709-v1";
  raw: Record<string, unknown>;
}

export interface SceneLook {
  schema: "aethyrforge.private-scene-look.v1";
  scenarios: Record<
    "day" | "night",
    {
      cameras: SourceCamera[];
      lights: SourceLight[];
      sun: SourceLight | null;
      environment: Array<Record<string, unknown>>;
      sourceActorCount: number;
    }
  >;
  galleryShots: Record<
    string,
    { scenario: "day" | "night"; cameraId: string; match: string }
  >;
  rawURL: string;
  conventions: Record<string, string>;
  limitations: string[];
}

export interface BuildingPlan {
  rooms: Array<{
    id: string;
    center: [number, number];
    size: [number, number];
    height: number;
    floorY: number;
  }>;
  openings: Array<{
    roomId: string;
    side: "north" | "east" | "south" | "west";
    center: number;
    width: number;
    bottom: number;
    height: number;
  }>;
  supports: Array<{
    id: string;
    center: [number, number];
    size: [number, number];
    floorY: number;
  }>;
  ramps: Array<{
    id: string;
    from: [number, number, number];
    to: [number, number, number];
    width: number;
  }>;
}

export interface CatalogAsset {
  id: string;
  name: string;
  aliases: string[];
  mesh: number;
  dimensions: number[];
  bounds: { min: number[]; max: number[] };
  kind: "building" | "prop";
  materialProfile: "legacy-base-color-recipe" | "gltf-pbr";
  wallParts: Array<{ wallId: string; primitiveIndices: number[] }>;
  plan?: BuildingPlan;
}

export interface CompiledCatalog {
  schema: "aethyrforge.private-catalog.v1";
  assets: CatalogAsset[];
  source: string;
  limitations: string[];
}
