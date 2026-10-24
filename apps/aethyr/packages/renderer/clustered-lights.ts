import type { SourceLight } from "../assets/src/scene.ts";
import { lightColor, lightIntensity } from "./lighting.ts";
import { inverse, type V3 } from "./math.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { ClusterFrusta } from "./cluster-frusta.ts";

export const CLUSTER_OVERFLOW = 0xffffffff;
export const CLUSTER_LIGHT_FLOATS = 16;
export const CLUSTER_CELL_WORDS = 2;
export const CLUSTER_LIMITS = Object.freeze({
  lights: 4096,
  viewport: 16384,
  cells: 262144,
  indices: 4194304,
  memberships: 16777216,
  depthSlices: 64,
});

/** All offsets count 32-bit words, including the float lanes. */
export const CLUSTER_HEADER = Object.freeze({
  words: 20,
  lightCount: 0,
  lightsOffset: 1,
  cellsOffset: 2,
  indicesOffset: 3,
  tilesX: 4,
  tilesY: 5,
  depthSlices: 6,
  tileSize: 7,
  width: 8,
  height: 9,
  near: 10,
  far: 11,
  logDepthScale: 12,
  indexCount: 13,
  overflowCellCount: 14,
  forwardX: 16,
  forwardY: 17,
  forwardZ: 18,
});

export interface ClusteredLightInput {
  /** Column-major WebGPU view-projection matrix, with clip depth in [0, w]. */
  viewProjection: ArrayLike<number>;
  eye: ArrayLike<number>;
  /** World-space camera forward, independent of screen reflection or roll. */
  forward: ArrayLike<number>;
  width: number;
  height: number;
  lights: readonly SourceLight[];
  /** Frame-light indices for the independently selected shadow casters. */
  shadowIndices?: ReadonlyMap<string, number>;
  tileSize?: number;
  depthSlices?: number;
  near?: number;
  far?: number;
  maxIndices?: number;
}

export interface ClusteredLights {
  /** GPU storage: header, light records, cell records, then light indices. */
  storage: Uint32Array;
  /** Shared views into storage; none of these sections allocate another copy. */
  lightFloats: Float32Array;
  cells: Uint32Array;
  indices: Uint32Array;
  sourceIndices: Uint32Array;
  lightIds: string[];
  viewProjection: Float64Array;
  eye: V3;
  forward: V3;
  grid: {
    width: number;
    height: number;
    tileSize: number;
    tilesX: number;
    tilesY: number;
    depthSlices: number;
    near: number;
    far: number;
    logDepthScale: number;
  };
  stats: {
    candidateMemberships: number;
    indexCount: number;
    overflowCellCount: number;
    /** Exact except in global fallback, where it is a conservative upper bound. */
    maxCellLightCount: number;
    globalOverflow: boolean;
    byteLength: number;
  };
}

function integer(
  value: number,
  name: string,
  min: number,
  max: number,
): number {
  if (!Number.isSafeInteger(value) || value < min || value > max)
    throw new Error(`Use ${name} between ${min} and ${max}.`);
  return value;
}

function vector(value: ArrayLike<number>, name: string): V3 {
  if (value.length !== 3 || !Array.from(value).every(Number.isFinite))
    throw new Error(`Use three finite ${name} coordinates.`);
  return Array.from(value) as V3;
}

function unit(value: ArrayLike<number>, name: string): V3 {
  const v = vector(value, name);
  const length = Math.hypot(...v);
  if (!Number.isFinite(length) || length < 1e-12)
    throw new Error(`Use a nonzero finite ${name} direction.`);
  return v.map((n) => n / length) as V3;
}

function packed(value: number, name: string): number {
  const f32 = Math.fround(value);
  if (!Number.isFinite(value) || !Number.isFinite(f32))
    throw new Error(`Use a finite float32 ${name}.`);
  return f32;
}

function clip(matrix: ArrayLike<number>, p: ArrayLike<number>): number[] {
  return [0, 1, 2, 3].map(
    (r) =>
      matrix[r] * p[0] +
      matrix[r + 4] * p[1] +
      matrix[r + 8] * p[2] +
      matrix[r + 12],
  );
}

function depthSlice(depth: number, grid: ClusteredLights["grid"]): number {
  return Math.min(
    grid.depthSlices - 1,
    Math.max(
      0,
      Math.floor(
        Math.log(Math.max(depth, grid.near) / grid.near) * grid.logDepthScale,
      ),
    ),
  );
}

