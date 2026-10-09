// The pure helpers of static/components/overlays.js (UI-05). Run: node
// --test crates/openagents-ui/tests/js/overlays.test.js (the crate's tests
// run it when node is installed). Kept out of static/components so the
// script bundle does not pick it up.
"use strict";
var test = require("node:test");
var assert = require("node:assert");
var overlays = require("../../static/components/overlays.js");

test("arrow keys wrap and start at the near end", function () {
  var step = overlays.step;
  assert.strictEqual(step(-1, 4, "ArrowDown"), 0);
  assert.strictEqual(step(-1, 4, "ArrowUp"), 3);
  assert.strictEqual(step(3, 4, "ArrowDown"), 0);
  assert.strictEqual(step(0, 4, "ArrowUp"), 3);
  assert.strictEqual(step(1, 4, "ArrowDown"), 2);
  assert.strictEqual(step(2, 4, "Home"), 0);
  assert.strictEqual(step(0, 4, "End"), 3);
  assert.strictEqual(step(2, 4, "x"), 2);
  assert.strictEqual(step(0, 0, "ArrowDown"), -1);
});

test("type-ahead finds a prefix, cycles a repeated letter, keeps a longer match", function () {
  var labels = ["Settings", "Sign out", "Billing", "  share link "];
  var typeahead = overlays.typeahead;
  assert.strictEqual(typeahead(labels, -1, "b"), 2);
  assert.strictEqual(typeahead(labels, -1, "s"), 0);
  assert.strictEqual(typeahead(labels, 0, "s"), 1);
  assert.strictEqual(typeahead(labels, 1, "ss"), 3);
  assert.strictEqual(typeahead(labels, 3, "s"), 0);
  assert.strictEqual(typeahead(labels, 1, "si"), 1);
  assert.strictEqual(typeahead(labels, 0, "SIG"), 1);
  assert.strictEqual(typeahead(labels, 0, "share l"), 3);
  assert.strictEqual(typeahead(labels, 0, "z"), -1);
  assert.strictEqual(typeahead([], 0, "a"), -1);
  assert.strictEqual(typeahead(labels, 0, ""), -1);
});

test("search matches case-insensitive substrings", function () {
  assert.ok(overlays.matches("OpenAgents", "agent"));
  assert.ok(overlays.matches("OpenAgents", ""));
  assert.ok(overlays.matches("Open  Agents", "open agents"));
  assert.ok(!overlays.matches("psionic", "probe"));
});

test("fallback placement opens below, flips above, aligns and clamps", function () {
  var position = overlays.position;
  var viewport = { width: 800, height: 600 };
  var size = { width: 200, height: 100 };
  var rect = { top: 100, bottom: 130, left: 50, right: 150, width: 100, height: 30 };
  assert.deepStrictEqual(position(rect, size, viewport, "bottom", "start", 4), { top: 134, left: 50 });
  var high = { top: 300, bottom: 330, left: 50, right: 150, width: 100, height: 30 };
  assert.deepStrictEqual(position(high, size, viewport, "top", "start", 4), { top: 196, left: 50 });
  // Not enough room above for "top": opens below instead.
  assert.deepStrictEqual(position(rect, size, viewport, "top", "start", 4), { top: 134, left: 50 });
  // Not enough room below for "bottom": flips above.
  var bottom = { top: 520, bottom: 550, left: 50, right: 150, width: 100, height: 30 };
  assert.deepStrictEqual(position(bottom, size, viewport, "bottom", "start", 4), { top: 416, left: 50 });
  // Center and end alignment.
  assert.strictEqual(position(rect, size, viewport, "bottom", "center", 4).left, 8);
  var mid = { top: 100, bottom: 130, left: 400, right: 500, width: 100, height: 30 };
  assert.strictEqual(position(mid, size, viewport, "bottom", "center", 4).left, 350);
  assert.strictEqual(position(mid, size, viewport, "bottom", "end", 4).left, 300);
  // Clamped inside the right edge.
  var edge = { top: 100, bottom: 130, left: 750, right: 790, width: 40, height: 30 };
  assert.strictEqual(position(edge, size, viewport, "bottom", "start", 4).left, 592);
  // Sides: right flips left near the edge; left/right align vertically.
  assert.deepStrictEqual(position(mid, size, viewport, "right", "start", 4), { top: 100, left: 504 });
  assert.deepStrictEqual(position(edge, size, viewport, "right", "center", 4), { top: 65, left: 546 });
});
