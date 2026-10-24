// Adapted from Quick_3D_MMORPG shared/noise.mjs and simplex-noise.mjs.
// Copyright (c) 2018 Jonas Wagner; see third_party/simplex-noise/LICENSE.
// Source seed 1 fixes the terrain for simulation, rendering, and asset placement.
const PERM = new Uint8Array([
  134, 32, 123, 160, 45, 84, 8, 75, 127, 33, 73, 189, 243, 14, 227, 213, 116,
  101, 71, 118, 232, 177, 38, 36, 132, 247, 23, 223, 201, 145, 29, 203, 113,
  131, 200, 129, 192, 37, 179, 195, 198, 158, 253, 34, 69, 242, 251, 30, 252,
  254, 110, 219, 186, 222, 89, 121, 185, 235, 2, 107, 218, 249, 119, 25, 124,
  156, 136, 238, 161, 55, 7, 40, 208, 43, 22, 144, 142, 39, 163, 80, 141, 140,
  99, 130, 44, 92, 31, 109, 209, 68, 204, 180, 115, 170, 199, 20, 173, 157, 27,
  175, 149, 230, 54, 137, 91, 26, 28, 85, 202, 63, 56, 205, 5, 67, 83, 96, 49,
  117, 62, 48, 229, 90, 42, 120, 150, 61, 153, 72, 212, 16, 59, 224, 220, 17,
  122, 228, 81, 174, 233, 6, 78, 112, 35, 57, 53, 143, 93, 47, 217, 105, 221,
  100, 250, 0, 50, 162, 10, 102, 194, 245, 111, 41, 128, 88, 13, 4, 184, 58,
  241, 155, 178, 172, 3, 248, 87, 214, 52, 171, 76, 196, 24, 240, 103, 97, 159,
  138, 82, 225, 108, 133, 135, 239, 176, 65, 11, 181, 79, 234, 246, 244, 86,
  125, 216, 9, 169, 187, 106, 206, 188, 74, 210, 1, 197, 226, 164, 60, 139, 193,
  168, 98, 66, 154, 182, 77, 215, 191, 147, 21, 167, 211, 114, 12, 166, 94, 207,
  51, 95, 165, 19, 183, 152, 237, 148, 255, 231, 151, 126, 15, 70, 236, 46, 104,
  18, 190, 146, 64,
]);
const PERM2 = new Uint8Array([
  117, 193, 231, 146, 225, 180, 92, 76, 249, 22, 228, 173, 138, 36, 243, 55, 4,
  182, 199, 17, 237, 102, 178, 218, 241, 248, 187, 154, 212, 198, 255, 65, 226,
  204, 7, 90, 158, 141, 109, 19, 155, 140, 70, 169, 101, 44, 68, 57, 2, 131,
  171, 46, 38, 59, 67, 221, 43, 175, 74, 184, 93, 1, 9, 127, 186, 132, 143, 110,
  95, 52, 227, 177, 174, 11, 116, 194, 27, 181, 49, 253, 112, 23, 73, 88, 208,
  251, 100, 242, 69, 83, 48, 10, 235, 63, 71, 79, 85, 5, 236, 164, 12, 159, 24,
  156, 130, 152, 250, 106, 56, 191, 197, 35, 82, 162, 246, 214, 240, 118, 201,
  94, 62, 134, 179, 125, 207, 120, 50, 128, 31, 14, 45, 136, 108, 219, 200, 151,
  72, 51, 15, 105, 89, 133, 223, 247, 129, 30, 205, 150, 6, 139, 58, 111, 220,
  123, 167, 91, 29, 8, 81, 37, 148, 245, 18, 190, 0, 210, 54, 172, 211, 60, 97,
  114, 244, 13, 170, 77, 66, 189, 222, 104, 224, 119, 96, 115, 153, 122, 233,
  40, 232, 103, 147, 99, 229, 47, 163, 234, 252, 209, 34, 145, 25, 80, 78, 41,
  86, 160, 166, 203, 213, 33, 144, 217, 192, 87, 39, 42, 21, 26, 113, 28, 157,
  126, 196, 254, 142, 20, 165, 64, 238, 135, 3, 185, 183, 202, 239, 61, 84, 195,
  32, 168, 53, 176, 107, 124, 137, 216, 161, 215, 75, 16, 98, 206, 121, 149,
  188, 230,
]);
const GRAD = [
  [1, 1, 0],
  [-1, 1, 0],
  [1, -1, 0],
  [-1, -1, 0],
  [1, 0, 1],
  [-1, 0, 1],
  [1, 0, -1],
  [-1, 0, -1],
  [0, 1, 1],
  [0, -1, 1],
  [0, 1, -1],
  [0, -1, -1],
];
export function mmorpgSimplex3(
  x: number,
  y: number,
  z: number,
  seed: 1 | 2 = 1,
): number {
  const table = seed === 1 ? PERM : PERM2;
  const p = (i: number) => table[i & 255];
  const skew = (x + y + z) / 3;
  const i = Math.floor(x + skew),
    j = Math.floor(y + skew),
    k = Math.floor(z + skew);
  const t = (i + j + k) / 6;
  const a = x - i + t,
    b = y - j + t,
    c = z - k + t;
  const first =
    a >= b
      ? b >= c
        ? [1, 0, 0]
        : a >= c
          ? [1, 0, 0]
          : [0, 0, 1]
      : b < c
        ? [0, 0, 1]
        : a < c
          ? [0, 1, 0]
          : [0, 1, 0];
  const second =
    a >= b
      ? b >= c
        ? [1, 1, 0]
        : a >= c
          ? [1, 0, 1]
          : [1, 0, 1]
      : b < c
        ? [0, 1, 1]
        : a < c
          ? [0, 1, 1]
          : [1, 1, 0];
  const corners = [
    [a, b, c, 0, 0, 0],
    [
      a - first[0] + 1 / 6,
      b - first[1] + 1 / 6,
      c - first[2] + 1 / 6,
      ...first,
    ],
    [
      a - second[0] + 1 / 3,
      b - second[1] + 1 / 3,
      c - second[2] + 1 / 3,
      ...second,
    ],
    [a - 0.5, b - 0.5, c - 0.5, 1, 1, 1],
  ];
  let sum = 0;
  for (const [dx, dy, dz, di, dj, dk] of corners) {
    let q = 0.6 - dx * dx - dy * dy - dz * dz;
    if (q < 0) continue;
    const g = GRAD[p(i + di + p(j + dj + p(k + dk))) % 12];
    q *= q;
    sum += q * q * (g[0] * dx + g[1] * dy + g[2] * dz);
  }
  return 32 * sum;
}
/** Exact source height in world metres; source units are six times larger. */
export function mmorpgHeight(x: number, z: number): number {
  let amplitude = 1,
    frequency = 1,
    total = 0,
    normalization = 0;
  for (let o = 0; o < 10; o++) {
    total +=
      (mmorpgSimplex3((x / 300) * frequency, 0, (z / 300) * frequency) * 0.5 +
        0.5) *
      amplitude;
    normalization += amplitude;
    amplitude *= 2 ** -0.5;
    frequency *= 1.6;
  }
  return (total / normalization) ** 7.5 * (800 / 6);
}
/** Match the globally aligned triangles used by the bounded visible patch. */
export function mmorpgGroundHeight(x: number, z: number, cell = 2): number {
  const gx = Math.floor(x / cell) * cell,
    gz = Math.floor(z / cell) * cell,
    fx = (x - gx) / cell,
    fz = (z - gz) / cell;
  const a = mmorpgHeight(gx, gz),
    b = mmorpgHeight(gx + cell, gz),
    c = mmorpgHeight(gx, gz + cell),
    d = mmorpgHeight(gx + cell, gz + cell);
  return fx + fz <= 1
    ? a + (b - a) * fx + (c - a) * fz
    : d + (c - d) * (1 - fx) + (b - d) * (1 - fz);
}

export interface MmorpgNoiseParams {
  seed: 1 | 2;
  octaves: number;
  persistence: number;
  lacunarity: number;
  exponentiation: number;
  scale: number;
  height: number;
}
export function sampleMmorpgNoise(
  params: MmorpgNoiseParams,
  x: number,
  y: number,
  z: number,
): number {
  let amplitude = 1,
    frequency = 1,
    total = 0,
    normalization = 0;
  for (let o = 0; o < params.octaves; o++) {
    total +=
      (mmorpgSimplex3(
        (x / params.scale) * frequency,
        (y / params.scale) * frequency,
        (z / params.scale) * frequency,
        params.seed,
      ) *
        0.5 +
        0.5) *
      amplitude;
    normalization += amplitude;
    amplitude *= 2 ** -params.persistence;
    frequency *= params.lacunarity;
  }
  return (total / normalization) ** params.exponentiation * params.height;
}
