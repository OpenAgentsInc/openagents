import assert from "node:assert/strict";
import { test } from "node:test";
import {
  SourceWaterSplashes,
  WaterSplashSampler,
  validateSourceWaterSplashes,
  type SourceWaterSplashesManifest,
} from "../packages/renderer/source-water-splashes.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";
const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
const keys = (value: number) => [
  { time: 0, value },
  { time: 1, value },
];
function fixture(): SourceWaterSplashesManifest {
  return {
    schema: "aethyrforge.source-water-splashes.v1",
    textures: [
      {
        id: "noise",
        url: "/private-assets/noise.bin",
        sha256: "a".repeat(64),
        byteLength: 20,
        width: 1,
        height: 1,
        colorSpace: "linear",
      },
    ],
    profiles: [
      {
        id: "foam",
        noise: "noise",
        spawnRate: 8,
        lifetime: [0.25, 0.25],
        diameter: [0.25, 0.4],
        velocityStart: [0, 0, 0],
        velocityEnd: [0, 0, 0],
        acceleration: [0, -4, 0],
        drag: 0,
        offset: [0, 0, 0],
        sphereRadius: 0,
        shapeScale: [1, 1, 1],
        sizeCurve: keys(1),
        alphaCurve: keys(0.4),
        redCurve: keys(3),
        uvScale: [1, 0.75],
        uvSpeed: [0, 0],
        uvFlip: "none",
      },
    ],
    material: {
      radialRadius: 0.5,
      radialDensity: 2.333,
      depthFadeMeters: 0.25,
    },
    lifecycle: {
      startSeconds: 0,
      warmupSeconds: 0,
      delaySeconds: 0,
      loop: "continuous",
    },
    scenarios: {
      day: [{ id: "source", profile: "foam", model: [...identity] }],
      night: [],
    },
  };
}
test("source foam has no pre-start population and continuous positive births", () => {
  const sampler = new WaterSplashSampler(fixture());
  assert.equal(sampler.sample("day", 0, [0, 0, 0], [0, 0, -1]).count, 0);
  assert.equal(sampler.sample("day", 0.124, [0, 0, 0], [0, 0, -1]).count, 0);
  assert.equal(sampler.sample("day", 0.125, [0, 0, 0], [0, 0, -1]).count, 1);
  assert.equal(sampler.sample("day", 3, [0, 0, 0], [0, 0, -1]).count, 2);
  assert.equal(sampler.sample("night", 3, [0, 0, 0], [0, 0, -1]).count, 0);
});
test("foam uses world clock without changing random history on repeated or reordered samples", () => {
  const sampler = new WaterSplashSampler(fixture());
  const first = Array.from(
    sampler.sample("day", 3, [0, 0, 0], [0, 0, -1]).data,
  );
  sampler.sample("night", 10, [0, 0, 0], [0, 0, -1]);
  assert.deepEqual(
    Array.from(sampler.sample("day", 3, [0, 0, 0], [0, 0, -1]).data),
    first,
  );
  assert.ok(
    sampler
      .sample("day", MAX_WORLD_ANIMATION_TIME, [0, 0, 0], [0, 0, -1])
      .data.every(Number.isFinite),
  );
  assert.throws(() =>
    sampler.sample("day", MAX_WORLD_ANIMATION_TIME + 1, [0, 0, 0], [0, 0, -1]),
  );
});
test("source foam preserves unit owner rotation, gravity, red and alpha independently", () => {
  const m = fixture();
  m.profiles[0].spawnRate = 1;
  m.profiles[0].lifetime = [2, 2];
  m.profiles[0].velocityStart = [2, 0, 0];
  m.profiles[0].velocityEnd = [2, 0, 0];
  m.scenarios.day[0].model = [0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 10, 2, 3, 1];
  const p = new WaterSplashSampler(m).sample("day", 1.5, [0, 0, 0], [0, 0, -1]);
  assert.equal(p.count, 1);
  assert.deepEqual(Array.from(p.data.slice(0, 3)), [10, 1.5, 2]);
  assert.ok(Math.abs(p.data[4] - 3) < 1e-6);
  assert.ok(Math.abs(p.data[5] - 0.4) < 1e-6);
});
test("reversed authored velocity endpoints remain supported and bounded", () => {
  const m = fixture();
  m.profiles[0].spawnRate = 1;
  m.profiles[0].lifetime = [2, 2];
  m.profiles[0].velocityStart = [0, 1.5, 0];
  m.profiles[0].velocityEnd = [0, 0.75, 0];
  m.profiles[0].acceleration = [0, 0, 0];
  assert.equal(validateSourceWaterSplashes(m), m);
  const p = new WaterSplashSampler(m).sample("day", 1.5, [0, 0, 0], [0, 0, -1]);
  assert.ok(p.data[1] >= 0.375 && p.data[1] <= 0.75);
});
test("source foam sorting is back to front and caller edits cannot alter installed data", () => {
  const m = fixture();
  const farther = [...identity];
  farther[14] = -4;
  m.scenarios.day.push({ id: "far", profile: "foam", model: farther });
  const sampler = new WaterSplashSampler(m);
  m.profiles[0].redCurve = keys(10);
  const p = sampler.sample("day", 3, [0, 0, 0], [0, 0, -1]);
  assert.equal(p.count, 4);
  assert.equal(p.data[2], -4);
  assert.equal(p.data[18], -4);
  assert.equal(p.data[34], 0);
  assert.equal(p.data[4], 3);
});
test("foam admission rejects resource escapes, unsupported clocks/transforms and population loss", () => {
  for (const mutate of [
    (m: SourceWaterSplashesManifest) => {
      m.textures[0].url = "https://example.com/noise";
    },
    (m: SourceWaterSplashesManifest) => {
      m.textures[0].height = 2;
    },
    (m: SourceWaterSplashesManifest) => {
      m.lifecycle.warmupSeconds = 1 as 0;
    },
    (m: SourceWaterSplashesManifest) => {
      m.scenarios.day[0].model[5] = 2;
    },
    (m: SourceWaterSplashesManifest) => {
      m.profiles[0].alphaCurve[1].time = 0;
    },
    (m: SourceWaterSplashesManifest) => {
      m.profiles[0].spawnRate = 100;
      m.profiles[0].lifetime = [2, 2];
      m.scenarios.day = Array.from({ length: 64 }, (_, i) => ({
        id: "many" + i,
        profile: "foam",
        model: [...identity],
      }));
    },
  ]) {
    const m = fixture();
    mutate(m);
    assert.throws(() => validateSourceWaterSplashes(m));
  }
});
test("altered private foam bytes never reach GPU allocation", async () => {
  let allocations = 0;
  const device = new Proxy(
    {},
    {
      get() {
        allocations++;
        throw Error("Unexpected GPU allocation");
      },
    },
  ) as GPUDevice;
  await assert.rejects(
    SourceWaterSplashes.create(
      device,
      fixture(),
      async () => new ArrayBuffer(20),
    ),
    /changed/,
  );
  assert.equal(allocations, 0);
});
test("owned small spray is explicit and cannot silently alter the source raster profile", () => {
  const source = fixture();
  assert.equal(source.appearanceProfile, undefined);
  source.material.minimumFootprintPixels = 2;
  assert.throws(
    () => validateSourceWaterSplashes(source),
    /appearance profile/,
  );
  source.appearanceProfile = "owned-subtle-spray-v1";
  assert.equal(validateSourceWaterSplashes(source), source);
  const sampler = new WaterSplashSampler(source);
  source.profiles[0].diameter = [0.01, 0.03];
  const installed = sampler.sample("day", 3, [0, 0, 0], [0, 0, -1]);
  assert.ok(installed.data[3] >= 0.25); // Appearance admission must not mutate authored physical size.
  delete source.material.minimumFootprintPixels;
  assert.throws(
    () => validateSourceWaterSplashes(source),
    /appearance profile/,
  );
});
