import test from "node:test";
import assert from "node:assert/strict";
import {
  validateEnemyPatrols,
  validEnemySnapshot,
} from "../packages/runtime/enemies.ts";
import { validDruidStatus } from "../packages/runtime/druid-abilities.ts";

const patrols = () => ({
  schema: "aethyrforge.enemy-patrols.v1",
  scenarios: {
    day: [
      {
        id: 0,
        speed: 3.4,
        route: [
          [0, 1.75, 0],
          [0, 1.75, 10],
        ],
      },
    ],
    night: [
      {
        id: 0,
        speed: 3.4,
        route: [
          [0, 1.75, 0],
          [0, 1.75, 10],
        ],
      },
    ],
  },
});

test("day and night patrol admission copies bounded routes without changing their source", () => {
  const input = patrols();
  const accepted = validateEnemyPatrols(input);
  accepted.scenarios.day[0].route[0][0] = 99;
  assert.equal(input.scenarios.day[0].route[0][0], 0);
  assert.equal(accepted.scenarios.night[0].id, 0);
});

test("patrol admission rejects duplicate identities, absent scenarios, and unsafe route budgets", () => {
  const variants = [
    (v: any) => delete v.scenarios.night,
    (v: any) => v.scenarios.day.push(v.scenarios.day[0]),
    (v: any) => (v.scenarios.day[0].id = "knight"),
    (v: any) => (v.scenarios.day[0].speed = 20),
    (v: any) => (v.scenarios.day[0].route[1][2] = Infinity),
    (v: any) => (v.scenarios.day[0].route[1] = [0, 1.75, 0]),
    (v: any) => v.scenarios.day[0].route.push([0, 1.75, 0]),
    (v: any) => (v.scenarios.day[0].route[1] = [300, 1.75, 0]),
    (v: any) =>
      (v.scenarios.day = Array.from({ length: 17 }, (_, id) => ({
        ...v.scenarios.day[0],
        id,
      }))),
  ];
  for (const change of variants) {
    const value = patrols();
    change(value);
    assert.throws(() => validateEnemyPatrols(value), /knight routes/);
  }
});

test("on-demand enemy inspection validates living and defeated counts against finite actor poses", () => {
  const actor = {
    id: 0,
    position: [0, 1.75, 0],
    yaw: 0,
    alive: false,
    deathSeconds: 2,
    animationTime: 1,
  };
  const valid = { total: 1, alive: 0, killed: 1, actors: [actor] };
  assert(validEnemySnapshot(valid));
  assert(!validEnemySnapshot({ ...valid, alive: 1 }));
  assert(!validEnemySnapshot({ ...valid, actors: [actor, actor] }));
  assert(
    !validEnemySnapshot({ ...valid, actors: [{ ...actor, deathSeconds: -1 }] }),
  );
  assert(
    !validEnemySnapshot({
      ...valid,
      actors: [{ ...actor, position: [NaN, 0, 0] }],
    }),
  );
});

test("UI enemy summaries reject invalid totals and renderer counts", () => {
  const status = {
    enabled: true,
    selected: null,
    destroyed: 0,
    debris: 0,
    effects: 0,
    casts: 0,
    impacts: 0,
    tick: 10,
    enemies: { total: 8, alive: 6, killed: 2, modelLoaded: true, rendered: 8 },
  };
  assert(validDruidStatus(status));
  assert(
    !validDruidStatus({ ...status, enemies: { ...status.enemies, alive: 8 } }),
  );
  assert(
    !validDruidStatus({
      ...status,
      enemies: { ...status.enemies, rendered: 17 },
    }),
  );
});
