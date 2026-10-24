import type { EnemyCharacters } from "./enemy-characters.ts";
import type { CharacterManifest } from "./character.ts";
import type { CharacterRenderer } from "./character-renderer.ts";
import type { loadCharacterManifest } from "./character.ts";
import type { sourceSkyDome } from "./sky.ts";
import type { InstanceBounds } from "./culling.ts";
import type {
  ClusteredLightInput,
  ClusteredLights,
} from "./clustered-lights.ts";

export interface RendererFrameSource {
  bounds: InstanceBounds;
  models: Float32Array;
  draws: readonly { firstInstance: number; count: number }[];
  wind?: Float32Array;
}
export interface RendererFrameResult {
  visibility: Uint8Array;
  models: Float32Array;
  wind: Float32Array;
  sourceIndices: Uint32Array;
  drawRanges: Uint32Array;
}
export interface RendererResidentFrame {
  version: number;
  update(): void;
  cull(matrix: Float32Array): RendererFrameResult;
  dispose?(): void;
}
export interface RendererFramePreparation {
  implementation: "c" | "pooled-typescript";
  readonly memoryBytes: number;
  prepare(
    source: () => RendererFrameSource,
    version: number,
  ): RendererResidentFrame;
  buildClusters(input: ClusteredLightInput): ClusteredLights;
  dispose?(): void;
}
export interface RendererGpuSlot {
  queries: GPUQuerySet;
  resolve: GPUBuffer;
  read: GPUBuffer;
  busy: boolean;
}
export interface RendererGpuTimer {
  acquire(): RendererGpuSlot | undefined;
  writes(
    slot?: RendererGpuSlot,
    phase?: "begin" | "end",
  ): GPURenderPassTimestampWrites | undefined;
  resolve(encoder: GPUCommandEncoder, slot?: RendererGpuSlot): void;
  submitted(frame: number, slot?: RendererGpuSlot): void;
  sample(frame: number): number | null;
  beginFrame?(encoder: GPUCommandEncoder, slot?: RendererGpuSlot): void;
  endFrame?(encoder: GPUCommandEncoder, slot?: RendererGpuSlot): void;
  sampleFrame?(frame: number): number | null;
  instrumentEncoder?(
    encoder: GPUCommandEncoder,
    slot?: RendererGpuSlot,
  ): GPUCommandEncoder;
  status?(): import("./frame-gpu-timer.ts").FrameGpuTimingStatus;
  clear(): void;
  dispose(): void;
}
export interface RendererExperiment {
  /** Reorders supported opaque draws; requires independent image qualification. */
  compiledSourceBatches?: boolean;
  backend: string;
  profiling: boolean;
  framePreparation?: RendererFramePreparation;
  createGpuTimer?: (device: GPUDevice) => RendererGpuTimer;
}
export interface RendererFeatures {
  /** Trusted comparison switch; original per-light math remains the default. */
  preparedSurfaceBrdf?: boolean;
  createCompiledSourceHost?: typeof import("./compiled-source-host.ts").createCompiledSourceHost;
  createGrass?: typeof import("./grass.ts").Grass.create;
  createSpellSwarmHost?: typeof import("./spell-swarm-host.ts").createSpellSwarmHost;
  createBattleCrowd?: typeof import("./battle-crowd.ts").BattleCrowd.create;
  loadBattleCrowdAssets?: typeof import("./battle-assets.ts").loadBattleCrowdAssets;
  prepareCanonicalSourceIndices?: typeof import("./canonical-source-indices.ts").prepareCanonicalSourceIndices;
  createMeshInspection?: typeof import("./mesh-inspection.ts").createMeshInspection;
  createMaterialEditor?: typeof import("./material-editor.ts").createMaterialEditor;
  planarScissor?: typeof import("./planar-scissor.ts").planarReceiverScissor;
  character?: {
    create: typeof CharacterRenderer.create;
    createEnemies?: typeof EnemyCharacters.create;
    load: typeof loadCharacterManifest;
    createMmorpg?: typeof import("./mmorpg-characters.ts").MmorpgCharacters.create;
    createMmorpgEffects?: typeof import("./mmorpg-effects.ts").MmorpgEffects.create;
    createMmorpgTerrain?: typeof import("./mmorpg-terrain.ts").MmorpgTerrain.create;
    createMmorpgNames?: typeof import("./mmorpg-names.ts").MmorpgNames.create;
  };
  sky?: {
    commonWGSL: string;
    backgroundWGSL: string;
    sourceDome: typeof sourceSkyDome;
  };
  createExperiment?: () => Promise<RendererExperiment>;
}
/** Trusted host configuration. Artifact data cannot provide executable functions. */
export interface RendererOptions {
  grass?: import("./grass.ts").GrassManifest;
  enemyCharacter?: CharacterManifest;
  /** Trusted diagnostics request; normal players do not request timestamp queries. */
  diagnostics?: boolean;
  experiment?: RendererExperiment;
  features?: RendererFeatures;
}
