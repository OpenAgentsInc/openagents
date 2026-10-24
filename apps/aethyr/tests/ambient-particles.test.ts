import assert from "node:assert/strict";
import { test } from "node:test";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";
import {
  AmbientParticleSampler,
  DUST_RADIAL_DENSITY,
  DUST_RADIAL_RADIUS,
  DUST_RADIAL_PRODUCT_INTEGRAL,
  DUST_RADIAL_OPACITY_INTEGRAL,
  dustRadialMask,
  particleCurve,
  particleDisplacement,
  validateAmbientParticles,
  type AmbientParticleManifest,
} from "../packages/renderer/ambient-particles.ts";

test("source-based dust has finite support and independent emission/opacity energy", () => {
  assert.equal(DUST_RADIAL_RADIUS, 0.5);
  assert.equal(DUST_RADIAL_DENSITY, 2.333);
  assert.ok(
    Math.abs(dustRadialMask([0.5, 0.5]) - (1 - Math.exp(-(2.333 ** 2)))) <
      1e-12,
  );
  for (const uv of [
    [0, 0],
    [0, 1],
    [1, 0],
    [1, 1],
    [0, 0.5],
    [0.5, 1],
    [2, 2],
  ])
    assert.equal(dustRadialMask(uv), 0);
  let previous = Infinity;
  for (let i = 0; i <= 1000; i++) {
    const mask = dustRadialMask([0.5 + i / 2000, 0.5]);
    assert.ok(Number.isFinite(mask) && mask >= 0 && mask <= previous);
    previous = mask;
  }
  const edge = dustRadialMask([1 - 1e-6, 0.5]);
  assert.ok(50000 * edge * edge < 0.000002);
  // Independent polar integration checks the analytic subpixel flux factor.
  let numerical = 0,
    opacity = 0;
  const n = 20000;
  for (let i = 0; i < n; i++) {
    const radius = ((i + 0.5) * DUST_RADIAL_RADIUS) / n;
    numerical +=
      (2 *
        Math.PI *
        radius *
        dustRadialMask([0.5 + radius, 0.5]) ** 2 *
        DUST_RADIAL_RADIUS) /
      n;
    opacity +=
      (2 *
        Math.PI *
        radius *
        dustRadialMask([0.5 + radius, 0.5]) *
        DUST_RADIAL_RADIUS) /
      n;
  }
  assert.ok(Math.abs(numerical - DUST_RADIAL_PRODUCT_INTEGRAL) < 1e-9);
  assert.ok(Math.abs(opacity - DUST_RADIAL_OPACITY_INTEGRAL) < 1e-9);
  assert.ok(Math.abs(DUST_RADIAL_PRODUCT_INTEGRAL - 0.2302941748575) < 1e-10);
});

function fixture(): AmbientParticleManifest {
  return {
    schema: "aethyrforge.ambient-particles.v1",
    profiles: [
      {
        id: "owned-dust",
        kind: "dust",
        spawnRate: 4,
        lifetime: [2, 3],
        size: [0.01, 0.02],
        velocityMin: [0, 1, 0],
        velocityMax: [0, 2, 0],
        acceleration: [0, 1, 0],
        drag: [1, 2],
        color: [0.5, 0.4, 0.2],
        emission: 100,
        sphereRadius: 0.2,
        boxSize: [1, 2, 1],
        shapeOffset: [0, 0, 0],
        noiseStrength: 0.1,
        noiseFrequency: 0.085,
        alphaCurve: [
          { time: 0, value: 0 },
          { time: 0.5, value: 1 },
          { time: 1, value: 0 },
        ],
        sizeCurve: [{ time: 0, value: 1 }],
        depthFade: 0,
      },
    ],
    scenarios: {
      day: [
        {
          id: "owned-emitter",
          profile: "owned-dust",
          model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 3, 4, 5, 1],
        },
      ],
      night: [],
    },
  };
}

test("particles start in a deterministic steady state and replay independently of frame history", () => {
  const input = fixture(),
    sampler = new AmbientParticleSampler(input);
  const start = sampler.sample("day", 0, [0, 0, 0]);
  assert.ok(start.count >= 8 && start.count <= 12);
  assert.equal(start.dust, start.count);
  assert.equal(start.smoke, 0);
  const initial = [...start.data];
  sampler.sample("day", 86400, [100, 100, 100]);
  sampler.sample("night", 100, [0, 0, 0]);
  assert.deepEqual([...sampler.sample("day", 0, [0, 0, 0]).data], initial);
  const later = [...sampler.sample("day", 0.01, [0, 0, 0]).data];
  assert.notDeepEqual(later, initial);
  const other = new AmbientParticleSampler(input);
  assert.deepEqual([...other.sample("day", 0.01, [0, 0, 0]).data], later);
  // Caller changes cannot mutate the prepared recipe already in use.
  input.profiles[0].emission = 0;
  assert.ok(sampler.sample("day", 0, [0, 0, 0]).data[12] > 0);
});

