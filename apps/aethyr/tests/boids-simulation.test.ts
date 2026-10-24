import assert from "node:assert/strict";
import test from "node:test";
import {
  BOID_DEFAULTS,
  BOID_STRIDE,
  CELL_QUANTIZATION,
  CELL_STRIDE,
  BoidSimulation,
  type BoidConfig,
  validateBoidConfig,
} from "../packages/boids/simulation.ts";

function normalize(vector: number[]): number[] {
  const squared = vector.reduce((sum, value) => sum + value * value, 0);
  return squared > 1e-12
    ? vector.map((value) => value / Math.sqrt(squared))
    : [0, 0, 0];
}

/** Slow specification: independently gather every member of each boid's cell. */
function referenceStep(
  state: Float32Array,
  config: BoidConfig,
  dt: number,
  target: number[],
  obstacle: number[],
): Float32Array {
  const output = new Float32Array(state.length);
  const dimension = Math.ceil((config.radius * 2) / config.cellSize);
  const cell = (i: number) =>
    [0, 1, 2]
      .map((axis) =>
        Math.max(
          0,
          Math.min(
            dimension - 1,
            Math.floor(
              Math.fround(
                Math.fround(state[i + axis] + Math.fround(config.radius)) /
                  Math.fround(config.cellSize),
              ),
            ),
          ),
        ),
      )
      .join(",");
  for (let i = 0; i < state.length; i += BOID_STRIDE) {
    const own = cell(i);
    const members: number[] = [];
    for (let j = 0; j < state.length; j += BOID_STRIDE)
      if (cell(j) === own) members.push(j);
    const quantizedMean = (axis: number) =>
      members.reduce(
        (sum, member) => sum + Math.trunc(state[member + axis] * 128),
        0,
      ) /
      (members.length * 128);
    const p = Array.from(state.slice(i, i + 3));
    const forward = Array.from(
      state.slice(i + 4, i + 7),
      (value) => value / config.speed,
    );
    const separation =
      members.length > 1
        ? normalize(p.map((value, axis) => value - quantizedMean(axis)))
        : [0, 0, 0];
    const toward = normalize(p.map((value, axis) => target[axis] - value));
    const distance = Math.hypot(...p);
    const boundary =
      (Math.max(0, (distance / config.radius - 0.78) / 0.22) * 3) /
      config.radius;
    let away = p.map((value, axis) => value - obstacle[axis]);
    const obstacleDistance = Math.hypot(...away);
    let avoid = [0, 0, 0];
    if (obstacleDistance < config.obstacleRadius) {
      if (obstacleDistance ** 2 <= 1e-12) away = [1, 0, 0];
      avoid = normalize(away).map(
        (value) =>
          value *
          config.obstacleWeight *
          (1 - obstacleDistance / config.obstacleRadius),
      );
    }
    const heading = forward.map(
      (value, axis) =>
        value +
        dt *
          config.turnRate *
          (config.alignment * (quantizedMean(axis + 4) / config.speed - value) +
            config.separation * separation[axis] +
            config.targetWeight * toward[axis] -
            p[axis] * boundary +
            avoid[axis]),
    );
    let direction = normalize(heading);
    if (direction.every((value) => value === 0)) direction = normalize(forward);
    if (direction.every((value) => value === 0)) direction = [1, 0, 0];
    const velocity = direction.map((value) => value * config.speed);
    let position = p.map((value, axis) => value + velocity[axis] * dt);
    if (
      position.reduce((sum, value) => sum + value * value, 0) >
      config.radius ** 2
    ) {
      const normal = normalize(position);
      position = normal.map((value) => value * config.radius * 0.999);
      const outward =
        Math.max(
          0,
          velocity.reduce((sum, value, axis) => sum + value * normal[axis], 0),
        ) * 2;
      for (let axis = 0; axis < 3; axis++)
        velocity[axis] -= outward * normal[axis];
    }
    output.set(position, i);
    output.set(velocity, i + 4);
  }
  return output;
}

function nearArrays(
  actual: Float32Array,
  expected: Float32Array,
  tolerance = 1e-5,
): void {
  assert.equal(actual.length, expected.length);
  for (let i = 0; i < actual.length; i++)
    assert(
      Math.abs(actual[i] - expected[i]) <= tolerance,
      `component ${i}: ${actual[i]} instead of ${expected[i]}`,
    );
}

