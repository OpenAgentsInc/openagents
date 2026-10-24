/** One synchronous call owns the pooled result until the next call. */
export type PlanarScissor = readonly [number, number, number, number];
const projected = new Float64Array(8);
const rectangle: [number, number, number, number] = [0, 0, 0, 0];

function project(
  m: ArrayLike<number>,
  x: number,
  y: number,
  z: number,
): boolean {
  for (let axis = 0; axis < 4; axis++) {
    const a = m[axis] * x,
      b = m[axis + 4] * y,
      c = m[axis + 8] * z,
      d = m[axis + 12];
    projected[axis] = a + b + c + d;
    // More than the seven IEEE f32 multiply/add roundings in a four-term dot.
    // Outward intervals also cover contracted multiply/add implementations.
    projected[axis + 4] =
      (Math.abs(a) + Math.abs(b) + Math.abs(c) + Math.abs(d)) * 1e-6;
    if (
      !Number.isFinite(projected[axis]) ||
      !Number.isFinite(projected[axis + 4])
    )
      return false;
  }
  return true;
}

/** Undefined keeps the complete pass; a zero-area rectangle has no visible receiver. */
export function planarReceiverScissor(
  view: ArrayLike<number>,
  reflected: ArrayLike<number>,
  bounds: ArrayLike<number>,
  heights: ArrayLike<number>,
  planeY: number,
  width: number,
  height: number,
  distortion: number,
  planeTolerance: number,
): PlanarScissor | undefined {
  if (
    view.length !== 16 ||
    reflected.length !== 16 ||
    heights.length > 64 ||
    bounds.length !== heights.length * 6 ||
    !Number.isFinite(planeY) ||
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    !Number.isFinite(distortion) ||
    distortion < 0 ||
    distortion > 0.1 ||
    !Number.isFinite(planeTolerance) ||
    planeTolerance <= 0 ||
    planeTolerance > 0.25
  )
    return;
  for (let i = 0; i < 16; i++)
    if (!Number.isFinite(view[i]) || !Number.isFinite(reflected[i])) return;
  let left = Infinity,
    top = Infinity,
    right = -Infinity,
    bottom = -Infinity;
  for (let basin = 0; basin < heights.length; basin++) {
    const waterY = heights[basin],
      offset = basin * 6;
    if (!Number.isFinite(waterY)) return;
    // The shader compares f32 water positions and settings. Include borderline
    // heights conservatively, rather than proving exclusion with f64 arithmetic.
    if (
      Math.abs(waterY - planeY) >
      planeTolerance + Math.max(1, Math.abs(waterY), Math.abs(planeY)) * 1e-6
    )
      continue;
    let magnitude = 1;
    for (let axis = 0; axis < 3; axis++) {
      const low = bounds[offset + axis],
        high = bounds[offset + axis + 3];
      if (!Number.isFinite(low) || !Number.isFinite(high) || low > high) return;
      magnitude = Math.max(magnitude, Math.abs(low), Math.abs(high));
    }
    // Enclose f32 interpolation error around the admitted, f32-derived AABB.
    const worldPad = magnitude * 1e-6;
    let outside = 63,
      uncertain = false,
      l = Infinity,
      t = Infinity,
      r = -Infinity,
      b = -Infinity;
    for (let corner = 0; corner < 8; corner++) {
      const x =
          bounds[offset + (corner & 1 ? 3 : 0)] +
          (corner & 1 ? worldPad : -worldPad),
        y =
          bounds[offset + 1 + (corner & 2 ? 3 : 0)] +
          (corner & 2 ? worldPad : -worldPad),
        z =
          bounds[offset + 2 + (corner & 4 ? 3 : 0)] +
          (corner & 4 ? worldPad : -worldPad);
      if (!project(view, x, y, z)) return;
      const p = projected,
        w = p[3],
        ew = p[7];
      outside &=
        (p[0] < -w - p[4] - ew ? 1 : 0) |
        (p[0] > w + p[4] + ew ? 2 : 0) |
        (p[1] < -w - p[5] - ew ? 4 : 0) |
        (p[1] > w + p[5] + ew ? 8 : 0) |
        (p[2] < -p[6] ? 16 : 0) |
        (p[2] > w + p[6] + ew ? 32 : 0);
      if (w - ew <= 1e-5 || p[2] - p[6] <= 1e-5) uncertain = true;
      if (!project(reflected, x, y, z)) return;
      const lowW = p[3] - p[7],
        highW = p[3] + p[7];
      if (lowW <= 1e-5) {
        uncertain = true;
        continue;
      }
      const lowX = p[0] - p[4],
        highX = p[0] + p[4],
        lowY = p[1] - p[5],
        highY = p[1] + p[5];
      l = Math.min(l, 0.5 + (0.5 * lowX) / (lowX < 0 ? lowW : highW));
      r = Math.max(r, 0.5 + (0.5 * highX) / (highX < 0 ? highW : lowW));
      t = Math.min(t, 0.5 - (0.5 * highY) / (highY < 0 ? highW : lowW));
      b = Math.max(b, 0.5 - (0.5 * lowY) / (lowY < 0 ? lowW : highW));
    }
    if (outside) continue;
    if (uncertain) return;
    left = Math.min(left, l);
    top = Math.min(top, t);
    right = Math.max(right, r);
    bottom = Math.max(bottom, b);
  }
  // The basin normal is normalized, so |n.x| and |n.z| <= 1. The extra
  // epsilon encloses f32 normalization/settings; two texels protect LOD0
  // bilinear filtering, MSAA coverage and neighboring derivative quads.
  const warp = distortion + 1e-6;
  const x = Math.max(0, Math.min(width, Math.floor((left - warp) * width) - 2)),
    y = Math.max(0, Math.min(height, Math.floor((top - warp) * height) - 2)),
    endX = Math.max(x, Math.min(width, Math.ceil((right + warp) * width) + 2)),
    endY = Math.max(
      y,
      Math.min(height, Math.ceil((bottom + warp) * height) + 2),
    );
  rectangle[0] = x;
  rectangle[1] = y;
  rectangle[2] = endX - x;
  rectangle[3] = endY - y;
  return rectangle;
}
