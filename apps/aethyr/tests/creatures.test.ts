import assert from "node:assert/strict";
import { test } from "node:test";
import {
  CreatureSampler,
  CREATURE_LIMITS,
  checkedCreatureBlob,
  validateCreatures,
  type CreaturesManifest,
} from "../packages/renderer/creatures.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../packages/runtime/world-clock.ts";

function fixture(): CreaturesManifest {
  return {
    schema: "aethyrforge.creatures.v1",
    geometry: {
      vertices: {
        url: "/private-assets/v.bin",
        sha256: "0".repeat(64),
        byteLength: 96,
      },
      indices: {
        url: "/private-assets/i.bin",
        sha256: "0".repeat(64),
        byteLength: 12,
      },
      vertexCount: 3,
      indexCount: 3,
    },
    meshes: [
      {
        id: "bird",
        firstIndex: 0,
        indexCount: 3,
        doubleSided: false,
        opacityCutoff: 0.3333,
        flap: { amplitude: 0.35, uvMultiplier: 1, frequency: [1, 1] },
      },
    ],
    profiles: [
      {
        id: "gull",
        kind: "seagull",
        mesh: "bird",
        spawnRate: 4,
        spawnProbability: 1,
        lifetime: [2, 2],
        scale: [1, 1],
        sphereRadius: 0,
        velocityMin: [1, 0, 0],
        velocityMax: [1, 0, 0],
        color: [1, 1, 1],
        alphaCurve: [{ time: 0, value: 1 }],
        alphaMultiplierCurve: [{ time: 0, value: 1 }],
        scaleCurve: [{ time: 0, value: 1 }],
        ditherCurve: [{ time: 0, value: 1 }],
        drag: 0,
        vortexStrength: 0,
        noiseStrength: 0,
        noiseFrequency: 0,
        emission: {
          activeSeconds: 1,
          delay: [0, 0],
          delayFirstLoopOnly: false,
          recalculateDelay: false,
        },
      },
    ],
    scenarios: {
      day: [
        {
          id: "owned-emitter",
          profile: "gull",
          model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 3, 4, 5, 1],
          lifecycle: { startSeconds: 0, warmupSeconds: 0, clock: "world" },
        },
      ],
      night: [],
    },
  };
}
test("creatures use positive-time source starts and deterministic lifetime-bounded replay", () => {
  const input = fixture(),
    sampler = new CreatureSampler(input);
  assert.equal(sampler.sample("day", 0, [0, 0, 0]).count, 0);
  assert.equal(sampler.sample("day", 0.249, [0, 0, 0]).count, 0);
  const first = sampler.sample("day", 0.5, [0, 0, 0]);
  assert.equal(first.count, 2);
  assert.deepEqual([first.data[0], first.data[20]].sort(), [3, 3.25]);
  assert.deepEqual([...first.data.slice(1, 3)], [4, 5]);
  const earlier = [...sampler.sample("day", 1.4, [0, 0, 0]).data];
  sampler.sample("day", MAX_WORLD_ANIMATION_TIME, [10, 10, 10]);
  sampler.sample("night", 4, [0, 0, 0]);
  assert.deepEqual([...sampler.sample("day", 1.4, [0, 0, 0]).data], earlier);
  assert.equal(sampler.sample("day", 10, [0, 0, 0]).count, 8);
  input.profiles[0].color[0] = 0;
  assert.equal(sampler.manifest.profiles[0].color[0], 1);
});
test("source delays, spawn probability and separate age-alpha inputs control births", () => {
  const input = fixture();
  input.profiles[0].emission.delay = [1, 1];
  const sampler = new CreatureSampler(input);
  assert.equal(sampler.sample("day", 1.249, [0, 0, 0]).count, 0);
  assert.equal(sampler.sample("day", 1.5, [0, 0, 0]).count, 2);
  assert.equal(sampler.sample("day", 3, [0, 0, 0]).count, 4);
  const none = fixture();
  none.profiles[0].spawnProbability = 0;
  assert.equal(
    new CreatureSampler(none).sample("day", 100, [0, 0, 0]).count,
    0,
  );
  const alpha = fixture();
  alpha.profiles[0].alphaCurve[0].value = 0.8;
  alpha.profiles[0].alphaMultiplierCurve[0].value = 0.5;
  alpha.profiles[0].ditherCurve[0].value = 0.25;
  assert.ok(
    Math.abs(
      new CreatureSampler(alpha).sample("day", 0.5, [0, 0, 0]).data[15] - 0.25,
    ) < 1e-7,
  );
  alpha.profiles[0].alphaCurve[0].value = 0.4;
  assert.equal(
    new CreatureSampler(alpha).sample("day", 0.5, [0, 0, 0]).count,
    0,
  );
});
test("bat motion remains finite, owner transforms rotate flight and basis remains orthonormal", () => {
  const input = fixture();
  const p = input.profiles[0];
  p.kind = "bat";
  p.sphereRadius = 8;
  p.velocityMin = [7.5, 3, 5];
  p.velocityMax = [8, 0, 7.5];
  p.drag = 0.25;
  p.vortexStrength = 4;
  p.noiseStrength = 5;
  p.noiseFrequency = 50;
  input.scenarios.day[0].model = [
    0, 0, -2, 0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 4, 5, 1,
  ];
  const sampler = new CreatureSampler(input, 3),
    data = sampler.sample("day", 100, [0, 0, 0]);
  assert.equal(data.count, 3);
  assert.ok(data.dropped > 0);
  assert.ok(data.data.every(Number.isFinite));
  for (let i = 0; i < data.count; i++) {
    const d = data.data.subarray(i * 20, i * 20 + 20);
    assert.ok(Math.abs(Math.hypot(...d.slice(4, 7)) - 1) < 1e-6);
    assert.ok(Math.abs(Math.hypot(...d.slice(8, 11)) - 1) < 1e-6);
    assert.ok(Math.abs(d[4] * d[8] + d[5] * d[9] + d[6] * d[10]) < 1e-6);
    assert.equal(d[3], 2);
  }
});
test("source flap phase remains accurate at large world ages without float32 time loss", () => {
  const sampler = new CreatureSampler(fixture());
  for (const time of [0.25, 0.5, MAX_WORLD_ANIMATION_TIME - 0.25]) {
    const s = sampler.sample("day", time, [0, 0, 0]);
    assert.ok(s.count);
    assert.ok(Math.abs(s.data[18] - Math.sin((time % 1) * 2 * Math.PI)) < 1e-6);
  }
});
test("malformed geometry, transforms, unknown profiles and unsupported lifecycle fail", () => {
  for (const mutate of [
    (x: CreaturesManifest) => {
      x.geometry.vertices.byteLength = 95;
    },
    (x: CreaturesManifest) => {
      x.geometry.indices.url = "/private-assets/../secret";
    },
    (x: CreaturesManifest) => {
      x.meshes[0].indexCount = 6;
    },
    (x: CreaturesManifest) => {
      x.scenarios.day[0].model[0] = 2;
    },
    (x: CreaturesManifest) => {
      x.scenarios.day[0].model[0] = -1;
    },
    (x: CreaturesManifest) => {
      x.scenarios.day[0].model[3] = 0.2;
    },
    (x: CreaturesManifest) => {
      x.scenarios.day[0].lifecycle.warmupSeconds = 1 as 0;
    },
    (x: CreaturesManifest) => {
      x.profiles[0].alphaMultiplierCurve = [];
    },
    (x: CreaturesManifest) => {
      x.profiles[0].emission.activeSeconds = 0.3;
    },
    (x: CreaturesManifest) => {
      x.profiles[0].kind = "unknown" as "bat";
    },
    (x: CreaturesManifest) => {
      x.profiles[0].spawnRate = NaN;
    },
  ]) {
    const input = fixture();
    mutate(input);
    assert.throws(() => validateCreatures(input));
  }
  assert.throws(
    () => new CreatureSampler(fixture(), CREATURE_LIMITS.creatures + 1),
  );
});
test("geometry blob hash and byte count are checked before GPU admission", async () => {
  const bytes = new Uint8Array([1, 2, 3, 4]).buffer,
    sha256 = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
      .map((v) => v.toString(16).padStart(2, "0"))
      .join("");
  const desc = { url: "/private-assets/owned.bin", sha256, byteLength: 4 };
  assert.equal(
    (await checkedCreatureBlob(desc, async () => bytes)).byteLength,
    4,
  );
  await assert.rejects(
    checkedCreatureBlob({ ...desc, sha256: "0".repeat(64) }, async () => bytes),
  );
  await assert.rejects(
    checkedCreatureBlob({ ...desc, byteLength: 8 }, async () => bytes),
  );
});
