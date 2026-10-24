import test from "node:test";
import assert from "node:assert/strict";
import { PlayerMode } from "./player-mode.ts";
test("player mode cannot be unlocked or given editor authority; load retries keep the pinned version", () => {
  const mode = new PlayerMode();
  mode.enter(true);
  mode.enter(false);
  for (const type of [
    "author",
    "command",
    "mmorpgEditing",
    "battle",
    "setShader",
    "setEnvironment",
    "setSceneEffects",
    "previewImpact",
    "exportProject",
    "materialView",
  ])
    assert.equal(mode.allows(type), false);
  for (const type of ["input", "setCameraPose", "status", "mesh", "capture"])
    assert.equal(mode.allows(type), true);
  assert.equal(mode.begin("a"), true);
  assert.throws(() => mode.begin("b"));
  mode.fail();
  assert.equal(mode.begin("a"), true);
  mode.finish("a");
  assert.equal(mode.begin("a"), false);
  assert.throws(() => mode.begin("b"));
});
