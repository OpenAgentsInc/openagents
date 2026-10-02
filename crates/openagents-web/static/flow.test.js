// The event-to-animation mapping of static/flow.js (#10197), the same table
// the desktop deck's `routes-live` scene follows
// (crates/openagents-desktop/src/route_live.rs). Run: node --test
// crates/openagents-web/static/flow.test.js (the crate's tests run it when
// node is installed). Not served.
"use strict";
var test = require("node:test");
var assert = require("node:assert");
var fs = require("node:fs");
var path = require("node:path");
var flow = require("./flow.js");

// The committed map's shape around Coder, with the snapshot's wire form:
// parents by id, positions as {position:{x,y}}.
var map = flow.topology([
  { id: "front", kind: "front", parent: null, position: { x: 0, y: 0 } },
  { id: "family:work", kind: "family", parent: "front", position: { x: -100, y: 0 } },
  { id: "route:work.dispatch", kind: "route", parent: "family:work", position: { x: -200, y: 0 } },
  { id: "coder", kind: "coder", parent: "route:work.dispatch", position: { x: -300, y: 0 } },
  { id: "plugin:crates/plugin-explain-error", kind: "plugin", parent: "coder", position: { x: -400, y: 30 } },
  { id: "plugin:crates/plugin-repo-map", kind: "plugin", parent: "coder", position: { x: -400, y: -30 } },
  { id: "engine:codex", kind: "engine", parent: "coder", position: { x: -400, y: 0 } }
]);
var fixture = fs.readFileSync(
  path.join(__dirname, "../../../docs/payments/fixtures/flow-stream.jsonl"), "utf8")
  .split("\n").filter(Boolean).map(function (line) { return JSON.parse(line); });

function id(i) { return map.nodes[i].id; }

test("the topology reads parents by id or index", function () {
  assert.strictEqual(map.nodes[3].parent, 2);
  var byIndex = flow.topology({ nodes: [{ id: "a", kind: "front", parent: null, x: 1, y: 2 },
    { id: "b", kind: "route", parent: 0, x: 3, y: 4 }] });
  assert.strictEqual(byIndex.nodes[1].parent, 0);
  assert.deepStrictEqual(flow.place(byIndex, 0), { x: 1, y: 2 });
});

test("an event's node finds its place on the map", function () {
  assert.strictEqual(id(flow.target(map, { type: "call", resource: "plugin", node: "plugin:explain-error" })),
    "plugin:crates/plugin-explain-error");
  assert.strictEqual(id(flow.target(map, { type: "call", node: "engine:codex" })), "engine:codex");
  assert.strictEqual(id(flow.target(map, { type: "call", resource: "plugin", node: "plugin:new-one" })), "coder");
  assert.strictEqual(id(flow.target(map, { type: "run", resource: "coder", node: "coder" })), "coder");
  assert.strictEqual(id(flow.target(map, { type: "call", resource: "route", node: "route:x" })), "front");
});

test("each event type sends its dot", function () {
  var leg = function (type) {
    var legs = flow.legs(map, { type: type, resource: "plugin", node: "plugin:repo-map" });
    assert.strictEqual(legs.length, 1, type);
    return legs[0];
  };
  var plugin = map.index["plugin:crates/plugin-repo-map"];
  var call = leg("call");
  assert.strictEqual(call.color, flow.REQUEST);
  assert.deepStrictEqual(call.stops, [0, 1, 2, 3, plugin]);
  var payment = leg("payment");
  assert.strictEqual(payment.color, flow.PAYMENT);
  assert.deepStrictEqual(payment.stops, [plugin, 3, 2, 1, 0]);
  var share = leg("share");
  assert.strictEqual(share.color, flow.PAYMENT);
  assert.strictEqual(share.ring, false);
  assert.deepStrictEqual(share.stops[share.stops.length - 1], { past: plugin, distance: flow.AUTHOR });
  var bonus = leg("bonus");
  assert.strictEqual(bonus.ring, true);
  assert.deepStrictEqual(bonus.stops, share.stops);
  var payout = leg("payout");
  assert.deepStrictEqual(payout.stops, [0, { past: plugin, distance: flow.WALLET }]);
  assert.strictEqual(flow.legs(map, { type: "run", resource: "coder", node: "coder" })[0].color, flow.REQUEST);
  assert.deepStrictEqual(flow.legs(map, { type: "refund", node: "front" }), []);
  assert.deepStrictEqual(flow.legs(flow.topology([]), { type: "call", node: "front" }), []);
});

test("the author and the wallet sit past the node, away from its parent", function () {
  var plugin = map.index["plugin:crates/plugin-repo-map"];
  var author = flow.place(map, { past: plugin, distance: flow.AUTHOR });
  var wallet = flow.place(map, { past: plugin, distance: flow.WALLET });
  assert.ok(author.x < -400 && wallet.x < author.x);
  var gap = Math.hypot(author.x + 400, author.y + 30);
  assert.ok(Math.abs(gap - flow.AUTHOR) < 0.01, String(gap));
});

test("a payment waits for its call to land, and landed dots go", function () {
  var schedule = new flow.Schedule();
  fixture.slice(0, 3).forEach(function (event) { schedule.push(map, event, 10); });
  var starts = schedule.flights.map(function (f) { return f.start; });
  assert.deepStrictEqual(starts, [10, 10 + flow.TRIP, 10 + flow.MAX_WAIT]);
  schedule.push(map, fixture[3], 10);
  assert.strictEqual(schedule.flights[3].start, 10);
  var mid = schedule.pulses(map, 10.8, false);
  assert.strictEqual(mid.length, 2);
  assert.ok(mid.every(function (p) { return p.color === flow.REQUEST; }));
  assert.ok(schedule.pulses(map, 10 + flow.TRIP + 0.5, false).some(function (p) { return p.color === flow.PAYMENT; }));
  // Reduced motion: each dot sits where it lands.
  var plugin = map.index["plugin:crates/plugin-explain-error"];
  var still = schedule.pulses(map, 10.1, true)[0];
  var end = flow.place(map, plugin);
  assert.ok(Math.hypot(still.x - end.x, still.y - end.y) < 0.1);
  schedule.land(100);
  assert.strictEqual(schedule.flights.length, 0);
});

test("the fixture counts its totals and reads plainly", function () {
  var totals = { received_sats: 0, paid_out_sats: 0, calls: 0 };
  fixture.forEach(function (event) { flow.count(totals, event); });
  assert.deepStrictEqual(totals, { received_sats: 64, paid_out_sats: 40, calls: 4 });
  assert.strictEqual(flow.grouped(1234567), "1,234,567");
  assert.strictEqual(flow.utc(1791043200123), "2026-10-03 16:00:00 UTC");
  assert.strictEqual(flow.describe(fixture[1]), "16:00:00  payment · 31 sats · explain-error");
  // Nothing private in a line: no payer alias.
  fixture.forEach(function (event) {
    assert.ok(flow.describe(event).indexOf("caller-") < 0);
  });
});
