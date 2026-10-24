import assert from "node:assert/strict";
import test from "node:test";
import { battleCamera } from "../packages/runtime/battle-view.ts";
import { mmorpgFogAmount } from "../packages/renderer/mmorpg-fog.ts";

test("wide battle cameras keep the army visible through fog at every supported scale", () => {
  for (const radius of [12, 100, 317, 548, 708, 1000]) {
    for (const view of ["overview", "orbit"] as const) {
      for (const seconds of [0, 24, 96]) {
        const camera = battleCamera([0, 0, 0], radius, view, seconds);
        const scale = camera.sourceFogScale!;
        assert(scale >= 1 && scale <= 100);
        for (const [x, z] of [
          [0, 0],
          [-radius, -radius],
          [radius, radius],
        ]) {
          const fog = mmorpgFogAmount(camera.eye[1] / scale, [
            (x - camera.eye[0]) / scale,
            -camera.eye[1] / scale,
            (z - camera.eye[2]) / scale,
          ]);
          assert(
            fog < 0.65,
            `${view} radius${radius}: fog${fog} hides the army`,
          );
        }
      }
    }
  }
});

test("the previous fixed spectator fog scale reproduces the blank lavender view", () => {
  for (const view of ["overview", "orbit"] as const) {
    const camera = battleCamera([0, 0, 0], 1000, view, 0);
    assert.equal(
      mmorpgFogAmount(
        camera.eye[1] / 10,
        camera.eye.map((v) => -v / 10),
      ),
      1,
    );
    const corrected = camera.sourceFogScale!;
    assert(
      mmorpgFogAmount(
        camera.eye[1] / corrected,
        camera.eye.map((v) => -v / corrected),
      ) < 0.15,
    );
  }
});
