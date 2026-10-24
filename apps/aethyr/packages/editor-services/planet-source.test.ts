import { personalSettingsOnly } from "./planet-source.ts";
import { checkPlanetContent, planetCameraAllowed } from "./planet-source.ts";
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  PlanetSources,
  checkPlanetRecipe,
  planetHour,
  planetEnvironment,
  PLANET_SOURCE,
} from "./planet-source.ts";
import { materialHash } from "./creator-material.ts";
import { DEFAULT_ENVIRONMENT } from "./protocol.ts";
const recipe = () => ({
  schema: "openagents.verse.planet.v1",
  seed: 42,
  side_metres: 64,
  spawn: [0, 1.62, 12],
  style: "soft-pastels",
  initial_hour: 8,
  cycle_enabled: true,
  day_minutes: 2,
  clock_anchor_unix_ms: 1000,
});
async function doc(value: any = recipe(), pretty = false) {
  const text = JSON.stringify(value, null, pretty ? 2 : undefined);
  return {
    namespace: "personal-planet",
    rooms: {},
    removed_walls: [],
    entities: {},
    originals: [
      {
        name: PLANET_SOURCE,
        text,
        sha256: await materialHash(new TextEncoder().encode(text)),
      },
    ],
  } as any;
}
test("clock advances, wraps, and pauses without changing source settings", () => {
  const r = checkPlanetRecipe(recipe());
  assert.equal(planetHour(r, 31000), 14);
  assert.equal(planetHour(r, 121000), 8);
  const initial = structuredClone(r);
  const day = planetEnvironment(r, DEFAULT_ENVIRONMENT, 1000),
    night = planetEnvironment(
      { ...r, initial_hour: 22 },
      DEFAULT_ENVIRONMENT,
      1000,
    );
  assert.equal(day.mode, "day");
  assert.equal(night.mode, "night");
  assert.ok(night.ambientIntensity >= 0.65);
  assert.notEqual(day.sunElevation, night.sunElevation);
  assert.deepEqual(r, initial);
  assert.equal(planetHour({ ...r, cycle_enabled: false }, 31000), 8);
  assert.throws(() => checkPlanetRecipe({ ...recipe(), seed: -1 }));
  assert.throws(() => checkPlanetRecipe({ ...recipe(), extra: true }));
});
test("pretty sources keep exact text and history chooses the accepted recipe", async () => {
  const sources = new PlanetSources();
  const a = await doc(recipe(), true),
    b = await doc({ ...recipe(), style: "bold-fantasy", initial_hour: 22 });
  const sa = await sources.accept(a),
    sb = await sources.accept(b);
  assert.deepEqual(sa, a.originals[0]);
  assert.equal(
    sources.current({ authoring: { planetSource: sa } })?.initial_hour,
    8,
  );
  assert.equal(
    sources.current({ authoring: { planetSource: sb } })?.initial_hour,
    22,
  );
  assert.equal(
    sources.current({ authoring: { planetSource: sa } })?.initial_hour,
    8,
    "Undo uses its own source",
  );
  const bad = structuredClone(b);
  bad.originals[0].text += " ";
  await assert.rejects(() => sources.accept(bad));
  assert.equal(
    sources.current({ authoring: { planetSource: sa } })?.initial_hour,
    8,
    "Failed import leaves the working clock alone",
  );
  const outside = await doc();
  outside.entities.x = { position: [32, 0, 0], scale: 1 };
  await assert.rejects(() => sources.accept(outside));
});

test("courtyard edits and saved cameras stay inside the finite walkable world", () => {
  const world = {
    entities: [{ position: [0, 0, 12], scale: 1 }],
    rooms: [],
    removedWalls: [],
  };
  checkPlanetContent(world);
  for (const position of [
    [29, 0, 0],
    [0, 13, 0],
    [0, 0, -29],
    [NaN, 0, 0],
  ])
    assert.throws(() =>
      checkPlanetContent({ ...world, entities: [{ position, scale: 1 }] }),
    );
  assert.throws(() =>
    checkPlanetContent({
      ...world,
      entities: Array.from({ length: 129 }, () => world.entities[0]),
    }),
  );
  assert.equal(planetCameraAllowed({ eye: [0, 1.62, 12], mode: "walk" }), true);
  assert.equal(
    planetCameraAllowed({ eye: [32, 1.62, 12], mode: "walk" }),
    false,
  );
  assert.equal(planetCameraAllowed({ eye: [0, 1.62, 12], mode: "fly" }), false);
});

test("settings updates preserve local history only for unchanged content and layout identity", async () => {
  const before = {
    ...(await doc(recipe())),
    source_revision: 4,
    materials: {},
    layers: [],
  };
  const after = {
    ...before,
    ...(await doc({ ...recipe(), style: "bold-fantasy", initial_hour: 22 })),
    source_revision: 4,
  };
  assert.equal(personalSettingsOnly(before, after), true);
  assert.equal(
    personalSettingsOnly(before, { ...after, source_revision: 3 }),
    false,
  );
  assert.equal(
    personalSettingsOnly(before, {
      ...after,
      entities: { new: { asset: "barrel", position: [0, 0, 0] } },
    }),
    false,
  );
  const other = {
    ...after,
    ...(await doc({ ...recipe(), seed: 999 })),
    source_revision: 4,
  };
  assert.equal(personalSettingsOnly(before, other), false);
});
