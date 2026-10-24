import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import {
  CharacterAnimator,
  characterPalette,
  sampleCharacterLayers,
  type CharacterAnimationLayer,
  type CharacterClip,
  type CharacterManifest,
} from "../packages/renderer/character.ts";
import { MmorpgAnimationClock } from "../packages/runtime/mmorpg-animation.ts";
import type { QuickMmorpgMotion } from "../packages/runtime/quick-mmorpg-controller.ts";

const durations = {
  idle: 2.5666666667,
  walk: 1.1333333333,
  run: 0.7333333333,
  attack: 1.5333333333,
  dance: 4.8,
  death: 2.2333333333,
};
const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
const f32 = (values: number[]) => Array.from(new Float32Array(values));
function rotation(x: number, y: number, z: number) {
  const sx = Math.sin(x / 2),
    sy = Math.sin(y / 2),
    sz = Math.sin(z / 2),
    cx = Math.cos(x / 2),
    cy = Math.cos(y / 2),
    cz = Math.cos(z / 2);
  return [
    sx * cy * cz + cx * sy * sz,
    cx * sy * cz - sx * cy * sz,
    cx * cy * sz + sx * sy * cz,
    cx * cy * cz - sx * sy * sz,
  ];
}
function manifest(npc = false): CharacterManifest {
  const file = (byteLength: number) => ({
    url: "/private-assets/character-test.bin",
    sha256: "0".repeat(64),
    byteLength,
  });
  const clips = Object.entries(durations)
    .map(([name, duration], i) => {
      const times = [0, duration / 2, duration];
      return {
        name,
        duration,
        distance: 0,
        loop: !["attack", "dance", "death"].includes(name),
        tracks: [
          {
            joint: 0,
            translation:
              name === "dance"
                ? []
                : times.map((time, k) => ({
                    time: Math.fround(time),
                    value: f32([
                      i * 0.4 + k * 0.2,
                      0.1 * i - k * 0.05,
                      k * 0.1 - i * 0.2,
                    ]),
                  })),
            rotation: times.map((time, k) => ({
              time: Math.fround(time),
              value: f32(
                rotation(
                  0.17 * i + 0.13 * k,
                  -0.11 * i + 0.3 * k,
                  0.09 * i - 0.14 * k,
                ),
              ),
            })),
            scale:
              name === "attack"
                ? []
                : times.map((time, k) => ({
                    time: Math.fround(time),
                    value: f32([
                      1 + 0.04 * i + 0.02 * k,
                      1 - 0.03 * i + 0.03 * k,
                      1 + 0.01 * i - 0.01 * k,
                    ]),
                  })),
          },
        ],
      } as CharacterClip;
    })
    .filter((clip) => !npc || !["run", "dance"].includes(clip.name));
  return {
    schema: "aethyrforge.character.v1",
    name: "synthetic mixer fixture",
    ...(npc
      ? { animationProfile: "quick-mmorpg-npc-v1" as const }
      : {
          controller: {
            profile: "quick-mmorpg-v1" as const,
            worldScale: 1 / 6,
          },
        }),
    geometry: {
      vertexCount: 3,
      indexCount: 3,
      vertices: file(192),
      indices: file(12),
    },
    joints: [
      {
        parent: -1,
        inverseBind: identity,
        translation: [0.2, -0.1, 0.4],
        rotation: rotation(0.13, -0.2, 0.11),
        scale: [1.1, 0.9, 1.2],
      },
    ],
    clips,
    materials: [
      {
        name: "fixture",
        color: [1, 1, 1, 1],
        alphaCutoff: 0,
        doubleSided: true,
      },
    ],
    draws: [{ material: 0, firstIndex: 0, indexCount: 3 }],
  };
}
interface Row {
  frame: number;
  layers: CharacterAnimationLayer[];
  position: number[];
  rotation: number[];
  scale: number[];
}
interface Recipe {
  name: string;
  initial: QuickMmorpgMotion;
  transitions: Record<number, QuickMmorpgMotion>;
  npc?: boolean;
  rows: Row[];
}
// Unmodified Three r124, source Enter methods, synthetic tracks only. The reference
// hash and source revision travel with the fixture; no models or Three dependency.
const oracle = JSON.parse(
  fs.readFileSync(
    new URL("./fixtures/mmorpg-animation-interrupted.json", import.meta.url),
    "utf8",
  ),
) as { cases: Recipe[] };
function near(
  actual: ArrayLike<number>,
  expected: ArrayLike<number>,
  tolerance = 2e-6,
) {
  assert.equal(actual.length, expected.length);
  for (let i = 0; i < actual.length; i++)
    assert(
      Math.abs(actual[i] - expected[i]) < tolerance,
      `${i}: ${actual[i]} != ${expected[i]}`,
    );
}
test("interrupted source fades retain all action weights and Three joint transforms", () => {
  let largest = 0;
  for (const recipe of oracle.cases) {
    const prepared = manifest(recipe.npc),
      clock = new MmorpgAnimationClock(prepared),
      animator = new CharacterAnimator(prepared);
    let state = recipe.initial,
      entered = 0;
    clock.sample({ state, actionTime: 0.37, animationTime: 0.37 }, 0, 0);
    for (let frame = 1; frame <= 72; frame++) {
      if (recipe.transitions[frame]) {
        state = recipe.transitions[frame];
        entered = frame;
      }
      const acceptedTime = (frame - entered + 1) / 60;
      const pose = clock.sample(
        { state, actionTime: acceptedTime, animationTime: acceptedTime },
        1 / 60,
        frame / 60,
      );
      const layers = pose.blend?.layers ?? [
        { animation: pose.animation, time: pose.time, weight: 1 },
      ];
      assert(layers.length <= 6);
      largest = Math.max(largest, layers.length);
      const expected = recipe.rows.find((row) => row.frame === frame);
      if (!expected) continue;
      assert.deepEqual(
        layers.map((layer) => layer.animation),
        expected.layers.map((layer) => layer.animation),
        recipe.name,
      );
      near(
        layers.flatMap((layer) => [layer.time, layer.weight]),
        expected.layers.flatMap((layer) => [layer.time, layer.weight]),
        1e-7,
      );
      const actual = sampleCharacterLayers(prepared, layers)[0];
      near(actual.translation, expected.position);
      near(actual.rotation, expected.rotation);
      near(actual.scale, expected.scale);
      const expectedPalette = characterPalette(prepared, [
        {
          translation: expected.position,
          rotation: expected.rotation,
          scale: expected.scale,
        },
      ]);
      near(
        animator.sample(pose.animation, pose.time, frame / 60, pose.blend),
        expectedPalette,
      );
    }
  }
  assert.equal(largest, 6);
});