/**
 * The sphere's world AABB encloses every illuminated point. With positive clip
 * w, extrema of each projected coordinate occur at its corners. A box crossing
 * the near plane or w=0 deliberately occupies the full viewport instead.
 */
function lightBounds(
  record: Float32Array,
  camera: Pick<ClusteredLights, "viewProjection" | "eye" | "forward" | "grid">,
): number[] | undefined {
  const { viewProjection: matrix, grid, eye, forward } = camera;
  const [x, y, z, range] = record;
  // Cover float32 projection and interpolation roundoff at tile/slice edges.
  const padding =
    1e-5 * Math.max(1, Math.abs(x), Math.abs(y), Math.abs(z), range);
  const radius = range + padding;
  const depth =
    (x - eye[0]) * forward[0] +
    (y - eye[1]) * forward[1] +
    (z - eye[2]) * forward[2];
  // Slice limits clamp visible fragments; they are not extra clipping planes.
  // A float32 projection's actual far plane can differ from its authored far.
  const z0 = depthSlice(depth - radius, grid);
  const z1 = depthSlice(depth + radius, grid);
  let minX = Infinity,
    minY = Infinity,
    maxX = -Infinity,
    maxY = -Infinity,
    minW = Infinity,
    minZ = Infinity,
    clipMagnitude = 1,
    maxLeft = -Infinity,
    maxRight = -Infinity,
    maxBottom = -Infinity,
    maxTop = -Infinity,
    maxNear = -Infinity,
    maxFar = -Infinity;
  for (let i = 0; i < 8; i++) {
    const px = x + (i & 1 ? radius : -radius),
      py = y + (i & 2 ? radius : -radius),
      pz = z + (i & 4 ? radius : -radius);
    const cx = matrix[0] * px + matrix[4] * py + matrix[8] * pz + matrix[12],
      cy = matrix[1] * px + matrix[5] * py + matrix[9] * pz + matrix[13],
      cz = matrix[2] * px + matrix[6] * py + matrix[10] * pz + matrix[14],
      cw = matrix[3] * px + matrix[7] * py + matrix[11] * pz + matrix[15];
    clipMagnitude = Math.max(
      clipMagnitude,
      Math.abs(cx),
      Math.abs(cy),
      Math.abs(cz),
      Math.abs(cw),
    );
    minW = Math.min(minW, cw);
    minZ = Math.min(minZ, cz);
    maxLeft = Math.max(maxLeft, cx + cw);
    maxRight = Math.max(maxRight, cw - cx);
    maxBottom = Math.max(maxBottom, cy + cw);
    maxTop = Math.max(maxTop, cw - cy);
    maxNear = Math.max(maxNear, cz);
    maxFar = Math.max(maxFar, cw - cz);
    if (cw > 0) {
      minX = Math.min(minX, cx / cw);
      maxX = Math.max(maxX, cx / cw);
      minY = Math.min(minY, cy / cw);
      maxY = Math.max(maxY, cy / cw);
    }
  }
  const epsilon = 1e-5 * clipMagnitude;
  // Six homogeneous half-spaces also handle off-axis and reflected cameras.
  if (
    Math.min(maxLeft, maxRight, maxBottom, maxTop, maxNear, maxFar) < -epsilon
  )
    return;
  if (minW <= epsilon || minZ <= epsilon)
    return [0, grid.tilesX - 1, 0, grid.tilesY - 1, z0, z1];
  const edge =
    1e-5 *
    Math.max(1, Math.abs(minX), Math.abs(maxX), Math.abs(minY), Math.abs(maxY));
  const tile = (pixel: number, count: number) =>
    Math.min(count - 1, Math.max(0, Math.floor(pixel / grid.tileSize)));
  return [
    tile(((minX - edge + 1) / 2) * grid.width, grid.tilesX),
    tile(((maxX + edge + 1) / 2) * grid.width, grid.tilesX),
    tile(((1 - maxY - edge) / 2) * grid.height, grid.tilesY),
    tile(((1 - minY + edge) / 2) * grid.height, grid.tilesY),
    z0,
    z1,
  ];
}

/**
 * Build conservative tile/log-depth lists without selecting nearest lights.
 * Cell count CLUSTER_OVERFLOW requires an all-light shader loop, never a partial
 * list. Excessive CPU assignment work uses the same complete-grid fallback.
 *
 * Light record (16 float32 lanes): position.xyz/range, RGB/strength,
 * emittedDirection.xyz/kind (0 point, 1 spot), innerCos/outerCos/shadowFrameIndex/
 * falloffExponent (-1 physical). Strength includes SCENE_LINEAR_SCALE exactly once.
 */
