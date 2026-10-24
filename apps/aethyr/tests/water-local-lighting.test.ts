import { test } from "node:test";
import assert from "node:assert/strict";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import { packWaterLocalLights } from "../packages/renderer/water-local-lighting.ts";
import {
  lightAttenuation,
  lightColor,
  lightIntensity,
} from "../packages/renderer/lighting.ts";
import { SCENE_LINEAR_SCALE } from "../packages/renderer/radiometry.ts";

// Retain the former allocation-heavy calculations as a finite-input reference.
function referencePack(
  lights: readonly SourceLight[],
  anchors: readonly (readonly number[])[],
) {
  const candidates = lights
    .filter((x) => x.enabled && x.kind !== "directional")
    .map((light) => {
      const intensity = lightIntensity(light),
        color = lightColor(light),
        distance = Math.min(
          ...anchors.map((p) =>
            Math.hypot(...p.map((v, i) => v - light.position[i])),
          ),
        );
      return {
        light,
        intensity,
        color,
        score:
          lightAttenuation(light, distance) * intensity * Math.max(...color),
      };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score || a.light.id.localeCompare(b.light.id))
    .slice(0, 32);
  const positions = new Float32Array(128),
    colors = new Float32Array(128),
    directions = new Float32Array(128),
    cones = new Float32Array(128);
  for (let i = 0; i < candidates.length; i++) {
    const { light, intensity, color } = candidates[i],
      at = i * 4;
    positions.set([...light.position, light.range ?? 10], at);
    colors.set(
      [...color.map((v) => v * intensity * SCENE_LINEAR_SCALE), 0],
      at,
    );
    if (light.kind === "spot") {
      const d = light.direction,
        length = Math.hypot(...d);
      directions.set([...d.map((v) => v / length), 1], at);
      cones.set(
        [
          Math.cos(light.innerConeAngleRadians ?? 0),
          Math.cos(light.outerConeAngleRadians ?? Math.PI / 4),
          0,
          0,
        ],
        at,
      );
    }
    cones[at + 3] =
      light.inverseSquaredFalloff === false ? (light.falloffExponent ?? 8) : -1;
  }
  return {
    count: candidates.length,
    ids: candidates.map((x) => x.light.id),
    positions,
    colors,
    directions,
    cones,
  };
}

function assertPackedEqual(
  lights: readonly SourceLight[],
  anchors: readonly (readonly number[])[],
) {
  const packed = packWaterLocalLights(lights, anchors),
    reference = referencePack(lights, anchors);
  assert.deepEqual(packed.ids, reference.ids);
  assert.equal(packed.count, reference.count);
  for (const name of ["positions", "colors", "directions", "cones"] as const)
    assert.deepEqual(
      new Uint8Array(packed[name].buffer),
      new Uint8Array(reference[name].buffer),
      name,
    );
  return packed;
}
function light(id: string): SourceLight {
  return {
    id,
    label: id,
    kind: "point",
    position: [0, 0, 1],
    direction: [0, 0, -1],
    color: [1, 0.5, 0.25],
    intensity: 100,
    intensityUnits: "candelas",
    inverseSquaredFalloff: true,
    range: 10,
    castShadows: true,
    enabled: true,
    temperatureKelvin: 6500,
    useTemperature: false,
    raw: {},
  };
}
test("water local highlights preserve physical and legacy units, cones and one storage scale", () => {
  const source = light("point"),
    spot = {
      ...light("spot"),
      kind: "spot" as const,
      intensityUnits: "lumens",
      intensity: 2 * Math.PI * (1 - Math.cos(Math.PI / 4)) * 100,
      innerConeAngleRadians: 0.2,
      outerConeAngleRadians: Math.PI / 4,
    };
  const legacy = {
    ...light("legacy"),
    intensityUnits: "unitless",
    inverseSquaredFalloff: false,
    falloffExponent: 4,
  };
  const packed = packWaterLocalLights([source, spot, legacy], [[0, 0, 0]]);
  for (const id of ["point", "spot", "legacy"]) {
    const at = packed.ids.indexOf(id) * 4;
    assert.equal(packed.colors[at], 1);
    assert.equal(packed.colors[at + 1], 0.5);
  }
  assert.equal(packed.cones[packed.ids.indexOf("legacy") * 4 + 3], 4);
  assert.equal(packed.directions[packed.ids.indexOf("spot") * 4 + 3], 1);
  assert.deepEqual(source, light("point"));
});
test("water light selection is stable, capped and excludes disabled/out-of-range sources", () => {
  const lights = Array.from({ length: 488 }, (_, i) => ({
    ...light(String(i).padStart(3, "0")),
    position: [0, 0, 1 + i * 0.1],
  }));
  const p = packWaterLocalLights(lights, [[0, 0, 0]]);
  assert.equal(p.count, 32);
  assert.deepEqual(
    p.ids,
    packWaterLocalLights([...lights].reverse(), [[0, 0, 0]]).ids,
  );
  assert.equal(
    packWaterLocalLights(
      [
        { ...light("off"), enabled: false },
        { ...light("far"), position: [0, 0, 100] },
      ],
      [[0, 0, 0]],
    ).count,
    0,
  );
  assert.throws(() =>
    packWaterLocalLights(
      [{ ...light("bad"), intensity: Infinity }],
      [[0, 0, 0]],
    ),
  );
});

test("scalar water-light calculations retain every packed byte across finite cases", () => {
  let state = 0xface;
  const random = () =>
    ((state = Math.imul(state, 1664525) + 1013904223) >>> 0) / 2 ** 32;
  for (let trial = 0; trial < 80; trial++) {
    const anchors = Array.from({ length: 1 + (trial % 64) }, () =>
      Array.from({ length: 3 }, () => (random() - 0.5) * 100),
    );
    const lights = Array.from({ length: 40 + (trial % 64) }, (_, i) => ({
      ...light(String(i)),
      kind: i % 3 ? "point" : "spot",
      enabled: i % 11 !== 0,
      position: Array.from({ length: 3 }, () => (random() - 0.5) * 100),
      direction: [random() - 0.5, random() - 0.5, random() - 0.5],
      color: [random(), random(), random()],
      intensity: random() * 100,
      intensityUnits: i % 2 ? "candelas" : "lumens",
      inverseSquaredFalloff: i % 4 !== 0,
      falloffExponent: random() * 16,
      range: 1 + random() * 100,
      useTemperature: i % 5 === 0,
      temperatureKelvin: 1000 + random() * 11000,
      innerConeAngleRadians: random() * 0.2,
      outerConeAngleRadians: 0.5 + random(),
    })) as SourceLight[];
    const before = structuredClone({ anchors, lights });
    assertPackedEqual(lights, anchors);
    assert.deepEqual({ anchors, lights }, before);
  }
});

test("water-light ties, zero distance, boundaries and unused slots stay byte-identical", () => {
  const ties = Array.from({ length: 1024 }, (_, i) =>
    light(String(i).padStart(4, "0")),
  );
  assert.equal(
    assertPackedEqual(ties, [
      [0, 0, 1],
      [1e6, -1e6, 1e6],
    ]).count,
    32,
  );
  assert.deepEqual(
    assertPackedEqual(ties, [[0, 0, 1]]).ids,
    ties.slice(0, 32).map((x) => x.id),
  );
  assert.equal(assertPackedEqual([], [[-0, 0, 0]]).count, 0);
  const spot = {
    ...light("spot"),
    kind: "spot" as const,
    direction: [-0, 3, 4],
    inverseSquaredFalloff: false,
    falloffExponent: 0,
    innerConeAngleRadians: 0,
    outerConeAngleRadians: Math.PI / 2,
  };
  const packed = assertPackedEqual(
    [spot, { ...light("boundary"), position: [0, 0, 10] }],
    [[0, 0, 0]],
  );
  assert.deepEqual(packed.ids, ["spot"]);
  assert.equal(packed.directions[1], Math.fround(3 / 5));
  assert.equal(packed.directions[2], Math.fround(4 / 5));
  assert.ok(Object.is(packed.directions[0], -0));
  assert.equal(packed.cones[3], 0);
  for (const field of ["positions", "colors", "directions", "cones"] as const)
    assert.ok(packed[field].subarray(4).every((x) => x === 0));
});

test("scalar water-light path keeps bounded-input validation", () => {
  for (const anchors of [[], [[0, Infinity, 0]], [[0, 0]], [[0, 0, 1e6 + 1]]])
    assert.throws(() => packWaterLocalLights([light("a")], anchors));
  for (const field of [
    { position: [0, NaN, 0] },
    { color: [1, -1, 0] },
    { range: 1e5 + 1 },
    { intensity: 1e9 + 1 },
    { kind: "spot", direction: [0, 0, 0] },
    { kind: "spot", direction: [0, 1] },
    { kind: "spot", innerConeAngleRadians: 1, outerConeAngleRadians: 0.5 },
    { inverseSquaredFalloff: false, falloffExponent: 1025 },
  ])
    assert.throws(() =>
      packWaterLocalLights(
        [{ ...light("a"), ...field } as SourceLight],
        [[0, 0, 0]],
      ),
    );
});
