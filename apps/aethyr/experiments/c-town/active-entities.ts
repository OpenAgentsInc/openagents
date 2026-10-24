import type { InstanceBounds } from "../../packages/renderer/culling.ts";

export const ACTIVE_ALGORITHM = "active-euler-bounce-f32-v1" as const;
export const ACTIVE_COUNTS = [1000, 10000, 100000] as const;
export const ACTIVE_SEED = 12999;
export const ACTIVE_DT = Math.fround(1 / 60);
export const ACTIVE_WORLD = {
  min: [-160, -60, -400],
  max: [160, 60, 20],
} as const;
const f = Math.fround;
const GRAVITY = f(-0.5),
  RESTITUTION = f(0.8);

export interface ActiveEntities {
  readonly algorithm: typeof ACTIVE_ALGORITHM;
  readonly count: number;
  readonly seed: number;
  readonly dt: number;
  tick: number;
  boundsTick: number;
  readonly positions: Float32Array<ArrayBuffer>;
  readonly velocities: Float32Array<ArrayBuffer>;
  readonly acceleration: Float32Array<ArrayBuffer>;
  readonly halfExtents: Float32Array<ArrayBuffer>;
  readonly bounds: InstanceBounds;
}
export interface ActiveReferenceEntity {
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  ax: number;
  az: number;
  hx: number;
  hy: number;
  hz: number;
}
export interface ActiveReference {
  count: number;
  seed: number;
  dt: number;
  tick: number;
  entities: ActiveReferenceEntity[];
}
export interface ActiveVerification {
  ok: boolean;
  mismatches: number;
  movingEntities: number;
  firstMismatch?: {
    entity: number;
    field: string;
    actual: number;
    expected: number;
  };
}
function config(count: number, seed: number, dt: number) {
  if (
    !Number.isSafeInteger(count) ||
    count < 1 ||
    count > 100_000 ||
    !Number.isSafeInteger(seed) ||
    seed < 0 ||
    seed > 0xffffffff ||
    !Number.isFinite(dt) ||
    dt <= 0 ||
    dt > 0.1 ||
    f(dt) !== dt
  )
    throw new Error(
      "Choose a bounded active population, uint32 seed, and Float32 timestep.",
    );
}
function advance(state: ActiveEntities) {
  if (
    !Number.isSafeInteger(state.tick) ||
    state.tick < 0 ||
    state.tick >= 1_000_000_000
  )
    throw new Error("Choose a bounded active simulation tick.");
}

/** Seeded resident storage. No renderer, browser worker, or game authority is created. */
export function createActiveEntities(
  count: number,
  seed = ACTIVE_SEED,
  dt = ACTIVE_DT,
): ActiveEntities {
  config(count, seed, dt);
  let randomState = seed >>> 0;
  const random = () =>
    (randomState = (Math.imul(randomState, 1664525) + 1013904223) >>> 0) /
    4294967296;
  const positions = new Float32Array(count * 3),
    velocities = new Float32Array(count * 3),
    acceleration = new Float32Array(count * 2),
    halfExtents = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    const j = i * 3;
    for (let axis = 0; axis < 3; axis++)
      halfExtents[j + axis] = f(0.1 + random() * 7);
    for (let axis = 0; axis < 3; axis++) {
      const half = halfExtents[j + axis],
        lo = f(ACTIVE_WORLD.min[axis] + half),
        hi = f(ACTIVE_WORLD.max[axis] - half);
      positions[j + axis] = f(lo + random() * (hi - lo));
    }
    for (let axis = 0; axis < 3; axis++)
      velocities[j + axis] = f((random() - 0.5) * 8);
    acceleration[i * 2] = f((random() - 0.5) * 3);
    acceleration[i * 2 + 1] = f((random() - 0.5) * 3);
  }
  const state: ActiveEntities = {
    algorithm: ACTIVE_ALGORITHM,
    count,
    seed,
    dt,
    tick: 0,
    boundsTick: -1,
    positions,
    velocities,
    acceleration,
    halfExtents,
    bounds: {
      count,
      spheres: new Float64Array(count * 4),
      aabbs: new Float64Array(count * 6),
    },
  };
  refreshActiveBounds(state);
  return state;
}

