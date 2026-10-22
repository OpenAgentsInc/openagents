import assert from "node:assert/strict";
import { test } from "node:test";
import {
  HouseFloor,
  DEFAULT_LAYOUT,
  extendLayout,
  readLayout,
} from "../src/layout.ts";
import {
  SceneHistory,
  PROJECT_ID,
  decisionOperation,
  type Operation,
} from "../src/project.ts";
import { move, houseGeometry } from "../src/scene.ts";
import { GhostSimulation } from "../src/behaviors.ts";
const apply = (s: SceneHistory, operation: Operation) =>
  s
    .prepare({
      id: crypto.randomUUID(),
      project: PROJECT_ID,
      baseRevision: s.project.revision,
      operation,
    })
    .commit();

test("shared walls, doorways and picking use the same floor plan", () => {
  const floor = new HouseFloor();
  assert.equal(
    floor.pick({ x: 0, z: 6.8, yaw: 0, pitch: 0 })?.id,
    "gallery:north",
  );
  assert.equal(
    floor.pick({ x: 0, z: 6.8, yaw: Math.PI / 2, pitch: 0 })?.id,
    "hall:east",
  );
  for (let z = 6.8; z > -16; z -= 0.1)
    assert.ok(floor.canStand(0, z), `blocked center at ${z}`);
  assert.equal(floor.canStand(3.5, 2), false);
  assert.equal(floor.canStand(5, 2), false);
  const wall = floor.pick({ x: 0, z: 6.8, yaw: Math.PI / 2, pitch: 0 })!;
  const layout = extendLayout(DEFAULT_LAYOUT, wall.id, 6, false, {
    x: wall.hitX,
    z: wall.hitZ,
  });
  const expanded = new HouseFloor(layout);
  for (let x = 0; x < 9; x += 0.1)
    assert.ok(expanded.canStand(x, 6.6), `doorway blocked at ${x}`);
  assert.ok(layout.rooms.at(-1)!.x > 3.5);
  const pose = { x: 0, z: 6.6, yaw: 0, pitch: 0 };
  for (let i = 0; i < 180; i++) move(pose, 0, 1, 1 / 60, expanded.canStand);
  assert.ok(pose.x > 7);
  assert.equal(floor.canStand(pose.x, pose.z), false);
});

test("room and wing edits save, validate and undo atomically", () => {
  const s = new SceneHistory();
  const original = structuredClone(s.project);
  apply(s, { kind: "add_wing", wall: "gallery:north", size: 6 });
  assert.equal(s.project.layout?.rooms.length, 6);
  assert.deepEqual(
    readLayout(JSON.parse(JSON.stringify(s.project.layout))),
    s.project.layout,
  );
  const floor = new HouseFloor(s.project.layout);
  for (let z = 0; z > -34; z -= 0.1) assert.ok(floor.canStand(0, z));
  const mesh = houseGeometry(s.project.entities, undefined, s.project.layout);
  assert.ok(mesh.every(Number.isFinite));
  apply(s, { kind: "undo" });
  assert.deepEqual(s.project.entities, original.entities);
  assert.equal(structuredClone(s.project).layout, undefined);
  apply(s, { kind: "redo" });
  assert.equal(s.project.layout?.rooms.length, 6);
  const before = structuredClone(s.project);
  for (const op of [
    { kind: "add_room", wall: "missing", size: 6 },
    { kind: "add_room", wall: "gallery:north", size: 100 },
    { kind: "remove_wall", wall: "missing" },
  ])
    assert.throws(() => apply(s, op as Operation));
  assert.deepEqual(s.project, before);
});

test("knocking down a wall removes the mesh and collision without inventing outdoor floor", () => {
  const s = new SceneHistory();
  apply(s, { kind: "add_room", wall: "hall:east", size: 6 });
  const before = new HouseFloor(s.project.layout);
  const shared = before.walls.find(
    (w) =>
      !w.exterior &&
      w.side === "east" &&
      Math.hypot(w.x2 - w.x1, w.z2 - w.z1) > 1,
  )!;
  const x = (shared.x1 + shared.x2) / 2,
    z = (shared.z1 + shared.z2) / 2;
  assert.equal(before.canStand(x, z), false);
  apply(s, { kind: "remove_wall", wall: shared.id });
  assert.equal(new HouseFloor(s.project.layout).canStand(x, z), true);
  assert.throws(() => apply(s, { kind: "remove_wall", wall: shared.id }));
  apply(s, { kind: "remove_wall", wall: "hall:west" });
  assert.equal(new HouseFloor(s.project.layout).canStand(-5, 2), false);
});

