/** The same complete indexed mesh is drawn for every fish, without LOD. */
export const FISH_TRIANGLES = 310;
export const FISH_VERTEX_STRIDE = 32;

export interface FishMesh {
  /** Position, normal, surface class, and longitudinal coordinate. */
  vertices: Float32Array;
  indices: Uint16Array;
  triangleCount: number;
}

export function createFishMesh(): FishMesh {
  const vertices: number[] = [],
    indices: number[] = [];
  const vertex = (
    x: number,
    y: number,
    z: number,
    nx: number,
    ny: number,
    nz: number,
    surface: number,
  ) => {
    const n = Math.hypot(nx, ny, nz);
    vertices.push(x, y, z, nx / n, ny / n, nz / n, surface, x);
    return vertices.length / 8 - 1;
  };
  const rings = [-0.78, -0.58, -0.34, -0.05, 0.23, 0.47, 0.65, 0.78];
  const radii = [0.18, 0.48, 0.81, 1, 0.99, 0.83, 0.55, 0.23];
  for (let ring = 0; ring < rings.length; ring++) {
    for (let side = 0; side < 16; side++) {
      const a = (side / 16) * Math.PI * 2;
      const y = Math.cos(a),
        z = Math.sin(a);
      vertex(
        rings[ring],
        y * radii[ring] * 0.27,
        z * radii[ring] * 0.17,
        rings[ring] * 0.3,
        y / 0.27,
        z / 0.17,
        0,
      );
    }
  }
  for (let ring = 0; ring < rings.length - 1; ring++) {
    for (let side = 0; side < 16; side++) {
      const a = ring * 16 + side,
        b = ring * 16 + ((side + 1) % 16);
      const c = a + 16,
        d = b + 16;
      indices.push(a, b, c, b, d, c);
    }
  }
  const tail = vertex(-0.88, 0, 0, -1, 0, 0, 0);
  const nose = vertex(0.87, 0, 0, 1, 0, 0, 0);
  for (let side = 0; side < 16; side++) {
    indices.push(tail, (side + 1) % 16, side);
    indices.push(nose, 112 + side, 112 + ((side + 1) % 16));
  }
  // Each planar fin has two real faces. Back-face culling selects its visible side.
  const fin = (
    points: readonly (readonly [number, number, number])[],
    normal: readonly [number, number, number],
    surface: number,
  ) => {
    for (const sign of [1, -1]) {
      const start = vertices.length / 8;
      for (const p of points)
        vertex(
          ...p,
          normal[0] * sign,
          normal[1] * sign,
          normal[2] * sign,
          surface,
        );
      for (let i = 1; i < points.length - 1; i++) {
        if (sign === 1) indices.push(start, start + i, start + i + 1);
        else indices.push(start, start + i + 1, start + i);
      }
    }
  };
  fin(
    [
      [-0.73, 0, 0],
      [-0.94, 0.13, 0],
      [-1.14, 0.31, 0],
      [-1.27, 0.38, 0],
      [-1.24, 0.2, 0],
      [-1.15, 0.06, 0],
      [-1.09, 0, 0],
      [-1.15, -0.06, 0],
      [-1.24, -0.2, 0],
      [-1.27, -0.38, 0],
      [-1.14, -0.31, 0],
      [-0.94, -0.13, 0],
    ],
    [0, 0, 1],
    1,
  );
  fin(
    [
      [-0.57, 0.12, 0],
      [-0.37, 0.29, 0],
      [-0.24, 0.52, 0],
      [-0.11, 0.5, 0],
      [0.03, 0.36, 0],
      [0.22, 0.27, 0],
      [0.06, 0.19, 0],
    ],
    [0, 0, 1],
    1,
  );
  for (const sign of [-1, 1]) {
    fin(
      [
        [0.2, -0.04, sign * 0.11],
        [0.06, -0.12, sign * 0.21],
        [-0.12, -0.21, sign * 0.44],
        [-0.36, -0.25, sign * 0.57],
        [-0.41, -0.19, sign * 0.48],
        [-0.31, -0.12, sign * 0.32],
        [-0.18, -0.06, sign * 0.18],
        [-0.02, -0.01, sign * 0.12],
      ],
      [0, 1, sign * 0.4],
      1,
    );
  }
  if (indices.length !== FISH_TRIANGLES * 3)
    throw new Error("The fish mesh must contain exactly 310 triangles.");
  return {
    vertices: new Float32Array(vertices),
    indices: new Uint16Array(indices),
    triangleCount: indices.length / 3,
  };
}
