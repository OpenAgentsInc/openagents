import {
  BATTLE_CLASSES,
  BATTLE_CLIPS,
  BATTLE_IMPOSTOR_BYTES,
  validateBattleImpostorLayout,
  type BattleCrowdClassAssets,
} from "./battle-assets.ts";
import {
  BATTLE_COMPACTION_WGSL,
  BATTLE_RENDER_WGSL,
  BATTLE_SIMULATION_WGSL,
} from "./battle-crowd-shader.ts";

import {
  BattleOrdnance,
  decodeBattleOrdnanceProbe,
  type BattleParticipantSnapshot,
} from "./battle-ordnance.ts";
import {
  BATTLE_COMBAT_BYTES,
  BATTLE_PROJECTILE_CAPACITY,
} from "./battle-combat-shader.ts";

import { BATTLE_GRID } from "./battle-grid.ts";
import {
  BATTLE_SQUAD_CAPACITY,
  BATTLE_SQUAD_INTERVAL,
  battleSquadCount,
} from "./battle-squad-shader.ts";
import {
  BattleParticipantContactSampler,
  type BattleParticipantContacts,
} from "./battle-participant-contacts.ts";
export type { BattleParticipantContacts } from "./battle-participant-contacts.ts";

export interface BattleParticipant {
  firstPerson?: boolean;
  meteorTargetValid?: boolean;
  active: boolean;
  position: readonly [number, number, number];
  yaw: number;
  team: 0 | 1;
  attackSequence: number;
  attack: "melee" | "ranged" | "meteor_shower";
  aim: readonly [number, number, number];
}
export function validateBattleParticipant(
  value: BattleParticipant,
): BattleParticipant {
  if (
    !value ||
    typeof value.active !== "boolean" ||
    (value.firstPerson !== undefined &&
      typeof value.firstPerson !== "boolean") ||
    (value.meteorTargetValid !== undefined &&
      typeof value.meteorTargetValid !== "boolean") ||
    !Array.isArray(value.position) ||
    !triple(value.position, 20000) ||
    !Array.isArray(value.aim) ||
    !triple(value.aim, 20000) ||
    !finite(value.yaw, -1e6, 1e6) ||
    (value.team !== 0 && value.team !== 1) ||
    !Number.isInteger(value.attackSequence) ||
    value.attackSequence < 0 ||
    value.attackSequence > 0xffffffff ||
    (value.attack !== "melee" &&
      value.attack !== "ranged" &&
      value.attack !== "meteor_shower")
  )
    throw new Error("Choose a complete local battle participant.");
  return {
    ...value,
    position: [value.position[0], value.position[1], value.position[2]],
    aim: [value.aim[0], value.aim[1], value.aim[2]],
  };
}

export const BATTLE_CROWD_LIMITS = Object.freeze({
  count: 1_000_000,
  actorBytes: 64,
  gridDimension: BATTLE_GRID.dimension,
  cellCapacity: 64,
  counters: 64,
  step: 1 / 60,
  stepsPerFrame: 3,
});
/**
 * Silhouette thresholds in projected pixels: `full` at Detail 1 and the
 * readability floor at Detail 0. The floor is half the 32-pixel atlas tile, so
 * a silhouette is never drawn larger than its tile resolves cleanly.
 */
export const BATTLE_SILHOUETTE_PIXELS = Object.freeze({ full: 8, floor: 16 });
const FRAME_BYTES = 336,
  COUNTER_BYTES = 256,
  BATCHES = 12;
export interface BattleCrowdTerrain {
  layout?: "mmorpg-patch";
  heights: Float32Array;
  resolution: number;
  size: number;
  center: readonly [number, number];
}
export interface BattleCrowdOptions {
  count?: number;
  capacity?: number;
  seed?: number;
  center?: readonly [number, number] | readonly [number, number, number];
  radius?: number;
  respawnSeconds?: number;
  nearDistance?: number;
  maxDistance?: number;
  format?: GPUTextureFormat;
  depthFormat?: GPUTextureFormat;
  sampleCount?: 1 | 4;
}
export interface BattleCrowdFrame {
  viewProjection: ArrayLike<number>;
  eye: readonly number[];
  time: number;
  sunDirection?: readonly number[];
  sunIlluminance?: readonly number[];
  sunColor?: readonly number[];
  ambient?: readonly number[];
  sourceFogScale?: number;
  viewportHeight?: number;
}
export interface BattleCrowdGpuCounters {
  alive: number;
  alivePlayers: number;
  aliveMonsters: number;
  dead: number;
  deadPlayers: number;
  deadMonsters: number;
  attacks: number;
  hits: number;
  deaths: number;
  respawned: number;
  damage: number;
  moving: number;
  attacking: number;
  visible: number;
  culled: number;
  near: number;
  far: number;
  visibleClasses: readonly number[];
  triangles: number;
  gridOverflows: number;
  gridOverflowsTotal: number;
  occupiedCells: number;
  maxCellPopulation: number;
  neighborVisits: number;
  invalid: number;
  ticks: number;
  healthSum: number;
  rangedActors: number;
  meleeActors: number;
  rangedShots: number;
  projectilesActive: number;
  projectileImpacts: number;
  projectileHits: number;
  projectileOverflows: number;
  siegeShots: number;
  siegeHits: number;
  participantHealth: number;
  participantDead: boolean;
  participantHits: number;
  participantKills: number;
  participantAcceptedSequence: number;
  participantAttacks: number;
  participantDamage: number;
  participantMeleeHits: number;
  projectilesVisible: number;
  participantProtectedTicks: number;
  participantEpoch: number;
  participantReceivedSequence: number;
  meteorCasts: number;
  meteorImpacts: number;
  meteorCooldownTicks: number;
  /** Occupied meteor slots: queued falls, flights, and their brief impact visuals. */
  meteorsActive: number;
  meteorHits: number;
}
export interface BattleCrowdStats {
  impostorClasses: number;
  enabled: boolean;
  count: number;
  capacity: number;
  localSimulation: true;
  seed: number;
  radius: number;
  center: readonly [number, number, number];
  submittedDraws: number;
  frameDraws: number;
  simulationSteps: number;
  droppedSeconds: number;
  accountedGpuBytes: number;
  participantResetEpoch: number;
  projectileCapacity: number;
  siegeCount: number;
  squadCount: number;
  squadCapacity: number;
  squadIntervalTicks: number;
  participant: BattleParticipant;
  sampleSerial: number;
  gpu: BattleCrowdGpuCounters | null;
}
export interface BattleCrowdProbe {
  count: number;
  finite: boolean;
  tick: number;
  sampleSha256: string;
  gpu: BattleCrowdGpuCounters;
  draws: {
    kind: string;
    lod: "near" | "far" | "impostor";
    indexCount: number;
    instanceCount: number;
  }[];
  participant: BattleParticipantSnapshot;
  projectiles: ReturnType<typeof decodeBattleOrdnanceProbe>["projectiles"];
  sieges: ReturnType<typeof decodeBattleOrdnanceProbe>["sieges"];
  samples: {
    id: number;
    kind: string;
    team: number;
    position: [number, number, number];
    yaw: number;
    health: number;
    maxHealth: number;
    state: string;
    actionTime: number;
    target: number | null;
  }[];
}
type Config = Required<Omit<BattleCrowdOptions, "radius" | "center">> & {
  radius?: number;
  center: readonly [number, number, number];
};
const finite = (value: number, min: number, max: number) =>
  typeof value === "number" &&
  Number.isFinite(value) &&
  value >= min &&
  value <= max;
