import assert from "node:assert/strict";
import test from "node:test";
import { mmorpgModelYaw } from "../packages/renderer/mmorpg-characters.ts";
import {
  modelMatrix,
  transformPoint,
  direction,
} from "../packages/renderer/math.ts";
import { SKY_BACKGROUND_WGSL } from "../packages/renderer/sky-shader.ts";
import { MATERIAL, POST } from "../packages/renderer/shaders.ts";
import { WGSL_FILMIC } from "../packages/renderer/filmic.ts";
import { WGSL_DISPLAY_LUT } from "../packages/renderer/display-lut.ts";

test("source model forward follows simulation motion at every heading", () => {
  assert.equal(mmorpgModelYaw(Math.PI), 0);
  for (const yaw of [-Math.PI, -1.2, 0, 0.7, Math.PI]) {
    const modelForward = transformPoint(
      modelMatrix([0, 0, 0], mmorpgModelYaw(yaw)),
      [0, 0, 1],
    );
    const movement = direction(yaw, 0);
    for (let axis = 0; axis < 3; axis++)
      assert(Math.abs(modelForward[axis] - movement[axis]) < 1e-12);
  }
});

test("the retained sky keeps its owned exposure, filmic curve and display LUT before the source post", () => {
  assert(SKY_BACKGROUND_WGSL.includes(WGSL_FILMIC));
  assert(SKY_BACKGROUND_WGSL.includes(WGSL_DISPLAY_LUT));
  assert(SKY_BACKGROUND_WGSL.includes("color*skyDisplay.settings.x"));
  assert(SKY_BACKGROUND_WGSL.includes("filmicColor(c,skyDisplay.filmic)"));
  assert(
    SKY_BACKGROUND_WGSL.includes("color=ownedSkyDisplay(color,v.uv)*0.01"),
  );
  assert(SKY_BACKGROUND_WGSL.includes("if frame.skyLayers.w>.5"));
  assert(!MATERIAL.includes("ownedSkyDisplay"));
  assert(POST.includes("display.settings.w<-.5{c=clamp(c"));
});
