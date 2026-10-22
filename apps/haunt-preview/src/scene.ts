import { walkable, type Pumpkin } from "./project.ts";
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

export function houseGeometry(pumpkins: Pumpkin[]): Float32Array {
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
  for (const p of pumpkins) pumpkin(p.x, p.z, p.scale);
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
