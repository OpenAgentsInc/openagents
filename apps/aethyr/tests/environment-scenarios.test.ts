import assert from "node:assert/strict";
import { test } from "node:test";
import { mergeEnvironmentScenario } from "../packages/assets/src/environment.ts";
import type {
  CompiledEnvironments,
  EnvironmentMap,
  MeasuredFogAmbientRecipe,
} from "../packages/assets/src/scene.ts";

test("separate lighting files retain each scenario's matching atmosphere and fog measurements", () => {
  const dayMap = { url: "/private-assets/day.bin" } as EnvironmentMap,
    nightMap = { url: "/private-assets/night.bin" } as EnvironmentMap;
  const dayFog = { scenario: "day" } as MeasuredFogAmbientRecipe,
    nightFog = { scenario: "night" } as MeasuredFogAmbientRecipe;
  const day: CompiledEnvironments = {
    schema: "aethyrforge.private-environments.v1",
    scenarios: { day: dayMap },
    atmosphere: { day: { id: "day" }, night: { id: "unselected" } },
    fogAmbient: { day: dayFog },
  };
  const night: CompiledEnvironments = {
    schema: day.schema,
    scenarios: { night: nightMap },
    atmosphere: { night: { id: "night" } },
    fogAmbient: { night: nightFog },
  };
  const first = mergeEnvironmentScenario(undefined, day, "day");
  assert.equal(first.atmosphere?.night, undefined);
  const merged = mergeEnvironmentScenario(first, night, "night");
  assert.deepEqual(merged.scenarios, { day: dayMap, night: nightMap });
  assert.deepEqual(merged.atmosphere, {
    day: { id: "day" },
    night: { id: "night" },
  });
  assert.deepEqual(merged.fogAmbient, { day: dayFog, night: nightFog });
  assert.notEqual(merged.fogAmbient, day.fogAmbient);
  assert.equal(first.scenarios.night, undefined);
  const without = mergeEnvironmentScenario(
    merged,
    { schema: day.schema, scenarios: { day: dayMap } },
    "day",
  );
  assert.equal(without.fogAmbient?.day, undefined);
  assert.equal(without.atmosphere?.day, undefined);
  assert.equal(without.fogAmbient?.night, nightFog);
  assert.equal(merged.fogAmbient?.day, dayFog);
  assert.throws(() => mergeEnvironmentScenario(undefined, day, "night"));
});
