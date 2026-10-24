/** Owned cell-aggregate flocking workload. It does not search exact-radius neighbors. */
export const BOID_STRIDE = 8;
/** Cell words: count, position xyz sums, velocity xyz sums, unused. */
export const CELL_STRIDE = 8;
export const CELL_QUANTIZATION = 128;
export const BOID_MIN_SCALE = 1 / CELL_QUANTIZATION;
export const BOID_MAX_COUNT = 200_000;
export const BOID_MAX_CELLS = 262_144;

export interface BoidConfig {
  readonly count: number;
  readonly seed: number;
  readonly radius: number;
  readonly cellSize: number;
  readonly speed: number;
  readonly turnRate: number;
  readonly alignment: number;
  readonly separation: number;
  readonly targetWeight: number;
  readonly obstacleWeight: number;
  readonly obstacleRadius: number;
}

export const BOID_DEFAULTS: Readonly<BoidConfig> = Object.freeze({
  count: 100_000,
  seed: 0x5eaf00d,
  radius: 32,
  cellSize: 4,
  speed: 5,
  turnRate: 2.8,
  alignment: 1,
  separation: 0.7,
  targetWeight: 0.25,
  obstacleWeight: 3,
  obstacleRadius: 7,
});

export function validateBoidConfig(
  input: Partial<BoidConfig> = {},
): Readonly<BoidConfig> {
  const config = { ...BOID_DEFAULTS, ...input };
  for (const [name, value] of Object.entries(config)) {
    if (!Number.isFinite(value))
      throw new RangeError(`${name} must be finite.`);
  }
  if (
    !Number.isInteger(config.count) ||
    config.count < 1 ||
    config.count > BOID_MAX_COUNT
  )
    throw new RangeError(
      `count must be an integer from 1 to ${BOID_MAX_COUNT}.`,
    );
  if (
    !Number.isInteger(config.seed) ||
    config.seed < 0 ||
    config.seed > 0xffffffff
  )
    throw new RangeError("seed must be an unsigned 32-bit integer.");
  if (config.radius < BOID_MIN_SCALE || config.radius > 64)
    throw new RangeError("radius must be from 1/128 to 64.");
  if (config.speed < BOID_MIN_SCALE || config.speed > 16)
    throw new RangeError("speed must be from 1/128 to 16.");
  if (config.cellSize < BOID_MIN_SCALE || config.cellSize > config.radius * 2)
    throw new RangeError("cellSize must be from 1/128 to the world diameter.");
  const dimension = Math.ceil((config.radius * 2) / config.cellSize);
  if (dimension ** 3 > BOID_MAX_CELLS)
    throw new RangeError(
      `The grid must contain at most ${BOID_MAX_CELLS} cells.`,
    );
  for (const name of [
    "turnRate",
    "alignment",
    "separation",
    "targetWeight",
    "obstacleWeight",
  ] as const) {
    if (config[name] < 0 || config[name] > 32)
      throw new RangeError(`${name} must be from 0 to 32.`);
  }
  if (
    config.obstacleRadius < BOID_MIN_SCALE ||
    config.obstacleRadius > config.radius
  )
    throw new RangeError("obstacleRadius must be from 1/128 to radius.");
  return Object.freeze(config);
}

/**
 * Two state buffers contain [position xyz, 0, velocity xyz, 0] per boid.
 * The arrays are exposed for direct GPU upload. Consumers must not change their
 * lengths or write nonfinite/out-of-bounds values into them.
 *
 * The integer grid makes cell reduction order independent. At the maximum
 * radius/count, each position sum is bounded by 64 * 128 * 200000 < 2^31.
 */
export class BoidSimulation {
  readonly config: Readonly<BoidConfig>;
  readonly gridDimension: number;
  readonly cellCount: number;
  readonly cellAggregates: Int32Array;
  readonly cellIndices: Uint32Array;
  private a: Float32Array;
  private b: Float32Array;
  private front = 0;
  private resetSeed: number;

  constructor(input: Partial<BoidConfig> = {}) {
    this.config = validateBoidConfig(input);
    this.gridDimension = Math.ceil(
      (this.config.radius * 2) / this.config.cellSize,
    );
    this.cellCount = this.gridDimension ** 3;
    this.cellAggregates = new Int32Array(this.cellCount * CELL_STRIDE);
    this.cellIndices = new Uint32Array(this.config.count);
    this.a = new Float32Array(this.config.count * BOID_STRIDE);
    this.b = new Float32Array(this.a.length);
    this.resetSeed = this.config.seed;
    this.reset();
  }

  get current(): Float32Array {
    return this.front === 0 ? this.a : this.b;
  }
  get previous(): Float32Array {
    return this.front === 0 ? this.b : this.a;
  }

