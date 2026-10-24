import assert from "node:assert/strict";
import test from "node:test";
import {
  AmbientParticleSampler,
  AMBIENT_PARTICLE_LIMITS,
  particleCurve,
  particleDisplacement,
  validateAmbientParticles,
  type AmbientParticleManifest,
  type AmbientParticleProfile,
} from "../packages/renderer/ambient-particles.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";
import { modelMatrix, normalize } from "../packages/renderer/math.ts";

function finite(value: number, low: number, high: number) {
  if (!Number.isFinite(value) || value < low || value > high)
    throw new Error("Prepared particles contain an unsupported number.");
}
function vector(
  value: readonly number[],
  length: number,
  low: number,
  high: number,
) {
  if (!Array.isArray(value) || value.length !== length)
    throw new Error("Prepared particles contain an incomplete vector.");
  for (const component of value) finite(component, low, high);
}

// The previous sampler is retained here to verify arithmetic and selection order.
function hash(v: number) {
  v = Math.imul(v ^ (v >>> 16), 0x7feb352d);
  v = Math.imul(v ^ (v >>> 15), 0x846ca68b);
  return (v ^ (v >>> 16)) >>> 0;
}
function identitySeed(v: string) {
  let h = 2166136261;
  for (let i = 0; i < v.length; i++)
    h = Math.imul(h ^ v.charCodeAt(i), 16777619);
  return hash(h);
}
const random = (seed: number, channel: number) =>
  hash(seed ^ Math.imul(channel + 1, 0x9e3779b9)) / 4294967296;
const mix = (a: number, b: number, t: number) => a + (b - a) * t;