export function buildClusteredLights(
  input: ClusteredLightInput,
): ClusteredLights {
  return buildClusteredLightLists(input, false);
}

/** Tighter conservative lists; the reference builder retains its original algorithm. */
export function buildTightClusteredLights(
  input: ClusteredLightInput,
): ClusteredLights {
  return buildClusteredLightLists(input, true);
}

function prepareLightRecords(
  source: readonly SourceLight[],
  shadowIndices?: ReadonlyMap<string, number>,
) {
  integer(source.length, "source light count", 0, CLUSTER_LIMITS.lights);
  const sourceIndices: number[] = [],
    lightIds: string[] = [],
    records: number[] = [],
    shadows: boolean[] = [];
  const ids = new Set<string>();
  for (const [sourceIndex, light] of source.entries()) {
    if (
      !["point", "spot", "directional"].includes(light.kind) ||
      typeof light.enabled !== "boolean"
    )
      throw new Error("Use a supported light kind and enabled flag.");
    if (!light.enabled || light.kind === "directional") continue;
    if (typeof light.id !== "string" || !light.id || ids.has(light.id))
      throw new Error("Use a distinct, nonempty local light ID.");
    ids.add(light.id);
    const position = vector(light.position, "light position").map((n) =>
      packed(n, "light position"),
    );
    const range = packed(light.range ?? 10, "light range");
    if (range <= 0) throw new Error("Use a positive light range.");
    if (vector(light.color, "light color").some((n) => n < 0))
      throw new Error("Use nonnegative light colors.");
    const color = lightColor(light).map((n) => packed(n, "light color"));
    if (color.some((n) => n < 0))
      throw new Error("Use nonnegative light colors.");
    const strength = packed(
      lightIntensity(light) * SCENE_LINEAR_SCALE,
      "light strength",
    );
    const direction =
      light.kind === "spot" ? unit(light.direction, "spotlight") : [0, -1, 0];
    const inner = light.innerConeAngleRadians ?? 0;
    const outer = light.outerConeAngleRadians ?? Math.PI / 4;
    if (
      light.kind === "spot" &&
      (!Number.isFinite(inner) ||
        !Number.isFinite(outer) ||
        inner < 0 ||
        inner > outer ||
        outer <= 0 ||
        outer > Math.PI)
    )
      throw new Error("Use ordered spotlight cone angles between zero and pi.");
    const exponent =
      light.inverseSquaredFalloff === false ? (light.falloffExponent ?? 8) : -1;
    if (
      !Number.isFinite(exponent) ||
      (light.inverseSquaredFalloff === false && exponent < 0)
    )
      throw new Error(
        "Use a finite, nonnegative legacy light falloff exponent.",
      );
    const shadow = light.castShadows
      ? (shadowIndices?.get(light.id) ?? -1)
      : -1;
    integer(shadow, "shadow frame index", -1, 65535);
    records.push(
      ...position,
      range,
      ...color,
      strength,
      ...direction,
      light.kind === "spot" ? 1 : 0,
      light.kind === "spot" ? Math.cos(inner) : 1,
      light.kind === "spot" ? Math.cos(outer) : -1,
      shadow,
      light.inverseSquaredFalloff === false ? Math.max(exponent, 0.0001) : -1,
    );
    sourceIndices.push(sourceIndex);
    lightIds.push(light.id);
    shadows.push(Boolean(light.castShadows));
  }
  const lights = new Float32Array(records);
  if (!lights.every(Number.isFinite))
    throw new Error("Use finite float32 light records.");
  return { lights, sourceIndices, lightIds, shadows };
}
type PreparedLights = ReturnType<typeof prepareLightRecords>;

/** One owned source cache; callers must change revision whenever source contents change. */
export function createTightClusteredLightBuilder() {
  let source: readonly SourceLight[] | undefined,
    version: string | number | undefined,
    cached: PreparedLights | undefined;
  return (input: ClusteredLightInput, revision?: string | number) => {
    let next: PreparedLights;
    const result = buildClusteredLightLists(input, true, () => {
      next =
        revision !== undefined &&
        source === input.lights &&
        version === revision &&
        cached
          ? cached
          : prepareLightRecords(input.lights);
      for (let i = 0; i < next.lightIds.length; i++)
        next.lights[i * CLUSTER_LIGHT_FLOATS + 14] = integer(
          next.shadows[i]
            ? (input.shadowIndices?.get(next.lightIds[i]) ?? -1)
            : -1,
          "shadow frame index",
          -1,
          65535,
        );
      return next;
    });
    if (revision !== undefined) {
      source = input.lights;
      version = revision;
      cached = next!;
    }
    return result;
  };
}