const triple = (value: readonly number[], bound: number) =>
  value.length === 3 && [...value].every((n) => finite(n, -bound, bound));
export function validateBattleCrowdOptions(
  options: BattleCrowdOptions = {},
): Config {
  const center = options.center ?? [0, 0, 0];
  const result: Config = {
    count: options.count ?? 10_000,
    capacity: options.capacity ?? BATTLE_CROWD_LIMITS.count,
    seed: options.seed ?? 0xb4771e,
    center:
      center.length === 2
        ? [center[0], 0, center[1]]
        : (center as readonly [number, number, number]),
    radius: options.radius,
    respawnSeconds: options.respawnSeconds ?? 8,
    nearDistance: options.nearDistance ?? 35,
    maxDistance: options.maxDistance ?? 5000,
    format: options.format ?? "rgba16float",
    depthFormat: options.depthFormat ?? "depth24plus",
    sampleCount: options.sampleCount ?? 4,
  };
  if (
    !Number.isInteger(result.capacity) ||
    result.capacity < 1 ||
    result.capacity > BATTLE_CROWD_LIMITS.count ||
    !Number.isInteger(result.count) ||
    result.count < 0 ||
    result.count > result.capacity ||
    !Number.isInteger(result.seed) ||
    result.seed < 0 ||
    result.seed > 0xffffffff ||
    !triple(result.center, 20_000) ||
    (result.radius !== undefined &&
      (!finite(result.radius, 8, BATTLE_GRID.maxRadius) ||
        result.radius < Math.ceil(Math.sqrt(result.count)))) ||
    !finite(result.respawnSeconds, 4, 60) ||
    !finite(result.nearDistance, 1, 500) ||
    !finite(result.maxDistance, result.nearDistance, 5000) ||
    (result.sampleCount !== 1 && result.sampleCount !== 4)
  )
    throw new Error(
      "Choose up to 1,000,000 battle characters and a supported battlefield.",
    );
  return Object.freeze({
    ...result,
    center: Object.freeze([...result.center]) as Config["center"],
  });
}
export function validateBattleTerrain(terrain: BattleCrowdTerrain) {
  if (
    !terrain ||
    !(terrain.heights instanceof Float32Array) ||
    !Number.isInteger(terrain.resolution) ||
    terrain.resolution < 2 ||
    terrain.resolution > 1025 ||
    terrain.heights.length !== terrain.resolution ** 2 ||
    !finite(terrain.size, 16, 4096) ||
    !Array.isArray(terrain.center) ||
    terrain.center.length !== 2 ||
    !terrain.center.every((n) => finite(n, -20_000, 20_000)) ||
    terrain.heights.some((n) => !finite(n, -20_000, 20_000)) ||
    (terrain.layout !== undefined &&
      (terrain.layout !== "mmorpg-patch" ||
        terrain.resolution !== 257 ||
        terrain.size !== 3328 ||
        terrain.center.some((n) => n % 16 !== 0)))
  )
    throw new Error("Prepare a bounded terrain height field for the battle.");
}
export function battleTerrainHeight(
  terrain: BattleCrowdTerrain,
  x: number,
  z: number,
) {
  const n = terrain.resolution;
  if (terrain.layout === "mmorpg-patch") {
    const coordinate = (local: number) => {
      const distance = Math.abs(local);
      const index =
        128 +
        Math.sign(local) *
          (distance <= 128 ? distance / 2 : 64 + (distance - 128) / 24);
      return Math.min(256, Math.max(0, index));
    };
    return battleHeightAtGrid(
      terrain,
      coordinate(x - terrain.center[0]),
      coordinate(z - terrain.center[1]),
    );
  }
  const u =
      Math.min(1, Math.max(0, (x - terrain.center[0]) / terrain.size + 0.5)) *
      (n - 1),
    v =
      Math.min(1, Math.max(0, (z - terrain.center[1]) / terrain.size + 0.5)) *
      (n - 1);
  return battleHeightAtGrid(terrain, u, v);
}
function battleHeightAtGrid(terrain: BattleCrowdTerrain, u: number, v: number) {
  const n = terrain.resolution;
  const a = Math.floor(u),
    b = Math.floor(v),
    c = Math.min(n - 1, a + 1),
    d = Math.min(n - 1, b + 1),
    f = u - a,
    g = v - b;
  const topLeft = terrain.heights[b * n + a],
    topRight = terrain.heights[b * n + c],
    bottomLeft = terrain.heights[d * n + a],
    bottomRight = terrain.heights[d * n + c];
  return f + g <= 1
    ? topLeft + (topRight - topLeft) * f + (bottomLeft - topLeft) * g
    : bottomRight +
        (bottomLeft - bottomRight) * (1 - f) +
        (topRight - bottomRight) * (1 - g);
}
const hash = (value: number) => {
  let x = value >>> 0;
  x = Math.imul(x ^ (x >>> 16), 0x7feb352d);
  x = Math.imul(x ^ (x >>> 15), 0x846ca68b);
  return (x ^ (x >>> 16)) >>> 0;
};
const random = (value: number) => (hash(value) & 0xffffff) / 16777216;
export const battleRadius = (count: number, configured?: number) =>
  configured ??
  Math.max(
    12,
    (Math.ceil(Math.sqrt(count)) * BATTLE_GRID.formationSpacing) / 2,
  );