/** Semi-implicit Euler, Float32 rounding at every arithmetic operation, then enclosure bounce. */
export function stepActiveEntities(state: ActiveEntities) {
  advance(state);
  const {
    positions: p,
    velocities: v,
    acceleration: a,
    halfExtents: h,
    dt,
    count,
  } = state;
  const steering = (state.tick & 32) === 0 ? 1 : -1;
  const gravity = f(GRAVITY * dt);
  for (let i = 0; i < count; i++) {
    const j = i * 3,
      k = i * 2;
    let vx = f(v[j] + f(f(a[k] * steering) * dt)),
      vy = f(v[j + 1] + gravity),
      vz = f(v[j + 2] + f(f(a[k + 1] * steering) * dt));
    let x = f(p[j] + f(vx * dt)),
      y = f(p[j + 1] + f(vy * dt)),
      z = f(p[j + 2] + f(vz * dt));
    const lx = f(-160 + h[j]),
      hx = f(160 - h[j]),
      ly = f(-60 + h[j + 1]),
      hy = f(60 - h[j + 1]),
      lz = f(-400 + h[j + 2]),
      hz = f(20 - h[j + 2]);
    if (x < lx) {
      x = lx;
      vx = f(Math.abs(vx) * RESTITUTION);
    } else if (x > hx) {
      x = hx;
      vx = f(-Math.abs(vx) * RESTITUTION);
    }
    if (y < ly) {
      y = ly;
      vy = f(Math.abs(vy) * RESTITUTION);
    } else if (y > hy) {
      y = hy;
      vy = f(-Math.abs(vy) * RESTITUTION);
    }
    if (z < lz) {
      z = lz;
      vz = f(Math.abs(vz) * RESTITUTION);
    } else if (z > hz) {
      z = hz;
      vz = f(-Math.abs(vz) * RESTITUTION);
    }
    p[j] = x;
    p[j + 1] = y;
    p[j + 2] = z;
    v[j] = vx;
    v[j + 1] = vy;
    v[j + 2] = vz;
  }
  state.tick++;
}

/** Derived bounds are a separately measured phase, suitable for a later culling comparison. */
export function refreshActiveBounds(state: ActiveEntities) {
  const { positions: p, halfExtents: h, bounds } = state;
  for (let i = 0; i < state.count; i++) {
    const j = i * 3,
      sphere = i * 4,
      box = i * 6;
    bounds.spheres[sphere] = p[j];
    bounds.spheres[sphere + 1] = p[j + 1];
    bounds.spheres[sphere + 2] = p[j + 2];
    bounds.spheres[sphere + 3] = Math.hypot(h[j], h[j + 1], h[j + 2]);
    for (let axis = 0; axis < 3; axis++) {
      bounds.aabbs[box + axis] = p[j + axis] - h[j + axis];
      bounds.aabbs[box + axis + 3] = p[j + axis] + h[j + axis];
    }
  }
  state.boundsTick = state.tick;
}

/** Object-based reference with its own initialization and update path, outside measured work. */
export function referenceActiveEntities(options: {
  count: number;
  seed?: number;
  dt?: number;
  steps: number;
}): ActiveReference {
  const { count, steps } = options,
    seed = options.seed ?? ACTIVE_SEED,
    dt = options.dt ?? ACTIVE_DT;
  config(count, seed, dt);
  if (!Number.isSafeInteger(steps) || steps < 0 || steps > 1024)
    throw new Error("Choose up to 1,024 independent reference steps.");
  let randomState = seed;
  const random = () => {
    randomState = (Math.imul(1664525, randomState) + 1013904223) >>> 0;
    return randomState * (1 / 4294967296);
  };
  const entities: ActiveReferenceEntity[] = [];
  for (let i = 0; i < count; i++) {
    const hx = Math.fround(0.1 + random() * 7),
      hy = Math.fround(0.1 + random() * 7),
      hz = Math.fround(0.1 + random() * 7);
    const x = Math.fround(
      Math.fround(-160 + hx) +
        random() * (Math.fround(160 - hx) - Math.fround(-160 + hx)),
    );
    const y = Math.fround(
      Math.fround(-60 + hy) +
        random() * (Math.fround(60 - hy) - Math.fround(-60 + hy)),
    );
    const z = Math.fround(
      Math.fround(-400 + hz) +
        random() * (Math.fround(20 - hz) - Math.fround(-400 + hz)),
    );
    entities.push({
      x,
      y,
      z,
      vx: Math.fround((random() - 0.5) * 8),
      vy: Math.fround((random() - 0.5) * 8),
      vz: Math.fround((random() - 0.5) * 8),
      ax: Math.fround((random() - 0.5) * 3),
      az: Math.fround((random() - 0.5) * 3),
      hx,
      hy,
      hz,
    });
  }
  for (let tick = 0; tick < steps; tick++) {
    const sign = tick & 32 ? -1 : 1;
    for (const entity of entities) {
      for (const axis of ["x", "y", "z"] as const) {
        const velocity = axis === "x" ? "vx" : axis === "y" ? "vy" : "vz";
        const half =
          axis === "x" ? entity.hx : axis === "y" ? entity.hy : entity.hz;
        const low = Math.fround(
          (axis === "x" ? -160 : axis === "y" ? -60 : -400) + half,
        );
        const high = Math.fround(
          (axis === "x" ? 160 : axis === "y" ? 60 : 20) - half,
        );
        const force =
          axis === "y"
            ? -0.5
            : Math.fround((axis === "x" ? entity.ax : entity.az) * sign);
        const nextVelocity = Math.fround(
          entity[velocity] + Math.fround(force * dt),
        );
        const next = Math.fround(entity[axis] + Math.fround(nextVelocity * dt));
        entity[axis] = Math.max(low, Math.min(high, next));
        entity[velocity] =
          next < low
            ? Math.fround(Math.abs(nextVelocity) * Math.fround(0.8))
            : next > high
              ? Math.fround(-Math.abs(nextVelocity) * Math.fround(0.8))
              : nextVelocity;
      }
    }
  }
  return { count, seed, dt, tick: steps, entities };
}