test("overlaps, disconnected layouts, non-finite sizes and resource excess are refused", () => {
  assert.throws(() => extendLayout(DEFAULT_LAYOUT, "hall:north", 6));
  assert.throws(() => extendLayout(DEFAULT_LAYOUT, "hall:east", NaN));
  assert.throws(() =>
    readLayout({
      ...DEFAULT_LAYOUT,
      rooms: [
        ...DEFAULT_LAYOUT.rooms,
        { id: "bad", name: "Bad", x: 40, z: 40, width: 6, depth: 6 },
      ],
    }),
  );
  assert.throws(() =>
    readLayout({
      ...DEFAULT_LAYOUT,
      rooms: Array(17).fill(DEFAULT_LAYOUT.rooms[0]),
    }),
  );
  assert.throws(() =>
    readLayout({ ...DEFAULT_LAYOUT, removedWalls: ["missing:east"] }),
  );
});

test("layout decisions capture the aimed wall and ghosts can chase into new rooms", () => {
  const s = new SceneHistory(),
    pose = { x: 0, z: 6.8, yaw: 0, pitch: 0 };
  const d = {
    action: "add_room",
    placement: "right",
    size: "normal",
    tint: "unsupported",
    confidence: 1,
    needs_review: false,
    model: "fixture",
    model_ms: 0,
  };
  const op = decisionOperation(
    d,
    "add a room to my right",
    pose,
    null,
    s.project,
  );
  assert.equal(op.kind, "add_room");
  apply(s, op);
  const floor = new HouseFloor(s.project.layout);
  apply(s, {
    kind: "add_asset",
    entity: {
      id: "ghost-new-room",
      kind: "ghost",
      x: 7,
      z: 6,
      scale: 1,
      behavior: "chase",
    },
  });
  const sim = new GhostSimulation();
  sim.sync(s.project.entities);
  for (let i = 0; i < 600; i++)
    sim.step(pose, 1 / 60, floor.canStand, floor.guide);
  const ghost = sim.snapshot()[0];
  assert.ok(ghost.x < 3.5);
  const aimed = { id: "hall:west", x: -3.5, z: 3 };
  assert.deepEqual(
    decisionOperation(
      { ...d, action: "remove_wall", placement: "left" },
      "knock down the wall on my left",
      pose,
      null,
      s.project,
      { id: "hall:east", x: 3.5, z: 3 },
    ),
    { kind: "remove_wall", wall: "hall:west" },
  );
  assert.deepEqual(
    decisionOperation(
      { ...d, action: "remove_wall", placement: "ahead" },
      "knock down this wall",
      pose,
      null,
      s.project,
      aimed,
    ),
    { kind: "remove_wall", wall: "hall:west" },
  );
});

test("removed wall references survive extensions and an internal opening does not hide a farther wall", () => {
  const base = structuredClone(DEFAULT_LAYOUT);
  base.removedWalls.push("hall:east");
  const next = extendLayout(base, "hall:east", 6);
  const floor = new HouseFloor(next);
  assert.ok(!floor.walls.some((w) => w.id === "hall:east"));
  const target = floor.pick({ x: 0, z: 1.5, yaw: Math.PI / 2, pitch: 0 });
  assert.equal(target?.id, next.rooms.at(-1)!.id + ":east");
});

test("old medieval additions leave Haunted House saves without removing its authored rooms", async () => {
  const { readProject, initialProject } = await import("../src/project.ts");
  const original = initialProject();
  const layout = extendLayout(
    DEFAULT_LAYOUT,
    "gallery:west",
    16,
    false,
    undefined,
    true,
  );
  const migrated = readProject({
    ...original,
    layout,
    entities: [
      ...original.entities,
      { id: "cottage-old", kind: "cottage", x: -10, z: -13, scale: 1 },
    ],
  });
  assert.deepEqual(migrated.entities, original.entities);
  assert.equal(migrated.layout?.rooms.length, 3);
  assert.ok(migrated.layout?.rooms.every((r) => !r.outdoor));
  assert.throws(() => readProject({ ...original, id: "medieval-town-v1" }));
});
