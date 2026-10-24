import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const { instance } = await WebAssembly.instantiate(
  await readFile(new URL("../public/kernel.wasm", import.meta.url)),
  {},
);
const api = instance.exports;
for (const name of ["af_alloc", "af_request", "af_response_len", "af_free"])
  assert.equal(typeof api[name], "function", name);
assert.ok(api.memory instanceof WebAssembly.Memory);
const encoder = new TextEncoder();
const decoder = new TextDecoder();
function call(value) {
  const bytes = encoder.encode(JSON.stringify(value));
  const ptr = api.af_alloc(bytes.length) >>> 0;
  assert.notEqual(ptr, 0);
  let response = 0,
    length = 0;
  try {
    new Uint8Array(api.memory.buffer, ptr, bytes.length).set(bytes);
    response = api.af_request(ptr, bytes.length) >>> 0;
    length = api.af_response_len() >>> 0;
    assert.notEqual(response, 0);
    return JSON.parse(
      decoder.decode(new Uint8Array(api.memory.buffer, response, length)),
    );
  } finally {
    if (response) api.af_free(response, length);
    api.af_free(ptr, bytes.length);
  }
}

assert.equal(
  call({ type: "init", spawn: { position: [0, 0, 0], mode: "walk" } }).ok,
  true,
);
const edit = {
  type: "command",
  commandId: "one-room",
  baseRevision: 0,
  operations: [
    { op: "add_room", room: { id: "hall", center: [0, 0], size: [6, 6] } },
  ],
};
const room = call(edit);
assert.equal(room.ok, true);
assert.ok(room.wallColliders.length >= 4);
assert.equal(call(edit).duplicate, true);
assert.equal(call({ type: "snapshot" }).revision, 1);
for (let i = 0; i < 120; i++)
  assert.equal(
    call({ type: "step", elapsed: 1 / 60, input: { forward: 1, sprint: true } })
      .ok,
    true,
  );
let snapshot = call({ type: "snapshot" });
assert.ok(snapshot.player.position[2] > -2.7, "wall stops the player");
const before = snapshot.player.position;
assert.equal(
  call({
    type: "command",
    commandId: "bad",
    baseRevision: 1,
    operations: [{ op: "move_entity", id: "missing", position: [1, 0, 0] }],
  }).ok,
  false,
);
snapshot = call({ type: "snapshot" });
assert.equal(snapshot.revision, 1);
assert.deepEqual(snapshot.player.position, before);
assert.equal(
  call({
    type: "set_camera",
    position: [20, 5, 20],
    yaw: 0,
    pitch: 0,
    mode: "fly",
  }).ok,
  true,
);
assert.equal(
  call({ type: "step", elapsed: 0.1, input: { up: 1 } }).player.mode,
  "fly",
);

const warmBytes = api.memory.buffer.byteLength;
for (let i = 0; i < 2000; i++) {
  const result = call({ type: "step", elapsed: 1 / 120, input: {} });
  assert.equal(result.ok, true);
  assert.equal(result.entities, undefined);
}
assert.ok(
  api.memory.buffer.byteLength <= warmBytes + 1024 * 1024,
  "freed bridge slabs stabilize",
);

// Reserve response space before applying a command when slabs exhaust the bridge.
const source = encoder.encode(
  JSON.stringify({
    type: "command",
    commandId: "oom-must-not-commit",
    baseRevision: 1,
    operations: [
      {
        op: "add_entity",
        entity: { id: "box", kind: "crate", position: [3, 0, 3] },
      },
    ],
  }),
);
const ptr = api.af_alloc(source.length) >>> 0;
new Uint8Array(api.memory.buffer, ptr, source.length).set(source);
const held = [];
for (let i = 0; i < 3; i++) {
  const slab = api.af_alloc(4 * 1024 * 1024) >>> 0;
  assert.notEqual(slab, 0);
  held.push(slab);
}
assert.equal(api.af_request(ptr, source.length) >>> 0, 0);
for (const slab of held) api.af_free(slab, 4 * 1024 * 1024);
api.af_free(ptr, source.length);
assert.equal(call({ type: "snapshot" }).revision, 1);
assert.equal(api.af_request(12345, 10) >>> 0, 0);
let catalogBuildingsChecked = 0;
if (process.env.AF_KERNEL_TEST_CATALOG) {
  const catalog = JSON.parse(
    await readFile(process.env.AF_KERNEL_TEST_CATALOG, "utf8"),
  );
  const townhouse = catalog.assets.find((asset) => asset.id === "townhouse");
  assert.ok(
    townhouse?.plan,
    "private catalog has the townhouse structural sidecar",
  );
  const opened = call({
    type: "init",
    groundY: 1.55,
    entities: [
      {
        id: "house",
        kind: "townhouse",
        position: [0, 1.55, 0],
        buildingPlan: townhouse.plan,
      },
    ],
    spawn: { position: [3.025, 1.55, 3.2], yaw: 0, pitch: 0, mode: "walk" },
  });
  assert.equal(opened.ok, true, JSON.stringify(opened));
  for (let i = 0; i < 135; i++)
    assert.equal(
      call({ type: "step", elapsed: 1 / 60, input: { forward: 1 } }).ok,
      true,
    );
  const upstairs = call({ type: "snapshot" });
  assert.ok(
    Math.abs(upstairs.player.position[1] - 5.55) < 0.02,
    JSON.stringify(upstairs.player),
  );
  const removed = call({
    type: "command",
    commandId: "open-sidecar-wall",
    baseRevision: 0,
    operations: [{ op: "remove_wall", wallId: "house:f0-front:east" }],
  });
  assert.equal(removed.ok, true);
  const reopened = call({
    type: "init",
    groundY: 1.55,
    entities: removed.entities,
    removedWalls: removed.removedWalls,
  });
  assert.equal(reopened.ok, true);
  assert.equal(
    reopened.wallColliders.some((w) => w.id === "house:f0-front:east"),
    false,
  );
  catalogBuildingsChecked = 1;
}
console.log(
  JSON.stringify({
    ok: true,
    checks: [
      "raw-wasm-exports",
      "typed-room-command",
      "movement-collision",
      "failed-edit-preserves-pose",
      "idempotency",
      "2000-frame-summaries",
      "allocation-pressure-atomicity",
    ],
    catalogBuildingsChecked,
    wasmBytes: (
      await readFile(new URL("../public/kernel.wasm", import.meta.url))
    ).length,
    memoryBytes: api.memory.buffer.byteLength,
  }),
);
