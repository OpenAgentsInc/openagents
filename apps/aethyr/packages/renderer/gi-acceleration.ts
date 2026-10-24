import type {
  CompiledScene,
  CompiledMaterial,
  CompiledInstance,
} from "../assets/src/scene.ts";
import {
  cross,
  dot,
  inverse,
  normalize,
  sub,
  transformPoint,
  type V3,
} from "./math.ts";

export const GI_ACCELERATION_PROFILE = "instanced-bvh-median-v1";
export const GI_LEAF_BIT = 0x80000000;
export const GI_INVALID = 0xffffffff;
export const GI_MAX_DEPTH = 32;
export const GI_NODE_BYTES = 32;
export const GI_TRIANGLE_BYTES = 32;
export const GI_INSTANCE_BYTES = 128;
export const GI_MESH_BYTES = 32;
export const GI_LEAF_SIZE = 4;

/** Masked geometry blocks as solid until a caller supplies an alpha evaluator. */
export const GiTransport = {
  excluded: 0,
  opaque: 1,
  maskedSolidApproximation: 2,
} as const;
export type GiBounds = { min: V3; max: V3 };
type InputScene = Pick<
  CompiledScene,
  | "meshes"
  | "materials"
  | "instances"
  | "vertexCount"
  | "indexCount"
  | "vertexStride"
>;

/**
 * All binary words are little endian when serialized. Nodes are 32 bytes:
 * float3 min,u32 leftFirst,float3 max,u32 meta. High meta bit marks a leaf;
 * other bits give its count. Internal nodes store left/right child indices.
 * BLAS leaves index triangles; TLAS leaves index packed instances. No recursion
 * or vendor acceleration-structure format is required by the consumer.
 */
export interface GiAcceleration {
  positions: Float32Array; // float4 per original vertex; w=1.
  triangles: Uint32Array; // v0,v1,v2,default material,slot,source triangle,flags,reserved.
  blasNodes: ArrayBuffer;
  tlasNodes: ArrayBuffer;
  /** Three float4 model rows,three inverse rows,then 8 u32 metadata words. */
  instances: ArrayBuffer;
  /** root,firstTriangle,triangleCount,sourceMesh,then 4 reserved u32. */
  meshes: Uint32Array;
  materialOverrides: Uint32Array;
  materialTransport: Uint32Array;
  tlasRoot: number;
  worldBounds?: GiBounds;
  stats: {
    sourceInstances: number;
    admittedInstances: number;
    excludedInstances: number;
    uniqueMeshes: number;
    uniqueTriangles: number;
    maskedMaterials: number;
    blasNodes: number;
    tlasNodes: number;
    maxDepth: number;
    bytes: number;
  };
}

export interface GiHit {
  distance: number; // meters along a normalized world ray.
  position: V3;
  normal: V3; // transformed triangle winding, before face-forwarding.
  barycentric: V3;
  sourceInstance: number;
  sourceMesh: number;
  sourceTriangle: number;
  material: number;
  maskedApproximation: boolean;
  frontFace: boolean;
}

export function giMaterialTransport(material: CompiledMaterial): number {
  if (/^(?:MI_|MM_)?(?:sky|cloud|fog|water)(?:_|$)/i.test(material.name))
    return GiTransport.excluded;
  if (material.alphaMode === "BLEND") return GiTransport.excluded;
  return material.alphaMode === "MASK"
    ? GiTransport.maskedSolidApproximation
    : GiTransport.opaque;
}

