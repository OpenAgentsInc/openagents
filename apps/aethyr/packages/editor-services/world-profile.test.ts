import { test } from "node:test";
import assert from "node:assert/strict";
import {
  selectWorldProfile,
  parseOverviewPresentation,
  profileSupportsShot,
} from "./world-profile";
import { DEFAULT_ENVIRONMENT } from "./protocol";

test("open ground has an independent walking scene without town shots", () => {
  const ground = selectWorldProfile("open-ground");
  const town = selectWorldProfile("medieval-town");
  assert.equal(ground.id, "open-ground");
  assert.equal(ground.mode, "walk");
  assert.notEqual(ground.database, town.database);
  assert.notEqual(ground.key, town.key);
  assert.notEqual(ground.scene, town.scene);
  assert.equal(profileSupportsShot(ground, "MT-02"), false);
  assert.equal(profileSupportsShot(ground, "MT-04"), false);
});

test("house and town have independent resources, saves, and starting modes", () => {
  const house = selectWorldProfile("haunted-house"),
    town = selectWorldProfile(undefined);
  assert.equal(house.id, "haunted-house");
  assert.equal(house.mode, "walk");
  assert.equal(house.groundY, 0);
  assert.notEqual(house.database, town.database);
  assert.notEqual(house.key, town.key);
  assert.notEqual(house.scene, town.scene);
  assert.equal(town.id, "medieval-town-lab");
  assert.equal(town.mode, "walk");
  assert.throws(() => selectWorldProfile("https://untrusted.example/world"));
});

test("overview is a deployment-selected day-only stage with isolated resources", () => {
  const overview = selectWorldProfile("medieval-overview"),
    town = selectWorldProfile("medieval-town"),
    house = selectWorldProfile("haunted-house");
  assert.equal(overview.readOnly, true);
  assert.equal(town.readOnly, false);
  assert.equal(house.readOnly, false);
  assert.equal(overview.scene, overview.nightScene);
  assert.equal(
    overview.presentationURL,
    "/private-assets/overview-presentation.json",
  );
  assert.notEqual(overview.database, town.database);
  assert.notEqual(overview.key, town.key);
  assert.equal(profileSupportsShot(overview, "MT-04"), true);
  assert.equal(profileSupportsShot(overview, "MT-02"), false);
  assert.equal(profileSupportsShot(town, "MT-04"), false);
  assert.equal(profileSupportsShot(town, "MT-02"), true);
});

const presentation = () => ({
  schema: "aethyrforge.owned-overview-presentation.v1",
  camera: {
    position: [-5, 20, 10],
    target: [0, 0, 0],
    up: [0, 1, 0],
    verticalFovDegrees: 57,
  },
  lighting: {
    kind: "owned-overview-daylight-v1",
    sourceOverviewLightingRecovered: false,
    environment: {
      ...DEFAULT_ENVIRONMENT,
      mode: "day",
      skySource: "kit",
      fogDensity: 0,
      exposure: 4,
    },
  },
});
test("overview presentation admits bounded camera and daylight data without origin controls", () => {
  const value = presentation(),
    result = parseOverviewPresentation(value);
  assert.equal(result.camera.verticalFovDegrees, 57);
  assert.equal(result.environment.exposure, 4);
  value.camera.position[0] = 999;
  assert.equal(result.camera.position[0], -5);
});
test("overview rejects incomplete, nonfinite, degenerate or incompatible presentation", () => {
  for (const mutate of [
    (v: any) => (v.schema = "unknown"),
    (v: any) => (v.camera.position[0] = NaN),
    (v: any) => (v.camera.up = [0, 0, 0]),
    (v: any) => (v.camera.up = [5, -20, -10]),
    (v: any) => (v.camera.target = v.camera.position),
    (v: any) => (v.camera.verticalFovDegrees = Infinity),
    (v: any) => (v.lighting.environment.mode = "night"),
    (v: any) => (v.lighting.environment.sunIntensity = 1000),
    (v: any) => delete v.lighting.environment,
  ]) {
    const value = presentation();
    mutate(value);
    assert.throws(() => parseOverviewPresentation(value));
  }
  for (const value of [null, [], {}, "https://untrusted.invalid"])
    assert.throws(() => parseOverviewPresentation(value));
});

test("personal courtyard keeps an independent walkable profile", () => {
  const personal = selectWorldProfile("personal-planet"),
    house = selectWorldProfile("haunted-house");
  assert.equal(personal.mode, "walk");
  assert.equal(personal.readOnly, false);
  assert.notEqual(personal.database, house.database);
  assert.notEqual(personal.scene, house.scene);
  assert.equal(personal.presentationURL, undefined);
});
