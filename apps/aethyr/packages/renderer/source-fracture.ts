import type { CompiledScene } from "../assets/src/scene.ts";
import { cross, dot, normalize, transformPoint, type V3 } from "./math.ts";

export const FRACTURE_LIMITS = Object.freeze({
  chunks: 64,
  sourceTriangles: 32768,
  vertices: 196608,
});
const WORK_CHUNK = Object.freeze({
  sourceTriangles: 32,
  outputVertices: 128,
  areaTriangles: 128,
});
export interface FractureSource {
  vertices: Float32Array;
  indices: Uint32Array;
  colors: Uint8Array;
  uv1: Float32Array;
}
export interface FractureChunk {
  center: V3;
  vertices: Float32Array;
  colors: Uint8Array;
  uv1: Float32Array;
  draws: { firstIndex: number; indexCount: number; material: number }[];
}
const EMPTY_CHUNK: FractureChunk = Object.freeze({
  center: [0, 0, 0] as V3,
  vertices: new Float32Array(),
  colors: new Uint8Array(),
  uv1: new Float32Array(),
  draws: [],
});
export function fractureSurfaceArea(chunks: readonly FractureChunk[]): number {
  return finish(fractureSurfaceAreaSteps(chunks));
}
export function* fractureSurfaceAreaSteps(
  chunks: readonly FractureChunk[],
): Generator<void, number> {
  let area = 0;
  let triangles = 0;
  for (const chunk of chunks)
    for (let i = 0; i < chunk.vertices.length; i += 36) {
      if (++triangles % WORK_CHUNK.areaTriangles === 0) yield;
      const v = chunk.vertices,
        a: V3 = [v[i + 12] - v[i], v[i + 13] - v[i + 1], v[i + 14] - v[i + 2]],
        b: V3 = [v[i + 24] - v[i], v[i + 25] - v[i + 1], v[i + 26] - v[i + 2]];
      area += Math.hypot(...cross(a, b)) * 0.5;
    }
  return area;
}
function finish<T>(steps: Generator<void, T>): T {
  for (;;) {
    const step = steps.next();
    if (step.done) return step.value;
  }
}
type Vertex = number[];
// Python and JavaScript affine transforms can differ by a few ULPs at shared bounds.
const CLIP_EPSILON = 1e-7;
const mix = (a: Vertex, b: Vertex, t: number) =>
  a.map((v, i) => v + (b[i] - v) * t);

/** Sutherland–Hodgman clipping retains UVs, colors, and shading frames at new edges. */
export function clipFracturePolygon(
  polygon: Vertex[],
  axis: number,
  plane: number,
  upper: boolean,
): Vertex[] {
  const result: Vertex[] = [];
  const snap = (vertex: Vertex) => {
    if (Math.abs(vertex[axis] - plane) > CLIP_EPSILON) return vertex;
    const snapped = vertex.slice();
    snapped[axis] = plane;
    return snapped;
  };
  for (let i = 0; i < polygon.length; i++) {
    const a = snap(polygon[i]),
      b = snap(polygon[(i + 1) % polygon.length]);
    const da = a[axis] - plane,
      db = b[axis] - plane;
    const ai = upper ? da >= 0 : da <= 0,
      bi = upper ? db >= 0 : db <= 0;
    if (ai) result.push(a);
    if (ai !== bi) result.push(mix(a, b, da / (da - db)));
  }
  return result;
}

/** Coarse bits x=1, y=2, z=4 and quarter bits x=8, y=16, z=32 match physics. */
export function fracturePlacement(
  scene: CompiledScene,
  source: FractureSource,
  sourceId: number,
  min: V3,
  max: V3,
  split: boolean | 4 = true,
  worldVertices?: Map<number, number[]>,
): FractureChunk[] {
  return finish(
    fracturePlacementSteps(
      scene,
      source,
      sourceId,
      min,
      max,
      split,
      worldVertices,
    ),
  );
}

