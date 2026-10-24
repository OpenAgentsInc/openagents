import assert from "node:assert/strict";
import { test } from "node:test";
import { squadMotion, squadClearance } from "../tools/battle-squad-metrics.mjs";
const actor = (id: number, x: number, z = 0) => ({
  id,
  position: [x, 0, z],
  health: 200,
  kind: 0,
});
test("shared motion requires multiple real members rather than one moving actor", () => {
  const initial = [actor(0, 0), actor(1, 3), actor(2, 6), actor(3, 9)];
  const one = [{ tick: 60, actors: [actor(0, 2), ...initial.slice(1)] }];
  assert.equal(squadMotion(initial, one, [0, 1, 2, 3], [1, 0]).ok, false);
  const all = [
    { tick: 60, actors: initial.map((a) => actor(a.id, a.position[0] + 1)) },
  ];
  assert.equal(squadMotion(initial, all, [0, 1, 2, 3], [1, 0]).ok, true);
  assert.equal(squadMotion(initial, all, [0, 1, 2, 3], [-1, 0]).ok, false);
});
test("duplicate identities, missing bodies, and repeated ticks cannot fake coordinated movement", () => {
  const initial = [actor(0, 0), actor(1, 3)];
  const frame = { tick: 1, actors: initial };
  assert.throws(() => squadMotion(initial, [frame], [0, 0], [1, 0]));
  assert.throws(() => squadMotion(initial, [frame], [0, 2], [1, 0]));
  assert.throws(() => squadMotion(initial, [frame, frame], [0, 1], [1, 0]));
});
test("clearance detects living core overlap and ignores dead bodies", () => {
  assert(
    Math.abs(
      squadClearance(
        [{ tick: 1, actors: [actor(0, 0), actor(1, 1.5)] }],
        [0.63],
      ).minimumCoreGap! - 0.24,
    ) < 1e-12,
  );
  assert(
    squadClearance([{ tick: 1, actors: [actor(0, 0), actor(1, 1)] }], [0.63])
      .minimumCoreGap! < 0,
  );
  assert.equal(
    squadClearance(
      [{ tick: 1, actors: [actor(0, 0), { ...actor(1, 0), health: 0 }] }],
      [0.63],
    ).minimumCoreGap,
    null,
  );
});
