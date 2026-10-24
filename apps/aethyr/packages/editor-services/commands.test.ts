import assert from "node:assert/strict";
import test from "node:test";
import { parseCommand } from "./commands.ts";

test("every suggested command resolves to a bounded supported action", () => {
  assert.deepEqual(parseCommand("Add a cottage in front of me"), {
    kind: "asset.add",
    assetId: "cottage",
    count: 1,
    placement: "ahead",
  });
  assert.deepEqual(parseCommand("Add two barrels to my left"), {
    kind: "asset.add",
    assetId: "barrel",
    count: 2,
    placement: "left",
  });
  assert.deepEqual(parseCommand("Undo"), { kind: "history.undo" });
});
test("all eight catalog assets have explicit aliases and plurals", () => {
  for (const [noun, assetId] of [
    ["houses", "cottage"],
    ["manors", "townhouse"],
    ["casks", "barrel"],
    ["benches", "bench"],
    ["chairs", "chair"],
    ["market stands", "market-stall"],
    ["wagons", "cart"],
    ["fountains", "fountain"],
  ]) {
    assert.deepEqual(parseCommand(`Place 3 ${noun} on my right`), {
      kind: "asset.add",
      assetId,
      count: 3,
      placement: "right",
    });
  }
});
test("all eight Quaternius starter props resolve to exact catalog identities", () => {
  for (const [noun, suffix] of [
    ["cauldrons", "cauldron"],
    ["lecterns", "bookstand"],
    ["books", "book_stack_2"],
    ["bookshelves", "bookcase_2"],
    ["candlesticks", "candlestick_stand"],
    ["small cages", "cage_small"],
    ["chain coils", "chain_coil"],
    ["wooden chests", "chest_wood"],
  ])
    assert.deepEqual(parseCommand(`Add two ${noun} to my left`), {
      kind: "asset.add",
      assetId: `quaternius:fantasy-standard:${suffix}`,
      count: 2,
      placement: "left",
    });
  assert.deepEqual(parseCommand("Add a cauldron in front of me"), {
    kind: "asset.add",
    assetId: "quaternius:fantasy-standard:cauldron",
    count: 1,
    placement: "ahead",
  });
});
test("counts and spatial clauses cannot silently drop or partially apply", () => {
  for (const text of [
    "add zero barrels",
    "add 9 barrels",
    "add twenty barrels",
    "add 10000000000000000 barrels",
  ])
    assert.throws(() => parseCommand(text), /between 1 and 8/);
  for (const text of [
    "add two barrels behind me",
    "add two barrels and three chairs",
    "add two barrels at (2, 3)",
  ])
    assert.throws(() => parseCommand(text), /Choose in front/);
});
test("room presets, exact metric dimensions, and directions remain explicit", () => {
  assert.deepEqual(parseCommand("add a room"), {
    kind: "room.add",
    size: 6,
    side: "ahead",
  });
  assert.deepEqual(parseCommand("Build a large room to the left"), {
    kind: "room.add",
    size: 8,
    side: "left",
  });
  assert.deepEqual(parseCommand("add a 4.5 meter room to my right"), {
    kind: "room.add",
    size: 4.5,
    side: "right",
  });
  assert.throws(() => parseCommand("add a 20 meter room"), /3 to 10/);
});
test("wall edits and history are not interpreted as asset deletion", () => {
  assert.deepEqual(parseCommand("Please knock down this wall!"), {
    kind: "wall.remove",
  });
  assert.deepEqual(parseCommand("remove the wall"), { kind: "wall.remove" });
  assert.deepEqual(parseCommand("Could you redo that?"), {
    kind: "history.redo",
  });
});
test("selected-object edits require a stable selected identity and respect scale limits", () => {
  const selection = { id: "selected-barrel", scale: 1 };
  assert.deepEqual(parseCommand("make it bigger", selection), {
    kind: "entity.transform",
    id: selection.id,
    scale: 1.25,
  });
  assert.deepEqual(parseCommand("make that smaller", selection), {
    kind: "entity.transform",
    id: selection.id,
    scale: 0.8,
  });
  assert.deepEqual(parseCommand("scale it to 2x", selection), {
    kind: "entity.transform",
    id: selection.id,
    scale: 2,
  });
  assert.deepEqual(parseCommand("delete this object", selection), {
    kind: "entity.remove",
    id: selection.id,
  });
  assert.throws(() => parseCommand("make it bigger"), /Select an object/);
  assert.throws(
    () => parseCommand("make it bigger", { ...selection, scale: 4 }),
    /0.25 to 4/,
  );
  assert.throws(() => parseCommand("remove a cottage", selection));
});
test("unknown assets, extra instructions, and oversized requests do not become executable source", () => {
  for (const text of [
    "add a dragon",
    "run fetch('https://example.com')",
    "add a room; delete all",
    "",
    "a".repeat(1201),
  ])
    assert.throws(() => parseCommand(text));
});