/** Stable little-endian state bytes; copying and hashing belong outside measured intervals. */
export function activeStateBytes(
  state: ActiveEntities | ActiveReference,
): Uint8Array<ArrayBuffer> {
  const bytes = new Uint8Array(24 + state.count * 44),
    view = new DataView(bytes.buffer);
  bytes.set([65, 70, 65, 67, 84, 86, 49, 0]);
  view.setUint32(8, state.count, true);
  view.setUint32(12, state.seed, true);
  view.setUint32(16, state.tick, true);
  view.setFloat32(20, state.dt, true);
  let offset = 24;
  if ("positions" in state) {
    for (const array of [
      state.positions,
      state.velocities,
      state.acceleration,
      state.halfExtents,
    ])
      for (const value of array) {
        view.setFloat32(offset, value, true);
        offset += 4;
      }
  } else {
    for (const names of [
      ["x", "y", "z"],
      ["vx", "vy", "vz"],
      ["ax", "az"],
      ["hx", "hy", "hz"],
    ] as const)
      for (const entity of state.entities)
        for (const name of names) {
          view.setFloat32(offset, entity[name], true);
          offset += 4;
        }
  }
  return bytes;
}

/** Exact independent checks, including freshly derived bounds and finite state. */
export function verifyActiveState(
  state: ActiveEntities,
  reference: ActiveReference,
): ActiveVerification {
  const result: ActiveVerification = {
    ok: true,
    mismatches: 0,
    movingEntities: 0,
  };
  const compare = (
    entity: number,
    field: string,
    actual: number,
    expected: number,
  ) => {
    if (!Number.isFinite(actual) || !Object.is(actual, expected)) {
      result.ok = false;
      result.mismatches++;
      result.firstMismatch ??= { entity, field, actual, expected };
    }
  };
  for (const field of ["count", "seed", "dt", "tick"] as const)
    compare(-1, field, state[field], reference[field]);
  compare(-1, "boundsTick", state.boundsTick, reference.tick);
  if (state.count !== reference.count) return result;
  for (let i = 0; i < state.count; i++) {
    const j = i * 3,
      entity = reference.entities[i];
    for (const [array, names] of [
      [state.positions, ["x", "y", "z"]],
      [state.velocities, ["vx", "vy", "vz"]],
      [state.acceleration, ["ax", "az"]],
      [state.halfExtents, ["hx", "hy", "hz"]],
    ] as const)
      for (let axis = 0; axis < names.length; axis++)
        compare(
          i,
          names[axis],
          array[i * names.length + axis],
          entity[names[axis]],
        );
    if (
      Math.abs(state.velocities[j]) +
        Math.abs(state.velocities[j + 1]) +
        Math.abs(state.velocities[j + 2]) >
      0
    )
      result.movingEntities++;
    for (let axis = 0; axis < 3; axis++) {
      const value = entity[axis === 0 ? "x" : axis === 1 ? "y" : "z"],
        half = entity[axis === 0 ? "hx" : axis === 1 ? "hy" : "hz"];
      compare(i, `sphere.${axis}`, state.bounds.spheres[i * 4 + axis], value);
      compare(i, `min.${axis}`, state.bounds.aabbs[i * 6 + axis], value - half);
      compare(
        i,
        `max.${axis}`,
        state.bounds.aabbs[i * 6 + axis + 3],
        value + half,
      );
    }
    compare(
      i,
      "radius",
      state.bounds.spheres[i * 4 + 3],
      Math.hypot(entity.hx, entity.hy, entity.hz),
    );
  }
  return result;
}

export function activeStorage(state: ActiveEntities) {
  const stateBytes =
    state.positions.byteLength +
    state.velocities.byteLength +
    state.acceleration.byteLength +
    state.halfExtents.byteLength;
  const boundsBytes =
    state.bounds.spheres.byteLength + state.bounds.aabbs.byteLength;
  return {
    stateBytes,
    boundsBytes,
    residentTypedBytes: stateBytes + boundsBytes,
    snapshotBytes: 24 + state.count * 44,
    timedLoopAllocatedTypedBytes: 0,
    referenceHeapBytes: null,
    browserHeapBytes: null,
  };
}