function excludedInstance(
  instance: CompiledInstance,
  meshName: string,
): boolean {
  return (
    /(?:SkySphere|SkyAtmosphere|VolumetricCloud|WaterBody|FX_fog_Card|Niagara|ParticleSystem)/i.test(
      instance.sourceClass,
    ) || /^(?:SM_)?(?:SM_Cloud|Cloud|FogCard|SkySphere)(?:_|$)/i.test(meshName)
  );
}
function integer(value: number, max: number, label: string): void {
  if (!Number.isInteger(value) || value < 0 || value > max)
    throw new Error(`Invalid GI ${label}.`);
}
function boundsValid(bounds: GiBounds): void {
  if (
    bounds.min.length !== 3 ||
    bounds.max.length !== 3 ||
    ![...bounds.min, ...bounds.max].every(Number.isFinite) ||
    bounds.min.some((v, i) => v > bounds.max[i])
  )
    throw new Error("Use finite, ordered GI bounds.");
}
const f32Scratch = new Float32Array(1),
  u32Scratch = new Uint32Array(f32Scratch.buffer);
/** One outward float32 step prevents rounded bounds from excluding real corners. */
function outward(value: number, upper: boolean): number {
  f32Scratch[0] = value;
  if (!Number.isFinite(f32Scratch[0]))
    throw new Error("GI bounds exceed float32 range.");
  if ((upper && f32Scratch[0] >= value) || (!upper && f32Scratch[0] <= value))
    return f32Scratch[0];
  if (f32Scratch[0] === 0) {
    u32Scratch[0] = upper ? 1 : 0x80000001;
    return f32Scratch[0];
  }
  u32Scratch[0] += f32Scratch[0] > 0 === upper ? 1 : -1;
  return f32Scratch[0];
}
function unionAt(
  bounds: Float64Array,
  order: Uint32Array,
  start: number,
  end: number,
): GiBounds {
  const min: V3 = [Infinity, Infinity, Infinity],
    max: V3 = [-Infinity, -Infinity, -Infinity];
  for (let j = start; j < end; j++)
    for (let axis = 0; axis < 3; axis++) {
      min[axis] = Math.min(min[axis], bounds[order[j] * 6 + axis]);
      max[axis] = Math.max(max[axis], bounds[order[j] * 6 + axis + 3]);
    }
  return { min, max };
}
function buildTree(bounds: Float64Array): {
  nodes: ArrayBuffer;
  order: Uint32Array;
  depth: number;
} {
  const count = bounds.length / 6,
    order = Uint32Array.from({ length: count }, (_, i) => i);
  if (!count) return { nodes: new ArrayBuffer(0), order, depth: 0 };
  const data = new ArrayBuffer(count * 2 * GI_NODE_BYTES),
    floats = new Float32Array(data),
    words = new Uint32Array(data);
  let used = 0,
    maxDepth = 0;
  function visit(start: number, end: number, depth: number): number {
    if (depth > GI_MAX_DEPTH)
      throw new Error("The GI hierarchy exceeds its traversal depth.");
    maxDepth = Math.max(maxDepth, depth);
    const node = used++,
      offset = node * 8,
      b = unionAt(bounds, order, start, end);
    for (let axis = 0; axis < 3; axis++) {
      floats[offset + axis] = outward(b.min[axis], false);
      floats[offset + 4 + axis] = outward(b.max[axis], true);
    }
    if (end - start <= GI_LEAF_SIZE) {
      words[offset + 3] = start;
      words[offset + 7] = (GI_LEAF_BIT | (end - start)) >>> 0;
      return node;
    }
    const cmin = [Infinity, Infinity, Infinity],
      cmax = [-Infinity, -Infinity, -Infinity];
    for (let j = start; j < end; j++)
      for (let axis = 0; axis < 3; axis++) {
        const center =
          (bounds[order[j] * 6 + axis] + bounds[order[j] * 6 + axis + 3]) * 0.5;
        cmin[axis] = Math.min(cmin[axis], center);
        cmax[axis] = Math.max(cmax[axis], center);
      }
    let axis = 0;
    for (let i = 1; i < 3; i++)
      if (cmax[i] - cmin[i] > cmax[axis] - cmin[axis]) axis = i;
    order
      .subarray(start, end)
      .sort(
        (a, b) =>
          bounds[a * 6 + axis] +
            bounds[a * 6 + axis + 3] -
            (bounds[b * 6 + axis] + bounds[b * 6 + axis + 3]) || a - b,
      );
    const middle = (start + end) >>> 1;
    words[offset + 3] = visit(start, middle, depth + 1);
    words[offset + 7] = visit(middle, end, depth + 1);
    return node;
  }
  visit(0, count, 0);
  return { nodes: data.slice(0, used * GI_NODE_BYTES), order, depth: maxDepth };
}
function matrixRows(
  matrix: ArrayLike<number>,
  output: Float32Array,
  offset: number,
): void {
  for (let row = 0; row < 3; row++)
    for (let col = 0; col < 4; col++)
      output[offset + row * 4 + col] = matrix[col * 4 + row];
}
function unpackRows(floats: Float32Array, offset: number): Float32Array {
  const m = new Float32Array(16);
  m[15] = 1;
  for (let row = 0; row < 3; row++)
    for (let col = 0; col < 4; col++)
      m[col * 4 + row] = floats[offset + row * 4 + col];
  return m;
}
function effectiveMaterial(
  instance: CompiledInstance,
  defaultMaterial: number,
  slot: number,
): number {
  const override = instance.materialOverrides[slot];
  return override === undefined || override === -1 ? defaultMaterial : override;
}
function transformedBounds(
  bounds: GiBounds,
  model: ArrayLike<number>,
): GiBounds {
  const min: V3 = [Infinity, Infinity, Infinity],
    max: V3 = [-Infinity, -Infinity, -Infinity];
  for (let corner = 0; corner < 8; corner++) {
    const p = transformPoint(
      model,
      [0, 1, 2].map((i) =>
        corner & (1 << i) ? bounds.max[i] : bounds.min[i],
      ) as V3,
    );
    for (let axis = 0; axis < 3; axis++) {
      min[axis] = Math.min(min[axis], p[axis]);
      max[axis] = Math.max(max[axis], p[axis]);
    }
  }
  boundsValid({ min, max });
  return { min, max };
}