/** Large bodies occupy staggered slots rather than adjacent random human-width slots. */
export function battleFormationKind(
  id: number,
  count: number,
  seed: number,
): number {
  const columns = Math.ceil(Math.sqrt(count)),
    column = id % columns,
    row = Math.floor(id / columns);
  if (column >= columns / 2)
    return (column + row * 3 + (hash(seed) % 10)) % 10 === 0 ? 3 : 2;
  return random(id ^ seed ^ 0x7ac1) < 0.7 ? 0 : 1;
}
/** Only explicit starts/restarts seed CPU data; no actor arrays cross the frame loop. */
export function createBattleCrowdSeed(
  count: number,
  terrain: BattleCrowdTerrain,
  options: BattleCrowdOptions = {},
) {
  const config = validateBattleCrowdOptions({ ...options, count });
  validateBattleTerrain(terrain);
  const data = new Float32Array(count * 16),
    words = new Uint32Array(data.buffer),
    columns = Math.ceil(Math.sqrt(count)),
    rows = Math.ceil(count / columns),
    radius = battleRadius(count, config.radius),
    spacing = (2 * radius) / columns;
  for (let id = 0; id < count; id++) {
    const column = id % columns,
      row = Math.floor(id / columns),
      faction = Number(column >= columns / 2),
      kind = battleFormationKind(id, count, config.seed);
    let shiftX = 0,
      shiftZ = 0;
    if (kind !== 3) {
      for (const [dx, dz] of [
        [-1, 0],
        [1, 0],
        [0, -1],
        [0, 1],
      ]) {
        const x = column + dx,
          z = row + dz,
          neighbor = z * columns + x;
        if (
          x >= 0 &&
          x < columns &&
          z >= 0 &&
          neighbor < count &&
          battleFormationKind(neighbor, count, config.seed) === 3
        ) {
          shiftX -= dx * BATTLE_GRID.largeNeighborOffset;
          shiftZ -= dz * BATTLE_GRID.largeNeighborOffset;
        }
      }
    }
    const x =
      config.center[0] +
      (column + 0.5 - columns / 2) * spacing +
      (random(id ^ config.seed) - 0.5) * spacing * BATTLE_GRID.jitterFraction +
      shiftX;
    const z =
      config.center[2] +
      (Math.floor(id / columns) + 0.5 - rows / 2) * spacing +
      (random(id ^ config.seed ^ 0xb123) - 0.5) *
        spacing *
        BATTLE_GRID.jitterFraction +
      shiftZ;
    const offset = id * 16,
      time = random(id ^ config.seed ^ 0x5aa1) * 2;
    data.set(
      [
        x,
        battleTerrainHeight(terrain, x, z),
        z,
        faction ? -Math.PI / 2 : Math.PI / 2,
        0,
        0,
        [200, 100, 20, 1000][kind],
        time,
        0,
        0,
        time,
        0,
      ],
      offset,
    );
    words.set([kind, faction, 0xffffffff, 0], offset + 12);
  }
  return data;
}
export function decodeBattleCounters(
  words: Uint32Array,
): BattleCrowdGpuCounters {
  if (words.length < 32) throw new Error("Read complete battle counters.");
  return {
    alive: words[0] + words[1],
    alivePlayers: words[0],
    aliveMonsters: words[1],
    dead: words[2] + words[3],
    deadPlayers: words[2],
    deadMonsters: words[3],
    attacks: words[4],
    hits: words[5],
    deaths: words[6],
    respawned: words[7],
    gridOverflows: words[8],
    gridOverflowsTotal: words[54] ?? 0,
    near: words[9],
    far: words[10],
    visible: words[9] + words[10],
    ticks: words[11],
    invalid: words[12],
    moving: words[13],
    attacking: words[14],
    healthSum: words[15],
    rangedActors: words[32] ?? 0,
    meleeActors: words[33] ?? 0,
    rangedShots: words[34] ?? 0,
    projectilesActive: words[35] ?? 0,
    projectileImpacts: words[36] ?? 0,
    projectileHits: words[37] ?? 0,
    projectileOverflows: words[38] ?? 0,
    siegeShots: words[39] ?? 0,
    siegeHits: words[40] ?? 0,
    participantHealth: words[41] ?? 200,
    participantDead: (words[42] ?? 0) === 1,
    participantHits: words[43] ?? 0,
    participantKills: words[44] ?? 0,
    participantAcceptedSequence: words[45] ?? 0,
    participantAttacks: words[46] ?? 0,
    participantDamage: words[47] ?? 0,
    participantMeleeHits: words[49] ?? 0,
    projectilesVisible: words[50] ?? 0,
    participantProtectedTicks: words[51] ?? 0,
    participantEpoch: words[52] ?? 0,
    participantReceivedSequence: words[53] ?? 0,
    meteorCasts: words[55] ?? 0,
    meteorImpacts: words[56] ?? 0,
    meteorCooldownTicks: words[57] ?? 0,
    meteorsActive: words[58] ?? 0,
    meteorHits: words[59] ?? 0,
    culled: words[16],
    visibleClasses: Array.from(words.slice(17, 21)),
    damage: words[21],
    triangles: words[24],
    occupiedCells: words[25],
    neighborVisits: words[26],
    maxCellPopulation: words[27],
  };
}
/** Normalized world-space planes for WebGPU's zero-to-one depth interval. */
export function battleFrustumPlanes(matrix: ArrayLike<number>): Float32Array {
  if (
    matrix.length !== 16 ||
    Array.from(matrix).some((n) => !Number.isFinite(n))
  )
    throw new Error("Choose a complete battle camera.");
  const row = (index: number) => [
    matrix[index],
    matrix[index + 4],
    matrix[index + 8],
    matrix[index + 12],
  ];
  const x = row(0),
    y = row(1),
    z = row(2),
    w = row(3);
  const combine = (a: number[], b: number[], sign: number) =>
    a.map((n, index) => n + b[index] * sign);
  const planes = [
    combine(w, x, 1),
    combine(w, x, -1),
    combine(w, y, 1),
    combine(w, y, -1),
    z,
    combine(w, z, -1),
  ];
  return new Float32Array(
    planes.flatMap((plane) => {
      const length = Math.hypot(...plane.slice(0, 3));
      if (length < 1e-12) throw new Error("Choose a complete battle camera.");
      return plane.map((n) => n / length);
    }),
  );
}
export function battleCrowdDeviceRequirements(
  capacity: number,
  terrain: BattleCrowdTerrain,
  classes: readonly BattleCrowdClassAssets[],
) {
  validateBattleCrowdOptions({ count: 0, capacity });
  validateBattleTerrain(terrain);
  const actorBytes = capacity * 64,
    damageBytes = capacity * 8,
    visibleBytes = capacity * BATCHES * 4,
    gridBytes = BATTLE_CROWD_LIMITS.gridDimension ** 2 * 268,
    largestStorageBytes = Math.max(
      actorBytes,
      damageBytes,
      visibleBytes,
      gridBytes,
      terrain.heights.byteLength,
      BATTLE_COMBAT_BYTES,
    ),
    largestBufferBytes = Math.max(
      largestStorageBytes,
      FRAME_BYTES,
      COUNTER_BYTES,
      ...classes.flatMap((c) => [c.vertices.byteLength, c.indices.byteLength]),
    );
  return {
    actorBytes,
    damageBytes,
    visibleBytes,
    gridBytes,
    largestStorageBytes,
    largestBufferBytes,
    clearWorkgroups: Math.ceil(
      Math.max(capacity, BATTLE_CROWD_LIMITS.gridDimension ** 2) / 128,
    ),
    actorWorkgroups: Math.ceil(capacity / 128),
    limits: {
      maxBufferSize: largestBufferBytes,
      maxStorageBufferBindingSize: largestStorageBytes,
      maxStorageBuffersPerShaderStage: 8,
      maxBindingsPerBindGroup: 9,
      maxBindGroups: 2,
      maxUniformBufferBindingSize: FRAME_BYTES,
      maxComputeWorkgroupsPerDimension: Math.max(
        128,
        Math.ceil(
          Math.max(capacity, BATTLE_CROWD_LIMITS.gridDimension ** 2) / 128,
        ),
      ),
      maxComputeInvocationsPerWorkgroup: 128,
      maxComputeWorkgroupSizeX: 128,
      maxComputeWorkgroupStorageSize: 268,
    },
  };
}
/** Check the actual device before creating pipelines or allocating its battle. */
export function validateBattleCrowdDevice(
  limits: GPUSupportedLimits,
  capacity: number,
  terrain: BattleCrowdTerrain,
  classes: readonly BattleCrowdClassAssets[],
) {
  const budget = battleCrowdDeviceRequirements(capacity, terrain, classes);
  for (const [name, required] of Object.entries(budget.limits)) {
    const available = limits?.[name as keyof GPUSupportedLimits];
    if (
      typeof available !== "number" ||
      !Number.isFinite(available) ||
      available < required
    )
      throw new Error(
        `This GPU cannot prepare this battle (${name}: needs ${required}, available ${available ?? 0}). Try fewer characters or another browser.`,
      );
  }
  return budget;
}

