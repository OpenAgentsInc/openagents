import assert from "node:assert/strict";
import { test } from "node:test";
import {
  initialTown,
  TownFloor,
  readTown,
  isHouse,
  housePositions,
} from "../src/town.ts";
import type { Pose } from "../src/scene.ts";
Object.defineProperty(globalThis, "location", {
  value: new URL("http://localhost/medieval/"),
});
const {
  SceneHistory,
  PROJECT_ID,
  initialProject,
  readProject,
  decisionOperation,
  ASSETS,
} = await import("../src/project.ts");
const apply = (
  store: InstanceType<typeof SceneHistory>,
  operation: Parameters<
    InstanceType<typeof SceneHistory>["prepare"]
  >[0]["operation"],
) =>
  store
    .prepare({
      id: crypto.randomUUID(),
      project: PROJECT_ID,
      baseRevision: store.project.revision,
      operation,
    })
    .commit();
const floor = (s: InstanceType<typeof SceneHistory>) =>
  new TownFloor(s.project.entities, s.project.town!);

test("town has its own project and catalog, walkable doors and solid exterior walls", () => {
  const s = new SceneHistory();
  assert.equal(PROJECT_ID, "medieval-town-v1");
  assert.equal(s.project.material, "medieval/material.wgsl");
  assert.equal("ghost" in ASSETS, false);
  assert.equal(Object.keys(ASSETS).length, 8);
  const f = floor(s);
  assert.equal(f.canStand(-11, -5), true);
  assert.equal(f.canStand(-9, -5), false);
  const p: Pose = { x: -11, z: -3, y: 0, yaw: 0, pitch: 0 };
  for (let i = 0; i < 80; i++) f.move(p, 1, 0, 0.05);
  assert.ok(p.z < -6 && p.z > -10);
  assert.equal(f.roomAt(p.x, p.z, p.y)?.room.name, "Front room");
  assert.equal(f.canStand(49, 0), false);
  assert.deepEqual(
    readProject(JSON.parse(JSON.stringify(s.project))),
    s.project,
  );
});
test("interior wall removal changes the visible kit pieces and collision, and undo restores both", () => {
  const s = new SceneHistory(),
    f = floor(s),
    pose = { x: -11, z: -8, y: 0, yaw: 0, pitch: 0 };
  const target = f.pick(pose)!;
  assert.ok(target.id.endsWith("-front:north"));
  assert.equal(target.exterior, false);
  const beforePieces = f.pieces.length;
  assert.equal(f.canStand(-11, -10), false);
  apply(s, { kind: "remove_wall", wall: target.id });
  assert.equal(floor(s).canStand(-11, -10), true);
  assert.ok(floor(s).pieces.length < beforePieces);
  const p = { ...pose };
  for (let i = 0; i < 30; i++) floor(s).move(p, 1, 0, 0.05);
  assert.ok(p.z < -10);
  const changed = structuredClone(s.project);
  apply(s, { kind: "undo" });
  assert.equal(floor(s).canStand(-11, -10), false);
  apply(s, { kind: "redo" });
  assert.deepEqual(s.project.town, changed.town);
});
test("room and wing extensions change one building, keep other houses and save in one history entry", () => {
  const s = new SceneHistory(),
    before = structuredClone(s.project),
    f = floor(s),
    pose = { x: -11, z: -7.5, y: 0, yaw: 0, pitch: 0 };
  const target = f.pick(pose, -Math.PI / 2)!;
  apply(s, {
    kind: "add_room",
    wall: target.id,
    size: 6,
    at: { x: target.hitX, z: target.hitZ },
  });
  assert.equal(
    s.project.town!.buildings["cottage-starter"].floors[0].rooms.length,
    3,
  );
  assert.deepEqual(
    s.project.town!.buildings["townhouse-starter"],
    before.town!.buildings["townhouse-starter"],
  );
  assert.equal(floor(s).canStand(-14, -7.5), true);
  const p = { ...pose };
  for (let i = 0; i < 35; i++) floor(s).move(p, 0, -1, 0.05);
  assert.ok(p.x < -14);
  apply(s, { kind: "undo" });
  assert.deepEqual(s.project.town, before.town);
  apply(s, { kind: "add_wing", wall: target.id, size: 4 });
  assert.equal(
    s.project.town!.buildings["cottage-starter"].floors[0].rooms.length,
    5,
  );
  apply(s, { kind: "undo" });
  assert.deepEqual(s.project.town, before.town);
});
test("townhouse stairs climb and descend between two real floors without jumping or walking off the upper floor", () => {
  const s = new SceneHistory(),
    f = floor(s),
    p: Pose = { x: 13.025, z: -6.8, y: 0, yaw: 0, pitch: 0 };
  for (let i = 0; i < 65; i++) f.move(p, 1, 0, 0.05);
  assert.ok(Math.abs((p.y ?? 0) - 4) < 0.01);
  assert.equal(f.roomAt(p.x, p.z, p.y)?.level, 1);
  assert.equal(f.canStand(-20, 3, 0.24, 4), false);
  for (let i = 0; i < 65; i++) f.move(p, -1, 0, 0.05);
  assert.equal(p.y, 0);
  assert.ok(p.z > -7);
});
test("house overlap, bad floor IDs, tiny entrances and excessive floors fail atomically", () => {
  const s = new SceneHistory(),
    before = structuredClone(s.project);
  assert.throws(() =>
    apply(s, { kind: "move_asset", id: "townhouse-starter", x: -9, z: -10 }),
  );
  assert.deepEqual(s.project, before);
  assert.throws(() =>
    apply(s, { kind: "resize_asset", id: "cottage-starter", scale: 0.3 }),
  );
  assert.deepEqual(s.project, before);
  const bad = structuredClone(s.project.town!);
  bad.buildings["cottage-starter"].floors.push(
    bad.buildings["cottage-starter"].floors[0],
  );
  assert.throws(() => readTown(bad, s.project.entities));
  const positions = housePositions(
    s.project.entities,
    s.project.town!,
    "cottage",
    2,
    1,
    { x: -11, z: 3, yaw: 0, pitch: 0 },
    "left",
    { x: -13, z: 3 },
  );
  assert.equal(positions.length, 2);
  assert.ok(positions.every((p) => p.x < -11));
});
test("fast commands target the current building floor and room geometry contains no spooky props", async () => {
  const s = new SceneHistory(),
    pose = { x: -11, z: -7.5, y: 0, yaw: 0, pitch: 0 };
  const d = {
    action: "add_room",
    placement: "left",
    size: "normal",
    tint: "unsupported",
    confidence: 1,
    needs_review: false,
    model: "fixture",
    model_ms: 1,
  };
  const op = decisionOperation(
    d,
    "Add a room to my left",
    pose,
    null,
    s.project,
  );
  apply(s, op);
  const { townGeometry } = await import("../src/town-geometry.ts");
  const data = townGeometry(floor(s));
  assert.ok(data.length > 0 && data.every(Number.isFinite));
  assert.ok(
    s.project.entities.every(
      (e) => !["ghost", "pumpkin", "coffin"].includes(e.kind),
    ),
  );
  assert.equal(s.project.entities.filter((e) => isHouse(e.kind)).length, 2);
});

