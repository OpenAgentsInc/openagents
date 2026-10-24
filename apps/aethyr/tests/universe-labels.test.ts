import { test } from "node:test";
import assert from "node:assert/strict";
import {
  pinchStep,
  placeLabels,
  type Label,
} from "../packages/universe/labels.ts";

const label = (
  id: string,
  x: number,
  y: number,
  depth: number,
  width = 90,
): Label => ({ id, x, y, width, height: 26, depth, inFrame: true });

test("labels that would overlap at phone width show only the nearest", () => {
  // Blue Rush, The far field and OpenAgents projected close together on a
  // 390 px wide frame (the live bug in #12588).
  const crowded = [
    label("openagents", 190, 300, 27),
    label("blue-rush", 205, 312, 30),
    label("far-field", 178, 290, 33),
    label("haunted-house", 320, 420, 25),
  ];
  assert.deepEqual(
    [...placeLabels(crowded, undefined)].sort(),
    ["haunted-house", "openagents"],
  );
});

test("the selected planet's label always shows and wins its space", () => {
  const crowded = [
    label("openagents", 190, 300, 27),
    label("blue-rush", 205, 312, 30),
    label("far-field", 178, 290, 33),
  ];
  assert.deepEqual([...placeLabels(crowded, "far-field")], ["far-field"]);
});

test("labels with room all show, and off-frame labels never do", () => {
  const spread = [
    label("openagents", 100, 100, 27),
    label("blue-rush", 300, 100, 30),
    label("far-field", 100, 400, 33),
    { ...label("haunted-house", 300, 400, 25), inFrame: false },
  ];
  assert.deepEqual(
    [...placeLabels(spread, "haunted-house")].sort(),
    ["blue-rush", "far-field", "openagents"],
  );
});

test("no two shown labels overlap, whatever the layout", () => {
  let seed = 7;
  const random = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let round = 0; round < 200; round++) {
    const labels = ["a", "b", "c", "d", "e", "f"].map((id) =>
      label(id, random() * 390, random() * 500, random() * 40, 60 + random() * 60),
    );
    const shown = labels.filter((l) => placeLabels(labels, "c").has(l.id));
    assert.ok(shown.some((l) => l.id === "c"));
    for (const a of shown)
      for (const b of shown)
        if (a !== b && a.id !== "c" && b.id !== "c")
          assert.ok(
            Math.abs(a.x - b.x) * 2 >= a.width + b.width ||
              Math.abs(a.y - b.y) * 2 >= a.height + b.height,
            `${a.id} overlaps ${b.id}`,
          );
  }
});

test("pinching apart zooms in, together zooms out, and nothing else moves", () => {
  assert.ok(pinchStep(100, 200, 30) < 0);
  assert.ok(pinchStep(200, 100, 30) > 0);
  assert.equal(pinchStep(100, 100, 30), 0);
  assert.equal(pinchStep(0, 100, 30), 0);
  // Spreading the fingers to twice the distance halves the camera distance.
  assert.equal(30 + pinchStep(100, 200, 30), 15);
});