function buildClusteredLightLists(
  input: ClusteredLightInput,
  tight: boolean,
  prepared?: () => PreparedLights,
): ClusteredLights {
  const width = integer(
    input.width,
    "viewport width",
    1,
    CLUSTER_LIMITS.viewport,
  );
  const height = integer(
    input.height,
    "viewport height",
    1,
    CLUSTER_LIMITS.viewport,
  );
  const tileSize = integer(
    input.tileSize ?? 64,
    "tile size",
    1,
    CLUSTER_LIMITS.viewport,
  );
  const depthSlices = integer(
    input.depthSlices ?? 16,
    "depth slices",
    1,
    CLUSTER_LIMITS.depthSlices,
  );
  const maxIndices = integer(
    input.maxIndices ?? 1048576,
    "light index capacity",
    0,
    CLUSTER_LIMITS.indices,
  );
  const tilesX = Math.ceil(width / tileSize),
    tilesY = Math.ceil(height / tileSize);
  const cellCount = integer(
    tilesX * tilesY * depthSlices,
    "cluster count",
    1,
    CLUSTER_LIMITS.cells,
  );
  const near = packed(input.near ?? 0.1, "near depth");
  const far = packed(input.far ?? 2000, "far depth");
  if (near <= 0 || far <= near)
    throw new Error("Use positive, increasing cluster depths.");
  if (
    input.viewProjection.length !== 16 ||
    !Array.from(input.viewProjection).every(Number.isFinite)
  )
    throw new Error(
      "Use a finite, invertible 16-value view-projection matrix.",
    );
  const viewProjection = Float64Array.from(input.viewProjection, (n) =>
    packed(n, "view-projection matrix"),
  );
  try {
    inverse(viewProjection);
  } catch {
    throw new Error(
      "Use a finite, invertible 16-value view-projection matrix.",
    );
  }
  const eye = vector(input.eye, "camera position").map((n) =>
    packed(n, "camera position"),
  ) as V3;
  const forward = unit(input.forward, "camera forward");
  const grid = {
    width,
    height,
    tileSize,
    tilesX,
    tilesY,
    depthSlices,
    near,
    far,
    logDepthScale: packed(
      depthSlices / Math.log(far / near),
      "log depth scale",
    ),
  };
  const { lights, sourceIndices, lightIds } = prepared
    ? prepared()
    : prepareLightRecords(input.lights, input.shadowIndices);
  const camera = { viewProjection, eye, forward, grid };
  const bounds = lightIds.map((_, i) =>
    lightBounds(
      lights.subarray(i * CLUSTER_LIGHT_FLOATS, (i + 1) * CLUSTER_LIGHT_FLOATS),
      camera,
    ),
  );
  if (tight) {
    const frusta = new ClusterFrusta(camera);
    for (let index = 0; index < bounds.length; index++) {
      const bound = bounds[index];
      if (bound)
        bounds[index] = frusta.tighten(
          bound,
          lights,
          index * CLUSTER_LIGHT_FLOATS,
        );
    }
  }
  const candidateMemberships = bounds.reduce(
    (sum, b) =>
      sum + (b ? (b[1] - b[0] + 1) * (b[3] - b[2] + 1) * (b[5] - b[4] + 1) : 0),
    0,
  );
  const globalOverflow = candidateMemberships > CLUSTER_LIMITS.memberships;
  const counts = new Uint32Array(cellCount);
  if (!globalOverflow)
    for (const bound of bounds)
      if (bound)
        for (let z = bound[4]; z <= bound[5]; z++)
          for (let y = bound[2]; y <= bound[3]; y++) {
            let cell = (z * tilesY + y) * tilesX + bound[0];
            for (let x = bound[0]; x <= bound[1]; x++, cell++) counts[cell]++;
          }
  let indexCount = 0,
    overflowCellCount = 0,
    maxCellLightCount = globalOverflow ? lightIds.length : 0;
  for (let cell = 0; cell < cellCount; cell++) {
    const count = counts[cell];
    maxCellLightCount = Math.max(maxCellLightCount, count);
    if (globalOverflow || count > maxIndices - indexCount) {
      counts[cell] = CLUSTER_OVERFLOW;
      overflowCellCount++;
    } else indexCount += count;
  }
  const lightsOffset = CLUSTER_HEADER.words;
  const cellsOffset = lightsOffset + lights.length;
  const indicesOffset = cellsOffset + cellCount * CLUSTER_CELL_WORDS;
  const storage = new Uint32Array(indicesOffset + indexCount);
  const floats = new Float32Array(storage.buffer);
  const lightFloats = floats.subarray(lightsOffset, cellsOffset);
  lightFloats.set(lights);
  const cells = storage.subarray(cellsOffset, indicesOffset);
  const indices = storage.subarray(indicesOffset);
  storage[CLUSTER_HEADER.lightCount] = lightIds.length;
  storage[CLUSTER_HEADER.lightsOffset] = lightsOffset;
  storage[CLUSTER_HEADER.cellsOffset] = cellsOffset;
  storage[CLUSTER_HEADER.indicesOffset] = indicesOffset;
  storage[CLUSTER_HEADER.tilesX] = tilesX;
  storage[CLUSTER_HEADER.tilesY] = tilesY;
  storage[CLUSTER_HEADER.depthSlices] = depthSlices;
  storage[CLUSTER_HEADER.tileSize] = tileSize;
  storage[CLUSTER_HEADER.width] = width;
  storage[CLUSTER_HEADER.height] = height;
  floats[CLUSTER_HEADER.near] = near;
  floats[CLUSTER_HEADER.far] = far;
  floats[CLUSTER_HEADER.logDepthScale] = grid.logDepthScale;
  floats[CLUSTER_HEADER.forwardX] = forward[0];
  floats[CLUSTER_HEADER.forwardY] = forward[1];
  floats[CLUSTER_HEADER.forwardZ] = forward[2];
  storage[CLUSTER_HEADER.indexCount] = indexCount;
  storage[CLUSTER_HEADER.overflowCellCount] = overflowCellCount;
  let cursor = 0;
  for (let cell = 0; cell < cellCount; cell++) {
    cells[cell * 2] = counts[cell] === CLUSTER_OVERFLOW ? 0 : cursor;
    cells[cell * 2 + 1] = counts[cell];
    if (counts[cell] !== CLUSTER_OVERFLOW) cursor += counts[cell];
  }
  if (!globalOverflow) {
    counts.fill(0);
    for (const [index, bound] of bounds.entries())
      if (bound)
        for (let z = bound[4]; z <= bound[5]; z++)
          for (let y = bound[2]; y <= bound[3]; y++) {
            let cell = (z * tilesY + y) * tilesX + bound[0];
            for (let x = bound[0]; x <= bound[1]; x++, cell++)
              if (cells[cell * 2 + 1] !== CLUSTER_OVERFLOW)
                indices[cells[cell * 2] + counts[cell]++] = index;
          }
  }
  return {
    storage,
    lightFloats,
    cells,
    indices,
    sourceIndices: Uint32Array.from(sourceIndices),
    lightIds: lightIds.slice(),
    ...camera,
    stats: {
      candidateMemberships,
      indexCount,
      overflowCellCount,
      maxCellLightCount,
      globalOverflow,
      byteLength: storage.byteLength,
    },
  };
}

