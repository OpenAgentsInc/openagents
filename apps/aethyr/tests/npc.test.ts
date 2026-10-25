import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  NPC_FIGURES,
  NPC_LIMITS,
  NpcDirector,
  npcOccluders,
  npcWorld,
  segmentBlocked,
  validateNpcWorld,
} from "../packages/runtime/npc.ts";
import {
  NPC_FIGURE_PARTS,
  NPC_LABEL_HEIGHT,
  npcInstanceLayout,
  npcInstanceMatrices,
} from "../packages/renderer/npc-figures.ts";

const dataset = JSON.parse(
  readFileSync(
    new URL("../packages/runtime/npc-dataset.json", import.meta.url),
    "utf8",
  ),
);

test("the checked-in dataset validates and names the house and planet characters", () => {
  assert.equal(dataset.schema, "openagents.aethyr.npcs.v1");
  for (const world of Object.values(dataset.worlds)) validateNpcWorld(world);
  assert.deepEqual(
    npcWorld("haunted-house")!.npcs.map((n) => n.id),
    ["cat", "witch", "ghost"],
  );
  assert.equal(npcWorld("medieval-town"), undefined);
  for (const figure of NPC_FIGURES) {
    assert.ok(NPC_FIGURE_PARTS[figure].length > 0);
    assert.ok(NPC_LABEL_HEIGHT[figure] > 0);
  }
});

test("a broken dataset is refused with a reason", () => {
  const world = structuredClone(dataset.worlds["haunted-house"]);
  world.npcs[1].id = world.npcs[0].id;
  assert.throws(() => validateNpcWorld(world), /character dataset is invalid/);
  const empty = structuredClone(dataset.worlds["haunted-house"]);
  empty.npcs[0].lines.greet = [];
  assert.throws(() => validateNpcWorld(empty), /character dataset is invalid/);
});

test("characters greet once per approach and re-arm after the player leaves", () => {
  const npcs = NpcDirector.forWorld("haunted-house")!;
  const cat = npcs.find("cat")!;
  const far: [number, number, number] = [
    cat.position[0],
    0,
    cat.position[2] + 8,
  ];
  const near: [number, number, number] = [
    cat.position[0],
    0,
    cat.position[2] + 1,
  ];
  assert.deepEqual(npcs.update(far, 1), []);
  const first = npcs.update(near, 2);
  assert.equal(first.length, 1);
  assert.equal(first[0].kind, "greet");
  assert.equal(first[0].npc, "cat");
  assert.deepEqual(npcs.update(near, 3), [], "no repeat while still close");
  npcs.update(far, 4);
  const again = npcs.update(near, 5);
  assert.equal(again.length, 1);
  assert.notEqual(again[0].text, first[0].text, "the next greeting differs");
  assert.ok(again[0].sequence > first[0].sequence);
  assert.equal(npcs.view().nearby, "cat");
  // The greeting carries a gesture in the pose.
  assert.ok(npcs.poses().find((p) => p.id === "cat")!.emote);
});

test("hints follow the player's progress through the house", () => {
  const npcs = NpcDirector.forWorld("haunted-house")!;
  npcs.update([1.7, 0, 10], 1); // yard, beside the cat
  assert.match(npcs.talk("hint").text, /front door into the hall/);
  npcs.update([2, 0, 0], 2); // the hall, away from the witch
  assert.match(npcs.talk("hint", "cat").text, /passage/);
  npcs.update([0, 0, -13], 3); // the gallery: the ghost greets
  assert.match(npcs.talk("hint", "ghost").text, /witch/);
  npcs.talk("reply", "witch");
  assert.match(npcs.talk("hint", "ghost").text, /Edit/);
});

test("talk resolves a named character, the nearest one, or a readable refusal", () => {
  const npcs = NpcDirector.forWorld("haunted-house")!;
  assert.throws(() => npcs.talk("reply"), /Walk up to someone/);
  assert.throws(() => npcs.talk("reply", "dracula"), /no one called dracula/);
  const reply = npcs.talk("reply", "Ghost");
  assert.equal(reply.npc, "ghost");
  assert.equal(reply.kind, "reply");
  const emote = npcs.talk("emote", "black-cat");
  assert.equal(emote.npc, "cat");
  assert.ok(emote.emote);
  const view = npcs.view();
  assert.ok(view.lines.length <= NPC_LIMITS.lines);
  assert.ok(
    view.lines.every(
      (l, i, all) => i === 0 || l.sequence > all[i - 1].sequence,
    ),
  );
  assert.equal(npcs.recent(7).get("cat")?.kind, "emote");
});

test("a planet shows one character per style and keeps line numbers rising", () => {
  const pastel = NpcDirector.forWorld("personal-planet", "soft-pastels")!;
  assert.deepEqual(
    pastel.npcs.map((n) => n.id),
    ["gardener"],
  );
  const line = pastel.talk("reply", "gardener");
  const bold = NpcDirector.forWorld(
    "personal-planet",
    "bold-fantasy",
    pastel.lastSequence,
  )!;
  assert.deepEqual(
    bold.npcs.map((n) => n.id),
    ["ranger"],
  );
  assert.ok(bold.talk("reply", "ranger").sequence > line.sequence);
});

test("walls hide labels; floors and ceilings never do", () => {
  const walls = npcOccluders([
    { id: "hall:wall:n", min: [-4, 0, -0.1], max: [4, 3, 0.1] },
    { id: "hall:floor", min: [-4, -0.1, -5], max: [4, 0, 5] },
    { id: "hall:ceiling", min: [-4, 3, -5], max: [4, 3.2, 5] },
  ]);
  assert.equal(walls.length, 1);
  assert.equal(segmentBlocked([0, 1.6, 2], [0, 1.6, -2], walls), true);
  assert.equal(segmentBlocked([0, 1.6, 2], [0, 1.6, 0.5], walls), false);
});

test("figure instances cover every part and stay finite while emoting", () => {
  const npcs = NpcDirector.forWorld("haunted-house")!;
  const figures = npcs.npcs.map((n) => n.figure);
  const layout = npcInstanceLayout(figures);
  const parts = figures.reduce((sum, f) => sum + NPC_FIGURE_PARTS[f].length, 0);
  assert.equal(layout.order.length, parts);
  assert.equal(
    layout.groups.reduce((sum, g) => sum + g.count, 0),
    parts,
  );
  npcs.talk("emote", "witch");
  const matrices = npcInstanceMatrices(layout, figures, npcs.poses(), 0.5);
  assert.equal(matrices.length, parts * 16);
  assert.ok(matrices.every(Number.isFinite));
});
