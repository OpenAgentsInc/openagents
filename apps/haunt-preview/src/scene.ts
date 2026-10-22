import { walkable, type SceneAsset } from "./project.ts";
type V3 = [number, number, number];
const data: number[] = [];
const paper: V3 = [0.94, 0.89, 0.72];
const wood: V3 = [0.39, 0.24, 0.19];
const plum: V3 = [0.25, 0.18, 0.25];
const orange: V3 = [0.88, 0.34, 0.1];

function face(points: V3[], normal: V3, color: V3, glow = 0) {
  for (const i of [0, 1, 2, 0, 2, 3])
    data.push(...points[i], ...normal, ...color, glow);
}

function box(
  x: number,
  y: number,
  z: number,
  w: number,
  h: number,
  d: number,
  color: V3,
  glow = 0,
) {
  const a = x - w / 2,
    b = x + w / 2,
    c = y - h / 2,
    e = y + h / 2,
    f = z - d / 2,
    g = z + d / 2;
  face(
    [
      [a, c, g],
      [b, c, g],
      [b, e, g],
      [a, e, g],
    ],
    [0, 0, 1],
    color,
    glow,
  );
  face(
    [
      [b, c, f],
      [a, c, f],
      [a, e, f],
      [b, e, f],
    ],
    [0, 0, -1],
    color,
    glow,
  );
  face(
    [
      [b, c, g],
      [b, c, f],
      [b, e, f],
      [b, e, g],
    ],
    [1, 0, 0],
    color,
    glow,
  );
  face(
    [
      [a, c, f],
      [a, c, g],
      [a, e, g],
      [a, e, f],
    ],
    [-1, 0, 0],
    color,
    glow,
  );
  face(
    [
      [a, e, g],
      [b, e, g],
      [b, e, f],
      [a, e, f],
    ],
    [0, 1, 0],
    color,
    glow,
  );
  face(
    [
      [a, c, f],
      [b, c, f],
      [b, c, g],
      [a, c, g],
    ],
    [0, -1, 0],
    color,
    glow,
  );
}

function pumpkin(x: number, z: number, scale = 1) {
  for (let i = 0; i < 10; i++) {
    const angle = (i * Math.PI) / 5;
    box(
      x + Math.sin(angle) * 0.22 * scale,
      0.34 * scale,
      z + Math.cos(angle) * 0.22 * scale,
      0.25 * scale,
      0.48 * scale,
      0.25 * scale,
      i % 2 ? orange : [0.74, 0.25, 0.08],
    );
  }
  box(
    x,
    0.66 * scale,
    z,
    0.1 * scale,
    0.18 * scale,
    0.12 * scale,
    [0.18, 0.25, 0.12],
  );
  for (const side of [-1, 1])
    box(
      x + side * 0.12 * scale,
      0.4 * scale,
      z + 0.35 * scale,
      0.07 * scale,
      0.1 * scale,
      0.03 * scale,
      [1, 0.67, 0.2],
      1,
    );
  box(
    x,
    0.24 * scale,
    z + 0.36 * scale,
    0.22 * scale,
    0.05 * scale,
    0.03 * scale,
    [1, 0.61, 0.12],
    1,
  );
}

function candle(x: number, y: number, z: number) {
  box(x, y, z, 0.12, 0.32, 0.12, paper);
  box(x, y + 0.23, z, 0.07, 0.13, 0.07, [1, 0.64, 0.18], 1);
}

