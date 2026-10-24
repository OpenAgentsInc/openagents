export type RasterOrientation = "positive" | "negative" | "degenerate";
export type RasterOrientedDraw<T> = T & {
  rasterOrientation: RasterOrientation;
};

const f32 = Math.fround;

/** Match dot(m0.xyz, cross(m1.xyz, m2.xyz)); uncertain signs keep shader facing. */
export function classifyRasterOrientation(
  model: ArrayLike<number>,
  offset = 0,
): RasterOrientation {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset + 16 > model.length)
    throw new Error("Choose one complete source model matrix.");
  for (let index = 0; index < 16; index++)
    if (!Number.isFinite(model[offset + index]))
      throw new Error("Source raster orientation needs a finite model matrix.");
  const x0 = f32(model[offset]),
    y0 = f32(model[offset + 1]),
    z0 = f32(model[offset + 2]);
  const x1 = f32(model[offset + 4]),
    y1 = f32(model[offset + 5]),
    z1 = f32(model[offset + 6]);
  const x2 = f32(model[offset + 8]),
    y2 = f32(model[offset + 9]),
    z2 = f32(model[offset + 10]);
  const cx = f32(f32(y1 * z2) - f32(z1 * y2));
  const cy = f32(f32(z1 * x2) - f32(x1 * z2));
  const cz = f32(f32(x1 * y2) - f32(y1 * x2));
  const determinant = f32(f32(f32(x0 * cx) + f32(y0 * cy)) + f32(z0 * cz));
  const precise =
    x0 * (y1 * z2 - z1 * y2) +
    y0 * (z1 * x2 - x1 * z2) +
    z0 * (x1 * y2 - y1 * x2);
  const scale =
    Math.abs(x0) * (Math.abs(y1 * z2) + Math.abs(z1 * y2)) +
    Math.abs(y0) * (Math.abs(z1 * x2) + Math.abs(x1 * z2)) +
    Math.abs(z0) * (Math.abs(x1 * y2) + Math.abs(y1 * x2));
  // WGSL dot/cross may fuse operations. A conservative relative error bound
  // avoids making hardware culling depend on their rounding near singularity.
  const uncertainty = scale * 1e-6;
  if (
    !Number.isFinite(determinant) ||
    determinant === 0 ||
    !Number.isFinite(precise) ||
    Math.abs(precise) <= uncertainty ||
    Math.abs(determinant) <= uncertainty ||
    precise < 0 !== determinant < 0
  )
    return "degenerate";
  return determinant < 0 ? "negative" : "positive";
}

/** Split contiguous instance runs without moving models, wind, or source IDs. */
export function splitRasterOrientationDraws<
  T extends { firstInstance: number; count: number },
>(models: Float32Array, draws: readonly T[]): RasterOrientedDraw<T>[] {
  if (models.length % 16 || models.length / 16 > 1_000_000)
    throw new Error("Use complete bounded source instance matrices.");
  const count = models.length / 16;
  for (const draw of draws)
    if (
      !Number.isSafeInteger(draw.firstInstance) ||
      draw.firstInstance < 0 ||
      !Number.isSafeInteger(draw.count) ||
      draw.count < 0 ||
      draw.firstInstance + draw.count > count
    )
      throw new Error(
        "Raster draw ranges must stay within their source instances.",
      );
  const orientations: RasterOrientation[] = Array.from(
    { length: count },
    (_, index) => classifyRasterOrientation(models, index * 16),
  );
  const result: RasterOrientedDraw<T>[] = [];
  for (const draw of draws) {
    const end = draw.firstInstance + draw.count;
    if (!draw.count) {
      result.push({ ...draw, rasterOrientation: "degenerate" });
      continue;
    }
    let start = draw.firstInstance;
    while (start < end) {
      const rasterOrientation = orientations[start];
      let stop = start + 1;
      while (stop < end && orientations[stop] === rasterOrientation) stop++;
      result.push({
        ...draw,
        firstInstance: start,
        count: stop - start,
        rasterOrientation,
      });
      start = stop;
    }
  }
  return result;
}

/** Mirrored camera and model transforms each reverse raster front-face winding. */
export function rasterFrontFace(
  base: "ccw" | "cw",
  orientation: RasterOrientation,
): "ccw" | "cw" {
  return orientation === "negative" ? (base === "ccw" ? "cw" : "ccw") : base;
}
