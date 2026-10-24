import assert from "node:assert/strict";
import { test } from "node:test";
import {
  aiHash,
  ledPoint,
  rangedBand,
  siegeChoice,
} from "../tools/battle-ai-metrics.mjs";
const actor = (
  id: number,
  x: number,
  z: number,
  team = 1,
  state = 0,
  health = 50,
) => ({ id, position: [x, 0, z], velocity: [0, 0], team, state, health });
test("ranged bands retain long range while fitting small arenas", () => {
  assert.deepEqual(rangedBand(0, 1, 12), {
    preferred: 7.199999999999999,
    inner: 5.76,
    outer: 8.064,
    pressure: 1.7999999999999998,
  });
  const large = rangedBand(0, 1, 100);
  assert(large.preferred >= 48 && large.preferred < 56);
  assert.equal(large.pressure, 8);
  assert.equal(aiHash(0), 0);
  assert.equal(aiHash(1), 1753845952);
});
test("lead uses velocity and caps displacement rather than flight alone", () => {
  const target = { ...actor(0, 100, 0), velocity: [30, 40] };
  const result = ledPoint(target, [0, 0, 0], 42, 1.8);
  assert.equal(result.flight, 1.8);
  assert(Math.abs(Math.hypot(...result.displacement) - 6) < 1e-12);
  assert.deepEqual(result.point, [103.6, 0.6, 4.8]);
});
test("siege prefers attacking live clusters, excludes dead/friendly candidates, and skips empty battle", () => {
  const actors = Array.from({ length: 24 }, (_, id) =>
    actor(id, -80 + id * 7, 70, 0),
  );
  actors[1] = actor(1, 0, 0);
  actors[2] = actor(2, 2, 0, 1, 2);
  actors[3] = actor(3, 0, 2, 1, 2);
  actors[4] = actor(4, 60, 50);
  actors[5] = actor(5, 1, 1, 1, 2, 0);
  const choice = siegeChoice(
    actors,
    { position: [-88, 0, 0], team: 0 },
    0,
    1,
    100,
  );
  assert([1, 2, 3].includes(choice.id!));
  assert.equal(choice.score, 7);
  assert(!choice.candidates.some((a) => a.id === 5));
  assert.equal(
    siegeChoice(
      actors.map((a) => ({ ...a, health: 0 })),
      { position: [0, 0, 0], team: 0 },
      0,
      1,
      100,
    ).id,
    null,
  );
});
