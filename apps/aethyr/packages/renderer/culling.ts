import type { CompiledScene } from "../assets/src/scene.ts";

export interface InstanceBounds {
  count: number;
  /** Packed center XYZ and radius, in world meters. CPU calculations retain float64 precision. */
  spheres: Float64Array;
  /** Packed world-space min XYZ, max XYZ. */
  aabbs: Float64Array;
}

/** Precomputes conservative bounds without changing source instance indices. */
export function buildInstanceBounds(
  scene: Pick<CompiledScene, "meshes" | "instances">,
): InstanceBounds {
  const count = scene.instances.length;
  const spheres = new Float64Array(count * 4),
    aabbs = new Float64Array(count * 6);
  scene.instances.forEach((instance, i) => {
    const mesh = scene.meshes[instance.mesh],
      m = instance.model;
    if (
      !Number.isInteger(instance.mesh) ||
      !mesh ||
      m.length !== 16 ||
      !m.every(Number.isFinite) ||
      Math.abs(m[3]) + Math.abs(m[7]) + Math.abs(m[11]) > 1e-10 ||
      Math.abs(m[15] - 1) > 1e-10
    )
      throw new Error(
        "Instance bounds need a valid mesh and finite affine transform.",
      );
    const { min, max } = mesh.bounds;
    if (
      min.length !== 3 ||
      max.length !== 3 ||
      !min.every(Number.isFinite) ||
      !max.every(Number.isFinite) ||
      min.some((n, axis) => n > max[axis])
    )
      throw new Error(
        "Mesh bounds need finite, ordered minimum and maximum points.",
      );
    const corners: number[][] = [];
    const low = [Infinity, Infinity, Infinity],
      high = [-Infinity, -Infinity, -Infinity];
    for (let corner = 0; corner < 8; corner++) {
      const x = (corner & 1 ? max : min)[0],
        y = (corner & 2 ? max : min)[1],
        z = (corner & 4 ? max : min)[2];
      const p = [
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14],
      ];
      if (!p.every(Number.isFinite))
        throw new Error(
          "Transformed mesh bounds exceed the finite coordinate range.",
        );
      corners.push(p);
      p.forEach((n, axis) => {
        low[axis] = Math.min(low[axis], n);
        high[axis] = Math.max(high[axis], n);
      });
    }
    const center = low.map((n, axis) => n + (high[axis] - n) * 0.5);
    const radius = Math.max(
      ...corners.map((p) =>
        Math.hypot(p[0] - center[0], p[1] - center[1], p[2] - center[2]),
      ),
    );
    if (![...center, radius].every(Number.isFinite))
      throw new Error("Transformed mesh bounds are too large.");
    spheres.set([...center, radius], i * 4);
    aabbs.set([...low, ...high], i * 6);
  });
  return { count, spheres, aabbs };
}

/** Six inward, normalized planes for column-major WebGPU clip space: -w <= XY <= w, 0 <= Z <= w. */
export function frustumPlanes(viewProjection: ArrayLike<number>): Float64Array {
  if (
    viewProjection.length !== 16 ||
    !Array.from(viewProjection).every(Number.isFinite) ||
    [3, 7, 11, 15].every((i) => viewProjection[i] === 0)
  )
    throw new Error("Frustum culling needs a finite view-projection matrix.");
  const m = viewProjection,
    planes = new Float64Array(24);
  const rows = [0, 1, 2, 3].map((r) => [m[r], m[r + 4], m[r + 8], m[r + 12]]);
  const raw = [
    rows[3].map((n, i) => n + rows[0][i]),
    rows[3].map((n, i) => n - rows[0][i]),
    rows[3].map((n, i) => n + rows[1][i]),
    rows[3].map((n, i) => n - rows[1][i]),
    rows[2],
    rows[3].map((n, i) => n - rows[2][i]),
  ];
  raw.forEach((plane, i) => {
    const length = Math.hypot(plane[0], plane[1], plane[2]);
    // An infinite far plane has no normal and a positive constant; it rejects nothing.
    planes.set(
      length > 1e-15
        ? plane.map((n) => n / length)
        : [0, 0, 0, plane[3] < 0 ? -1 : 1],
      i * 4,
    );
  });
  return planes;
}

