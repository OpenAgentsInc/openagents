import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  GAMEPLAY_VISUAL_LIMITS,
  GameplayEffectSampler,
  gameplayLightRevision,
  gameplayLights,
  type GameplayEffect,
} from "../packages/renderer/gameplay-effects.ts";
import { ClusteredLighting } from "../packages/renderer/clustered-lighting.ts";
import { CLUSTER_HEADER } from "../packages/renderer/clustered-lights.ts";
import { perspective, type V3 } from "../packages/renderer/math.ts";

function effect(
  kind: GameplayEffect["kind"],
  age: number,
  radius = 4,
): GameplayEffect {
  return {
    id: 0xffffffed,
    kind,
    origin: [3.25, 12.5, -4.125],
    target: [-5.5, 0.375, -19.25],
    age,
    life: 1,
    radius,
  };
}

function sampleHash(kind: GameplayEffect["kind"]) {
  const hash = createHash("sha256"),
    sampler = new GameplayEffectSampler();
  for (const [eye, forward] of [
    [
      [4.3, 7.1, 9.5],
      [0.2, -0.1, -1],
    ],
    [
      [3, 0.5, -6],
      [0, 1, 0],
    ],
  ] as [V3, V3][])
    for (const radius of [0.1, 24])
      for (const age of [
        -0.01, 0, 0.001, 0.125, 0.449999, 0.45, 0.999999, 1, 1.1,
      ]) {
        const sample = sampler.sample(
            [effect(kind, age, radius)],
            eye,
            forward,
          ),
          count = Buffer.alloc(4);
        count.writeUInt32LE(sample.count);
        hash.update(count);
        hash.update(
          new Uint8Array(
            sample.data.buffer,
            sample.data.byteOffset,
            sample.data.byteLength,
          ),
        );
      }
  return hash.digest("hex");
}

// Recorded from the original array-based sampler before the scalar-write change.
const ORIGINAL_SAMPLE_SHA256: Record<GameplayEffect["kind"], string> = {
  beam: "acc48ddf594ab29a36b339501200830ea9fb8d6a25011c51f3b36cac01dda6b9",
  lightning: "d48a35033662e7f03c7a3e75d6a61fc2a2a90bdb94c60bb0d9bc9a66154ea471",
  meteor: "f014c05756cb6797ac4b8711ef31232d1cc23d8292b18b47f737c6841bd05eac",
  impact: "a9ec8d9c2d91d4f81873aa6d5cbe7095feb0c6e2d0a071c19c7b3bd4ea5ba708",
};

test("scalar particle writes preserve original bytes for every spell, age and camera", () => {
  for (const kind of ["beam", "lightning", "meteor", "impact"] as const)
    assert.equal(sampleHash(kind), ORIGINAL_SAMPLE_SHA256[kind], kind);
});

test("maximum effects retain original capacity bytes and reuse the same particle slab", () => {
  const effects: GameplayEffect[] = Array.from({ length: 64 }, (_, id) => ({
    id,
    kind: "lightning",
    origin: [id, 100, 0],
    target: [id, 0, 0],
    age: 0.1,
    life: 1,
    radius: 24,
  }));
  const original = structuredClone(effects),
    sampler = new GameplayEffectSampler(),
    sample = sampler.sample(effects, [0, 5, 10], [0, 0, -1]);
  const hash = createHash("sha256")
    .update(
      new Uint8Array(
        sample.data.buffer,
        sample.data.byteOffset,
        sample.data.byteLength,
      ),
    )
    .digest("hex");
  assert.equal(
    hash,
    "591d263a4345ef2fc9081dc30276fdcb7d4b081c54a825d7dfa8b47fd95888a2",
  );
  assert.equal(sample.count, 64 * 240);
  assert(sample.count < GAMEPLAY_VISUAL_LIMITS.vertices);
  assert.equal(sample.data.buffer, sampler.data.buffer);
  const accepted = sample.data.slice();
  assert.deepEqual(
    sampler.sample([...effects, effect("impact", 0.1)], [0, 5, 10], [0, 0, -1])
      .data,
    accepted,
  );
  assert.equal(
    sampler.sample([], [0, 5, 10], [0, 0, -1]).data.buffer,
    sample.data.buffer,
  );
  assert.equal(sampler.sample([], [0, 5, 10], [0, 0, -1]).count, 0);
  assert.equal(sampler.data.length, GAMEPLAY_VISUAL_LIMITS.vertices * 10);
  assert.deepEqual(effects, original);
});

test("transient light revisions track fading and expiry with a stationary camera", () => {
  const initial = gameplayLights([effect("impact", 0.05)]),
    faded = gameplayLights([effect("impact", 0.3)]),
    expired = gameplayLights([effect("impact", 0.45)]);
  assert.notEqual(
    gameplayLightRevision("night", initial),
    gameplayLightRevision("night", faded),
  );
  assert.equal(gameplayLightRevision("night", expired), "night");
  assert.equal(gameplayLightRevision("day", []), "day");
  assert.equal(
    gameplayLightRevision("night", structuredClone(initial)),
    gameplayLightRevision("night", initial),
  );
  const uploads: Uint32Array[] = [],
    savedUsage = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { STORAGE: 1, COPY_DST: 2 },
  });
  const lighting = new ClusteredLighting({
    limits: { maxStorageBufferBindingSize: 1024 * 1024 },
    createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
    queue: {
      writeBuffer(_buffer: unknown, _offset: number, words: Uint32Array) {
        uploads.push(words.slice());
      },
    },
  } as unknown as GPUDevice);
  const input = {
    viewProjection: perspective(1, 1, 0.1, 100),
    eye: [0, 0, 0] as V3,
    forward: [0, 0, -1] as V3,
    width: 128,
    height: 128,
    tileSize: 64,
    depthSlices: 4,
  };
  try {
    for (const lights of [initial, initial, faded, expired, expired])
      lighting.update(
        "main",
        { ...input, lights },
        gameplayLightRevision("night", lights),
      );
    assert.equal(uploads.length, 3);
    assert.equal(uploads[0][CLUSTER_HEADER.lightCount], 1);
    assert.equal(uploads[1][CLUSTER_HEADER.lightCount], 1);
    assert.equal(uploads[2][CLUSTER_HEADER.lightCount], 0);
    const intensity = (words: Uint32Array) =>
      new Float32Array(words.buffer)[words[CLUSTER_HEADER.lightsOffset] + 7];
    assert(intensity(uploads[0]) > intensity(uploads[1]));
    assert(intensity(uploads[1]) > 0);
  } finally {
    lighting.dispose();
    if (savedUsage)
      Object.defineProperty(globalThis, "GPUBufferUsage", savedUsage);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});