/** Builds shared mesh hierarchies; source instances never duplicate triangles. */
export function buildGiAcceleration(
  scene: InputScene,
  vertices: Float32Array,
  indices: Uint32Array,
): GiAcceleration {
  integer(scene.vertexCount, 2_000_000, "vertex count");
  integer(scene.indexCount, 6_000_000, "index count");
  integer(scene.meshes.length, 65_536, "mesh count");
  integer(scene.instances.length, 100_000, "instance count");
  integer(scene.materials.length, 65_536, "material count");
  if (
    scene.vertexStride !== 48 ||
    vertices.length !== scene.vertexCount * 12 ||
    indices.length !== scene.indexCount
  )
    throw new Error(
      "The GI source geometry layout does not match its manifest.",
    );
  const positions = new Float32Array(scene.vertexCount * 4);
  for (let i = 0; i < scene.vertexCount; i++) {
    for (let axis = 0; axis < 3; axis++) {
      const v = vertices[i * 12 + axis];
      if (!Number.isFinite(v))
        throw new Error("The GI geometry contains a nonfinite position.");
      positions[i * 4 + axis] = v;
    }
    positions[i * 4 + 3] = 1;
  }
  const materialTransport = Uint32Array.from(
    scene.materials,
    giMaterialTransport,
  );
  const admitted: Array<{
    source: number;
    instance: CompiledInstance;
    model: Float32Array;
    inverse: Float32Array;
  }> = [];
  const usedMeshes = new Set<number>();
  let overrideCount = 0;
  scene.instances.forEach((instance, source) => {
    integer(instance.mesh, scene.meshes.length - 1, "mesh reference");
    const mesh = scene.meshes[instance.mesh];
    if (excludedInstance(instance, mesh.name)) return;
    integer(instance.materialOverrides.length, 256, "material override count");
    overrideCount += instance.materialOverrides.length;
    if (overrideCount > 1_000_000)
      throw new Error(
        "The GI instance overrides exceed their preparation limit.",
      );
    for (const m of instance.materialOverrides)
      if (m !== -1) integer(m, scene.materials.length - 1, "material override");
    const contributes = mesh.primitives.some((p) => {
      integer(p.material, scene.materials.length - 1, "material reference");
      integer(p.materialSlot, 65535, "material slot");
      return (
        materialTransport[
          effectiveMaterial(instance, p.material, p.materialSlot)
        ] !== GiTransport.excluded
      );
    });
    if (!contributes) return;
    if (
      instance.model.length !== 16 ||
      !instance.model.every(Number.isFinite) ||
      Math.abs(instance.model[3]) +
        Math.abs(instance.model[7]) +
        Math.abs(instance.model[11]) +
        Math.abs(instance.model[15] - 1) >
        1e-6
    )
      throw new Error("Use a finite affine GI instance transform.");
    const model = new Float32Array(instance.model),
      inv = inverse(model);
    if (![...model, ...inv].every(Number.isFinite))
      throw new Error("The GI instance transform exceeds float32 range.");
    admitted.push({ source, instance, model, inverse: inv });
    usedMeshes.add(instance.mesh);
  });
  const meshChunks: Array<{
    source: number;
    nodes: ArrayBuffer;
    triangles: Uint32Array;
    bounds: GiBounds;
    depth: number;
  }> = [];
  let totalTriangles = 0,
    totalNodes = 0,
    maxDepth = 0;
  for (const source of [...usedMeshes].sort((a, b) => a - b)) {
    const mesh = scene.meshes[source];
    let count = 0;
    for (const p of mesh.primitives) {
      integer(p.firstIndex, indices.length, "first index");
      integer(p.indexCount, indices.length, "primitive index count");
      integer(p.baseVertex, scene.vertexCount, "base vertex");
      if (
        p.firstIndex % 3 ||
        p.indexCount % 3 ||
        p.firstIndex + p.indexCount > indices.length
      )
        throw new Error("The GI primitive index range is invalid.");
      count += p.indexCount / 3;
    }
    totalTriangles += count;
    if (totalTriangles > 2_000_000)
      throw new Error("The GI source exceeds two million unique triangles.");
    if (!count) throw new Error("An admitted GI mesh has no triangles.");
    const triangles = new Uint32Array(count * 8),
      bounds = new Float64Array(count * 6);
    let n = 0;
    for (const p of mesh.primitives)
      for (let j = 0; j < p.indexCount; j += 3, n++) {
        const min = [Infinity, Infinity, Infinity],
          max = [-Infinity, -Infinity, -Infinity];
        for (let corner = 0; corner < 3; corner++) {
          const vertex = indices[p.firstIndex + j + corner] + p.baseVertex;
          integer(vertex, scene.vertexCount - 1, "vertex reference");
          triangles[n * 8 + corner] = vertex;
          for (let axis = 0; axis < 3; axis++) {
            const v = positions[vertex * 4 + axis];
            min[axis] = Math.min(min[axis], v);
            max[axis] = Math.max(max[axis], v);
          }
        }
        triangles.set(
          [
            p.material,
            p.materialSlot,
            (p.firstIndex + j) / 3,
            materialTransport[p.material],
            0,
          ],
          n * 8 + 3,
        );
        bounds.set([...min, ...max], n * 6);
      }
    const tree = buildTree(bounds),
      ordered = new Uint32Array(triangles.length);
    tree.order.forEach((original, j) =>
      ordered.set(triangles.subarray(original * 8, original * 8 + 8), j * 8),
    );
    const f = new Float32Array(tree.nodes),
      rootBounds = {
        min: [f[0], f[1], f[2]] as V3,
        max: [f[4], f[5], f[6]] as V3,
      };
    meshChunks.push({
      source,
      nodes: tree.nodes,
      triangles: ordered,
      bounds: rootBounds,
      depth: tree.depth,
    });
    totalNodes += tree.nodes.byteLength / GI_NODE_BYTES;
    maxDepth = Math.max(maxDepth, tree.depth);
  }
  const blasNodes = new ArrayBuffer(totalNodes * GI_NODE_BYTES),
    nodeWords = new Uint32Array(blasNodes),
    triangles = new Uint32Array(totalTriangles * 8),
    meshes = new Uint32Array(meshChunks.length * 8);
  const sourceMeshToPacked = new Map<number, number>();
  let nodeBase = 0,
    triangleBase = 0;
  meshChunks.forEach((mesh, i) => {
    const words = new Uint32Array(mesh.nodes);
    nodeWords.set(words, nodeBase * 8);
    for (let n = 0; n < words.length / 8; n++) {
      const offset = (nodeBase + n) * 8;
      if (nodeWords[offset + 7] & GI_LEAF_BIT)
        nodeWords[offset + 3] += triangleBase;
      else {
        nodeWords[offset + 3] += nodeBase;
        nodeWords[offset + 7] += nodeBase;
      }
    }
    triangles.set(mesh.triangles, triangleBase * 8);
    meshes.set(
      [
        nodeBase,
        triangleBase,
        mesh.triangles.length / 8,
        mesh.source,
        0,
        0,
        0,
        0,
      ],
      i * 8,
    );
    sourceMeshToPacked.set(mesh.source, i);
    nodeBase += words.length / 8;
    triangleBase += mesh.triangles.length / 8;
  });
  const worldBoundsData = new Float64Array(admitted.length * 6);
  admitted.forEach((a, i) => {
    const b = transformedBounds(
      meshChunks[sourceMeshToPacked.get(a.instance.mesh)!].bounds,
      a.model,
    );
    worldBoundsData.set([...b.min, ...b.max], i * 6);
  });
  const tree = buildTree(worldBoundsData),
    instances = new ArrayBuffer(admitted.length * GI_INSTANCE_BYTES),
    instanceFloats = new Float32Array(instances),
    instanceWords = new Uint32Array(instances),
    overrides: number[] = [];
  tree.order.forEach((original, i) => {
    const a = admitted[original],
      mesh = sourceMeshToPacked.get(a.instance.mesh)!,
      offset = i * 32;
    matrixRows(a.model, instanceFloats, offset);
    matrixRows(a.inverse, instanceFloats, offset + 12);
    instanceWords.set(
      [
        meshes[mesh * 8],
        a.instance.mesh,
        overrides.length,
        a.instance.materialOverrides.length,
        a.source,
        0,
        0,
        0,
      ],
      offset + 24,
    );
    overrides.push(
      ...a.instance.materialOverrides.map((v) => (v === -1 ? GI_INVALID : v)),
    );
  });
  const materialOverrides = Uint32Array.from(overrides),
    tlasFloats = new Float32Array(tree.nodes);
  const worldBounds = admitted.length
    ? {
        min: [tlasFloats[0], tlasFloats[1], tlasFloats[2]] as V3,
        max: [tlasFloats[4], tlasFloats[5], tlasFloats[6]] as V3,
      }
    : undefined;
  const result: GiAcceleration = {
    positions,
    triangles,
    blasNodes,
    tlasNodes: tree.nodes,
    instances,
    meshes,
    materialOverrides,
    materialTransport,
    tlasRoot: admitted.length ? 0 : GI_INVALID,
    worldBounds,
    stats: {
      sourceInstances: scene.instances.length,
      admittedInstances: admitted.length,
      excludedInstances: scene.instances.length - admitted.length,
      uniqueMeshes: meshChunks.length,
      uniqueTriangles: totalTriangles,
      maskedMaterials: materialTransport.filter(
        (v) => v === GiTransport.maskedSolidApproximation,
      ).length,
      blasNodes: totalNodes,
      tlasNodes: tree.nodes.byteLength / GI_NODE_BYTES,
      maxDepth: Math.max(maxDepth, tree.depth),
      bytes: 0,
    },
  };
  result.stats.bytes = [
    positions,
    triangles,
    blasNodes,
    tree.nodes,
    instances,
    meshes,
    materialOverrides,
    materialTransport,
  ].reduce((n, b) => n + b.byteLength, 0);
  return result;
}