test("explicit starts exclude preactivation births and retain source rate, age curves and finite lifetimes", () => {
  const m = fixture(),
    e = m.scenarios.day[0],
    p = m.profiles[0];
  e.lifecycle = {
    schema: "aethyrforge.particle-start.v1",
    startSeconds: 10,
    warmupSeconds: 0,
    delaySeconds: 2,
    loop: "continuous",
  };
  Object.assign(p, {
    lifetime: [2, 2],
    velocityMin: [0, 1, 0],
    velocityMax: [0, 1, 0],
    acceleration: [0, 0, 0],
    drag: [0, 0],
    sphereRadius: 0,
    boxSize: [0, 0, 0],
    noiseStrength: 0,
  });
  const sampler = new AmbientParticleSampler(m);
  for (const time of [0, 10, 11.99, 12, 12.249])
    assert.equal(
      sampler.sample("day", time, [0, 0, 0]).count,
      0,
      `time ${time}`,
    );
  const first = sampler.sample("day", 12.25, [0, 0, 0]);
  assert.equal(first.count, 1);
  assert.equal(first.data[1], 4);
  assert.equal(first.data[7], 0, "source alpha starts at zero for a newborn");
  const atOne = sampler.sample("day", 13, [0, 0, 0]);
  assert.equal(atOne.count, 4);
  const ages = Array.from(
    { length: atOne.count },
    (_, i) => atOne.data[i * 16 + 1] - 4,
  ).sort();
  assert.deepEqual(ages, [0, 0.25, 0.5, 0.75]);
  const expected = [...sampler.sample("day", 14.25, [0, 0, 0]).data];
  assert.equal(
    expected.length / 16,
    8,
    "a birth at age two dies as another is born",
  );
  sampler.sample("day", 86400, [0, 0, 0]);
  sampler.sample("day", 0, [0, 0, 0]);
  assert.deepEqual([...sampler.sample("day", 14.25, [0, 0, 0]).data], expected);
  const fresh = new AmbientParticleSampler(m);
  assert.deepEqual([...fresh.sample("day", 14.25, [0, 0, 0]).data], expected);
  const longSession = fresh.sample("day", MAX_WORLD_ANIMATION_TIME, [0, 0, 0]);
  assert.equal(longSession.count, 8);
  assert.ok([...longSession.data].every(Number.isFinite));
});

test("optional start validation excludes unsupported warmup, loops and unbounded activation", () => {
  const valid = {
    schema: "aethyrforge.particle-start.v1",
    startSeconds: 0,
    warmupSeconds: 0,
    delaySeconds: 0,
    loop: "continuous",
  };
  for (const damage of [
    null,
    [],
    { ...valid, warmupSeconds: 1 },
    { ...valid, loop: "once" },
    { ...valid, schema: "other" },
    { ...valid, startSeconds: -1 },
    { ...valid, startSeconds: NaN },
    { ...valid, delaySeconds: Infinity },
    { ...valid, startSeconds: 86400, delaySeconds: 1 },
    { ...valid, extra: true },
  ]) {
    const m = fixture();
    m.scenarios.day[0].lifecycle = damage as any;
    assert.throws(() => new AmbientParticleSampler(m));
  }
  const m = fixture();
  m.scenarios.day[0].lifecycle = valid as any;
  const sample = new AmbientParticleSampler(m);
  assert.equal(sample.sample("day", 0, [0, 0, 0]).count, 0);
  assert.equal(sample.sample("day", 0.25, [0, 0, 0]).count, 1);
});

test("world transforms and ordering use camera depth; budget keeps nearest identities", () => {
  const input = fixture();
  Object.assign(input.profiles[0], {
    spawnRate: 1,
    lifetime: [1, 1],
    velocityMin: [0, 0, 0],
    velocityMax: [0, 0, 0],
    acceleration: [0, 0, 0],
    sphereRadius: 0,
    boxSize: [0, 0, 0],
    noiseStrength: 0,
  });
  const emitter = input.scenarios.day[0];
  input.scenarios.day = [
    [20, 0, 1],
    [0, 0, 4],
    [0, 0, 2],
  ].map((position, i) => ({
    ...emitter,
    id: "owned-" + i,
    model: [...emitter.model.slice(0, 12), ...position, 1],
  }));
  const all = new AmbientParticleSampler(input).sample(
    "day",
    0,
    [0, 0, 0],
    [0, 0, 1],
  );
  assert.deepEqual([all.data[2], all.data[18], all.data[34]], [4, 2, 1]);
  const limited = new AmbientParticleSampler(input, 2).sample(
    "day",
    0,
    [0, 0, 0],
    [0, 0, 1],
  );
  assert.equal(limited.count, 2);
  assert.equal(limited.dropped, 1);
  assert.deepEqual([limited.data[2], limited.data[18]], [4, 2]);
  assert.equal(
    new AmbientParticleSampler(input, 0).sample("day", 0, [0, 0, 0]).count,
    0,
  );
});

