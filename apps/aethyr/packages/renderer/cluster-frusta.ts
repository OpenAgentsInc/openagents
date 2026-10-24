import type { ClusteredLights } from "./clustered-lights.ts";

/** Conservative world planes for the shader's screen tiles and depth slices. */
export class ClusterFrusta {
  private readonly x: Float64Array;
  private readonly y: Float64Array;
  private readonly depths: Float64Array;
  constructor(
    private readonly camera: Pick<
      ClusteredLights,
      "viewProjection" | "eye" | "forward" | "grid"
    >,
  ) {
    const { viewProjection: m, grid } = camera;
    this.x = new Float64Array((grid.tilesX + 1) * 4);
    this.y = new Float64Array((grid.tilesY + 1) * 4);
    this.depths = new Float64Array(grid.depthSlices + 1);
    const plane = (
      out: Float64Array,
      index: number,
      row: number,
      edge: number,
    ) => {
      const a = m[row] - edge * m[3],
        b = m[row + 4] - edge * m[7];
      const c = m[row + 8] - edge * m[11],
        d = m[row + 12] - edge * m[15];
      const length = Math.hypot(a, b, c);
      // A degenerate plane cannot safely exclude an influence sphere.
      if (!Number.isFinite(length) || length < 1e-30) return;
      out[index * 4] = a / length;
      out[index * 4 + 1] = b / length;
      out[index * 4 + 2] = c / length;
      out[index * 4 + 3] = d / length;
    };
    for (let tile = 0; tile <= grid.tilesX; tile++)
      plane(
        this.x,
        tile,
        0,
        (2 * Math.min(tile * grid.tileSize, grid.width)) / grid.width - 1,
      );
    for (let tile = 0; tile <= grid.tilesY; tile++)
      plane(
        this.y,
        tile,
        1,
        1 - (2 * Math.min(tile * grid.tileSize, grid.height)) / grid.height,
      );
    for (let slice = 1; slice < grid.depthSlices; slice++)
      this.depths[slice] = grid.near * Math.exp(slice / grid.logDepthScale);
    // The shader clamps into the end slices, including depths outside near/far.
    this.depths[0] = -Infinity;
    this.depths[grid.depthSlices] = Infinity;
  }

  private outside(
    planes: Float64Array,
    boundary: number,
    sign: number,
    x: number,
    y: number,
    z: number,
    radius: number,
  ) {
    const offset = boundary * 4;
    const a = planes[offset] * x,
      b = planes[offset + 1] * y;
    const c = planes[offset + 2] * z,
      d = planes[offset + 3];
    // Cover float32 clip/interpolation error and cancellation at distant views.
    const epsilon =
      1e-5 *
      (1 + Math.abs(a) + Math.abs(b) + Math.abs(c) + Math.abs(d) + radius);
    return sign * (a + b + c + d) < -radius - epsilon;
  }

  /** Trim only wholly separated edge cells; retained interiors may overestimate. */
  tighten(
    bound: number[],
    lights: Float32Array,
    offset: number,
  ): number[] | undefined {
    const x = lights[offset],
      y = lights[offset + 1],
      z = lights[offset + 2];
    const radius =
      lights[offset + 3] +
      1e-5 *
        Math.max(1, Math.abs(x), Math.abs(y), Math.abs(z), lights[offset + 3]);
    const xOutside = (tile: number) =>
      this.outside(this.x, tile, 1, x, y, z, radius) ||
      this.outside(this.x, tile + 1, -1, x, y, z, radius);
    const yOutside = (tile: number) =>
      this.outside(this.y, tile, -1, x, y, z, radius) ||
      this.outside(this.y, tile + 1, 1, x, y, z, radius);
    while (bound[0] <= bound[1] && xOutside(bound[0])) bound[0]++;
    while (bound[1] >= bound[0] && xOutside(bound[1])) bound[1]--;
    if (bound[0] > bound[1]) return;
    while (bound[2] <= bound[3] && yOutside(bound[2])) bound[2]++;
    while (bound[3] >= bound[2] && yOutside(bound[3])) bound[3]--;
    if (bound[2] > bound[3]) return;
    const { eye, forward } = this.camera;
    const depth =
      (x - eye[0]) * forward[0] +
      (y - eye[1]) * forward[1] +
      (z - eye[2]) * forward[2];
    const epsilon =
      1e-5 *
      (1 +
        Math.abs(x) +
        Math.abs(y) +
        Math.abs(z) +
        Math.abs(eye[0]) +
        Math.abs(eye[1]) +
        Math.abs(eye[2]) +
        radius +
        Math.abs(depth));
    while (
      bound[4] <= bound[5] &&
      depth - radius > this.depths[bound[4] + 1] + epsilon
    )
      bound[4]++;
    while (
      bound[5] >= bound[4] &&
      depth + radius < this.depths[bound[5]] - epsilon
    )
      bound[5]--;
    return bound[4] <= bound[5] ? bound : undefined;
  }
}