test("seed and reset reproduce both state buffers without replacing them", () => {
  const simulation = new BoidSimulation({ count: 97, seed: 0 });
  const first = simulation.current;
  const second = simulation.previous;
  const initial = first.slice();
  simulation.step(1 / 60, 8, 2, -4, 1, 3, 4);
  assert.strictEqual(simulation.current, second);
  assert.strictEqual(simulation.previous, first);
  assert.notDeepEqual(simulation.current, initial);
  simulation.reset();
  assert.strictEqual(simulation.current, first);
  assert.strictEqual(simulation.previous, second);
  assert.deepEqual(simulation.current, initial);
  assert.deepEqual(simulation.previous, initial);
  simulation.reset(1);
  assert.notDeepEqual(simulation.current, initial);
  assert.deepEqual(
    simulation.current,
    new BoidSimulation({ count: 97, seed: 1 }).current,
  );
  simulation.reset();
  assert.deepEqual(
    simulation.current,
    new BoidSimulation({ count: 97, seed: 1 }).current,
  );
});

test("cell steering matches an independent gather reference across steps and boundaries", () => {
  const simulation = new BoidSimulation({
    count: 31,
    seed: 47,
    radius: 16,
    cellSize: 4,
    speed: 6,
  });
  simulation.current.set([15.99, 0, 0, 0, 6, 0, 0, 0], 0);
  simulation.current.set([0, 0, 0, 0, 0, 0, 0, 0], 8);
  simulation.current.set([-4, 0, 0, 0, 0, 0, 6, 0], 16);
  simulation.current.set([-4.00001, 0, 0, 0, 0, 6, 0, 0], 24);
  let reference: Float32Array = simulation.current.slice();
  for (let step = 0; step < 8; step++) {
    const dt = step % 2 ? 0.1 : 1 / 60;
    reference = referenceStep(
      reference,
      simulation.config,
      dt,
      [8, 2, -7],
      [0, 0, 0],
    );
    simulation.step(dt, 8, 2, -7);
    nearArrays(simulation.current, reference, 2e-5);
  }
});

test("integer reduction has the documented layout, counts every boid, and truncates toward zero", () => {
  const simulation = new BoidSimulation({
    count: 3,
    radius: 4,
    cellSize: 8,
    speed: 2,
    obstacleRadius: 2,
  });
  simulation.current.set([
    0.125, -0.129, 0.5, 0, 2, 0, 0, 0, -0.125, 0.129, -0.5, 0, 0, 2, 0, 0,
    0.002, -0.002, 0, 0, 0, 0, -2, 0,
  ]);
  simulation.step(1 / 60);
  assert.equal(CELL_QUANTIZATION, 128);
  assert.equal(CELL_STRIDE, 8);
  assert.deepEqual(
    Array.from(simulation.cellAggregates),
    [3, 0, 0, 0, 256, 256, -256, 0],
  );
  assert.deepEqual(Array.from(simulation.cellIndices), [0, 0, 0]);
});

test("a solitary boid does not repel its own quantization remainder", () => {
  const simulation = new BoidSimulation({
    count: 1,
    alignment: 0,
    separation: 32,
    targetWeight: 0,
    obstacleWeight: 0,
  });
  simulation.current.set([0.123, 0.456, 0.789, 0, 5, 0, 0, 0]);
  simulation.step(0.1);
  assert.equal(simulation.current[4], 5);
  assert.equal(simulation.current[5], 0);
  assert.equal(simulation.current[6], 0);
});

test("cell assignment rounds at f32 cell planes like the GPU", () => {
  const simulation = new BoidSimulation({ count: 3 });
  simulation.current.set([
    -4.0000005, 0, 0, 0, 5, 0, 0, 0, -4.000003, 0, 0, 0, 5, 0, 0, 0, 4.0000005,
    0, 0, 0, 5, 0, 0, 0,
  ]);
  // The first position plus 32 rounds to 28 in f32, although its double sum
  // lies just below 28. The second remains on the other side of that plane.
  assert.equal(Math.floor((simulation.current[0] + 32) / 4), 6);
  simulation.step(1 / 60);
  assert.deepEqual(Array.from(simulation.cellIndices), [2183, 2182, 2185]);
  for (const cell of [2183, 2182, 2185])
    assert.equal(simulation.cellAggregates[cell * CELL_STRIDE], 1);
});

test("target, obstacle, and spherical boundaries change the actual trajectory", () => {
  const a = new BoidSimulation({
    count: 1,
    alignment: 0,
    separation: 0,
    obstacleWeight: 0,
    targetWeight: 4,
  });
  a.current.set([0, 0, 0, 0, 5, 0, 0, 0]);
  a.step(0.1, 0, 10, 0);
  assert(a.current[5] > 0);
  const b = new BoidSimulation({
    count: 1,
    alignment: 0,
    separation: 0,
    targetWeight: 0,
    obstacleWeight: 8,
  });
  b.current.set([0, 1, 0, 0, 5, 0, 0, 0]);
  b.step(0.1);
  assert(b.current[5] > 0);
  b.current.set([31.999, 0, 0, 0, 5, 0, 0, 0]);
  b.step(0.1);
  assert(b.current[4] < 0);
  assert(
    Math.hypot(b.current[0], b.current[1], b.current[2]) < b.config.radius,
  );
});

