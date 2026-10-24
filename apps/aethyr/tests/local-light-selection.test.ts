import assert from "node:assert/strict";
import test from "node:test";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import { LocalLightSelection } from "../packages/renderer/local-light-selection.ts";

function light(
  id: string,
  position = [0, 0, 0],
  range: number | undefined = 50,
): SourceLight {
  return {
    id,
    label: id,
    position,
    range,
    kind: "point",
    direction: [0, -1, 0],
    color: [1, 0.5, 0.2],
    intensity: 1,
    intensityUnits: "candela",
    castShadows: true,
    enabled: true,
    temperatureKelvin: 6500,
    useTemperature: false,
    raw: {},
  };
}
function reference(
  lights: readonly SourceLight[] | undefined,
  eye: readonly number[],
): SourceLight[] {
  return (lights ?? [])
    .filter((light) => light.enabled && light.range && light.range > 0)
    .map((light) => ({
      light,
      distance: Math.hypot(
        ...[
          light.position[0] - eye[0],
          light.position[1] - eye[1],
          light.position[2] - eye[2],
        ],
      ),
    }))
    .filter(({ light, distance }) => distance < (light.range ?? 0) + 45)
    .sort((a, b) => a.distance - b.distance)
    .slice(0, 32)
    .map(({ light }) => light);
}
function exact(actual: SourceLight[], expected: SourceLight[]) {
  assert.equal(actual.length, expected.length);
  for (let i = 0; i < actual.length; i++) assert.equal(actual[i], expected[i]);
}

test("nearest light selection matches the original pipeline across moving cameras and transient arrays", () => {
  let seed = 0x5eaf00d;
  const random = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) | 0;
    return (seed >>> 0) / 2 ** 32;
  };
  const source = Array.from({ length: 488 }, (_, index) =>
    light(
      String(index),
      [random() * 240 - 120, random() * 30 - 5, random() * 240 - 120],
      random() * 80,
    ),
  );
  const selector = new LocalLightSelection();
  for (let frame = 0; frame < 200; frame++) {
    const eye = [random() * 200 - 100, random() * 25, random() * 200 - 100];
    const lights =
      frame % 2
        ? [...source, light("spell", [eye[0] + 1, eye[1], eye[2]], 2)]
        : source;
    exact(selector.select(lights, eye), reference(lights, eye));
  }
});

test("equal-distance ties preserve original order even at the 32-light boundary", () => {
  const source = Array.from({ length: 80 }, (_, i) =>
    light(String(i), [i % 2 ? -1 : 1, 0, 0]),
  );
  const selector = new LocalLightSelection();
  exact(selector.select(source, [0, 0, 0]), source.slice(0, 32));
  source.unshift(light("near", [0, 0, 0]));
  exact(selector.select(source, [0, 0, 0]), source.slice(0, 32));
  source.reverse();
  exact(selector.select(source, [0, 0, 0]), reference(source, [0, 0, 0]));
});

test("eligibility and exact range thresholds retain legacy edge behavior", () => {
  const source = [
    light("threshold", [46, 0, 0], 1),
    light("inside", [46 - Number.EPSILON * 32, 0, 0], 1),
    light("outside", [46 + Number.EPSILON * 32, 0, 0], 1),
    light("zero", [0, 0, 0], 0),
    light("negative-zero", [0, 0, 0], -0),
    light("negative", [0, 0, 0], -1),
    light("nan-range", [0, 0, 0], NaN),
    light("infinite-range", [1e200, 0, 0], Infinity),
    light("nan-position", [NaN, 0, 0]),
    light("infinite-position", [Infinity, 0, 0], Infinity),
    light("missing-position", []),
    light("missing-range"),
    { ...light("disabled"), enabled: false },
  ];
  source.find((item) => item.id === "missing-range")!.range = undefined;
  const selector = new LocalLightSelection();
  for (const eye of [
    [0, 0, 0],
    [-0, 0, -0],
    [NaN, 0, 0],
    [Infinity, 0, 0],
    [1e308, -1e308, 0],
  ])
    exact(selector.select(source, eye), reference(source, eye));
  assert.deepEqual(
    selector.select(source, [0, 0, 0]).map((item) => item.id),
    ["inside", "infinite-range"],
  );
});

test("repeated calls reuse the output and candidate records and clear obsolete references", () => {
  const selector = new LocalLightSelection();
  const source = Array.from({ length: 64 }, (_, i) =>
    light(String(i), [64 - i, 0, 0]),
  );
  const records = (selector as any).records as {
    light: SourceLight | undefined;
  }[];
  const originalRecords = new Set(records);
  const first = selector.select(source, [0, 0, 0]);
  const selected = first[0];
  selected.position[0] = 1000;
  const second = selector.select(source, [0, 0, 0]);
  assert.equal(first, second);
  exact(second, reference(source, [0, 0, 0]));
  const empty = selector.select(undefined, [0, 0, 0]);
  assert.equal(empty, first);
  assert.equal(empty.length, 0);
  assert.equal((selector as any).records, records);
  assert.equal(new Set(records).size, 32);
  assert.ok(
    records.every(
      (record) => originalRecords.has(record) && record.light === undefined,
    ),
  );
  exact(
    selector.select(source.slice(0, 2), [0, 0, 0]),
    reference(source.slice(0, 2), [0, 0, 0]),
  );
  assert.ok(records.slice(2).every((record) => record.light === undefined));
});