test("large town plans are refused before they can create a save that cannot reopen", () => {
  const base = initialProject();
  const entities = [] as typeof base.entities;
  const town = { buildings: {} } as NonNullable<typeof base.town>;
  for (let n = 0; n < 12; n++) {
    const id = `townhouse-capacity-${n}`;
    entities.push({
      id,
      kind: "townhouse",
      x: -27 + (n % 4) * 18,
      z: -18 + Math.floor(n / 4) * 18,
      scale: 0.6,
    });
    town.buildings[id] = {
      floors: [0, 1].map((level) => ({
        removedWalls: [],
        rooms: Array.from({ length: 16 }, (_, i) => ({
          id: `room-${id}-${level}-${i}`.padEnd(64, "x"),
          name: "A".repeat(60),
          x: -4.5 + (i % 4) * 3,
          z: -4.5 + Math.floor(i / 4) * 3,
          width: 3,
          depth: 3,
        })),
      })),
    };
  }
  assert.throws(
    () => readProject({ ...base, entities, town }),
    /scene is full/,
  );
  assert.throws(
    () => readProject({ ...base, entities: [null] }),
    /supported town assets/,
  );
});

test("delegated buildings preserve exact counts, choose usable styles and clear plots, and undo as one edit", () => {
  const s = new SceneHistory(),
    before = structuredClone(s.project);
  const pose = { x: -11, z: 3, y: 0, yaw: 0, pitch: 0 };
  const d = {
    action: "add_asset",
    asset: "building",
    placement: "anywhere",
    size: "normal",
    confidence: 0.99,
    needs_review: false,
    tint: "unsupported",
    model: "fixture",
    model_ms: 1,
  };
  const text = "add 2 buildings your choice anywhere u want";
  apply(s, decisionOperation(d, text, pose, null, s.project));
  const added = s.project.entities.slice(before.entities.length);
  assert.equal(added.length, 2);
  assert.deepEqual(
    added.map((e) => e.kind),
    ["cottage", "townhouse"],
  );
  const f = floor(s);
  assert.ok(f.canStand(pose.x, pose.z));
  for (const e of added) {
    assert.ok(f.canStand(e.x - 2 * e.scale, e.z + 5 * e.scale));
    assert.equal(
      f.roomAt(e.x - 2 * e.scale, e.z + 2 * e.scale, 0)?.room.name,
      "Front room",
    );
  }
  assert.deepEqual(
    readProject(JSON.parse(JSON.stringify(s.project))),
    s.project,
  );
  apply(s, { kind: "undo" });
  assert.deepEqual(s.project.entities, before.entities);
  apply(s, { kind: "redo" });
  assert.deepEqual(s.project.entities.slice(before.entities.length), added);
  for (const text of [
    "add 0 buildings anywhere",
    "add 9 buildings anywhere",
    "add 2.5 buildings anywhere",
  ]) {
    const unchanged = structuredClone(s.project);
    assert.throws(() => decisionOperation(d, text, pose, null, s.project));
    assert.deepEqual(s.project, unchanged);
  }
  for (const text of [
    "place two buildings wherever you choose",
    "add 2 huge buildings anywhere",
  ]) {
    const next = new SceneHistory();
    apply(
      next,
      decisionOperation(
        { ...d, size: text.includes("huge") ? "huge" : "normal" },
        text,
        pose,
        null,
        next.project,
      ),
    );
    assert.equal(next.project.entities.length, before.entities.length + 2);
  }
  const next = new SceneHistory();
  const distantPose = { ...pose, x: 35, z: -35 };
  apply(next, decisionOperation(d, text, distantPose, null, next.project));
  assert.equal(next.project.entities.length, before.entities.length + 2);
});