/** Checkpoints expose bounded clipping and packing work without publishing partial geometry. */
export function* fracturePlacementSteps(
  scene: CompiledScene,
  source: FractureSource,
  sourceId: number,
  min: V3,
  max: V3,
  split: boolean | 4 = true,
  worldVertices?: Map<number, number[]>,
): Generator<void, FractureChunk[]> {
  const instance = scene.instances[sourceId];
  if (
    !instance ||
    !Number.isSafeInteger(sourceId) ||
    sourceId < 0 ||
    ![...min, ...max].every(Number.isFinite) ||
    min.some((v, i) => v > max[i])
  )
    throw Error("Destruction needs a valid source placement and bounds.");
  const divisions = split === 4 ? 4 : split ? 2 : 1;
  const centers = Array.from(
    { length: divisions ** 3 },
    (_, chunk) =>
      min.map((v, axis) => {
        const digit =
          divisions === 4
            ? 2 * ((chunk >> axis) & 1) + ((chunk >> (axis + 3)) & 1)
            : divisions === 2
              ? (chunk >> axis) & 1
              : 0;
        return v + ((digit + 0.5) / divisions) * (max[axis] - v);
      }) as V3,
  );
  const parts = centers.map(() => new Map<number, number[]>());
  const sourceMax: V3 = [-Infinity, -Infinity, -Infinity];
  const localBounds = scene.meshes[instance.mesh].bounds;
  for (let corner = 0; corner < 8; corner++) {
    const p = transformPoint(
      instance.model,
      [0, 1, 2].map((axis) =>
        corner & (1 << axis) ? localBounds.max[axis] : localBounds.min[axis],
      ) as V3,
    );
    for (let axis = 0; axis < 3; axis++)
      sourceMax[axis] = Math.max(sourceMax[axis], p[axis]);
  }
  const m = instance.model,
    a: V3 = [m[0], m[1], m[2]],
    b: V3 = [m[4], m[5], m[6]],
    c: V3 = [m[8], m[9], m[10]];
  const cofactors = [cross(b, c), cross(c, a), cross(a, b)];
  const sign = dot(a, cofactors[0]) < 0 ? -1 : 1;
  const transformNormal = (v: number[]) =>
    normalize(
      [0, 1, 2].map(
        (i) =>
          sign *
          (cofactors[0][i] * v[0] +
            cofactors[1][i] * v[1] +
            cofactors[2][i] * v[2]),
      ) as V3,
    );
  const vertex = (index: number): Vertex => {
    const cached = worldVertices?.get(index);
    if (cached) return cached;
    const start = index * 12;
    if (index < 0 || start + 12 > source.vertices.length)
      throw Error("A fragment refers to missing source geometry.");
    const v = source.vertices.subarray(start, start + 12);
    const point = transformPoint(m, [v[0], v[1], v[2]]);
    const normal = transformNormal(Array.from(v.subarray(3, 6)));
    const tangent = normalize(
      [0, 1, 2].map(
        (i) => m[i] * v[8] + m[i + 4] * v[9] + m[i + 8] * v[10],
      ) as V3,
    );
    const result = [
      ...point,
      ...normal,
      v[6],
      v[7],
      ...tangent,
      v[11] * sign,
      ...Array.from(source.colors.subarray(index * 4, index * 4 + 4)),
      ...Array.from(source.uv1.subarray(index * 2, index * 2 + 2)),
    ];
    worldVertices?.set(index, result);
    return result;
  };
  let triangles = 0,
    emitted = 0;
  for (const primitive of scene.meshes[instance.mesh].primitives) {
    const replacement = instance.materialOverrides[primitive.materialSlot];
    const candidate =
      replacement !== undefined && replacement >= 0
        ? replacement
        : primitive.material;
    const material =
      candidate >= 0 && candidate < scene.materials.length ? candidate : 0;
    for (
      let offset = primitive.firstIndex;
      offset < primitive.firstIndex + primitive.indexCount;
      offset += 3
    ) {
      if (++triangles > FRACTURE_LIMITS.sourceTriangles)
        throw Error(
          "This source placement exceeds the fragment triangle budget.",
        );
      if (triangles % WORK_CHUNK.sourceTriangles === 0) yield;
      const ids = [0, 1, 2].map(
        (i) => source.indices[offset + i] + primitive.baseVertex,
      );
      const initial = ids.map(vertex);
      if (sign < 0) [initial[1], initial[2]] = [initial[2], initial[1]];
      if (
        [0, 1, 2].some(
          (axis) =>
            max[axis] < sourceMax[axis] - CLIP_EPSILON &&
            initial.every((v) => Math.abs(v[axis] - max[axis]) <= CLIP_EPSILON),
        )
      )
        continue;
      if (
        [0, 1, 2].some(
          (axis) =>
            Math.max(...initial.map((v) => v[axis])) <
              min[axis] - CLIP_EPSILON ||
            Math.min(...initial.map((v) => v[axis])) > max[axis] + CLIP_EPSILON,
        )
      )
        continue;
      let clipped = initial;
      for (let axis = 0; axis < 3 && clipped.length; axis++) {
        clipped = clipFracturePolygon(clipped, axis, min[axis], true);
        if (clipped.length)
          clipped = clipFracturePolygon(clipped, axis, max[axis], false);
      }
      if (clipped.length < 3) continue;
      let polygons = [{ chunk: 0, polygon: clipped }];
      for (
        let pass = 0;
        pass < (divisions === 4 ? 6 : divisions === 2 ? 3 : 0);
        pass++
      ) {
        const axis = pass % 3;
        const next: typeof polygons = [];
        for (const item of polygons) {
          const plane =
            min[axis] +
            (max[axis] - min[axis]) *
              (pass < 3 ? 0.5 : item.chunk & (1 << axis) ? 0.75 : 0.25);
          // Coplanar triangles belong to the upper half once, not to both fragments.
          const coordinates = item.polygon.map((v) => v[axis]);
          const lo = Math.min(...coordinates),
            hi = Math.max(...coordinates);
          if (lo >= plane)
            next.push({
              chunk: item.chunk | (1 << pass),
              polygon: item.polygon,
            });
          else if (hi <= plane) next.push(item);
          else {
            next.push({
              chunk: item.chunk,
              polygon: clipFracturePolygon(item.polygon, axis, plane, false),
            });
            next.push({
              chunk: item.chunk | (1 << pass),
              polygon: clipFracturePolygon(item.polygon, axis, plane, true),
            });
          }
        }
        polygons = next;
      }
      for (const { chunk, polygon } of polygons) {
        const data = parts[chunk].get(material) ?? [];
        for (let i = 1; i < polygon.length - 1; i++) {
          const triangle = [polygon[0], polygon[i], polygon[i + 1]];
          const e1 = triangle[1]
            .slice(0, 3)
            .map((v, j) => v - triangle[0][j]) as V3;
          const e2 = triangle[2]
            .slice(0, 3)
            .map((v, j) => v - triangle[0][j]) as V3;
          if (Math.hypot(...cross(e1, e2)) < 1e-10) continue;
          if (emitted + 3 > FRACTURE_LIMITS.vertices)
            throw Error(
              "This source placement exceeds the fragment vertex budget.",
            );
          for (const v of triangle) data.push(...v);
          emitted += 3;
        }
        parts[chunk].set(material, data);
      }
    }
  }
  const result: FractureChunk[] = [];
  let packedVertices = 0;
  for (const [chunk, materials] of parts.entries()) {
    let count = 0;
    for (const data of materials.values()) count += data.length / 18;
    if (!count) {
      result.push(EMPTY_CHUNK);
      continue;
    }
    const vertices = new Float32Array(count * 12),
      colors = new Uint8Array(count * 4),
      uv1 = new Float32Array(count * 2);
    const draws: FractureChunk["draws"] = [];
    let vertexCount = 0;
    for (const [material, data] of materials) {
      const firstIndex = vertexCount;
      for (let offset = 0; offset < data.length; offset += 18) {
        const v = data.slice(offset, offset + 18);
        const n = normalize(v.slice(3, 6) as V3),
          t = normalize(v.slice(8, 11) as V3);
        vertices.set(
          [
            ...v.slice(0, 3).map((value, i) => value - centers[chunk][i]),
            ...n,
            v[6],
            v[7],
            ...t,
            v[11],
          ],
          vertexCount * 12,
        );
        colors.set(
          [
            ...v
              .slice(12, 16)
              .map((value) => Math.max(0, Math.min(255, Math.round(value)))),
          ],
          vertexCount * 4,
        );
        uv1.set([v[16], v[17]], vertexCount * 2);
        vertexCount++;
        if (++packedVertices % WORK_CHUNK.outputVertices === 0) yield;
      }
      if (vertexCount > firstIndex)
        draws.push({
          firstIndex,
          indexCount: vertexCount - firstIndex,
          material,
        });
    }
    result.push({
      center: centers[chunk],
      vertices,
      colors,
      uv1,
      draws,
    });
  }
  yield;
  return result;
}