test("analytic drag follows constant acceleration, handles zero drag and remains stable", () => {
  assert.equal(particleDisplacement(0, 2, 3, 1), 0);
  assert.equal(particleDisplacement(2, 2, 3, 0), 10);
  const a = particleDisplacement(2, 2, 3, 1);
  assert.ok(Math.abs(a - (6 - (1 - Math.exp(-2)))) < 1e-12);
  assert.ok(Math.abs(particleDisplacement(2, 2, 3, 1e-7) - 10) < 1e-5);
});

test("recovered age curves preserve cubic tangents, linear and constant intervals", () => {
  const smooth = [
    { time: 0, value: 0, mode: "cubic" as const, leaveTangent: 0 },
    { time: 1, value: 1, arriveTangent: 0 },
  ];
  assert.equal(particleCurve(smooth, 0.25), 0.15625);
  assert.equal(
    particleCurve(
      [
        { time: 0, value: 0 },
        { time: 1, value: 1 },
      ],
      0.25,
    ),
    0.25,
  );
  assert.equal(
    particleCurve(
      [
        { time: 0, value: 0, mode: "constant" },
        { time: 1, value: 1 },
      ],
      0.25,
    ),
    0,
  );
  assert.equal(particleCurve(smooth, -1), 0);
  assert.equal(particleCurve(smooth, 2), 1);
});

test("admission rejects unbounded populations, malformed matrices and unsupported atlases", () => {
  const invalid = [
    (m: AmbientParticleManifest) => (m.profiles[0].spawnRate = Infinity),
    (m: AmbientParticleManifest) => m.profiles[0].alphaCurve.reverse(),
    (m: AmbientParticleManifest) => (m.scenarios.day[0].model[15] = 0),
    (m: AmbientParticleManifest) =>
      m.scenarios.day.push({ ...m.scenarios.day[0] }),
    (m: AmbientParticleManifest) => (m.profiles[0].lifetime = [60, 1]),
    (m: AmbientParticleManifest) => {
      m.profiles[0].spawnRate = 100;
      m.profiles[0].lifetime = [60, 60];
      m.scenarios.day = Array.from({ length: 3 }, (_, i) => ({
        ...m.scenarios.day[0],
        id: "owned-" + i,
      }));
    },
    (m: AmbientParticleManifest) => (m.profiles[0].kind = "smoke"),
  ];
  for (const mutate of invalid) {
    const m = fixture();
    mutate(m);
    assert.throws(() => validateAmbientParticles(m));
  }
  const sampler = new AmbientParticleSampler(fixture());
  for (const time of [-1, Infinity, MAX_WORLD_ANIMATION_TIME + 1])
    assert.throws(() => sampler.sample("day", time, [0, 0, 0]));
  assert.throws(() => sampler.sample("day", 0, [NaN, 0, 0]));
  assert.throws(() => sampler.sample("day", 0, [0, 0, 0], [0, 0, 0]));
});

test("shared wind moves analytic particles without changing births, size, color, or replay", async () => {
  const { DEFAULT_WIND_FIELD } =
    await import("../packages/renderer/shared-wind.ts");
  const input = fixture();
  input.profiles[0].windResponse = 0.25;
  const sampler = new AmbientParticleSampler(input);
  const disabled = { ...DEFAULT_WIND_FIELD, enabled: false };
  const steady = {
    ...DEFAULT_WIND_FIELD,
    direction: [1, 0] as const,
    gustStrength: 0,
  };
  const legacy = sampler.sample("day", 0, [0, 0, 0]).data.slice();
  assert.deepEqual(
    sampler.sample("day", 0, [0, 0, 0], [0, 0, -1], disabled).data,
    legacy,
  );
  const windy = sampler
    .sample("day", 0, [0, 0, 0], [0, 0, -1], steady)
    .data.slice();
  assert.equal(windy.length, legacy.length);
  for (let i = 0; i < legacy.length; i++) {
    if (i % 16 === 0) assert.ok(windy[i] >= legacy[i]);
    else assert.equal(windy[i], legacy[i]);
  }
  sampler.sample("day", 50, [0, 0, 0], [0, 0, -1], steady);
  assert.deepEqual(
    sampler.sample("day", 0, [0, 0, 0], [0, 0, -1], steady).data,
    windy,
  );
  input.profiles[0].windResponse = 0;
  assert.deepEqual(
    new AmbientParticleSampler(input).sample(
      "day",
      0,
      [0, 0, 0],
      [0, 0, -1],
      steady,
    ).data,
    legacy,
  );
  input.profiles[0].windResponse = Infinity;
  assert.throws(() => new AmbientParticleSampler(input));
});
