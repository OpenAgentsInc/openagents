import { inverse, type V3 } from "./math.ts";

/** WebGPU uses clip depths zero and one; unproject both before normalizing. */
export function rayFromNdc(
  viewProjection: ArrayLike<number>,
  ndc: readonly [number, number],
): V3 {
  if (
    !viewProjection ||
    viewProjection.length !== 16 ||
    Array.from(viewProjection).some(
      (n) => !Number.isFinite(n) || Math.abs(n) > 1e12,
    ) ||
    !Array.isArray(ndc) ||
    ndc.length !== 2 ||
    ndc.some((n) => !Number.isFinite(n) || Math.abs(n) > 1)
  )
    throw new Error("Choose a valid camera and screen position.");
  let matrix: Float32Array;
  try {
    matrix = inverse(viewProjection);
  } catch {
    throw new Error("Choose a valid camera and screen position.");
  }
  const point = (depth: number): V3 => {
    const w =
      matrix[3] * ndc[0] + matrix[7] * ndc[1] + matrix[11] * depth + matrix[15];
    if (!Number.isFinite(w) || Math.abs(w) < 1e-12)
      throw new Error("The camera cannot pick this screen position.");
    const position = [0, 1, 2].map(
      (axis) =>
        (matrix[axis] * ndc[0] +
          matrix[4 + axis] * ndc[1] +
          matrix[8 + axis] * depth +
          matrix[12 + axis]) /
        w,
    ) as V3;
    if (position.some((n) => !Number.isFinite(n) || Math.abs(n) > 1e12))
      throw new Error("The camera cannot pick this screen position.");
    return position;
  };
  const near = point(0),
    far = point(1),
    delta = far.map((n, axis) => n - near[axis]) as V3;
  const length = Math.hypot(...delta);
  if (!Number.isFinite(length) || length < 1e-12)
    throw new Error("The camera cannot pick this screen position.");
  return delta.map((n) => n / length) as V3;
}

export interface Bounds {
  min: V3;
  max: V3;
}
export interface Transform {
  position: V3;
  yaw: number;
  scale: number;
}
/** Ray parameter stays in world meters, including when testing scaled local bounds. */
export function rayBox(
  origin: V3,
  ray: V3,
  bounds: Bounds,
  transform?: Transform,
): number | null {
  let o = origin,
    d = ray;
  if (transform) {
    const c = Math.cos(transform.yaw),
      s = Math.sin(transform.yaw),
      k = transform.scale;
    const x = origin[0] - transform.position[0],
      z = origin[2] - transform.position[2];
    o = [
      (c * x - s * z) / k,
      (origin[1] - transform.position[1]) / k,
      (s * x + c * z) / k,
    ];
    d = [
      (c * ray[0] - s * ray[2]) / k,
      ray[1] / k,
      (s * ray[0] + c * ray[2]) / k,
    ];
  }
  let near = 0,
    far = Infinity;
  for (let i = 0; i < 3; i++) {
    if (Math.abs(d[i]) < 1e-9) {
      if (o[i] < bounds.min[i] || o[i] > bounds.max[i]) return null;
      continue;
    }
    const a = (bounds.min[i] - o[i]) / d[i],
      b = (bounds.max[i] - o[i]) / d[i];
    near = Math.max(near, Math.min(a, b));
    far = Math.min(far, Math.max(a, b));
    if (near > far) return null;
  }
  return near;
}