/** Original procedural props share the scene's vertex buffer and material. */
function prop(asset: SceneAsset) {
  const { x, z, scale: s } = asset;
  const b = (
    dx: number,
    y: number,
    dz: number,
    w: number,
    h: number,
    d: number,
    color: V3,
    glow = 0,
  ) => box(x + dx * s, y * s, z + dz * s, w * s, h * s, d * s, color, glow);
  const dark: V3 = [0.055, 0.065, 0.085];
  switch (asset.kind) {
    case "pumpkin":
      pumpkin(x, z, s);
      break;
    case "ghost":
      b(0, 0.95, 0, 0.6, 0.65, 0.46, [0.8, 0.92, 0.94]);
      b(0, 1.3, 0, 0.4, 0.18, 0.36, [0.9, 0.98, 1]);
      for (const dx of [-0.22, 0, 0.22])
        b(
          dx,
          0.56 + (dx === 0 ? 0.05 : 0),
          0,
          0.16,
          0.22,
          0.45,
          [0.8, 0.92, 0.94],
        );
      for (const dx of [-0.14, 0.14]) b(dx, 1.1, 0.24, 0.1, 0.14, 0.025, dark);
      b(0, 0.87, 0.24, 0.12, 0.15, 0.025, dark);
      break;
    case "gravestone":
      b(0, 0.08, 0, 0.8, 0.16, 0.5, [0.3, 0.33, 0.37]);
      b(0, 0.6, 0, 0.64, 0.95, 0.24, [0.5, 0.54, 0.58]);
      b(0, 1.12, 0, 0.46, 0.15, 0.24, [0.5, 0.54, 0.58]);
      b(0, 0.78, 0.13, 0.08, 0.38, 0.025, dark);
      b(0, 0.84, 0.13, 0.3, 0.07, 0.025, dark);
      for (const y of [0.4, 0.3]) b(0, y, 0.13, 0.35, 0.035, 0.025, dark);
      break;
    case "coffin":
      b(0, 0.72, 0, 0.62, 1.4, 0.32, [0.24, 0.11, 0.08]);
      b(0, 0.74, 0.18, 0.48, 1.22, 0.06, wood);
      b(0, 0.83, 0.22, 0.055, 0.6, 0.025, [0.8, 0.64, 0.3]);
      b(0, 0.95, 0.22, 0.29, 0.055, 0.025, [0.8, 0.64, 0.3]);
      for (const dx of [-0.34, 0.34])
        for (const y of [0.4, 1])
          b(dx, y, 0, 0.08, 0.12, 0.15, [0.5, 0.48, 0.4]);
      break;
    case "candle":
      b(0, 0.04, 0, 0.5, 0.08, 0.5, [0.36, 0.25, 0.1]);
      b(0, 0.38, 0, 0.24, 0.62, 0.24, paper);
      b(0, 0.73, 0, 0.035, 0.1, 0.035, dark);
      b(0, 0.86, 0, 0.13, 0.2, 0.13, [1, 0.5, 0.08], 1);
      b(0, 0.9, 0.02, 0.065, 0.13, 0.07, [1, 0.9, 0.4], 1);
      break;
    case "crate":
      b(0, 0.38, 0, 0.7, 0.72, 0.7, [0.34, 0.19, 0.1]);
      for (const dz of [-0.36, 0.36]) {
        for (const dx of [-0.27, 0.27])
          b(dx, 0.38, dz, 0.08, 0.74, 0.045, [0.59, 0.37, 0.19]);
        for (const y of [0.1, 0.37, 0.66])
          b(0, y, dz, 0.72, 0.07, 0.045, [0.59, 0.37, 0.19]);
      }
      b(0, 0.76, 0, 0.72, 0.05, 0.12, wood);
      break;
    case "cauldron":
      for (const dx of [-0.22, 0.22])
        for (const dz of [-0.22, 0.22]) b(dx, 0.12, dz, 0.1, 0.24, 0.1, dark);
      b(0, 0.4, 0, 0.65, 0.5, 0.65, dark);
      for (const side of [-1, 1]) {
        b(side * 0.35, 0.68, 0, 0.08, 0.12, 0.75, [0.14, 0.17, 0.18]);
        b(0, 0.68, side * 0.35, 0.75, 0.12, 0.08, [0.14, 0.17, 0.18]);
        b(side * 0.43, 0.47, 0, 0.12, 0.22, 0.18, dark);
      }
      b(0, 0.66, 0, 0.59, 0.025, 0.59, [0.2, 0.95, 0.29], 1);
      for (const dx of [-0.15, 0.12])
        b(dx, 0.75, dx, 0.09, 0.1, 0.09, [0.4, 1, 0.35], 1);
      break;
  }
}