function boundsHit(
  floats: Float32Array,
  node: number,
  origin: V3,
  direction: V3,
  minimum: number,
  maximum: number,
): boolean {
  const offset = node * 8;
  for (let axis = 0; axis < 3; axis++) {
    if (direction[axis] === 0) {
      if (
        origin[axis] < floats[offset + axis] ||
        origin[axis] > floats[offset + 4 + axis]
      )
        return false;
      continue;
    }
    let a = (floats[offset + axis] - origin[axis]) / direction[axis],
      b = (floats[offset + 4 + axis] - origin[axis]) / direction[axis];
    if (a > b) [a, b] = [b, a];
    minimum = Math.max(minimum, a);
    maximum = Math.min(maximum, b);
    if (minimum > maximum) return false;
  }
  return true;
}
function visitTree(
  buffer: ArrayBuffer,
  root: number,
  origin: V3,
  direction: V3,
  minimum: number,
  maximum: () => number,
  leaf: (first: number, count: number) => void,
): void {
  if (root === GI_INVALID) return;
  const f = new Float32Array(buffer),
    u = new Uint32Array(buffer),
    stack = [root];
  while (stack.length) {
    const node = stack.pop()!;
    if (!boundsHit(f, node, origin, direction, minimum, maximum())) continue;
    const a = u[node * 8 + 3],
      meta = u[node * 8 + 7];
    if (meta & GI_LEAF_BIT) leaf(a, meta & ~GI_LEAF_BIT);
    else {
      stack.push(meta, a);
    }
  }
}
function triangleHit(
  a: V3,
  b: V3,
  c: V3,
  origin: V3,
  direction: V3,
  minimum: number,
  maximum: number,
): { distance: number; barycentric: V3 } | undefined {
  const e1 = sub(b, a),
    e2 = sub(c, a),
    p = cross(direction, e2),
    det = dot(e1, p);
  // Local rays keep their scale so distance stays in world meters.
  if (
    det * det <=
    1e-24 * dot(e1, e1) * dot(e2, e2) * dot(direction, direction)
  )
    return;
  const inv = 1 / det,
    t = sub(origin, a),
    u = dot(t, p) * inv;
  if (u < 0 || u > 1) return;
  const q = cross(t, e1),
    v = dot(direction, q) * inv;
  if (v < 0 || u + v > 1) return;
  const distance = dot(e2, q) * inv;
  if (distance < minimum || distance > maximum) return;
  return { distance, barycentric: [1 - u - v, u, v] };
}
/** CPU reference for the packed GPU contract; masked alpha is explicitly solid. */
export function traceGi(
  acceleration: GiAcceleration,
  origin: V3,
  direction: V3,
  minimum = 0.001,
  maximum = 1000,
): GiHit | undefined {
  if (
    ![...origin, ...direction, minimum, maximum].every(Number.isFinite) ||
    minimum < 0 ||
    maximum <= minimum ||
    Math.hypot(...direction) < 1e-10
  )
    throw new Error("Use a finite GI ray and ordered distances.");
  direction = normalize(direction);
  let nearest = maximum,
    result: GiHit | undefined;
  const f = new Float32Array(acceleration.instances),
    u = new Uint32Array(acceleration.instances);
  const vertex = (index: number): V3 => [
    acceleration.positions[index * 4],
    acceleration.positions[index * 4 + 1],
    acceleration.positions[index * 4 + 2],
  ];
  visitTree(
    acceleration.tlasNodes,
    acceleration.tlasRoot,
    origin,
    direction,
    minimum,
    () => nearest,
    (first, count) => {
      for (let i = first; i < first + count; i++) {
        const offset = i * 32,
          inv = unpackRows(f, offset + 12),
          model = unpackRows(f, offset),
          o = transformPoint(inv, origin);
        const d: V3 = [0, 1, 2].map(
          (row) =>
            inv[row] * direction[0] +
            inv[4 + row] * direction[1] +
            inv[8 + row] * direction[2],
        ) as V3;
        visitTree(
          acceleration.blasNodes,
          u[offset + 24],
          o,
          d,
          minimum,
          () => nearest,
          (triangleFirst, triangleCount) => {
            for (
              let t = triangleFirst;
              t < triangleFirst + triangleCount;
              t++
            ) {
              const index = t * 8,
                slot = acceleration.triangles[index + 4],
                override =
                  slot < u[offset + 27]
                    ? acceleration.materialOverrides[u[offset + 26] + slot]
                    : GI_INVALID;
              const material =
                  override === GI_INVALID
                    ? acceleration.triangles[index + 3]
                    : override,
                transport = acceleration.materialTransport[material];
              if (!transport) continue;
              const a = vertex(acceleration.triangles[index]),
                b = vertex(acceleration.triangles[index + 1]),
                c = vertex(acceleration.triangles[index + 2]),
                hit = triangleHit(a, b, c, o, d, minimum, nearest);
              if (!hit) continue;
              const worldA = transformPoint(model, a),
                worldB = transformPoint(model, b),
                worldC = transformPoint(model, c),
                normal = normalize(
                  cross(sub(worldB, worldA), sub(worldC, worldA)),
                );
              nearest = hit.distance;
              result = {
                ...hit,
                position: origin.map(
                  (v, axis) => v + direction[axis] * hit.distance,
                ) as V3,
                normal,
                sourceInstance: u[offset + 28],
                sourceMesh: u[offset + 25],
                sourceTriangle: acceleration.triangles[index + 5],
                material,
                maskedApproximation:
                  transport === GiTransport.maskedSolidApproximation,
                frontFace: dot(normal, direction) < 0,
              };
            }
          },
        );
      }
    },
  );
  return result;
}