class ReferenceAmbientParticleSampler {
  readonly manifest: AmbientParticleManifest;
  private candidates = new Float32Array(
    AMBIENT_PARTICLE_LIMITS.candidates * 16,
  );
  private output = new Float32Array(AMBIENT_PARTICLE_LIMITS.particles * 16);
  private distances = new Float64Array(AMBIENT_PARTICLE_LIMITS.candidates);
  private depths = new Float64Array(AMBIENT_PARTICLE_LIMITS.candidates);
  private local = new Float64Array(3);
  private order: number[] = [];
  private profiles: Map<string, AmbientParticleProfile>;
  constructor(
    manifest: AmbientParticleManifest,
    readonly budget: number = AMBIENT_PARTICLE_LIMITS.particles,
  ) {
    validateAmbientParticles(manifest);
    if (
      !Number.isInteger(budget) ||
      budget < 0 ||
      budget > AMBIENT_PARTICLE_LIMITS.particles
    )
      throw new Error("Use a supported ambient particle budget.");
    this.manifest = structuredClone(manifest);
    this.profiles = new Map(this.manifest.profiles.map((p) => [p.id, p]));
  }
  sample(
    mode: "day" | "night",
    time: number,
    eye: readonly number[],
    forward: readonly number[] = [0, 0, -1],
  ) {
    finite(time, 0, MAX_WORLD_ANIMATION_TIME);
    vector(eye, 3, -1e6, 1e6);
    vector(forward, 3, -1, 1);
    if (Math.abs(Math.hypot(...forward) - 1) > 0.001)
      throw new Error("Particle ordering needs a normalized camera direction.");
    if (mode !== "day" && mode !== "night")
      throw new Error("Choose a prepared particle scenario.");
    let count = 0;
    for (const emitter of this.manifest.scenarios[mode]) {
      const p = this.profiles.get(emitter.profile)!,
        m = emitter.model,
        base = identitySeed(emitter.id);
      const localTime = emitter.lifecycle
        ? time - emitter.lifecycle.startSeconds - emitter.lifecycle.delaySeconds
        : time;
      if (emitter.lifecycle && localTime <= 0) continue;
      const phase = emitter.lifecycle ? 0 : random(base, 0) / p.spawnRate,
        head = Math.floor((localTime + phase) * p.spawnRate),
        capacity = Math.ceil(p.spawnRate * p.lifetime[1]) + 1;
      const sizeScale = Math.cbrt(
        Math.hypot(m[0], m[1], m[2]) *
          Math.hypot(m[4], m[5], m[6]) *
          Math.hypot(m[8], m[9], m[10]),
      );
      for (let slot = 0; slot < capacity; slot++) {
        const birth = head - slot;
        if (emitter.lifecycle && birth < 1) break;
        const seed = hash(base ^ Math.imul(birth, 0x27d4eb2d));
        const age = localTime - birth / p.spawnRate + phase,
          life = mix(...p.lifetime, random(seed, 1));
        if (age < 0 || age >= life) continue;
        const u = age / life,
          azimuth = random(seed, 2) * Math.PI * 2,
          y = random(seed, 3) * 2 - 1;
        const radius = Math.cbrt(random(seed, 4)) * p.sphereRadius,
          radial = Math.sqrt(Math.max(0, 1 - y * y));
        const local = this.local;
        local[0] = Math.cos(azimuth) * radial * radius;
        local[1] = y * radius;
        local[2] = Math.sin(azimuth) * radial * radius;
        const drag = mix(...p.drag, random(seed, 5));
        for (let axis = 0; axis < 3; axis++) {
          local[axis] +=
            (random(seed, 6 + axis) - 0.5) * p.boxSize[axis] +
            p.shapeOffset[axis];
          local[axis] += particleDisplacement(
            age,
            mix(
              p.velocityMin[axis],
              p.velocityMax[axis],
              random(seed, 9 + axis),
            ),
            p.acceleration[axis],
            drag,
          );
          // Owned periodic drift replaces the source spatial curl-field solver.
          const omega = 2 * Math.PI * (0.17 + axis * 0.031),
            phi =
              random(seed, 12 + axis) * Math.PI * 2 +
              local[axis] * p.noiseFrequency;
          local[axis] +=
            (p.noiseStrength / Math.max(0.1, drag * drag + omega * omega)) *
            (Math.sin(phi + age * omega) - Math.sin(phi));
        }
        const offset = count * 16,
          x = m[0] * local[0] + m[4] * local[1] + m[8] * local[2] + m[12],
          py = m[1] * local[0] + m[5] * local[1] + m[9] * local[2] + m[13],
          z = m[2] * local[0] + m[6] * local[1] + m[10] * local[2] + m[14];
        const out = this.candidates;
        out[offset] = x;
        out[offset + 1] = py;
        out[offset + 2] = z;
        out[offset + 3] =
          mix(...p.size, random(seed, 15)) *
          sizeScale *
          particleCurve(p.sizeCurve, u);
        for (let channel = 0; channel < 3; channel++)
          out[offset + 4 + channel] = p.color[channel];
        out[offset + 7] = Math.min(1, particleCurve(p.alphaCurve, u));
        out[offset + 8] = random(seed, 16) * Math.PI * 2;
        out[offset + 9] = p.phaseCurve ? particleCurve(p.phaseCurve, u) : u;
        out[offset + 10] = Number(p.kind === "smoke");
        out[offset + 11] = p.depthFade;
        out[offset + 12] = p.emission;
        for (let channel = 0; channel < 3; channel++)
          out[offset + 13 + channel] = p.tint?.[channel] ?? 1;
        this.distances[count] =
          (x - eye[0]) ** 2 + (py - eye[1]) ** 2 + (z - eye[2]) ** 2;
        this.depths[count] =
          (x - eye[0]) * forward[0] +
          (py - eye[1]) * forward[1] +
          (z - eye[2]) * forward[2];
        this.order[count] = count;
        count++;
      }
    }
    this.order.length = count;
    // Keep nearest candidates under pressure, then compose back to front. Stable
    // candidate indices settle equal distances without changing emitter seeds.
    this.order.sort((a, b) => this.distances[a] - this.distances[b] || a - b);
    const retained = Math.min(count, this.budget);
    this.order.length = retained;
    this.order.sort((a, b) => this.depths[b] - this.depths[a] || a - b);
    let dust = 0,
      smoke = 0;
    for (let i = 0; i < retained; i++) {
      const index = this.order[i];
      for (let field = 0; field < 16; field++)
        this.output[i * 16 + field] = this.candidates[index * 16 + field];
      if (this.output[i * 16 + 10] > 0.5) smoke++;
      else dust++;
    }
    return {
      data: this.output.subarray(0, retained * 16),
      count: retained,
      dust,
      smoke,
      dropped: count - retained,
    };
  }
}

function seeded(seed: number) {
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x1_0000_0000;
  };
}