/** CPU reference for the shader's top-left pixel and logarithmic-depth lookup. */
export function clusterCellForPoint(
  clusters: ClusteredLights,
  point: ArrayLike<number>,
): number | undefined {
  const p = vector(point, "surface position");
  const projected = clip(clusters.viewProjection, p);
  const [x, y, z, w] = projected;
  const depth = p.reduce(
    (sum, n, i) => sum + (n - clusters.eye[i]) * clusters.forward[i],
    0,
  );
  const g = clusters.grid;
  if (w <= 0 || z < 0 || z > w || x < -w || x > w || y < -w || y > w) return;
  const tx = Math.min(
    g.tilesX - 1,
    Math.max(0, Math.floor(((x / w + 1) * 0.5 * g.width) / g.tileSize)),
  );
  const ty = Math.min(
    g.tilesY - 1,
    Math.max(0, Math.floor(((1 - y / w) * 0.5 * g.height) / g.tileSize)),
  );
  return (depthSlice(depth, g) * g.tilesY + ty) * g.tilesX + tx;
}

/** Overflow expands to all lights, matching the required shader fallback. */
export function clusterLightIndices(
  clusters: ClusteredLights,
  cell: number,
): Uint32Array {
  integer(cell, "cluster index", 0, clusters.cells.length / 2 - 1);
  const start = clusters.cells[cell * 2],
    count = clusters.cells[cell * 2 + 1];
  return count === CLUSTER_OVERFLOW
    ? Uint32Array.from({ length: clusters.lightIds.length }, (_, i) => i)
    : clusters.indices.subarray(start, start + count);
}
