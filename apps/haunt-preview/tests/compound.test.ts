import assert from "node:assert/strict";
import { test } from "node:test";
import { compileDecision } from "../src/compound.ts";
import {
  SceneHistory,
  PROJECT_ID,
  type Decision,
  type Operation,
} from "../src/project.ts";
const d: Decision = {
  action: "add_asset",
  asset: "ghost",
  placement: "anywhere",
  size: "normal",
  tint: "blue",
  behavior: "chase",
  confidence: 1,
  needs_review: false,
  model: "fixture",
  model_ms: 1,
};
function plan(
  parts: { text: string; decision: Partial<Decision>; reference?: string }[],
) {
  const text = parts.map((p) => p.text).join(" and ");
  let position = 0;
  const steps = parts.map((p) => {
    const start = new TextEncoder().encode(text.slice(0, position)).length;
    position += p.text.length + 5;
    return {
      start,
      end: start + new TextEncoder().encode(p.text).length,
      text: p.text,
      reference: p.reference ?? "none",
      decision: { ...d, ...p.decision },
    };
  });
  return { text, decision: { ...d, action: "compound", steps } };
}
const apply = (s: SceneHistory, operation: Operation) =>
  s
    .prepare({
      id: crypto.randomUUID(),
      project: PROJECT_ID,
      baseRevision: s.project.revision,
      operation,
    })
    .commit();
const pose = { x: 0, z: 6.8, yaw: 0, pitch: 0 };
test("new ghosts chase as one saved edit, with existing ghosts and history untouched until commit", () => {
  for (const count of [3, 8]) {
    const s = new SceneHistory();
    apply(s, {
      kind: "add_asset",
      entity: {
        id: "ghost-existing",
        kind: "ghost",
        x: 1,
        z: 2,
        scale: 1,
        behavior: "float",
      },
    });
    const before = structuredClone(s.project),
      oldHistory = s.past.length;
    const p = plan([
      { text: `add ${count} ghosts`, decision: {} },
      {
        text: "make them chase me",
        decision: { action: "set_behavior" },
        reference: "new_assets",
      },
    ]);
    const op = compileDecision(
      p.decision,
      p.text,
      pose,
      "ghost-existing",
      s.project,
    );
    assert.deepEqual(s.project, before);
    assert.equal(s.past.length, oldHistory);
    apply(s, op);
    assert.equal(s.project.revision, before.revision + 1);
    assert.equal(s.past.length, oldHistory + 1);
    assert.equal(
      s.project.entities.find((e) => e.id === "ghost-existing")!.behavior,
      "float",
    );
    const added = s.project.entities.slice(before.entities.length);
    assert.equal(added.length, count);
    assert.ok(added.every((e) => e.kind === "ghost" && e.behavior === "chase"));
    apply(s, { kind: "undo" });
    assert.deepEqual(s.project.entities, before.entities);
    apply(s, { kind: "redo" });
    assert.deepEqual(s.project.entities.slice(before.entities.length), added);
  }
});
test("independent additions and lighting preserve each clause's quantity and form one transaction", () => {
  const s = new SceneHistory(),
    before = structuredClone(s.project);
  const p = plan([
    { text: "add 2 pumpkins", decision: { asset: "pumpkin" } },
    { text: "add 3 candles", decision: { asset: "candle" } },
    { text: "make the light blue", decision: { action: "set_tint" } },
  ]);
  apply(s, compileDecision(p.decision, p.text, pose, null, s.project));
  assert.equal(s.project.entities.filter((e) => e.kind === "candle").length, 3);
  assert.equal(
    s.project.entities.filter((e) => e.kind === "pumpkin").length,
    7,
  );
  assert.equal(s.project.tint, "blue");
  assert.equal(s.past.length, 1);
  apply(s, { kind: "undo" });
  assert.deepEqual(s.project.entities, before.entities);
  assert.equal(s.project.tint, before.tint);
});
test("a failed later clause, invalid references, source edits, and nested history commands never partially apply", () => {
  for (const second of [
    { action: "set_behavior", behavior: "attack" },
    { action: "undo" },
    { action: "compound" },
    { action: "add_asset", asset: "dragon" },
    { action: "add_asset", needs_review: true },
  ]) {
    const s = new SceneHistory(),
      before = structuredClone(s.project);
    const p = plan([
      { text: "add 3 ghosts", decision: {} },
      { text: "change them", decision: second, reference: "new_assets" },
    ]);
    assert.throws(() =>
      compileDecision(p.decision, p.text, pose, null, s.project),
    );
    assert.deepEqual(s.project, before);
    assert.equal(s.past.length, 0);
  }
  const p = plan([
    { text: "add 3 ghosts", decision: {} },
    {
      text: "make them chase me",
      decision: { action: "set_behavior" },
      reference: "new_assets",
    },
  ]);
  p.decision.steps[0].text = "add 8 ghosts";
  assert.throws(() =>
    compileDecision(p.decision, p.text, pose, null, new SceneHistory().project),
  );
  const wrong = plan([
    { text: "add 3 pumpkins", decision: { asset: "pumpkin" } },
    {
      text: "make them chase me",
      decision: { action: "set_behavior" },
      reference: "new_assets",
    },
  ]);
  assert.throws(() =>
    compileDecision(
      wrong.decision,
      wrong.text,
      pose,
      null,
      new SceneHistory().project,
    ),
  );
});
test("UTF-8 source spans do not lose references after non-ASCII text", () => {
  const s = new SceneHistory();
  const p = plan([
    { text: "add 2 ghosts 👻", decision: {} },
    {
      text: "make them float",
      decision: { action: "set_behavior", behavior: "float" },
      reference: "new_assets",
    },
  ]);
  apply(s, compileDecision(p.decision, p.text, pose, null, s.project));
  assert.equal(
    s.project.entities.filter(
      (e) => e.kind === "ghost" && e.behavior === "float",
    ).length,
    2,
  );
});

test("a counted subset changes only that many new objects and excessive target counts do not commit", () => {
  const s = new SceneHistory();
  const p = plan([
    { text: "add 5 ghosts", decision: {} },
    {
      text: "make 2 of them chase me",
      decision: { action: "set_behavior" },
      reference: "new_assets",
    },
  ]);
  apply(s, compileDecision(p.decision, p.text, pose, null, s.project));
  assert.equal(
    s.project.entities.filter((e) => e.behavior === "chase").length,
    2,
  );
  const bad = plan([
    { text: "add 3 ghosts", decision: {} },
    {
      text: "make 9 of them chase me",
      decision: { action: "set_behavior" },
      reference: "new_assets",
    },
  ]);
  const before = structuredClone(s.project);
  assert.throws(() =>
    compileDecision(bad.decision, bad.text, pose, null, s.project),
  );
  assert.deepEqual(s.project, before);
});