/** Includes both old/new geometry and the finite ray reach; this does not rebuild a hierarchy. */
export function giAffectedBounds(
  previous: GiBounds | undefined,
  next: GiBounds | undefined,
  rayReach = 64,
): GiBounds {
  if (!previous && !next)
    throw new Error("Supply previous or next geometry bounds.");
  if (!Number.isFinite(rayReach) || rayReach < 0 || rayReach > 1000)
    throw new Error("Use a bounded GI ray reach.");
  for (const b of [previous, next]) if (b) boundsValid(b);
  return {
    min: [0, 1, 2].map(
      (i) =>
        Math.min(previous?.min[i] ?? Infinity, next?.min[i] ?? Infinity) -
        rayReach,
    ) as V3,
    max: [0, 1, 2].map(
      (i) =>
        Math.max(previous?.max[i] ?? -Infinity, next?.max[i] ?? -Infinity) +
        rayReach,
    ) as V3,
  };
}

/** Encodes float/int bit patterns without relying on the host's byte order. */
export function giBinaryLE(source: ArrayBuffer | ArrayBufferView): Uint8Array {
  const bytes =
    source instanceof ArrayBuffer
      ? new Uint8Array(source)
      : new Uint8Array(source.buffer, source.byteOffset, source.byteLength);
  if (bytes.byteLength % 4)
    throw new Error("GI buffers must contain whole 32-bit words.");
  const words = new Uint32Array(
      bytes.buffer,
      bytes.byteOffset,
      bytes.byteLength / 4,
    ),
    output = new Uint8Array(bytes.byteLength),
    view = new DataView(output.buffer);
  words.forEach((value, i) => view.setUint32(i * 4, value, true));
  return output;
}