test("invalid setup and step inputs fail before state or cell mutation", () => {
  for (const invalid of [
    { count: 0 },
    { count: 200001 },
    { count: 1.1 },
    { seed: -1 },
    { seed: 2 ** 32 },
    { radius: 65 },
    { radius: 0 },
    { speed: 0 },
    { speed: 17 },
    { cellSize: 0 },
    { cellSize: 0.01 },
    { turnRate: NaN },
    { alignment: -1 },
    { separation: 33 },
    { obstacleRadius: 0 },
    { obstacleRadius: 33 },
    { targetWeight: Infinity },
  ])
    assert.throws(() => new BoidSimulation(invalid), RangeError);
  const simulation = new BoidSimulation({ count: 8 });
  const before = simulation.current.slice();
  const cells = simulation.cellAggregates.slice();
  for (const dt of [0, -1, NaN, Infinity, 0.100001])
    assert.throws(() => simulation.step(dt), RangeError);
  assert.throws(() => simulation.step(1 / 60, NaN), RangeError);
  assert.throws(() => simulation.step(1 / 60, 0, 0, 0, Infinity), RangeError);
  assert.throws(() => simulation.step(1 / 60, 1025), RangeError);
  assert.throws(() => simulation.reset(-1), RangeError);
  assert.deepEqual(simulation.current, before);
  assert.deepEqual(simulation.cellAggregates, cells);
  assert(Object.isFrozen(BOID_DEFAULTS));
  assert(Object.isFrozen(validateBoidConfig()));
});

test("100000 boids all update with finite positions, bounded speed, and bounded integer sums", () => {
  const simulation = new BoidSimulation();
  const initial = simulation.current.slice();
  const firstBuffer = simulation.current.buffer;
  const secondBuffer = simulation.previous.buffer;
  for (let step = 0; step < 12; step++)
    simulation.step(1 / 60, 12, 5, -8, -4, 1, 3);
  let moved = 0;
  for (let i = 0; i < simulation.current.length; i += BOID_STRIDE) {
    const state = simulation.current;
    assert(
      Number.isFinite(state[i]) &&
        Number.isFinite(state[i + 1]) &&
        Number.isFinite(state[i + 2]),
    );
    assert(
      Math.hypot(state[i], state[i + 1], state[i + 2]) <=
        simulation.config.radius + 1e-5,
    );
    assert(
      Math.abs(
        Math.hypot(state[i + 4], state[i + 5], state[i + 6]) -
          simulation.config.speed,
      ) < 1e-5,
    );
    if (
      state[i] !== initial[i] ||
      state[i + 1] !== initial[i + 1] ||
      state[i + 2] !== initial[i + 2]
    )
      moved++;
    assert.equal(state[i + 3], 0);
    assert.equal(state[i + 7], 0);
  }
  assert.equal(moved, 100000);
  let counted = 0;
  for (let i = 0; i < simulation.cellAggregates.length; i += CELL_STRIDE)
    counted += simulation.cellAggregates[i];
  assert.equal(counted, 100000);
  assert.strictEqual(simulation.current.buffer, firstBuffer);
  assert.strictEqual(simulation.previous.buffer, secondBuffer);
});

test("the maximum population in one cell does not overflow signed integer sums", () => {
  const simulation = new BoidSimulation({
    count: 200_000,
    radius: 64,
    cellSize: 128,
    speed: 16,
  });
  for (let i = 0; i < simulation.current.length; i += BOID_STRIDE) {
    simulation.current.set([-64, 0, 0, 0, -16, 0, 0, 0], i);
  }
  simulation.step(1 / 60);
  assert.deepEqual(
    Array.from(simulation.cellAggregates),
    [200_000, -1_638_400_000, 0, 0, -409_600_000, 0, 0, 0],
  );
  assert(simulation.current[4] > 0);
});

test("float32-friendly scale thresholds accept the minimum and reject smaller values", () => {
  const minimum = 1 / 128;
  for (const key of [
    "radius",
    "speed",
    "cellSize",
    "obstacleRadius",
  ] as const) {
    assert.throws(() => validateBoidConfig({ [key]: minimum / 2 }), RangeError);
  }
  const simulation = new BoidSimulation({
    count: 5,
    radius: minimum,
    cellSize: minimum,
    speed: minimum,
    obstacleRadius: minimum,
  });
  simulation.step(0.1);
  for (const value of simulation.current) assert(Number.isFinite(value));
  for (let i = 0; i < simulation.current.length; i += BOID_STRIDE) {
    assert(Math.hypot(...simulation.current.slice(i, i + 3)) <= minimum);
    assert(
      Math.abs(
        Math.hypot(...simulation.current.slice(i + 4, i + 7)) - minimum,
      ) < 1e-8,
    );
  }
});
