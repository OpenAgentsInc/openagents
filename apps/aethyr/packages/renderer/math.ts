export type V3 = [number, number, number];
export const add = (a: V3, b: V3): V3 => [
  a[0] + b[0],
  a[1] + b[1],
  a[2] + b[2],
];
export const sub = (a: V3, b: V3): V3 => [
  a[0] - b[0],
  a[1] - b[1],
  a[2] - b[2],
];
export const scale = (v: V3, x: number): V3 => [v[0] * x, v[1] * x, v[2] * x];
export const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const cross = (a: V3, b: V3): V3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
export const normalize = (v: V3): V3 =>
  scale(v, 1 / Math.max(Math.hypot(...v), 1e-10));
export function multiply(
  a: ArrayLike<number>,
  b: ArrayLike<number>,
): Float32Array {
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++)
    for (let r = 0; r < 4; r++)
      for (let k = 0; k < 4; k++) out[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return out;
}
export function perspective(
  fov: number,
  aspect: number,
  near = 0.1,
  far = 2000,
): Float32Array {
  const f = 1 / Math.tan(fov / 2);
  return new Float32Array([
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
  ]);
}
export function lookAt(eye: V3, target: V3, up: V3 = [0, 1, 0]): Float32Array {
  const z = normalize(sub(eye, target)),
    x = normalize(cross(up, z)),
    y = cross(z, x);
  return new Float32Array([
    x[0],
    y[0],
    z[0],
    0,
    x[1],
    y[1],
    z[1],
    0,
    x[2],
    y[2],
    z[2],
    0,
    -dot(x, eye),
    -dot(y, eye),
    -dot(z, eye),
    1,
  ]);
}
export function orthographic(
  left: number,
  right: number,
  bottom: number,
  top: number,
  near: number,
  far: number,
): Float32Array {
  return new Float32Array([
    2 / (right - left),
    0,
    0,
    0,
    0,
    2 / (top - bottom),
    0,
    0,
    0,
    0,
    1 / (near - far),
    0,
    -(right + left) / (right - left),
    -(top + bottom) / (top - bottom),
    near / (near - far),
    1,
  ]);
}
export function inverse(m: ArrayLike<number>): Float32Array {
  const a = new Float64Array(32);
  for (let r = 0; r < 4; r++)
    for (let c = 0; c < 8; c++)
      a[r * 8 + c] = c < 4 ? m[c * 4 + r] : Number(c - 4 === r);
  for (let k = 0; k < 4; k++) {
    const row = k * 8;
    let pivot = k;
    for (let j = k + 1; j < 4; j++)
      if (Math.abs(a[j * 8 + k]) > Math.abs(a[pivot * 8 + k])) pivot = j;
    for (let c = 0; c < 8; c++) {
      const value = a[row + c];
      a[row + c] = a[pivot * 8 + c];
      a[pivot * 8 + c] = value;
    }
    const d = a[row + k];
    if (Math.abs(d) < 1e-12) throw new Error("Camera matrix is singular");
    for (let c = 0; c < 8; c++) a[row + c] /= d;
    for (let r = 0; r < 4; r++)
      if (r !== k) {
        const target = r * 8,
          f = a[target + k];
        for (let c = 0; c < 8; c++) a[target + c] -= f * a[row + c];
      }
  }
  const result = new Float32Array(16);
  for (let c = 0; c < 4; c++)
    for (let r = 0; r < 4; r++) result[c * 4 + r] = a[r * 8 + 4 + c];
  return result;
}
export function transformPoint(m: ArrayLike<number>, p: V3): V3 {
  return [
    m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
    m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
    m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
  ];
}
export function modelMatrix(p: V3, yaw = 0, size = 1): number[] {
  const c = Math.cos(yaw) * size,
    s = Math.sin(yaw) * size;
  return [c, 0, -s, 0, 0, size, 0, 0, s, 0, c, 0, ...p, 1];
}
export function direction(yaw: number, pitch: number): V3 {
  return [
    Math.sin(yaw) * Math.cos(pitch),
    Math.sin(pitch),
    -Math.cos(yaw) * Math.cos(pitch),
  ];
}
export function viewAngles(eye: V3, target: V3) {
  const d = normalize(sub(target, eye));
  return { yaw: Math.atan2(d[0], -d[2]), pitch: Math.asin(d[1]) };
}
