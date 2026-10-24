import assert from "node:assert/strict";
import { test } from "node:test";
import {
  PLAYER_WORLD_SCHEMA,
  validatePlayerWorld,
} from "../packages/runtime/player-world.ts";
import { createStarterGameConfig } from "../packages/runtime/starter-game.ts";

function fixture() {
  const game = createStarterGameConfig();
  return {
    schema: PLAYER_WORLD_SCHEMA,
    groundY: 0,
    mode: "walk",
    spawn: { position: [0, 0, 4], yaw: 0, pitch: 0, mode: "walk" },
    entities: [game.key, game.exit].map((marker) => ({
      id: marker.id,
      kind: "barrel",
      position: [...marker.position],
    })),
    colliders: [{ id: "wall", min: [-2, 0, -2], max: [-1, 2, 2] }],
    rooms: [{ id: "room", center: [5, 5], size: [8, 8] }],
    game,
  };
}
test("published world validation copies authored settings and normalizes native defaults", () => {
  const input = fixture();
  const world = validatePlayerWorld(input);
  assert.equal(world.entities[0].yaw, 0);
  assert.equal(world.entities[0].scale, 1);
  assert.equal(world.entities[0].collidable, false);
  assert.equal(world.rooms[0].floorY, 0);
  assert.equal(world.rooms[0].height, 3);
  input.entities[0].position[0] = 999;
  input.game.key.position[0] = 999;
  input.spawn.position[0] = 999;
  assert.notEqual(world.entities[0].position[0], 999);
  assert.notEqual(world.game!.key.position[0], 999);
  assert.notEqual(world.spawn.position[0], 999);
});
test("invalid fields, bounds, counts, and mismatched gameplay markers reject without changing the input", () => {
  const cases: Array<(value: any) => void> = [
    (value) => (value.spawn.position[0] = Infinity),
    (value) => (value.spawn.position[1] = -1),
    (value) => (value.spawn.mode = "fly"),
    (value) => (value.extra = "unsupported"),
    (value) => (value.entities[0].position = [0, 0]),
    (value) => (value.entities[0].id = "not a name"),
    (value) => (value.entities[1].id = value.entities[0].id),
    (value) => (value.entities[0].scale = 0),
    (value) => (value.entities[0].collidable = true),
    (value) => (value.entities[0].halfExtents = [1, 0, 1]),
    (value) => (value.colliders[0].max = value.colliders[0].min),
    (value) => (value.rooms[0].size = [33, 8]),
    (value) => (value.rooms[0].floorY = 2),
    (value) => (value.rooms = new Array(129).fill(value.rooms[0])),
    (value) => (value.game.key.position[0] += 1),
    (value) => (value.presentationURL = "https://example.com/settings.json"),
    (value) => (value.presentationURL = "/starter/../settings.json"),
  ];
  for (const mutate of cases) {
    const value = fixture();
    mutate(value);
    const before = structuredClone(value);
    assert.throws(() => validatePlayerWorld(value));
    assert.deepEqual(value, before);
  }
});