export type AssetRange = { start: number; end: number };
export function houseGeometry(
  assets: SceneAsset[],
  ranges?: Map<string, AssetRange>,
): Float32Array {
  data.length = 0;
  // Original simple geometry and the Haunted House palette, not imported game art.
  for (const [cx, cz, w, d] of [
    [0, 1.5, 7, 13],
    [0, -7, 2.5, 4],
    [0, -13, 8, 8],
  ]) {
    for (let x = cx - w / 2; x < cx + w / 2; x += 0.7) {
      for (let z = cz - d / 2; z < cz + d / 2; z += 1.3)
        box(
          x + 0.34,
          -0.1,
          z + 0.63,
          0.66,
          0.18,
          1.24,
          (Math.round(x * 10) + Math.round(z * 10)) % 3
            ? [0.29, 0.23, 0.22]
            : [0.35, 0.27, 0.24],
        );
    }
    for (const side of [-1, 1]) {
      box(cx + (side * w) / 2, 1.8, cz, 0.18, 3.6, d, [0.35, 0.34, 0.28]);
      box(cx + side * (w / 2 - 0.1), 0.5, cz, 0.1, 1.0, d, wood);
      box(
        cx + side * (w / 2 - 0.13),
        1.02,
        cz,
        0.16,
        0.09,
        d,
        [0.53, 0.37, 0.22],
      );
      box(cx + side * (w / 2 - 0.12), 3.35, cz, 0.16, 0.18, d, wood);
      for (let z = cz - d / 2; z < cz + d / 2; z += 0.46)
        box(
          cx + side * (w / 2 - 0.1),
          2.13,
          z,
          0.055,
          2.1,
          0.055,
          [0.47, 0.43, 0.32],
        );
    }
    box(cx, 3.65, cz, w, 0.15, d, [0.17, 0.18, 0.2]);
    // A deep red runner guides the walk between rooms.
    box(
      cx,
      0.01,
      cz,
      Math.min(1.8, w - 0.3),
      0.025,
      d - 0.1,
      [0.36, 0.12, 0.13],
    );
    for (const side of [-1, 1])
      box(cx + side * 0.8, 0.03, cz, 0.045, 0.02, d - 0.1, [0.67, 0.44, 0.21]);
  }
  for (const z of [-5, -9]) {
    for (const side of [-1, 1]) {
      box(side * 2.32, 1.75, z, 2.2, 3.5, 0.22, [0.35, 0.32, 0.27]);
      box(side * 1.23, 1.5, z, 0.22, 3.0, 0.4, wood);
    }
    box(0, 3.1, z, 2.7, 0.4, 0.4, wood);
    box(0, 3.0, z + 0.23, 0.45, 0.22, 0.08, [0.75, 0.42, 0.15]);
  }
  box(0, 1.75, 8, 7, 3.5, 0.18, plum);
  box(0, 1.75, -17, 8, 3.5, 0.18, plum);
  // Portraits, wainscoting, candles, and paper ghosts.
  for (const [x, z] of [
    [-3.3, 2],
    [3.3, -1],
    [-3.8, -12],
    [3.8, -15],
  ]) {
    box(x, 1.9, z, 0.16, 1.45, 1.05, [0.62, 0.4, 0.18]);
    box(x - Math.sign(x) * 0.1, 1.9, z, 0.07, 1.22, 0.83, [0.13, 0.16, 0.17]);
    box(x - Math.sign(x) * 0.15, 1.9, z, 0.04, 0.72, 0.5, paper);
    candle(x - Math.sign(x) * 0.45, 1.18, z + 0.9);
  }
  ranges?.clear();
  for (const asset of assets) {
    const start = data.length;
    prop(asset);
    ranges?.set(asset.id, { start, end: data.length });
  }
  box(-2.2, 0.65, -15.5, 1.5, 0.15, 0.75, wood);
  for (const x of [-2.8, -1.6]) box(x, 0.3, -15.5, 0.15, 0.7, 0.6, wood);
  candle(-2.6, 0.9, -15.5);
  candle(-1.9, 0.9, -15.5);
  box(0, 1.9, -16.4, 1.0, 1.1, 0.08, paper);
  for (const side of [-1, 1])
    box(side * 0.19, 2.05, -16.32, 0.13, 0.18, 0.04, [0.08, 0.08, 0.1]);
  for (let i = 0; i < 5; i++)
    box(-0.4 + i * 0.2, 1.3 + (i % 2) * 0.1, -16.4, 0.2, 0.3, 0.08, paper);
  return new Float32Array(data);
}

export type Pose = { x: number; z: number; yaw: number; pitch: number };
export const canWalk = walkable;

export function move(pose: Pose, forward: number, right: number, dt: number) {
  const length = Math.max(1, Math.hypot(forward, right));
  const step = (Math.min(dt, 0.05) * 2.6) / length;
  const dx = (Math.sin(pose.yaw) * forward + Math.cos(pose.yaw) * right) * step;
  const dz =
    (-Math.cos(pose.yaw) * forward + Math.sin(pose.yaw) * right) * step;
  if (canWalk(pose.x + dx, pose.z)) pose.x += dx;
  if (canWalk(pose.x, pose.z + dz)) pose.z += dz;
}

export function cameraMatrix(out: Float32Array, pose: Pose, aspect: number) {
  const s = Math.sin(pose.yaw),
    c = Math.cos(pose.yaw),
    p = Math.sin(pose.pitch),
    q = Math.cos(pose.pitch);
  const right = [c, 0, s],
    up = [-s * p, q, c * p],
    forward = [s * q, p, -c * q];
  const eye = [pose.x, 1.55, pose.z];
  const dot = (a: number[], b: number[]) =>
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const view = [
    right[0],
    up[0],
    -forward[0],
    0,
    right[1],
    up[1],
    -forward[1],
    0,
    right[2],
    up[2],
    -forward[2],
    0,
    -dot(right, eye),
    -dot(up, eye),
    dot(forward, eye),
    1,
  ];
  const f = 1 / Math.tan(Math.PI / 5),
    near = 0.08,
    far = 65;
  const proj = [
    f / aspect,
    0,
    0,
    0,
    0,
    f,
    0,
    0,
    0,
    0,
    far / (near - far),
    -1,
    0,
    0,
    (near * far) / (near - far),
    0,
  ];
  for (let col = 0; col < 4; col++)
    for (let row = 0; row < 4; row++) {
      out[col * 4 + row] = 0;
      for (let k = 0; k < 4; k++)
        out[col * 4 + row] += proj[k * 4 + row] * view[col * 4 + k];
    }
}