/** Conservative sphere and AABB rejection. The optional output is reused without allocation. */
export function frustumVisibility(
  bounds: InstanceBounds,
  viewProjection: ArrayLike<number>,
  output?: Uint8Array,
): Uint8Array {
  if (
    !Number.isInteger(bounds.count) ||
    bounds.count < 0 ||
    bounds.spheres.length !== bounds.count * 4 ||
    bounds.aabbs.length !== bounds.count * 6 ||
    (output && output.length !== bounds.count)
  )
    throw new Error(
      "Visibility buffers must match the number of instance bounds.",
    );
  const planes = frustumPlanes(viewProjection),
    visible = output ?? new Uint8Array(bounds.count);
  for (let i = 0; i < bounds.count; i++) {
    const s = i * 4,
      a = i * 6,
      x = bounds.spheres[s],
      y = bounds.spheres[s + 1],
      z = bounds.spheres[s + 2],
      r = bounds.spheres[s + 3];
    const epsilon =
      1e-5 * Math.max(1, r) + 1e-7 * (Math.abs(x) + Math.abs(y) + Math.abs(z));
    visible[i] = 1;
    for (let p = 0; p < planes.length; p += 4) {
      const nx = planes[p],
        ny = planes[p + 1],
        nz = planes[p + 2],
        d = planes[p + 3];
      if (
        nx * x + ny * y + nz * z + d < -r - epsilon ||
        nx * bounds.aabbs[a + (nx >= 0 ? 3 : 0)] +
          ny * bounds.aabbs[a + (ny >= 0 ? 4 : 1)] +
          nz * bounds.aabbs[a + (nz >= 0 ? 5 : 2)] +
          d <
          -epsilon
      ) {
        visible[i] = 0;
        break;
      }
    }
  }
  return visible;
}

/** Splits each original draw into contiguous visible runs; material and geometry fields stay intact. */
export function visibleDraws<
  T extends { firstInstance: number; count: number },
>(draws: readonly T[], visibility: Uint8Array): T[] {
  const result: T[] = [];
  for (const draw of draws) {
    if (
      !Number.isInteger(draw.firstInstance) ||
      draw.firstInstance < 0 ||
      !Number.isInteger(draw.count) ||
      draw.count < 0 ||
      draw.firstInstance + draw.count > visibility.length
    )
      throw new Error(
        "Draw ranges must stay inside the instance visibility buffer.",
      );
    const end = draw.firstInstance + draw.count;
    let start = -1;
    for (let i = draw.firstInstance; i <= end; i++) {
      if (i < end && visibility[i] !== 0) {
        if (start < 0) start = i;
      } else if (start >= 0) {
        result.push({ ...draw, firstInstance: start, count: i - start });
        start = -1;
      }
    }
  }
  return result;
}

/** Gathers GPU matrices in source order and keeps one draw per original primitive/batch. */
export function compactInstanceDraws<
  T extends { firstInstance: number; count: number },
>(
  models: Float32Array,
  draws: readonly T[],
  visibility: Uint8Array,
): { models: Float32Array; draws: T[] } {
  if (
    !(models instanceof Float32Array) ||
    models.length % 16 !== 0 ||
    !(visibility instanceof Uint8Array) ||
    visibility.length !== models.length / 16
  )
    throw new Error(
      "Compacted instances need 16 floats per matrix and one visibility flag per instance.",
    );
  const count = visibility.length;
  // Check every source transform, including invisible instances, before publishing a buffer.
  for (let i = 0; i < count; i++) {
    const start = i * 16;
    for (let column = 0; column < 16; column++)
      if (!Number.isFinite(models[start + column]))
        throw new Error(
          "Compacted instance matrices must contain finite values.",
        );
    if (
      Math.abs(models[start + 3]) +
        Math.abs(models[start + 7]) +
        Math.abs(models[start + 11]) >
        1e-6 ||
      Math.abs(models[start + 15] - 1) > 1e-6
    )
      throw new Error("Compacted instances need affine model matrices.");
    if (visibility[i] !== 0 && visibility[i] !== 1)
      throw new Error("Use zero or one for each instance visibility flag.");
  }
  for (const draw of draws)
    if (
      !Number.isInteger(draw.firstInstance) ||
      draw.firstInstance < 0 ||
      !Number.isInteger(draw.count) ||
      draw.count < 0 ||
      draw.firstInstance + draw.count > count
    )
      throw new Error(
        "Draw ranges must stay inside the source instance matrices.",
      );
  const rank = new Uint32Array(count + 1);
  for (let i = 0; i < count; i++) rank[i + 1] = rank[i] + visibility[i];
  const compacted = new Float32Array(rank[count] * 16);
  for (let i = 0; i < count; i++)
    if (visibility[i])
      compacted.set(models.subarray(i * 16, i * 16 + 16), rank[i] * 16);
  const compactedDraws: T[] = [];
  for (const draw of draws) {
    const firstInstance = rank[draw.firstInstance],
      visibleCount = rank[draw.firstInstance + draw.count] - firstInstance;
    if (visibleCount > 0)
      compactedDraws.push({ ...draw, firstInstance, count: visibleCount });
  }
  return { models: compacted, draws: compactedDraws };
}
