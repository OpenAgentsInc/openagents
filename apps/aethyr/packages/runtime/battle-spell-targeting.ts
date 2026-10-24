import type { BattleCrowdTerrain } from "../renderer/battle-crowd.ts";
import {
  add,
  cross,
  dot,
  normalize,
  scale,
  sub,
  type V3,
} from "../renderer/math.ts";
import {
  pointerCameraRay,
  resolveSpellTarget,
  type AimCamera,
  type AimPointer,
  type SpellRay,
  type SpellTarget,
} from "../renderer/spell-targeting.ts";

const RANGE = 80;
const coordinate = (value: number) => {
  const distance = Math.abs(value);
  return (
    128 +
    Math.sign(value) *
      (distance <= 128 ? distance / 2 : 64 + (distance - 128) / 24)
  );
};
const displacement = (index: number) => {
  const offset = index - 128,
    distance = Math.abs(offset);
  return (
    Math.sign(offset) *
    (distance <= 64 ? distance * 2 : 128 + (distance - 64) * 24)
  );
};

function triangle(ray: SpellRay, a: V3, b: V3, c: V3, limit: number) {
  const ab = sub(b, a),
    ac = sub(c, a),
    p = cross(ray.direction, ac),
    determinant = dot(ab, p);
  if (Math.abs(determinant) < 1e-9) return;
  const inverse = 1 / determinant,
    offset = sub(ray.origin, a),
    u = dot(offset, p) * inverse;
  if (u < -1e-7 || u > 1.0000001) return;
  const q = cross(offset, ab),
    v = dot(ray.direction, q) * inverse;
  if (v < -1e-7 || u + v > 1.0000001) return;
  const distance = dot(ac, q) * inverse;
  if (distance < 0 || distance > limit) return;
  let normal = normalize(cross(ab, ac));
  if (normal[1] < 0) normal = scale(normal, -1);
  return {
    point: add(ray.origin, scale(ray.direction, distance)),
    normal,
    distance,
  };
}

/** Traverse the actual nonuniform terrain triangles, including the outer 24 m cells. */
export function battleTerrainRaycast(
  terrain: BattleCrowdTerrain,
  ray: SpellRay,
  limit: number,
): SpellTarget | undefined {
  if (
    terrain.layout !== "mmorpg-patch" ||
    terrain.resolution !== 257 ||
    !Number.isFinite(limit) ||
    limit <= 0 ||
    limit > 90 ||
    ![...ray.origin, ...ray.direction].every(Number.isFinite) ||
    Math.abs(Math.hypot(...ray.direction) - 1) > 1e-5
  )
    return;
  let start = 0,
    end = limit;
  for (const axis of [0, 2] as const) {
    const center = terrain.center[axis === 0 ? 0 : 1],
      delta = ray.direction[axis];
    if (Math.abs(delta) < 1e-10) {
      if (Math.abs(ray.origin[axis] - center) > 1664) return;
    } else {
      const a = (center - 1664 - ray.origin[axis]) / delta;
      const b = (center + 1664 - ray.origin[axis]) / delta;
      start = Math.max(start, Math.min(a, b));
      end = Math.min(end, Math.max(a, b));
    }
  }
  if (start > end || end < 0) return;
  const entry = add(
    ray.origin,
    scale(ray.direction, Math.min(end, start + 1e-7)),
  );
  let x = Math.max(
    0,
    Math.min(255, Math.floor(coordinate(entry[0] - terrain.center[0]))),
  );
  let z = Math.max(
    0,
    Math.min(255, Math.floor(coordinate(entry[2] - terrain.center[1]))),
  );
  const vertex = (i: number, j: number): V3 => [
    terrain.center[0] + displacement(i),
    terrain.heights[j * 257 + i],
    terrain.center[1] + displacement(j),
  ];
  // A <=90 m ray crosses at most 92 minimum-width (2 m) x/z boundaries.
  for (
    let step = 0;
    step < 96 && x >= 0 && x < 256 && z >= 0 && z < 256;
    step++
  ) {
    const a = vertex(x, z),
      b = vertex(x + 1, z),
      c = vertex(x, z + 1),
      d = vertex(x + 1, z + 1);
    const one = triangle(ray, a, b, c, end),
      two = triangle(ray, d, c, b, end);
    const hit =
      one && two ? (one.distance < two.distance ? one : two) : (one ?? two);
    if (hit && hit.distance >= start - 1e-6) return hit;
    const next = (axis: 0 | 2, index: number) =>
      Math.abs(ray.direction[axis]) < 1e-10
        ? Infinity
        : (terrain.center[axis === 0 ? 0 : 1] +
            displacement(index + (ray.direction[axis] > 0 ? 1 : 0)) -
            ray.origin[axis]) /
          ray.direction[axis];
    const tx = next(0, x),
      tz = next(2, z),
      time = Math.min(tx, tz);
    if (!Number.isFinite(time) || time > end + 1e-6) break;
    if (tx <= tz + 1e-7) x += ray.direction[0] > 0 ? 1 : -1;
    if (tz <= tx + 1e-7) z += ray.direction[2] > 0 ? 1 : -1;
    start = Math.max(start, time);
  }
}

/** Reuse Town's camera/hand-ray convention without querying the unrelated Town collision world. */
export function battleMeteorTarget(
  terrain: BattleCrowdTerrain,
  camera: AimCamera,
  aspect: number,
  pointer: AimPointer,
  hand: V3,
  thirdPerson: boolean,
): SpellTarget | undefined {
  return resolveSpellTarget(
    pointerCameraRay(camera, aspect, pointer, thirdPerson),
    hand,
    RANGE,
    (ray, length) => battleTerrainRaycast(terrain, ray, Math.min(90, length)),
  )?.target;
}
