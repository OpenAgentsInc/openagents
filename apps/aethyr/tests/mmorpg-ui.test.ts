import assert from "node:assert/strict";
import test from "node:test";
import {
  mmoLogLines,
  MMO_STAT_ROWS,
  projectMmoName,
  mmoNameSpriteSize,
  mmoUiImageURL,
  paintMmoName,
  mmoDraftLabel,
} from "../apps/editor/src/mmorpg-ui-model.ts";
import type {
  MmoActor,
  MmoEvent,
} from "../packages/runtime/mmorpg-protocol.ts";
const player = { id: 1, name: "Kiki" } as MmoActor;
test("the local draft label follows the last save result", () => {
  assert.equal(mmoDraftLabel(true), "Saved on this device");
  assert.equal(mmoDraftLabel(false), "Local draft");
});
const actors = [
  { id: 2, name: "Zombie" },
  { id: 3, name: "Chris" },
] as MmoActor[];
test("the source chat feed includes nearby chat and only fights involving this player", () => {
  const events: MmoEvent[] = [
    { id: 1, tick: 1, kind: "join", actor: 3, name: "Chris" },
    {
      id: 2,
      tick: 2,
      kind: "chat",
      actor: 3,
      name: "Chris",
      text: "Hello <script>",
    },
    { id: 3, tick: 3, kind: "damage", actor: 1, target: 2, amount: 300 },
    { id: 4, tick: 4, kind: "damage", actor: 2, target: 1, amount: 5 },
    { id: 5, tick: 5, kind: "damage", actor: 2, target: 3, amount: 5 },
    { id: 6, tick: 6, kind: "inventory", actor: 1 },
    { id: 7, tick: 7, kind: "death", actor: 2, target: 1 },
    { id: 8, tick: 8, kind: "death", actor: 1, target: 2 },
  ];
  assert.deepEqual(
    mmoLogLines(events, player, actors).map((line) => line.text),
    [
      "[Chris has entered the game]",
      "[Chris]: Hello <script>",
      "You hit Zombie for 300 damage!",
      "Zombie hit you for 5 damage!",
      "You were defeated.",
    ],
  );
});
test("name projection respects camera orientation, aspect ratio and near/offscreen hiding", () => {
  const view = {
    eye: [0, 0, 0] as [number, number, number],
    target: [0, 0, -1] as [number, number, number],
  };
  assert.deepEqual(projectMmoName([0, 0, -10], view, 1280, 720), [640, 360]);
  assert.ok(projectMmoName([1, 1, -10], view, 1280, 720)![0] > 640);
  assert.ok(projectMmoName([1, 1, -10], view, 1280, 720)![1] < 360);
  assert.equal(projectMmoName([0, 0, 10], view, 1280, 720), null);
  assert.equal(projectMmoName([0, 0, -0.1], view, 1280, 720), null);
  assert.equal(projectMmoName([1000, 0, -10], view, 1280, 720), null);
  assert.equal(
    projectMmoName(
      [0, 0, -10],
      { eye: [0, 0, 0], target: [0, 0, 0] },
      1280,
      720,
    ),
    null,
  );
  assert.equal(projectMmoName([0, 0, -10], view, 0, 720), null);
});

test("source UI pictures stay on the editor origin under cross-origin isolation", () => {
  assert.equal(
    mmoUiImageURL("/private-assets/health.png", "http://localhost:55316"),
    "http://localhost:55316/private-assets/health.png",
  );
  assert.equal(
    mmoUiImageURL(
      "/private-assets/health.png",
      "https://editor.example.test/app/",
    ),
    "https://editor.example.test/private-assets/health.png",
  );
  for (const path of [
    "https://preview.example.test/private-assets/health.png",
    "/private-assets/../health.png",
    "javascript:alert(1)",
  ])
    assert.throws(() => mmoUiImageURL(path, "https://editor.example.test"));
});

test("name canvases keep the source white Helvetica text and world-sized sprite projection", () => {
  const calls: unknown[][] = [];
  const context = {
    clearRect: (...args: number[]) => calls.push(args),
    fillText: (...args: unknown[]) => calls.push(args),
  } as unknown as CanvasRenderingContext2D;
  paintMmoName(context, "Zombie");
  assert.equal(context.font, "18pt Helvetica");
  assert.equal(context.fillStyle, "#FFF");
  assert.equal(context.shadowOffsetX, 3);
  assert.equal(context.shadowOffsetY, 3);
  assert.equal(context.shadowBlur, 4);
  assert.equal(context.textAlign, "center");
  assert.deepEqual(calls, [
    [0, 0, 256, 128],
    ["Zombie", 128, 64],
  ]);
  const view = {
    eye: [0, 0, 0] as [number, number, number],
    target: [0, 0, -1] as [number, number, number],
  };
  const near = mmoNameSpriteSize([0, 0, -10], view, 720)!;
  const far = mmoNameSpriteSize([0, 0, -20], view, 720)!;
  assert.equal(near[0] / near[1], 2);
  assert.equal(near[0], far[0] * 2);
  assert.ok(
    Math.abs(near[0] - ((10 / 6) * 720) / (2 * Math.tan(Math.PI / 6) * 10)) <
      1e-9,
  );
  assert.equal(mmoNameSpriteSize([0, 0, 1], view, 720), null);
});

test("all five source statistics keep their exact tooltip copy and key mapping", () => {
  assert.deepEqual(
    MMO_STAT_ROWS.map(({ key, label }) => [key, label]),
    [
      ["strength", "Strength"],
      ["wisdomness", "Wisdomness"],
      ["benchpress", "Benchpress"],
      ["curl", "Curl"],
      ["experience", "XP"],
    ],
  );
  assert.deepEqual(
    MMO_STAT_ROWS.map(({ tooltip }) => tooltip),
    [
      "How strong you are, affects how much damage you do. So blah blah if you're doing stuff then its stronger or whatever, the damage is up. This is text to show the tooltip.",
      "Wisdom is the guage of something to do with wisdom in the game because wisdom is important and wisdom is wise to wisdoming.",
      "Gotta have a good bench to be a warrior or something.",
      "The ultimate expression of strength, this affects literally everything in your life.",
      "How much xp you've accumulated by killing things for xp. Get enough and you'll gain a level or something.",
    ],
  );
});