function recipe(seed: number): AmbientParticleManifest {
  const next = seeded(seed);
  const profiles = Array.from(
    { length: 3 },
    (_, index): AmbientParticleProfile => ({
      id: "profile-" + index,
      kind: index === 2 ? "smoke" : "dust",
      spawnRate: 2 + next() * 12,
      lifetime: [0.5 + next(), 2 + next() * 2],
      size: [0.001 + next() * 0.02, 0.1 + next()],
      velocityMin: [next() - 1, next() - 1, next() - 1],
      velocityMax: [next() + 1, next() + 1, next() + 1],
      acceleration: [next() * 4 - 2, next() * 4 - 2, next() * 4 - 2],
      drag:
        index === 0 ? [0, 0] : index === 1 ? [0.000009, 0.000011] : [0.1, 2],
      color: [next(), next(), next()],
      tint: [next(), next(), next()],
      emission: next() * 100,
      sphereRadius: next(),
      boxSize: [next(), next(), next()],
      shapeOffset: [next() - 0.5, next() - 0.5, next() - 0.5],
      noiseStrength: next(),
      noiseFrequency: next(),
      alphaCurve: [
        { time: 0, value: 0, mode: "cubic", leaveTangent: 0.4 },
        { time: 0.3, value: 0.7, arriveTangent: -0.2 },
        { time: 0.6, value: 0.5, mode: "constant" },
        { time: 1, value: 0 },
      ],
      sizeCurve: [
        { time: 0, value: 0.7 },
        { time: 1, value: 1.3 },
      ],
      phaseCurve: [
        { time: 0, value: 0 },
        { time: 1, value: 1 },
      ],
      depthFade: next(),
    }),
  );
  return {
    schema: "aethyrforge.ambient-particles.v1",
    profiles,
    scenarios: {
      day: Array.from({ length: 6 }, (_, index) => emitter(index, "day")),
      night: Array.from({ length: 7 }, (_, index) => emitter(index, "night")),
    },
    atlas: {
      url: "/private-assets/owned-test-atlas.bin",
      sha256: "0".repeat(64),
      byteLength: 16 + 8 * 8 * 4,
      width: 8,
      height: 8,
      grid: [8, 8],
      colorSpace: "srgb",
    },
  };
  function emitter(index: number, mode: string) {
    const model = modelMatrix(
      [next() * 20 - 10, next() * 5, next() * 20 - 10],
      next() * Math.PI * 2,
      0.2 + next() * 2,
    );
    for (let column = 0; column < 3; column++) {
      const scale = (index & (1 << column) ? -1 : 1) * (0.2 + next());
      for (let row = 0; row < 3; row++) model[column * 4 + row] *= scale;
    }
    return {
      id: mode + "-emitter-" + index + "-" + seed,
      profile: profiles[index % profiles.length].id,
      model,
      ...(index % 2
        ? {
            lifecycle: {
              schema: "aethyrforge.particle-start.v1" as const,
              startSeconds: 1.5,
              warmupSeconds: 0 as const,
              delaySeconds: 0.25,
              loop: "continuous" as const,
            },
          }
        : {}),
    };
  }
}

function compare(
  reference: ReferenceAmbientParticleSampler,
  current: AmbientParticleSampler,
  mode: "day" | "night",
  time: number,
  eye: number[] = [2, 3, 4],
  forward: number[] = [0, 0, -1],
) {
  const expected = reference.sample(mode, time, eye, forward),
    actual = current.sample(mode, time, eye, forward);
  assert.deepEqual(
    {
      count: actual.count,
      dust: actual.dust,
      smoke: actual.smoke,
      dropped: actual.dropped,
    },
    {
      count: expected.count,
      dust: expected.dust,
      smoke: expected.smoke,
      dropped: expected.dropped,
    },
  );
  assert.deepEqual(
    new Uint32Array(
      actual.data.buffer,
      actual.data.byteOffset,
      actual.data.length,
    ),
    new Uint32Array(
      expected.data.buffer,
      expected.data.byteOffset,
      expected.data.length,
    ),
    `${mode} at ${time}, budget ${current.budget}`,
  );
  return actual;
}