interface Model {
  vertices: GPUBuffer;
  indices: GPUBuffer;
  group: GPUBindGroup;
  impostor: boolean;
}
interface Readback {
  buffer: GPUBuffer;
  busy: boolean;
  pending: boolean;
  generation: number;
}

/** Fixed-capacity local GPU combat, compacted source animation draws, and small async counters. */
export class BattleCrowd {
  readonly bytes: number;
  /** Detail's share of the LOD and draw distances (ADR 0007). */
  private reach = 1;
  /** Projected height, in pixels, below which a fighter draws as a silhouette. */
  private silhouettePixels: number = BATTLE_SILHOUETTE_PIXELS.full;
  private count: number;
  private radius: number;
  private front = 0;
  private generation = 0;
  private accumulator = 0;
  private dropped = 0;
  private steps = 0;
  private frames = 0;
  private submittedDraws = 0;
  private frameDraws = 0;
  private serial = 0;
  private latest: BattleCrowdGpuCounters | null = null;
  private pending = false;
  private disposed = false;
  private participantEpoch = 0;
  private participant: BattleParticipant = {
    active: false,
    position: [0, 0, 0],
    yaw: 0,
    team: 0,
    attackSequence: 0,
    attack: "melee",
    aim: [0, 0, -10],
  };
  private words = new Float32Array(FRAME_BYTES / 4);
  private uints = new Uint32Array(this.words.buffer);
  private constructor(
    private device: GPUDevice,
    private config: Config,
    private terrain: BattleCrowdTerrain,
    private ownedBuffers: GPUBuffer[],
    private ownedTextures: GPUTexture[],
    private states: [GPUBuffer, GPUBuffer],
    private uniform: GPUBuffer,
    private counters: GPUBuffer,
    private indirect: GPUBuffer,
    private models: Model[],
    private simulation: GPUComputePipeline[],
    private simulationGroups: GPUBindGroup[],
    private compaction: GPUComputePipeline[],
    private compactionGroups: GPUBindGroup[],
    private pipeline: GPURenderPipeline,
    private impostorPipeline: GPURenderPipeline,
    private quadIndices: GPUBuffer,
    private renderGroups: GPUBindGroup[][],
    private readbacks: Readback[],
    private ordnance: BattleOrdnance,
    private contactSampler: BattleParticipantContactSampler,
  ) {
    this.count = config.count;
    this.radius = battleRadius(this.count, config.radius);
    this.bytes =
      ordnance.bytes +
      contactSampler.bytes +
      ownedBuffers.reduce((sum, buffer) => sum + buffer.size, 0) +
      ownedTextures.reduce(
        (sum, texture) =>
          sum +
          texture.width *
            texture.height *
            texture.depthOrArrayLayers *
            (texture.format === "rgba16float" ? 8 : 4),
        0,
      );
    this.reset();
  }
  static async create(
    device: GPUDevice,
    classes: readonly BattleCrowdClassAssets[],
    terrain: BattleCrowdTerrain,
    options: BattleCrowdOptions = {},
  ) {
    const config = validateBattleCrowdOptions(options);
    validateBattleTerrain(terrain);
    if (
      classes.length !== 4 ||
      new Set(classes.map((model) => model.kind)).size !== 4 ||
      BATTLE_CLASSES.some(
        (kind) => !classes.some((model) => model.kind === kind),
      )
    )
      throw new Error("Prepare all four battle character classes.");
    const ordered = BATTLE_CLASSES.map((kind) =>
      classes.find((model) => model.kind === kind)!,
    );
    for (const model of ordered) {
      if (model.impostor) {
        validateBattleImpostorLayout(model.impostor, model.clips);
        if (
          model.impostor.color.byteLength !== BATTLE_IMPOSTOR_BYTES ||
          model.impostor.normal.byteLength !== BATTLE_IMPOSTOR_BYTES
        )
          throw Error("Prepare complete distant battle images.");
      }
      if (
        !model.vertices.byteLength ||
        model.vertices.byteLength % 16 ||
        model.vat.positions.byteLength !==
          model.vat.width * model.vat.height * 8 ||
        model.vat.normals.byteLength !== model.vat.positions.byteLength ||
        model.materials.alphaMode !== "OPAQUE" ||
        model.materials.rgba.byteLength !==
          model.materials.layers * 256 * 256 * 4 ||
        model.lods.length !== 2 ||
        model.lods.some(
          (lod, index) =>
            !Number.isInteger(lod.firstIndex) ||
            !Number.isInteger(lod.indexCount) ||
            lod.firstIndex < 0 ||
            lod.indexCount < 3 ||
            lod.indexCount % 3 ||
            lod.indexCount > (index ? 150 : 600) * 3 ||
            lod.firstIndex + lod.indexCount > model.indices.length,
        ) ||
        BATTLE_CLIPS.some(
          (name) =>
            !model.clips[name] ||
            !finite(model.clips[name].duration, 0.001, 120),
        )
      )
        throw new Error(
          "Prepare complete battle geometry, animation and materials.",
        );
    }
    validateBattleCrowdDevice(device.limits, config.capacity, terrain, ordered);
    const ownedBuffers: GPUBuffer[] = [],
      ownedTextures: GPUTexture[] = [];
    const buffer = (
      label: string,
      size: number,
      usage: GPUBufferUsageFlags,
    ) => {
      const result = device.createBuffer({
        label,
        size: Math.max(4, (size + 3) & ~3),
        usage,
      });
      ownedBuffers.push(result);
      return result;
    };
    const upload = (
      label: string,
      data: ArrayBuffer | ArrayBufferView<ArrayBuffer>,
      usage: GPUBufferUsageFlags,
    ) => {
      const result = buffer(
        label,
        data.byteLength,
        usage | GPUBufferUsage.COPY_DST,
      );
      if (data instanceof ArrayBuffer)
        device.queue.writeBuffer(result, 0, data);
      else
        device.queue.writeBuffer(
          result,
          0,
          data.buffer,
          data.byteOffset,
          data.byteLength,
        );
      return result;
    };
    const layout = (entries: GPUBindGroupLayoutEntry[]) =>
      device.createBindGroupLayout({ entries });
    const entry = (
      binding: number,
      type: GPUBufferBindingType,
      visibility = GPUShaderStage.COMPUTE,
    ): GPUBindGroupLayoutEntry => ({ binding, visibility, buffer: { type } });
    const bind = (binding: number, resource: GPUBuffer): GPUBindGroupEntry => ({
      binding,
      resource: { buffer: resource },
    });
    let ordnance: BattleOrdnance | undefined;
    let contactSampler: BattleParticipantContactSampler | undefined;
    try {
      const simulationLayout = layout([
        entry(0, "read-only-storage"),
        entry(1, "storage"),
        entry(2, "storage"),
        entry(3, "storage"),
        entry(4, "read-only-storage"),
        entry(5, "storage"),
        entry(6, "storage"),
        entry(7, "read-only-storage"),
        entry(8, "uniform"),
      ]);
      const compactLayout = layout([
        entry(0, "read-only-storage"),
        entry(1, "storage"),
        entry(2, "storage"),
        entry(3, "storage"),
        entry(4, "uniform"),
        entry(5, "uniform"),
      ]);
      const renderLayout = layout([
        entry(0, "uniform", GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT),
        entry(1, "read-only-storage", GPUShaderStage.VERTEX),
        entry(2, "read-only-storage", GPUShaderStage.VERTEX),
        entry(3, "uniform", GPUShaderStage.VERTEX),
      ]);
      const modelLayout = layout([
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX,
          texture: { sampleType: "float" },
        },
        {
          binding: 1,
          visibility: GPUShaderStage.VERTEX,
          texture: { sampleType: "float" },
        },
        {
          binding: 2,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        {
          binding: 3,
          visibility: GPUShaderStage.FRAGMENT,
          sampler: { type: "filtering" },
        },
        entry(4, "uniform", GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT),
        {
          binding: 5,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
        {
          binding: 6,
          visibility: GPUShaderStage.FRAGMENT,
          texture: { sampleType: "float", viewDimension: "2d-array" },
        },
      ]);
      const compute = async (
        code: string,
        names: string[],
        group: GPUBindGroupLayout,
      ) => {
        const module = device.createShaderModule({
          label: "Battle crowd: bounded GPU stages",
          code,
        });
        const pipelineLayout = device.createPipelineLayout({
          bindGroupLayouts: [group],
        });
        return Promise.all(
          names.map((entryPoint) =>
            device.createComputePipelineAsync({
              label: `Battle crowd: ${entryPoint}`,
              layout: pipelineLayout,
              compute: { module, entryPoint },
            }),
          ),
        );
      };
      const simulation = await compute(
          BATTLE_SIMULATION_WGSL,
          [
            "clear",
            "scatter",
            "commandSquads",
            "simulate",
            "participants",
            "projectiles",
          ],
          simulationLayout,
        ),
        compaction = await compute(
          BATTLE_COMPACTION_WGSL,
          ["clearDraws", "compact"],
          compactLayout,
        );
      const shader = device.createShaderModule({
        label: "Battle crowd: source vertex animation",
        code: BATTLE_RENDER_WGSL,
      });
      const pipeline = await device.createRenderPipelineAsync({
        label: "Battle crowd: source material indirect LOD",
        layout: device.createPipelineLayout({
          bindGroupLayouts: [renderLayout, modelLayout],
        }),
        vertex: {
          module: shader,
          entryPoint: "vertexMain",
          buffers: [
            {
              arrayStride: 16,
              attributes: [
                { shaderLocation: 0, offset: 0, format: "float32x2" },
                { shaderLocation: 1, offset: 8, format: "uint32" },
                { shaderLocation: 2, offset: 12, format: "uint32" },
              ],
            },
          ],
        },
        fragment: {
          module: shader,
          entryPoint: "fragmentMain",
          targets: [{ format: config.format }],
        },
        primitive: { topology: "triangle-list", cullMode: "none" },
        depthStencil: {
          format: config.depthFormat,
          depthWriteEnabled: true,
          depthCompare: "less-equal",
        },
        multisample: {
          count: config.sampleCount,
          alphaToCoverageEnabled: false,
        },
      });
      const impostorPipeline = await device.createRenderPipelineAsync({
        label: "Battle crowd: distant source silhouettes",
        layout: device.createPipelineLayout({
          bindGroupLayouts: [renderLayout, modelLayout],
        }),
        vertex: { module: shader, entryPoint: "vertexImpostor" },
        fragment: {
          module: shader,
          entryPoint: "fragmentImpostor",
          constants: { impostorSampleCount: config.sampleCount },
          targets: [{ format: config.format }],
        },
        primitive: { topology: "triangle-list", cullMode: "none" },
        depthStencil: {
          format: config.depthFormat,
          depthWriteEnabled: true,
          depthCompare: "less-equal",
        },
        multisample: {
          count: config.sampleCount,
          alphaToCoverageEnabled: config.sampleCount > 1,
        },
      });
      const quadIndices = upload(
        "Battle crowd: distant silhouette quad",
        new Uint16Array([0, 1, 2, 2, 1, 3]),
        GPUBufferUsage.INDEX,
      );
      const states = [0, 1].map((i) =>
        buffer(
          `Battle crowd: actor state ${i}`,
          config.capacity * 64,
          GPUBufferUsage.STORAGE |
            GPUBufferUsage.COPY_DST |
            GPUBufferUsage.COPY_SRC,
        ),
      ) as [GPUBuffer, GPUBuffer];
      const damage = [0, 1].map((i) =>
        buffer(
          `Battle crowd: delayed damage ${i}`,
          config.capacity * 8,
          GPUBufferUsage.STORAGE |
            GPUBufferUsage.COPY_DST |
            GPUBufferUsage.COPY_SRC,
        ),
      );
      const cells = buffer(
        "Battle crowd: spatial cells",
        BATTLE_CROWD_LIMITS.gridDimension ** 2 * 268,
        GPUBufferUsage.STORAGE,
      );
      const heights = upload(
        "Battle crowd: terrain heights",
        terrain.heights as Float32Array<ArrayBuffer>,
        GPUBufferUsage.STORAGE,
      );
      const uniform = buffer(
        "Battle crowd: frame and simulation parameters",
        FRAME_BYTES,
        GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      );
      const counters = buffer(
        "Battle crowd: actual GPU counters",
        COUNTER_BYTES,
        GPUBufferUsage.STORAGE |
          GPUBufferUsage.COPY_SRC |
          GPUBufferUsage.COPY_DST,
      );
      const visible = buffer(
        "Battle crowd: compacted visible actor IDs",
        config.capacity * BATCHES * 4,
        GPUBufferUsage.STORAGE,
      );
      const args = new Uint32Array(BATCHES * 5);
      ordered.forEach((model, kind) =>
        model.lods.forEach((lod, level) =>
          args.set(
            [lod.indexCount, 0, lod.firstIndex, 0, 0],
            (kind * 2 + level) * 5,
          ),
        ),
      );
      for (let kind = 0; kind < 4; kind++)
        args.set([6, 0, 0, 0, 0], (8 + kind) * 5);
      const indirect = upload(
        "Battle crowd: indexed indirect arguments",
        args,
        GPUBufferUsage.STORAGE |
          GPUBufferUsage.INDIRECT |
          GPUBufferUsage.COPY_SRC,
      );
      ordnance = await BattleOrdnance.create(device, uniform, counters, config);
      contactSampler = await BattleParticipantContactSampler.create(
        device,
        states,
        cells,
        ordnance.arena,
        counters,
        uniform,
      );
      const simulationGroups = states.map((state, index) =>
        device.createBindGroup({
          layout: simulationLayout,
          entries: [
            bind(0, state),
            bind(1, states[1 - index]),
            bind(2, cells),
            bind(3, ordnance!.arena),
            bind(4, damage[index]),
            bind(5, damage[1 - index]),
            bind(6, counters),
            bind(7, heights),
            bind(8, uniform),
          ],
        }),
      );
      const modelInfo = upload(
        "Battle crowd: projected character sizes",
        new Float32Array(
          ordered.flatMap((model) => [
            model.height,
            Number(!!model.impostor),
            0,
            0,
          ]),
        ),
        GPUBufferUsage.UNIFORM,
      );
      const compactionGroups = states.map((state) =>
        device.createBindGroup({
          layout: compactLayout,
          entries: [
            bind(0, state),
            bind(1, visible),
            bind(2, indirect),
            bind(3, counters),
            bind(4, uniform),
            bind(5, modelInfo),
          ],
        }),
      );
      const drawUniforms = Array.from({ length: BATCHES }, (_, batch) =>
        upload(
          `Battle crowd: visible batch ${batch}`,
          new Uint32Array([batch * config.capacity, 0, 0, 0]),
          GPUBufferUsage.UNIFORM,
        ),
      );
      const renderGroups = states.map((state) =>
        drawUniforms.map((draw) =>
          device.createBindGroup({
            layout: renderLayout,
            entries: [
              bind(0, uniform),
              bind(1, state),
              bind(2, visible),
              bind(3, draw),
            ],
          }),
        ),
      );
      const sampler = device.createSampler({
        magFilter: "linear",
        minFilter: "linear",
        addressModeU: "repeat",
        addressModeV: "repeat",
      });
      const models = ordered.map((model): Model => {
        const texture = (
          label: string,
          width: number,
          height: number,
          layers: number,
          format: GPUTextureFormat,
          data: Uint8Array<ArrayBuffer> | Uint16Array<ArrayBuffer>,
          stride: number,
        ) => {
          const image = device.createTexture({
            label,
            size: [width, height, layers],
            format,
            usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
          });
          ownedTextures.push(image);
          device.queue.writeTexture(
            { texture: image },
            data,
            { bytesPerRow: width * stride, rowsPerImage: height },
            [width, height, layers],
          );
          return image;
        };
        const position = texture(
            `Battle crowd: ${model.kind} animated positions`,
            model.vat.width,
            model.vat.height,
            1,
            "rgba16float",
            model.vat.positions,
            8,
          ),
          normal = texture(
            `Battle crowd: ${model.kind} animated normals`,
            model.vat.width,
            model.vat.height,
            1,
            "rgba16float",
            model.vat.normals,
            8,
          );
        const materials = texture(
          `Battle crowd: ${model.kind} source materials`,
          256,
          256,
          model.materials.layers,
          "rgba8unorm-srgb",
          model.materials.rgba,
          4,
        );
        const sprite = model.impostor;
        const spriteColor = texture(
          `Battle crowd: ${model.kind} distant albedo`,
          sprite ? 512 : 1,
          sprite ? 512 : 1,
          sprite ? 8 : 1,
          "rgba8unorm-srgb",
          sprite?.color ?? new Uint8Array(4),
          4,
        );
        const spriteNormal = texture(
          `Battle crowd: ${model.kind} distant normals`,
          sprite ? 512 : 1,
          sprite ? 512 : 1,
          sprite ? 8 : 1,
          "rgba8unorm",
          sprite?.normal ?? new Uint8Array([128, 128, 255, 255]),
          4,
        );
        const data = new Float32Array(48),
          uints = new Uint32Array(data.buffer);
        BATTLE_CLIPS.forEach((name, index) => {
          const clip = model.clips[name];
          data.set(
            [
              clip.duration,
              clip.firstFrame,
              clip.frameCount,
              Number(clip.loop),
            ],
            index * 4,
          );
        });
        uints.set(
          [model.vat.width, model.vat.height, model.materials.layers, 0],
          16,
        );
        data[20] = model.height;
        if (sprite) {
          BATTLE_CLIPS.forEach((name, index) => {
            const clip = sprite.clips[name];
            data.set(
              [
                clip.duration,
                clip.firstFrame,
                clip.frameCount,
                Number(clip.loop),
              ],
              24 + index * 4,
            );
          });
          uints.set(
            [sprite.width, sprite.height, sprite.views, sprite.tileSize],
            40,
          );
          data.set(
            [sprite.bounds.radius, sprite.bounds.minY, sprite.bounds.maxY, 0],
            44,
          );
        }
        const settings = upload(
          `Battle crowd: ${model.kind} clip table`,
          data,
          GPUBufferUsage.UNIFORM,
        );
        const group = device.createBindGroup({
          layout: modelLayout,
          entries: [
            { binding: 0, resource: position.createView() },
            { binding: 1, resource: normal.createView() },
            {
              binding: 2,
              resource: materials.createView({ dimension: "2d-array" }),
            },
            { binding: 3, resource: sampler },
            bind(4, settings),
            {
              binding: 5,
              resource: spriteColor.createView({ dimension: "2d-array" }),
            },
            {
              binding: 6,
              resource: spriteNormal.createView({ dimension: "2d-array" }),
            },
          ],
        });
        return {
          vertices: upload(
            `Battle crowd: ${model.kind} source LOD vertices`,
            model.vertices,
            GPUBufferUsage.VERTEX,
          ),
          indices: upload(
            `Battle crowd: ${model.kind} source LOD indices`,
            model.indices,
            GPUBufferUsage.INDEX,
          ),
          group,
          impostor: !!sprite,
        };
      });
      const readbacks = Array.from({ length: 3 }, () => ({
        buffer: buffer(
          "Battle crowd: asynchronous counter readback",
          COUNTER_BYTES,
          GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        ),
        busy: false,
        pending: false,
        generation: 0,
      }));
      return new BattleCrowd(
        device,
        config,
        terrain,
        ownedBuffers,
        ownedTextures,
        states,
        uniform,
        counters,
        indirect,
        models,
        simulation,
        simulationGroups,
        compaction,
        compactionGroups,
        pipeline,
        impostorPipeline,
        quadIndices,
        renderGroups,
        readbacks,
        ordnance,
        contactSampler,
      );
    } catch (error) {
      contactSampler?.dispose();
      ordnance?.dispose();
      ownedBuffers.forEach((value) => value.destroy());
      ownedTextures.forEach((value) => value.destroy());
      throw error;
    }
  }
  private assertOpen() {
    if (this.disposed) throw new Error("This battle has closed.");
  }
  private reset() {
    this.contactSampler.invalidate();
    this.generation++;
    this.front = 0;
    this.accumulator = this.dropped = this.steps = 0;
    this.latest = null;
    this.serial = 0;
    const seed = createBattleCrowdSeed(this.count, this.terrain, this.config);
    if (seed.byteLength)
      for (const state of this.states)
        this.device.queue.writeBuffer(state, 0, seed);
    const counters = new Uint32Array(64);
    counters[41] = 200;
    this.device.queue.writeBuffer(this.counters, 0, counters);
    this.ordnance.reset(
      this.config.center,
      this.radius,
      (x, z) => battleTerrainHeight(this.terrain, x, z),
      this.participant.attackSequence,
    );
    // New damage buffers start at zero; switching count also clears previous strikes.
    for (const value of this.ownedBuffers)
      if (value.label.startsWith("Battle crowd: delayed damage"))
        this.device.queue.writeBuffer(
          value,
          0,
          new Uint32Array(this.config.capacity * 2),
        );
  }
  /** Scale the LOD tier and draw distances for Detail; 1 is configured. */
  setReach(reach: number) {
    if (!Number.isFinite(reach) || reach < 0.1 || reach > 1)
      throw new Error("Use a crowd reach from 0.1 to 1.");
    this.reach = reach;
  }
  /**
   * Set the silhouette threshold for Detail. It stays between the full value
   * and the readability floor, so a fighter taller than the floor always
   * keeps its mesh.
   */
  setSilhouettePixels(pixels: number) {
    if (
      !Number.isFinite(pixels) ||
      pixels < BATTLE_SILHOUETTE_PIXELS.full ||
      pixels > BATTLE_SILHOUETTE_PIXELS.floor
    )
      throw new Error(
        `Use a silhouette threshold from ${BATTLE_SILHOUETTE_PIXELS.full} to ${BATTLE_SILHOUETTE_PIXELS.floor} pixels.`,
      );
    this.silhouettePixels = pixels;
  }
  setCount(count: number) {
    this.assertOpen();
    if (this.pending)
      throw new Error("Submit the current battle frame before restarting it.");
    if (!Number.isInteger(count) || count < 0 || count > this.config.capacity)
      throw new Error("Choose a battle count within its prepared capacity.");
    this.count = count;
    this.radius = battleRadius(count, this.config.radius);
    this.reset();
  }
  setParticipant(value: BattleParticipant) {
    this.assertOpen();
    if (this.participant.active && !value.active)
      this.contactSampler.invalidate();
    this.participant = validateBattleParticipant(value);
  }
  resetParticipant() {
    this.assertOpen();
    if (this.pending) throw new Error("Submit the battle before respawning.");
    this.contactSampler.invalidate();
    const epoch = ++this.participantEpoch;
    this.ordnance.resetParticipant(this.participant.attackSequence, epoch);
    return epoch;
  }
  prepare(
    encoder: GPUCommandEncoder,
    frame: BattleCrowdFrame,
    elapsed: number,
  ) {
    this.assertOpen();
    if (this.pending)
      throw new Error(
        "Submit the current battle frame before preparing another.",
      );
    if (
      !finite(elapsed, 0, 1e6) ||
      !Number.isFinite(frame.time) ||
      frame.viewProjection.length !== 16 ||
      [...Array.from(frame.viewProjection), ...frame.eye].some(
        (n) => !finite(n, -1e9, 1e9),
      ) ||
      frame.eye.length !== 3
    )
      throw new Error("Choose a complete battle camera and elapsed time.");
    const sun = frame.sunDirection ?? [-0.3, 0.87, 0.38],
      color = frame.sunIlluminance ?? frame.sunColor ?? [2, 1.8, 1.6],
      ambient = frame.ambient ?? [0.3, 0.35, 0.45];
    if (![sun, color, ambient].every((value) => triple(value, 1e6)))
      throw new Error("Choose complete battle lighting.");
    const sourceFogScale = frame.sourceFogScale ?? 1;
    if (!finite(sourceFogScale, 1, 100))
      throw new Error("Choose a supported battle fog distance.");
    const viewportHeight = frame.viewportHeight ?? 720;
    if (!finite(viewportHeight, 1, 32768))
      throw Error("Choose a supported battle viewport height.");
    const pixelScale =
      (viewportHeight *
        Math.hypot(
          frame.viewProjection[1],
          frame.viewProjection[5],
          frame.viewProjection[9],
        )) /
      2;
    this.words.set(Array.from(frame.viewProjection), 0);
    this.words.set([...frame.eye, frame.time], 16);
    this.words.set(
      [this.config.center[0], this.config.center[2], this.radius, 1 / 60],
      20,
    );
    this.uints.set(
      [
        this.count,
        this.config.capacity,
        this.config.seed,
        Number(this.terrain.layout === "mmorpg-patch"),
      ],
      24,
    );
    this.words.set(
      [...this.terrain.center, this.terrain.size, this.terrain.resolution],
      28,
    );
    this.words.set(
      [
        this.config.nearDistance * this.reach,
        Math.max(
          this.config.nearDistance * this.reach,
          this.config.maxDistance * this.reach,
        ),
        this.config.respawnSeconds,
        pixelScale,
      ],
      32,
    );
    this.words.set([...sun, this.silhouettePixels], 36);
    this.words.set([...color, 0], 40);
    this.words.set([...ambient, sourceFogScale], 44);
    this.words.set(battleFrustumPlanes(frame.viewProjection), 48);
    this.words.set([...this.participant.position, this.participant.yaw], 72);
    this.words.set(
      [
        ...this.participant.aim,
        Number(this.participant.firstPerson ?? false) +
          2 * Number(this.participant.meteorTargetValid ?? false),
      ],
      76,
    );
    this.uints.set(
      [
        this.participant.team,
        Number(this.participant.active),
        this.participant.attackSequence,
        this.participant.attack === "meteor_shower"
          ? 2
          : Number(this.participant.attack === "ranged"),
      ],
      80,
    );
    this.device.queue.writeBuffer(this.uniform, 0, this.words);
    const accepted = Math.min(elapsed, 0.1);
    this.dropped += elapsed - accepted;
    this.accumulator += accepted;
    const steps = Math.min(
      BATTLE_CROWD_LIMITS.stepsPerFrame,
      Math.floor((this.accumulator + 1e-10) / BATTLE_CROWD_LIMITS.step),
    );
    this.accumulator -= steps * BATTLE_CROWD_LIMITS.step;
    if (this.accumulator >= BATTLE_CROWD_LIMITS.step) {
      const rest = this.accumulator % BATTLE_CROWD_LIMITS.step;
      this.dropped += this.accumulator - rest;
      this.accumulator = rest;
    }
    if (this.count && steps) {
      const pass = encoder.beginComputePass({
        label: "Battle crowd: grid, pursuit and combat",
      });
      for (let step = 0; step < steps; step++) {
        pass.setBindGroup(0, this.simulationGroups[this.front]);
        pass.setPipeline(this.simulation[0]);
        pass.dispatchWorkgroups(
          Math.ceil(
            Math.max(BATTLE_CROWD_LIMITS.gridDimension ** 2, this.count) / 128,
          ),
        );
        pass.setPipeline(this.simulation[1]);
        pass.dispatchWorkgroups(Math.ceil(this.count / 128));
        pass.setPipeline(this.simulation[2]);
        pass.dispatchWorkgroups(Math.ceil(battleSquadCount(this.count) / 128));
        pass.setPipeline(this.simulation[3]);
        pass.dispatchWorkgroups(Math.ceil(this.count / 128));
        pass.setPipeline(this.simulation[4]);
        pass.dispatchWorkgroups(1);
        pass.setPipeline(this.simulation[5]);
        pass.dispatchWorkgroups(BATTLE_PROJECTILE_CAPACITY / 128);
        this.front = 1 - this.front;
        this.steps++;
      }
      pass.end();
    }
    const compact = encoder.beginComputePass({
      label: "Battle crowd: frustum and LOD indirect compaction",
    });
    compact.setBindGroup(0, this.compactionGroups[this.front]);
    compact.setPipeline(this.compaction[0]);
    compact.dispatchWorkgroups(1);
    if (this.count) {
      compact.setPipeline(this.compaction[1]);
      compact.dispatchWorkgroups(Math.ceil(this.count / 128));
    }
    compact.end();
    this.ordnance.prepare(encoder);
    this.contactSampler.prepare(
      encoder,
      this.front,
      this.participant.active && this.count > 0 && this.steps > 0,
      this.count,
      this.participantEpoch,
    );
    const slot = this.readbacks.find((value) => !value.busy && !value.pending);
    if (slot && this.frames % 30 === 0) {
      slot.pending = true;
      slot.generation = this.generation;
      encoder.copyBufferToBuffer(
        this.counters,
        0,
        slot.buffer,
        0,
        COUNTER_BYTES,
      );
    }
    this.frames++;
    this.frameDraws = 0;
    this.pending = true;
  }
  render(pass: GPURenderPassEncoder) {
    this.assertOpen();
    if (!this.pending) throw new Error("Prepare the battle before drawing it.");
    if (!this.count) return;
    pass.setPipeline(this.pipeline);
    for (let kind = 0; kind < this.models.length; kind++) {
      const model = this.models[kind];
      pass.setBindGroup(1, model.group);
      pass.setVertexBuffer(0, model.vertices);
      pass.setIndexBuffer(model.indices, "uint32");
      for (let lod = 0; lod < 2; lod++) {
        const batch = kind * 2 + lod;
        pass.setBindGroup(0, this.renderGroups[this.front][batch]);
        pass.drawIndexedIndirect(this.indirect, batch * 20);
        this.submittedDraws++;
        this.frameDraws++;
      }
    }
    pass.setPipeline(this.impostorPipeline);
    pass.setIndexBuffer(this.quadIndices, "uint16");
    for (let kind = 0; kind < this.models.length; kind++) {
      const model = this.models[kind];
      if (!model.impostor) continue;
      pass.setBindGroup(1, model.group);
      pass.setBindGroup(0, this.renderGroups[this.front][8 + kind]);
      pass.drawIndexedIndirect(this.indirect, (8 + kind) * 20);
      this.submittedDraws++;
      this.frameDraws++;
    }
    const extraDraws = this.ordnance.render(
      pass,
      this.participant.active && !!this.participant.firstPerson,
    );
    this.frameDraws += extraDraws;
    this.submittedDraws += extraDraws;
  }
  afterSubmit() {
    if (this.disposed || !this.pending) return;
    this.pending = false;
    this.contactSampler.afterSubmit();
    for (const slot of this.readbacks)
      if (slot.pending) {
        slot.pending = false;
        slot.busy = true;
        void slot.buffer
          .mapAsync(GPUMapMode.READ)
          .then(() => {
            if (!this.disposed && slot.generation === this.generation) {
              this.latest = decodeBattleCounters(
                new Uint32Array(slot.buffer.getMappedRange().slice(0)),
              );
              this.serial++;
            }
            slot.buffer.unmap();
          })
          .catch(() => {})
          .finally(() => {
            slot.busy = false;
          });
      }
  }
  latestParticipantContacts(): BattleParticipantContacts | null {
    return this.contactSampler.read();
  }
  inspect(): BattleCrowdStats {
    return {
      enabled: !this.disposed && this.count > 0,
      count: this.count,
      capacity: this.config.capacity,
      localSimulation: true,
      seed: this.config.seed,
      center: [...this.config.center],
      radius: this.radius,
      submittedDraws: this.submittedDraws,
      frameDraws: this.frameDraws,
      impostorClasses: this.models.filter((model) => model.impostor).length,
      simulationSteps: this.steps,
      droppedSeconds: this.dropped,
      accountedGpuBytes: this.disposed ? 0 : this.bytes,
      participantResetEpoch: this.participantEpoch,
      projectileCapacity: BATTLE_PROJECTILE_CAPACITY,
      siegeCount: 8,
      squadCount: battleSquadCount(this.count),
      squadCapacity: BATTLE_SQUAD_CAPACITY,
      squadIntervalTicks: BATTLE_SQUAD_INTERVAL,
      participant: { ...this.participant },
      sampleSerial: this.serial,
      gpu: this.latest,
    };
  }
  async probe(): Promise<BattleCrowdProbe> {
    this.assertOpen();
    if (this.pending)
      throw new Error("Submit the battle before reading its state.");
    const generation = this.generation,
      count = this.count,
      ids = new Set<number>();
    for (let id = 0; id < Math.min(4, count); id++) ids.add(id);
    const columns = Math.ceil(Math.sqrt(count)),
      rows = Math.ceil(count / columns);
    for (let index = 0; index < 8 && count > 0; index++) {
      const row = Math.min(
        rows - 1,
        Math.floor(((rows - 1) * (index + 0.5)) / 8),
      );
      const column = Math.max(0, Math.floor(columns / 2) - 1 + (index % 2)),
        id = row * columns + column;
      if (id < count) ids.add(id);
    }
    for (let index = 0; ids.size < Math.min(32, count); index++)
      ids.add(hash(index ^ this.config.seed) % count);
    const selected = [...ids],
      actorBytes = COUNTER_BYTES + selected.length * 64,
      drawOffset = actorBytes + BATTLE_COMBAT_BYTES,
      bytes = drawOffset + BATCHES * 20;
    const read = this.device.createBuffer({
      label: "Battle crowd: explicit sampled state probe",
      size: bytes,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    try {
      const encoder = this.device.createCommandEncoder();
      encoder.copyBufferToBuffer(this.counters, 0, read, 0, COUNTER_BYTES);
      selected.forEach((id, index) =>
        encoder.copyBufferToBuffer(
          this.states[this.front],
          id * 64,
          read,
          COUNTER_BYTES + index * 64,
          64,
        ),
      );
      encoder.copyBufferToBuffer(
        this.ordnance.arena,
        0,
        read,
        actorBytes,
        BATTLE_COMBAT_BYTES,
      );
      encoder.copyBufferToBuffer(
        this.indirect,
        0,
        read,
        drawOffset,
        BATCHES * 20,
      );
      this.device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      if (this.disposed || generation !== this.generation)
        throw new Error("The battle changed while its state was being read.");
      const raw = read.getMappedRange().slice(0),
        counters = decodeBattleCounters(new Uint32Array(raw, 0, 64));
      let finiteState = true;
      const samples = selected.map((id, index) => {
        const floats = new Float32Array(raw, COUNTER_BYTES + index * 64, 16),
          words = new Uint32Array(floats.buffer, floats.byteOffset, 16);
        if (
          floats.slice(0, 12).some((n) => !Number.isFinite(n)) ||
          words[12] > 3 ||
          words[13] > 1 ||
          words[15] > 3
        )
          finiteState = false;
        return {
          id,
          kind: BATTLE_CLASSES[words[12]] ?? "invalid",
          team: words[13],
          position: Array.from(floats.slice(0, 3)) as [number, number, number],
          yaw: floats[3],
          health: floats[6],
          maxHealth: [200, 100, 50, 1000][words[12]] ?? 0,
          state: ["idle", "walk", "attack", "death"][words[15]] ?? "invalid",
          actionTime: floats[7],
          target: words[14] < count ? words[14] : null,
        };
      });
      const digest = await crypto.subtle.digest(
        "SHA-256",
        new Uint8Array(raw, COUNTER_BYTES),
      );
      return {
        count,
        finite: finiteState,
        tick: counters.ticks,
        sampleSha256: Array.from(new Uint8Array(digest), (n) =>
          n.toString(16).padStart(2, "0"),
        ).join(""),
        gpu: counters,
        draws: Array.from({ length: BATCHES }, (_, batch) => {
          const args = new Uint32Array(raw, drawOffset + batch * 20, 5);
          return {
            kind: BATTLE_CLASSES[batch < 8 ? Math.floor(batch / 2) : batch - 8],
            lod:
              batch >= 8
                ? ("impostor" as const)
                : batch % 2
                  ? ("far" as const)
                  : ("near" as const),
            indexCount: args[0],
            instanceCount: args[1],
          };
        }),
        ...decodeBattleOrdnanceProbe(raw, actorBytes),
        samples,
      };
    } finally {
      if (read.mapState === "mapped") read.unmap();
      read.destroy();
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.pending = false;
    this.generation++;
    this.contactSampler.dispose();
    this.ordnance.dispose();
    this.ownedBuffers.forEach((value) => value.destroy());
    this.ownedTextures.forEach((value) => value.destroy());
    this.latest = null;
  }
}