test("town chooses furniture, props, and fitting extensions without inventing exact arguments", () => {
  const pose = { x: -11, z: 3, y: 0, yaw: 0, pitch: 0 };
  const d = {
    action: "add_asset",
    asset: "furniture",
    placement: "anywhere",
    size: "normal",
    confidence: 1,
    needs_review: false,
    tint: "warm",
    model: "fixture",
    model_ms: 1,
  };
  for (const [text, count, asset] of [
    ["add a couple of furnishings wherever they fit", 2, "furniture"],
    ["add 2 cool decorations of your choice anywhere", 2, "any_asset"],
    ["add buildings anywhere", 3, "building"],
  ] as const) {
    const s = new SceneHistory();
    apply(s, decisionOperation({ ...d, asset }, text, pose, null, s.project));
    assert.equal(
      s.project.entities.length,
      initialProject().entities.length + count,
    );
    assert.deepEqual(readProject(s.project), s.project);
  }
  for (const action of ["add_room", "add_wing"]) {
    const s = new SceneHistory(),
      before = structuredClone(s.project);
    apply(
      s,
      decisionOperation(
        { ...d, action },
        "add an extension wherever it fits",
        pose,
        null,
        s.project,
      ),
    );
    assert.ok(
      Object.values(s.project.town!.buildings).some(
        (b) => b.floors[0].rooms.length > 2,
      ),
    );
    apply(s, { kind: "undo" });
    assert.deepEqual(s.project.town, before.town);
  }
});

test("compound town edits put benches outside each new house and save or fail together", async () => {
  const { compileDecision } = await import("../src/compound.ts");
  const base = {
    action: "add_asset",
    asset: "cottage",
    placement: "anywhere",
    size: "normal",
    tint: "warm",
    confidence: 1,
    needs_review: false,
    model: "fixture",
    model_ms: 1,
  };
  const pose = { x: -11, z: 3, y: 0, yaw: 0, pitch: 0 };
  for (const [outsideText, reference, count] of [
    ["put benches outside", "outside_new_buildings", 2],
    ["put 2 benches outside each house", "each_new_building", 4],
    ["put 3 benches outside", "outside_new_buildings", 3],
  ] as const) {
    const s = new SceneHistory(),
      before = structuredClone(s.project);
    const first = "add 2 houses",
      text = `${first} and ${outsideText}`;
    const decision = {
      ...base,
      action: "compound",
      steps: [
        {
          start: 0,
          end: first.length,
          text: first,
          reference: "none",
          decision: base,
        },
        {
          start: first.length + 5,
          end: text.length,
          text: outsideText,
          reference,
          decision: { ...base, asset: "bench", placement: "outside" },
        },
      ],
    };
    const operation = compileDecision(decision, text, pose, null, s.project);
    assert.deepEqual(s.project, before);
    apply(s, operation);
    const added = s.project.entities.slice(before.entities.length),
      houses = added.filter((e) => isHouse(e.kind)),
      benches = added.filter((e) => e.kind === "bench");
    assert.equal(houses.length, 2);
    assert.equal(benches.length, count);
    assert.equal(s.past.length, 1);
    const f = floor(s);
    for (const bench of benches) {
      assert.equal(f.roomAt(bench.x, bench.z, 0), undefined);
      assert.equal(f.canStand(bench.x, bench.z), true);
    }
    for (const house of houses)
      assert.ok(
        benches.some((b) => Math.hypot(b.x - house.x, b.z - house.z) < 12),
      );
    assert.deepEqual(readProject(s.project), s.project);
    apply(s, { kind: "undo" });
    assert.deepEqual(s.project.entities, before.entities);
    apply(s, { kind: "redo" });
    assert.deepEqual(s.project.entities.slice(before.entities.length), added);
  }
  const s = new SceneHistory(),
    before = structuredClone(s.project),
    first = "add 2 houses",
    second = "put 9 benches outside",
    text = `${first} and ${second}`;
  const decision = {
    ...base,
    action: "compound",
    steps: [
      {
        start: 0,
        end: first.length,
        text: first,
        reference: "none",
        decision: base,
      },
      {
        start: first.length + 5,
        end: text.length,
        text: second,
        reference: "outside_new_buildings",
        decision: { ...base, asset: "bench", placement: "outside" },
      },
    ],
  };
  assert.throws(() => compileDecision(decision, text, pose, null, s.project));
  assert.deepEqual(s.project, before);
  assert.equal(s.past.length, 0);
});