test("multi-action sampling preserves sparse channels and bind-pose residual weight", () => {
  const prepared = manifest(),
    rest = prepared.joints[0];
  const attack = prepared.clips.find((clip) => clip.name === "attack")!;
  attack.tracks[0].translation = [{ time: 0, value: [4, 2, -1] }];
  const pose = sampleCharacterLayers(prepared, [
    { animation: "attack", time: 0, weight: 0.25 },
    { animation: "dance", time: 0, weight: 0.25 },
  ])[0];
  // Dance has no translation track; it cannot add its rest translation to the mixer.
  near(
    pose.translation,
    [4, 2, -1].map((v, i) => v * 0.25 + rest.translation[i] * 0.75),
  );
  // Attack has no scale track; the remaining weight comes from the original joint.
  near(
    pose.scale,
    prepared.clips
      .find((clip) => clip.name === "dance")!
      .tracks[0].scale[0].value.map((v, i) => v * 0.25 + rest.scale[i] * 0.75),
  );
  assert.deepEqual(sampleCharacterLayers(prepared, []), [
    {
      translation: rest.translation,
      rotation: rest.rotation,
      scale: rest.scale,
    },
  ]);
});

test("action reuse remains bounded during repeated interruptions and resets on capture rewind", () => {
  const prepared = manifest(),
    clock = new MmorpgAnimationClock(prepared),
    states = Object.keys(durations) as QuickMmorpgMotion[];
  clock.sample({ state: "idle", actionTime: 0, animationTime: 0 }, 0, 0);
  for (let frame = 1; frame <= 1200; frame++) {
    const state = states[frame % states.length],
      pose = clock.sample(
        { state, actionTime: 1 / 60, animationTime: 1 / 60 },
        1 / 60,
        frame / 60,
      );
    const layers = pose.blend?.layers ?? [];
    assert(layers.length <= 6);
    assert.equal(
      new Set(layers.map((layer) => layer.animation)).size,
      layers.length,
    );
    assert(
      layers.every(
        (layer) => Number.isFinite(layer.time) && Number.isFinite(layer.weight),
      ),
    );
  }
  assert.deepEqual(
    clock.sample(
      { state: "walk", actionTime: 0.2, animationTime: 0.2 },
      1 / 60,
      -1,
    ),
    { animation: "walk", time: 0.2 },
  );
});

test("multi-action blending rejects duplicate, unbounded, and invalid contributions", () => {
  const prepared = manifest(),
    good = { animation: "idle" as const, time: 0, weight: 0.5 };
  for (const layers of [
    [good, good],
    Array(7).fill(good),
    [{ ...good, time: NaN }],
    [{ ...good, weight: -0.1 }],
    [{ ...good, weight: 1.1 }],
  ])
    assert.throws(() => sampleCharacterLayers(prepared, layers));
});