  reset(seed = this.resetSeed): void {
    if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff)
      throw new RangeError("seed must be an unsigned 32-bit integer.");
    this.resetSeed = seed;
    let randomState = seed >>> 0;
    const random = () => {
      randomState = (randomState + 0x6d2b79f5) | 0;
      let word = Math.imul(randomState ^ (randomState >>> 15), 1 | randomState);
      word ^= word + Math.imul(word ^ (word >>> 7), 61 | word);
      return ((word ^ (word >>> 14)) >>> 0) / 4294967296;
    };
    const state = this.a;
    const schoolRadius = this.config.radius * 0.5625;
    const speed = this.config.speed;
    for (let i = 0; i < state.length; i += BOID_STRIDE) {
      const z = random() * 2 - 1;
      const theta = random() * Math.PI * 2;
      const radial = Math.cbrt(random()) * schoolRadius;
      const ring = Math.sqrt(Math.max(0, 1 - z * z));
      const x = Math.cos(theta) * ring * radial;
      const y = Math.sin(theta) * ring * radial;
      const pz = z * radial;
      // A shared orbit starts a school rather than unrelated noise particles.
      let vx = -pz + (random() - 0.5) * 5;
      let vy = (random() - 0.5) * 3;
      let vz = x + (random() - 0.5) * 5;
      let velocitySquared = vx * vx + vy * vy + vz * vz;
      if (velocitySquared <= 1e-12) {
        vx = 1;
        vy = 0;
        vz = 0;
        velocitySquared = 1;
      }
      const inverse = speed / Math.sqrt(velocitySquared);
      vx *= inverse;
      vy *= inverse;
      vz *= inverse;
      state[i] = x;
      state[i + 1] = y;
      state[i + 2] = pz;
      state[i + 3] = 0;
      state[i + 4] = vx;
      state[i + 5] = vy;
      state[i + 6] = vz;
      state[i + 7] = 0;
    }
    this.b.set(this.a);
    this.front = 0;
    this.cellAggregates.fill(0);
    this.cellIndices.fill(0);
  }

  /** Run one complete cell reduction and steering/integration update. */
  step(
    dt: number,
    targetX = 0,
    targetY = 0,
    targetZ = 0,
    obstacleX = 0,
    obstacleY = 0,
    obstacleZ = 0,
  ): void {
    if (!Number.isFinite(dt) || dt <= 0 || dt > 0.1)
      throw new RangeError(
        "dt must be greater than 0 and at most 0.1 seconds.",
      );
    if (
      !Number.isFinite(targetX) ||
      !Number.isFinite(targetY) ||
      !Number.isFinite(targetZ) ||
      !Number.isFinite(obstacleX) ||
      !Number.isFinite(obstacleY) ||
      !Number.isFinite(obstacleZ) ||
      Math.abs(targetX) > 1024 ||
      Math.abs(targetY) > 1024 ||
      Math.abs(targetZ) > 1024 ||
      Math.abs(obstacleX) > 1024 ||
      Math.abs(obstacleY) > 1024 ||
      Math.abs(obstacleZ) > 1024
    )
      throw new RangeError(
        "Target and obstacle coordinates must be finite and within 1024 units of the origin.",
      );
    const config = this.config;
    const source = this.current;
    const output = this.previous;
    const cells = this.cellAggregates;
    const indices = this.cellIndices;
    const radius = config.radius;
    const dim = this.gridDimension;
    const dim2 = dim * dim;
    // Cell assignment uses the same f32 operations as the WebGPU workload.
    // Steering below uses JavaScript numbers and is compared with a tolerance.
    const gridRadius = Math.fround(radius);
    const gridCellSize = Math.fround(config.cellSize);
    cells.fill(0);
    for (let boid = 0, i = 0; i < source.length; boid++, i += BOID_STRIDE) {
      const cx = Math.min(
        dim - 1,
        Math.max(
          0,
          Math.floor(
            Math.fround(Math.fround(source[i] + gridRadius) / gridCellSize),
          ),
        ),
      );
      const cy = Math.min(
        dim - 1,
        Math.max(
          0,
          Math.floor(
            Math.fround(Math.fround(source[i + 1] + gridRadius) / gridCellSize),
          ),
        ),
      );
      const cz = Math.min(
        dim - 1,
        Math.max(
          0,
          Math.floor(
            Math.fround(Math.fround(source[i + 2] + gridRadius) / gridCellSize),
          ),
        ),
      );
      const cell = cx + cy * dim + cz * dim2;
      indices[boid] = cell;
      const c = cell * CELL_STRIDE;
      cells[c]++;
      cells[c + 1] += Math.trunc(source[i] * CELL_QUANTIZATION);
      cells[c + 2] += Math.trunc(source[i + 1] * CELL_QUANTIZATION);
      cells[c + 3] += Math.trunc(source[i + 2] * CELL_QUANTIZATION);
      cells[c + 4] += Math.trunc(source[i + 4] * CELL_QUANTIZATION);
      cells[c + 5] += Math.trunc(source[i + 5] * CELL_QUANTIZATION);
      cells[c + 6] += Math.trunc(source[i + 6] * CELL_QUANTIZATION);
    }
    const speed = config.speed;
    const inverseSpeed = 1 / speed;
    const turn = dt * config.turnRate;
    const radiusSquared = radius * radius;
    const obstacleRadiusSquared = config.obstacleRadius * config.obstacleRadius;
    for (let boid = 0, i = 0; i < source.length; boid++, i += BOID_STRIDE) {
      const x = source[i],
        y = source[i + 1],
        z = source[i + 2];
      const fx = source[i + 4] * inverseSpeed,
        fy = source[i + 5] * inverseSpeed,
        fz = source[i + 6] * inverseSpeed;
      const c = indices[boid] * CELL_STRIDE;
      const average = 1 / (cells[c] * CELL_QUANTIZATION);
      let sx = x - cells[c + 1] * average;
      let sy = y - cells[c + 2] * average;
      let sz = z - cells[c + 3] * average;
      const separationLengthSquared = sx * sx + sy * sy + sz * sz;
      const inverseSeparation =
        cells[c] > 1 && separationLengthSquared > 1e-12
          ? 1 / Math.sqrt(separationLengthSquared)
          : 0;
      sx *= inverseSeparation;
      sy *= inverseSeparation;
      sz *= inverseSeparation;
      let tx = targetX - x,
        ty = targetY - y,
        tz = targetZ - z;
      const targetLengthSquared = tx * tx + ty * ty + tz * tz;
      const inverseTarget =
        targetLengthSquared > 1e-12 ? 1 / Math.sqrt(targetLengthSquared) : 0;
      tx *= inverseTarget;
      ty *= inverseTarget;
      tz *= inverseTarget;
      const bound =
        (Math.max(
          0,
          (Math.sqrt(x * x + y * y + z * z) / radius - 0.78) / 0.22,
        ) *
          3) /
        radius;
      let ox = x - obstacleX,
        oy = y - obstacleY,
        oz = z - obstacleZ;
      const obstacleLengthSquared = ox * ox + oy * oy + oz * oz;
      let obstacleScale = 0;
      if (obstacleLengthSquared < obstacleRadiusSquared) {
        if (obstacleLengthSquared <= 1e-12) {
          ox = 1;
          oy = 0;
          oz = 0;
          obstacleScale = config.obstacleWeight;
        } else {
          const distance = Math.sqrt(obstacleLengthSquared);
          obstacleScale =
            (config.obstacleWeight * (1 - distance / config.obstacleRadius)) /
            distance;
        }
      }
      const ax = cells[c + 4] * average * inverseSpeed - fx;
      const ay = cells[c + 5] * average * inverseSpeed - fy;
      const az = cells[c + 6] * average * inverseSpeed - fz;
      let vx =
        fx +
        turn *
          (config.alignment * ax +
            config.separation * sx +
            config.targetWeight * tx -
            x * bound +
            ox * obstacleScale);
      let vy =
        fy +
        turn *
          (config.alignment * ay +
            config.separation * sy +
            config.targetWeight * ty -
            y * bound +
            oy * obstacleScale);
      let vz =
        fz +
        turn *
          (config.alignment * az +
            config.separation * sz +
            config.targetWeight * tz -
            z * bound +
            oz * obstacleScale);
      let lengthSquared = vx * vx + vy * vy + vz * vz;
      if (lengthSquared <= 1e-12) {
        vx = fx;
        vy = fy;
        vz = fz;
        lengthSquared = vx * vx + vy * vy + vz * vz;
        if (lengthSquared <= 1e-12) {
          vx = 1;
          vy = 0;
          vz = 0;
          lengthSquared = 1;
        }
      }
      const velocityScale = speed / Math.sqrt(lengthSquared);
      vx *= velocityScale;
      vy *= velocityScale;
      vz *= velocityScale;
      let nx = x + vx * dt,
        ny = y + vy * dt,
        nz = z + vz * dt;
      const nextLengthSquared = nx * nx + ny * ny + nz * nz;
      if (nextLengthSquared > radiusSquared) {
        const inverseLength = 1 / Math.sqrt(nextLengthSquared);
        const bx = nx * inverseLength,
          by = ny * inverseLength,
          bz = nz * inverseLength;
        nx = bx * radius * 0.999;
        ny = by * radius * 0.999;
        nz = bz * radius * 0.999;
        const outward = 2 * Math.max(0, vx * bx + vy * by + vz * bz);
        vx -= outward * bx;
        vy -= outward * by;
        vz -= outward * bz;
      }
      output[i] = nx;
      output[i + 1] = ny;
      output[i + 2] = nz;
      output[i + 3] = 0;
      output[i + 4] = vx;
      output[i + 5] = vy;
      output[i + 6] = vz;
      output[i + 7] = 0;
    }
    this.front ^= 1;
  }
}

export function createBoidSimulation(
  config: Partial<BoidConfig> = {},
): BoidSimulation {
  return new BoidSimulation(config);
}