test("ambient sampling keeps every output bit across seeds, modes, curves, clocks and budgets", () => {
  const times = [
    0,
    1e-12,
    0.1,
    1.75,
    2,
    2.75,
    10,
    86400,
    MAX_WORLD_ANIMATION_TIME,
    5,
    0,
  ];
  for (let seed = 1; seed <= 12; seed++) {
    const manifest = recipe(seed);
    for (const budget of [0, 1, 17, AMBIENT_PARTICLE_LIMITS.particles]) {
      const reference = new ReferenceAmbientParticleSampler(manifest, budget),
        current = new AmbientParticleSampler(manifest, budget);
      for (const mode of ["day", "night"] as const)
        for (const time of times)
          compare(
            reference,
            current,
            mode,
            time,
            [seed * 0.5, 3, -seed],
            normalize([1, 0.2, -1]),
          );
    }
  }
});

test("ambient birth and death boundaries retain exact drag branches and reversed-time replay", () => {
  for (const drag of [
    0,
    0.000001,
    0.00001 * (1 - Number.EPSILON),
    0.00001,
    1,
    100,
  ]) {
    const manifest = recipe(15),
      profile = manifest.profiles[0],
      emitter = manifest.scenarios.day[0];
    Object.assign(profile, {
      spawnRate: 4,
      lifetime: [2, 2],
      drag: [drag, drag],
    });
    emitter.lifecycle = {
      schema: "aethyrforge.particle-start.v1",
      startSeconds: 10,
      warmupSeconds: 0,
      delaySeconds: 2,
      loop: "continuous",
    };
    manifest.scenarios = { day: [emitter], night: [structuredClone(emitter)] };
    const reference = new ReferenceAmbientParticleSampler(manifest),
      current = new AmbientParticleSampler(manifest);
    for (const time of [
      0,
      12,
      12.25 - 1e-12,
      12.25,
      12.25 + 1e-12,
      14.25 - 1e-12,
      14.25,
      14.25 + 1e-12,
      100000,
      12.25,
      0,
    ])
      for (const mode of ["day", "night"] as const)
        compare(reference, current, mode, time);
  }
});

test("ambient nearest-budget and depth ties preserve source order under maximum candidate pressure", () => {
  const manifest = recipe(30),
    profile = manifest.profiles[0],
    emitter = manifest.scenarios.day[0];
  Object.assign(profile, {
    spawnRate: 100,
    lifetime: [60, 60],
    velocityMin: [0, 0, 0],
    velocityMax: [0, 0, 0],
    acceleration: [0, 0, 0],
    sphereRadius: 0,
    boxSize: [0, 0, 0],
    shapeOffset: [0, 0, 0],
    noiseStrength: 0,
  });
  emitter.model = modelMatrix([1, 1, 1]);
  manifest.scenarios = {
    day: [emitter, { ...emitter, id: "second-coincident-emitter" }],
    night: [],
  };
  for (const budget of [0, 1, 4096]) {
    const reference = new ReferenceAmbientParticleSampler(manifest, budget),
      current = new AmbientParticleSampler(manifest, budget);
    const result = compare(reference, current, "day", 5);
    assert.ok(result.dropped > 0);
    compare(reference, current, "night", 5);
  }
  // Without pressure, both passes retain all candidates; the depth comparator
  // uses the original source index to resolve equal positions independently.
  profile.spawnRate = 2;
  profile.lifetime = [1, 1];
  const reference = new ReferenceAmbientParticleSampler(manifest),
    current = new AmbientParticleSampler(manifest);
  compare(reference, current, "day", 5);
});

test("ambient mutable owned recipes remain live without a stale seed or transform cache", () => {
  const manifest = recipe(42),
    reference = new ReferenceAmbientParticleSampler(manifest),
    current = new AmbientParticleSampler(manifest);
  compare(reference, current, "day", 5);
  for (const owned of [reference.manifest, current.manifest]) {
    owned.scenarios.day[0].id = "edited-seed";
    owned.scenarios.day[0].model[12] += 10;
    owned.scenarios.day[0].model[0] *= 1.5;
    owned.profiles[0].noiseFrequency *= 2;
    owned.profiles[0].drag = [0.5, 0.9];
    owned.profiles[0].spawnRate *= 0.8;
  }
  compare(reference, current, "day", 5);
  compare(reference, current, "night", 5);
  for (const time of [
    -1,
    -1e-12,
    Infinity,
    NaN,
    MAX_WORLD_ANIMATION_TIME + 1,
  ]) {
    assert.throws(() => reference.sample("day", time, [0, 0, 0]));
    assert.throws(() => current.sample("day", time, [0, 0, 0]));
  }
});
