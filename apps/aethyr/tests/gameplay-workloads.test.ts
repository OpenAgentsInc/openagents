import test from "node:test";
import assert from "node:assert/strict";
import {
  enemyMotion,
  assertHeldCorpses,
  projectWorldTarget,
  gameplayActivityComplete,
  ENEMY_WORKLOADS,
  AIM_WORKLOADS,
} from "../tools/gameplay-workloads.mjs";
import { gameplayActivity } from "./fixtures/gameplay-activity.ts";

test("native patrol evidence accumulates a closed loop and rejects topology changes or deaths", () => {
  const sample = (x: number, alive = true) => ({
    enemies: {
      total: 1,
      alive: Number(alive),
      killed: Number(!alive),
      actors: [{ id: 7, position: [x, 0, 0], alive }],
    },
  });
  assert.equal(enemyMotion([sample(0), sample(2), sample(0)])[0].pathMeters, 4);
  assert.throws(() => enemyMotion([sample(0), sample(2, false)]));
  const foreign = sample(2);
  foreign.enemies.actors[0].id = 9;
  assert.throws(() => enemyMotion([sample(0), foreign]));
});
test("corpse evidence requires frozen native position and advancing death time", () => {
  const value = (x: number, seconds: number, alive = false) => ({
    actors: [{ id: 4, position: [x, 0, 0], deathSeconds: seconds, alive }],
  });
  assert.deepEqual(assertHeldCorpses(value(0, 0), value(0, 2.2)), [4]);
  assert.throws(() => assertHeldCorpses(value(0, 0), value(1, 2.2)));
  assert.throws(() => assertHeldCorpses(value(0, 0), value(0, 0.5)));
  assert.throws(() => assertHeldCorpses(value(0, 0), value(0, 2.2, true)));
});
test("target projection separates actual camera eye from controller look direction", () => {
  const lens = Math.tan(0.5);
  const actual = projectWorldTarget({
    eye: [1, 2, 10],
    viewEye: [0, 0, 10],
    target: [0, 0, 0],
    point: [1 + 0.2 * 10 * lens * 2, 2 + 0.22 * 10 * lens, 0],
    width: 1000,
    height: 500,
  });
  assert(Math.abs(actual[0] - 0.2) < 1e-8);
  assert(Math.abs(actual[1] - 0.22) < 1e-8);
  assert.throws(() =>
    projectWorldTarget({
      eye: [0, 0, 0],
      target: [0, 0, -1],
      point: [0, 0, 1],
      width: 800,
      height: 500,
    }),
  );
});
test("suite re-evaluates real native/targeting outcomes rather than optimistic feature labels", () => {
  for (const feature of [...ENEMY_WORKLOADS, ...AIM_WORKLOADS]) {
    const value = gameplayActivity(feature, true);
    assert(gameplayActivityComplete(value, feature, { editor: true }), feature);
    value.phases.pop();
    assert.equal(
      gameplayActivityComplete(value, feature, { editor: true }),
      false,
      feature,
    );
  }
  const noShader = gameplayActivity("enemy-lightning", false);
  assert(gameplayActivityComplete(noShader, "enemy-lightning"));
  assert.equal(
    gameplayActivityComplete(noShader, "enemy-lightning", { editor: true }),
    false,
  );
  const staleAim = gameplayActivity("aim-third-cursor");
  staleAim.samples[0].actualNDC = [0, 0];
  assert.equal(gameplayActivityComplete(staleAim, "aim-third-cursor"), false);
  const noCorpse = gameplayActivity("enemy-meteor");
  noCorpse.phases.find(
    (p: any) => p.phase === "corpse-held-after-death-clip",
  ).enemies.actors[6].position[0] += 3;
  assert.equal(gameplayActivityComplete(noCorpse, "enemy-meteor"), false);
});
