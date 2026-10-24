import assert from "node:assert/strict";
import { test } from "node:test";
import { EnvironmentProfiles } from "./environment.ts";
import { DEFAULT_ENVIRONMENT } from "./protocol.ts";

test("source kit defaults do not change generated sky brightness", () => {
  const profiles = new EnvironmentProfiles();
  profiles.configureSourceIntensity({ day: 3, night: 2 });
  assert.equal(
    profiles.initial({ ...DEFAULT_ENVIRONMENT, skySource: "kit" })
      .ambientIntensity,
    3,
  );
  assert.equal(
    profiles.initial({
      ...DEFAULT_ENVIRONMENT,
      skySource: "kit",
      mode: "night",
    }).ambientIntensity,
    2,
  );
  assert.equal(profiles.initial(DEFAULT_ENVIRONMENT).ambientIntensity, 0.3);
  assert.throws(() => profiles.configureSourceIntensity({ day: 9 }));
  assert.equal(profiles.sourceIntensity("day"), 3);
});
test("UI source changes restore each accepted fill while direct API values remain explicit", () => {
  const profiles = new EnvironmentProfiles();
  profiles.configureSourceIntensity({ day: 3, night: 3 });
  let current = profiles.initial({ ...DEFAULT_ENVIRONMENT, skySource: "kit" });
  current = profiles.apply(current, { ...current, ambientIntensity: 4 });
  current = profiles.apply(
    current,
    { ...current, skySource: "generated" },
    true,
  );
  assert.equal(current.ambientIntensity, 0.3);
  current = profiles.apply(current, { ...current, ambientIntensity: 0.8 });
  current = profiles.apply(current, { ...current, skySource: "kit" }, true);
  assert.equal(current.ambientIntensity, 4);
  current = profiles.apply(current, {
    ...current,
    skySource: "generated",
    ambientIntensity: 4,
  });
  assert.equal(
    current.ambientIntensity,
    4,
    "An explicit API value must not be replaced just because it equals the prior fill.",
  );
  current = profiles.apply(current, { ...current, skySource: "kit" }, true);
  current = profiles.apply(
    current,
    { ...current, skySource: "generated" },
    true,
  );
  assert.equal(current.ambientIntensity, 4);
});
test("camera mode profiles preserve independent day and night fill edits", () => {
  const profiles = new EnvironmentProfiles();
  profiles.configureSourceIntensity({ day: 3, night: 2 });
  let current = profiles.initial({ ...DEFAULT_ENVIRONMENT, skySource: "kit" });
  current = profiles.apply(current, { ...current, ambientIntensity: 5 });
  current = profiles.apply(current, { ...current, mode: "night" }, true);
  assert.equal(current.ambientIntensity, 2);
  current = profiles.apply(current, { ...current, ambientIntensity: 1 });
  current = profiles.apply(current, { ...current, mode: "day" }, true);
  assert.equal(current.ambientIntensity, 5);
  assert.equal(profiles.sourceIntensity("night"), 2);
});

test("kit glow defaults and artist edits stay independent by mode and sky source", () => {
  const profiles = new EnvironmentProfiles();
  profiles.configureSourceBloom({ day: 0, night: 0.12 });
  assert.throws(() => profiles.configureSourceBloom({ day: NaN, night: 0 }));
  let current = profiles.initial({ ...DEFAULT_ENVIRONMENT, skySource: "kit" });
  assert.equal(current.bloomStrength, 0);
  current = profiles.apply(current, { ...current, bloomStrength: 0.3 });
  current = profiles.apply(current, { ...current, mode: "night" }, true);
  assert.equal(current.bloomStrength, 0.12);
  current = profiles.apply(current, { ...current, mode: "day" }, true);
  assert.equal(current.bloomStrength, 0.3);
  current = profiles.apply(
    current,
    { ...current, skySource: "generated" },
    true,
  );
  assert.equal(current.bloomStrength, DEFAULT_ENVIRONMENT.bloomStrength);
  current = profiles.apply(current, {
    ...current,
    skySource: "kit",
    bloomStrength: 0.7,
  });
  assert.equal(
    current.bloomStrength,
    0.7,
    "Explicit API values must be preserved.",
  );
});

test("background brightness survives settings updates without changing any lighting gain", () => {
  const profiles = new EnvironmentProfiles();
  profiles.configureSourceIntensity({ day: 3, night: 2 });
  const { skyBackgroundIntensity, ...legacy } = DEFAULT_ENVIRONMENT;
  assert.equal(profiles.initial(legacy).skyBackgroundIntensity, 1);
  const previous = profiles.initial({
    ...DEFAULT_ENVIRONMENT,
    skySource: "kit",
  });
  const accepted = profiles.apply(previous, {
    ...previous,
    skyBackgroundIntensity: 0.6,
  });
  assert.deepEqual(accepted, { ...previous, skyBackgroundIntensity: 0.6 });
  const changedSource = profiles.apply(
    accepted,
    { ...accepted, skySource: "generated" },
    true,
  );
  assert.equal(changedSource.skyBackgroundIntensity, 0.6);
  assert.equal(changedSource.skyIntensity, previous.skyIntensity);
  assert.equal(changedSource.sunIntensity, previous.sunIntensity);
  assert.equal(
    changedSource.ambientIntensity,
    DEFAULT_ENVIRONMENT.ambientIntensity,
  );
  const restored = profiles.apply(
    changedSource,
    { ...changedSource, skySource: "kit" },
    true,
  );
  assert.equal(restored.skyBackgroundIntensity, 0.6);
  assert.equal(restored.ambientIntensity, 3);
});
