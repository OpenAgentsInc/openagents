import type { CompiledScene } from "../assets/src/scene.ts";
import {
  GI_INVALID,
  GI_LEAF_BIT,
  type GiAcceleration,
  type GiHit,
} from "./gi-acceleration.ts";
import {
  evaluateGiCoverage,
  type GiMaterialContext,
  type GiTextureSampler,
} from "./gi-materials.ts";
import { dot, normalize, type V3 } from "./math.ts";
export interface GiSurfaceHit extends GiHit {
  context: GiMaterialContext;
}
export interface GiTransportAttributes {
  vertices: Float32Array;
  colors: Uint8Array;
  uv1?: Float32Array;
}
export interface GiTransportBudget {
  maxQueries: number;
  maxMaskTestsPerRay: number;
  maxMilliseconds: number;
  startedAt?: number;
}
export interface GiSurfaceTransport {
  trace: (
    origin: V3,
    direction: V3,
    minimum: number,
    maximum: number,
  ) => GiSurfaceHit | undefined;
  occluded: (
    origin: V3,
    direction: V3,
    minimum: number,
    maximum: number,
  ) => boolean;
  stats: { queries: number; maskTests: number; transparentHits: number };
}
/** Immutable instanced hierarchy queries with source material alpha checked before nearest-hit selection. */
export function createGiSurfaceTransport(
  a: GiAcceleration,
  scene: Pick<CompiledScene, "materials" | "vertexCount">,
  attributes: GiTransportAttributes,
  sample: GiTextureSampler,
  options: Partial<GiTransportBudget> = {},
): GiSurfaceTransport {
  const budget = {
    maxQueries: 6_000_000,
    maxMaskTestsPerRay: 512,
    maxMilliseconds: 120_000,
    ...options,
  };
  if (
    !Number.isSafeInteger(budget.maxQueries) ||
    budget.maxQueries < 1 ||
    budget.maxQueries > 6_000_000 ||
    !Number.isSafeInteger(budget.maxMaskTestsPerRay) ||
    budget.maxMaskTestsPerRay < 1 ||
    budget.maxMaskTestsPerRay > 512 ||
    !Number.isFinite(budget.maxMilliseconds) ||
    budget.maxMilliseconds <= 0 ||
    budget.maxMilliseconds > 120_000
  )
    throw new Error("Use bounded GI transport budgets.");
  if (
    attributes.vertices.length !== scene.vertexCount * 12 ||
    attributes.colors.length !== scene.vertexCount * 4 ||
    (attributes.uv1 && attributes.uv1.length !== scene.vertexCount * 2) ||
    !attributes.vertices.every(Number.isFinite) ||
    (attributes.uv1 && !attributes.uv1.every(Number.isFinite))
  )
    throw new Error("GI surface attributes do not match source vertices.");
  const started = options.startedAt ?? performance.now(),
    stats = { queries: 0, maskTests: 0, transparentHits: 0 };
  if (
    !Number.isFinite(started) ||
    started < 0 ||
    started > performance.now() + 1
  )
    throw new Error("Use a finite GI preparation start time.");
  function contextAt(
    instance: number,
    triangle: number,
    baryU: number,
    baryV: number,
    origin: V3,
    direction: V3,
    distance: number,
  ): GiMaterialContext {
    const offset = instance * 32,
      t = triangle * 8,
      bary = [1 - baryU - baryV, baryU, baryV],
      ids = [triangles[t], triangles[t + 1], triangles[t + 2]];
    const interpolate = (
      array: ArrayLike<number>,
      stride: number,
      start: number,
      channels: number,
    ) =>
      Array.from({ length: channels }, (_, c) =>
        ids.reduce((s, v, i) => s + bary[i] * array[v * stride + start + c], 0),
      );
    const ia = ids[0] * 4,
      ib = ids[1] * 4,
      ic = ids[2] * 4,
      ex = positions[ib] - positions[ia],
      ey = positions[ib + 1] - positions[ia + 1],
      ez = positions[ib + 2] - positions[ia + 2],
      gx = positions[ic] - positions[ia],
      gy = positions[ic + 1] - positions[ia + 1],
      gz = positions[ic + 2] - positions[ia + 2];
    const e = [
        f[offset] * ex + f[offset + 1] * ey + f[offset + 2] * ez,
        f[offset + 4] * ex + f[offset + 5] * ey + f[offset + 6] * ez,
        f[offset + 8] * ex + f[offset + 9] * ey + f[offset + 10] * ez,
      ],
      g = [
        f[offset] * gx + f[offset + 1] * gy + f[offset + 2] * gz,
        f[offset + 4] * gx + f[offset + 5] * gy + f[offset + 6] * gz,
        f[offset + 8] * gx + f[offset + 9] * gy + f[offset + 10] * gz,
      ];
    let n = normalize([
      e[1] * g[2] - e[2] * g[1],
      e[2] * g[0] - e[0] * g[2],
      e[0] * g[1] - e[1] * g[0],
    ]);
    if (dot(n, direction) > 0) n = n.map((v) => -v) as V3;
    return {
      uv: interpolate(attributes.vertices, 12, 6, 2) as [number, number],
      uv1: attributes.uv1
        ? (interpolate(attributes.uv1, 2, 0, 2) as [number, number])
        : [0, 0],
      vertexColor: interpolate(attributes.colors, 4, 0, 4).map(
        (v) => v / 255,
      ) as [number, number, number, number],
      world: origin.map((v, i) => v + direction[i] * distance) as V3,
      geometricNormal: n,
      objectScale: [
        Math.hypot(f[offset], f[offset + 4], f[offset + 8]),
        Math.hypot(f[offset + 1], f[offset + 5], f[offset + 9]),
        Math.hypot(f[offset + 2], f[offset + 6], f[offset + 10]),
      ],
    };
  }
  const bf = new Float32Array(a.blasNodes),
    bu = new Uint32Array(a.blasNodes),
    tf = new Float32Array(a.tlasNodes),
    tu = new Uint32Array(a.tlasNodes),
    f = new Float32Array(a.instances),
    u = new Uint32Array(a.instances),
    positions = a.positions,
    triangles = a.triangles;
  const outer = new Uint32Array(64),
    inner = new Uint32Array(64);
  function box(
    data: Float32Array,
    node: number,
    ox: number,
    oy: number,
    oz: number,
    dx: number,
    dy: number,
    dz: number,
    min: number,
    max: number,
  ): boolean {
    const o = node * 8;
    if (dx === 0) {
      if (ox < data[o] || ox > data[o + 4]) return false;
    } else {
      let lo = (data[o] - ox) / dx,
        hi = (data[o + 4] - ox) / dx;
      if (lo > hi) {
        const t = lo;
        lo = hi;
        hi = t;
      }
      min = Math.max(min, lo);
      max = Math.min(max, hi);
      if (min > max) return false;
    }
    if (dy === 0) {
      if (oy < data[o + 1] || oy > data[o + 5]) return false;
    } else {
      let lo = (data[o + 1] - oy) / dy,
        hi = (data[o + 5] - oy) / dy;
      if (lo > hi) {
        const t = lo;
        lo = hi;
        hi = t;
      }
      min = Math.max(min, lo);
      max = Math.min(max, hi);
      if (min > max) return false;
    }
    if (dz === 0) {
      if (oz < data[o + 2] || oz > data[o + 6]) return false;
    } else {
      let lo = (data[o + 2] - oz) / dz,
        hi = (data[o + 6] - oz) / dz;
      if (lo > hi) {
        const t = lo;
        lo = hi;
        hi = t;
      }
      min = Math.max(min, lo);
      max = Math.min(max, hi);
      if (min > max) return false;
    }
    return true;
  }
  const cast = (
    origin: V3,
    direction: V3,
    minimum: number,
    maximum: number,
    anyHit = false,
  ): GiSurfaceHit | boolean | undefined => {
    if (
      ![...origin, ...direction, minimum, maximum].every(Number.isFinite) ||
      minimum < 0 ||
      maximum <= minimum ||
      maximum > 1e6 ||
      Math.abs(Math.hypot(...direction) - 1) > 1e-4
    )
      throw new Error("Use a bounded normalized GI ray.");
    stats.queries++;
    if (
      stats.queries > budget.maxQueries ||
      (stats.queries % 128 === 1 &&
        performance.now() - started > budget.maxMilliseconds)
    )
      throw new Error("The GI preparation exceeded its ray or time budget.");
    let rayMaskTests = 0;
    if (a.tlasRoot === GI_INVALID) return;
    let nearest = maximum,
      bestInstance = -1,
      bestTriangle = -1,
      bestMaterial = 0,
      bestU = 0,
      bestV = 0;
    const [wx, wy, wz] = origin,
      [vx, vy, vz] = direction;
    let top = 1;
    outer[0] = a.tlasRoot;
    while (top) {
      const node = outer[--top];
      if (!box(tf, node, wx, wy, wz, vx, vy, vz, minimum, nearest)) continue;
      const first = tu[node * 8 + 3],
        meta = tu[node * 8 + 7];
      if (!(meta & GI_LEAF_BIT)) {
        outer[top++] = meta;
        outer[top++] = first;
        continue;
      }
      for (
        let instance = first;
        instance < first + (meta & ~GI_LEAF_BIT);
        instance++
      ) {
        const offset = instance * 32,
          q = offset + 12;
        const ox = f[q] * wx + f[q + 1] * wy + f[q + 2] * wz + f[q + 3],
          oy = f[q + 4] * wx + f[q + 5] * wy + f[q + 6] * wz + f[q + 7],
          oz = f[q + 8] * wx + f[q + 9] * wy + f[q + 10] * wz + f[q + 11];
        const dx = f[q] * vx + f[q + 1] * vy + f[q + 2] * vz,
          dy = f[q + 4] * vx + f[q + 5] * vy + f[q + 6] * vz,
          dz = f[q + 8] * vx + f[q + 9] * vy + f[q + 10] * vz;
        let depth = 1;
        inner[0] = u[offset + 24];
        while (depth) {
          const b = inner[--depth];
          if (!box(bf, b, ox, oy, oz, dx, dy, dz, minimum, nearest)) continue;
          const start = bu[b * 8 + 3],
            flags = bu[b * 8 + 7];
          if (!(flags & GI_LEAF_BIT)) {
            inner[depth++] = flags;
            inner[depth++] = start;
            continue;
          }
          for (
            let triangle = start;
            triangle < start + (flags & ~GI_LEAF_BIT);
            triangle++
          ) {
            const t = triangle * 8,
              slot = triangles[t + 4],
              override =
                slot < u[offset + 27]
                  ? a.materialOverrides[u[offset + 26] + slot]
                  : GI_INVALID,
              material = override === GI_INVALID ? triangles[t + 3] : override;
            if (!a.materialTransport[material]) continue;
            const ia = triangles[t] * 4,
              ib = triangles[t + 1] * 4,
              ic = triangles[t + 2] * 4;
            const ax = positions[ia],
              ay = positions[ia + 1],
              az = positions[ia + 2],
              e1x = positions[ib] - ax,
              e1y = positions[ib + 1] - ay,
              e1z = positions[ib + 2] - az,
              e2x = positions[ic] - ax,
              e2y = positions[ic + 1] - ay,
              e2z = positions[ic + 2] - az;
            const px = dy * e2z - dz * e2y,
              py = dz * e2x - dx * e2z,
              pz = dx * e2y - dy * e2x,
              det = e1x * px + e1y * py + e1z * pz;
            // The local ray is deliberately unnormalized. Use an angular test
            // so equivalent mesh/instance scales preserve the same world hit.
            const determinantScale =
              (e1x * e1x + e1y * e1y + e1z * e1z) *
              (e2x * e2x + e2y * e2y + e2z * e2z) *
              (dx * dx + dy * dy + dz * dz);
            if (det * det <= 1e-24 * determinantScale) continue;
            const tx = ox - ax,
              ty = oy - ay,
              tz = oz - az,
              inv = 1 / det,
              baryU = (tx * px + ty * py + tz * pz) * inv;
            if (baryU < 0 || baryU > 1) continue;
            const qx = ty * e1z - tz * e1y,
              qy = tz * e1x - tx * e1z,
              qz = tx * e1y - ty * e1x,
              baryV = (dx * qx + dy * qy + dz * qz) * inv;
            if (baryV < 0 || baryU + baryV > 1) continue;
            const distance = (e2x * qx + e2y * qy + e2z * qz) * inv;
            if (distance < minimum || distance > nearest) continue;
            if (a.materialTransport[material] === 2) {
              stats.maskTests++;
              rayMaskTests++;
              if (rayMaskTests > budget.maxMaskTestsPerRay)
                throw new Error("A GI ray exceeded its alpha-test budget.");
              const context = contextAt(
                instance,
                triangle,
                baryU,
                baryV,
                origin,
                direction,
                distance,
              );
              const coverage = evaluateGiCoverage(
                scene.materials[material],
                context,
                sample,
              );
              if (coverage.coverage < coverage.cutoff) {
                stats.transparentHits++;
                continue;
              }
            }
            if (anyHit) return true;
            nearest = distance;
            bestInstance = instance;
            bestTriangle = triangle;
            bestMaterial = material;
            bestU = baryU;
            bestV = baryV;
          }
        }
      }
    }
    if (bestInstance < 0) return;
    const t = bestTriangle * 8,
      offset = bestInstance * 32,
      ia = triangles[t] * 4,
      ib = triangles[t + 1] * 4,
      ic = triangles[t + 2] * 4;
    const ex = positions[ib] - positions[ia],
      ey = positions[ib + 1] - positions[ia + 1],
      ez = positions[ib + 2] - positions[ia + 2],
      gx = positions[ic] - positions[ia],
      gy = positions[ic + 1] - positions[ia + 1],
      gz = positions[ic + 2] - positions[ia + 2];
    const e: [number, number, number] = [
      f[offset] * ex + f[offset + 1] * ey + f[offset + 2] * ez,
      f[offset + 4] * ex + f[offset + 5] * ey + f[offset + 6] * ez,
      f[offset + 8] * ex + f[offset + 9] * ey + f[offset + 10] * ez,
    ];
    const g: [number, number, number] = [
      f[offset] * gx + f[offset + 1] * gy + f[offset + 2] * gz,
      f[offset + 4] * gx + f[offset + 5] * gy + f[offset + 6] * gz,
      f[offset + 8] * gx + f[offset + 9] * gy + f[offset + 10] * gz,
    ];
    const normal = normalize([
      e[1] * g[2] - e[2] * g[1],
      e[2] * g[0] - e[0] * g[2],
      e[0] * g[1] - e[1] * g[0],
    ]);
    return {
      distance: nearest,
      normal,
      position: origin.map((v, i) => v + direction[i] * nearest) as V3,
      barycentric: [1 - bestU - bestV, bestU, bestV],
      sourceInstance: u[offset + 28],
      sourceMesh: u[offset + 25],
      sourceTriangle: triangles[t + 5],
      material: bestMaterial,
      context: contextAt(
        bestInstance,
        bestTriangle,
        bestU,
        bestV,
        origin,
        direction,
        nearest,
      ),
      frontFace: dot(normal, direction) < 0,
      maskedApproximation: a.materialTransport[bestMaterial] === 2,
    };
  };
  return {
    trace: (o, d, min, max) => cast(o, d, min, max) as GiSurfaceHit | undefined,
    occluded: (o, d, min, max) => cast(o, d, min, max, true) === true,
    stats,
  };
}
