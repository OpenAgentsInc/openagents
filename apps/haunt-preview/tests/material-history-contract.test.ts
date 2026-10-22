import assert from "node:assert/strict";
import { test } from "node:test";
import {
  SceneHistory,
  initialProject,
  PROJECT_ID,
  type Operation,
  type Transaction,
} from "../src/project.ts";

function fixture() {
  const project = initialProject();
  const history = new SceneHistory({
    ...project,
    materialDraft: { asset: project.material, source: "baseline" },
  });
  const source = (text: string): Operation => ({
    kind: "set_material_source",
    asset: project.material,
    source: text,
  });
  const tx = (operation: Operation): Transaction => ({
    id: crypto.randomUUID(),
    project: PROJECT_ID,
    baseRevision: history.project.revision,
    operation,
  });
  const apply = (operation: Operation) => {
    const transaction = tx(operation);
    history.prepare(transaction).commit();
    return transaction;
  };
  const snapshot = () => structuredClone({
    project: history.project,
    past: history.past,
    future: history.future,
    seen: [...history.seen],
  });
  return { history, source, tx, apply, snapshot };
}

test("source and scene edits share undo and redo while keeping stable scene data", () => {
  const { history, source, apply } = fixture();
  const original = structuredClone(history.project);
  apply(source("invalid wgsl {"));
  apply({ kind: "set_tint", tint: "blue" });
  apply(source(""));
  apply({ kind: "undo" });
  assert.equal(history.project.materialDraft?.source, "invalid wgsl {");
  assert.equal(history.project.tint, "blue");
  apply({ kind: "undo" });
  assert.equal(history.project.tint, original.tint);
  apply({ kind: "undo" });
  assert.equal(history.project.materialDraft?.source, "baseline");
  for (let i = 0; i < 3; i++) apply({ kind: "redo" });
  assert.equal(history.project.materialDraft?.source, "");
  assert.equal(history.project.tint, "blue");
  assert.equal(history.project.revision, 9);
  assert.deepEqual(history.project.entities, original.entities);
  assert.deepEqual(history.project.layout, original.layout);
});

test("replayed, stale, foreign, and oversized source edits leave history unchanged", () => {
  const { history, source, tx, apply, snapshot } = fixture();
  const replay = apply(source("first"));
  const before = snapshot();
  const foreign = {
    kind: "set_material_source",
    asset: "../other.wgsl",
    source: "foreign",
  } as unknown as Operation;
  for (const bad of [
    replay,
    { ...tx(source("stale")), baseRevision: 0 },
    tx(foreign),
    tx(source("a".repeat(65_537))),
  ]) assert.throws(() => history.prepare(bad));
  assert.deepEqual(snapshot(), before);
  const pending = history.prepare(tx(source("late")));
  apply({ kind: "set_tint", tint: "blue" });
  const concurrent = snapshot();
  assert.throws(() => pending.commit());
  assert.deepEqual(snapshot(), concurrent);
});

test("mixed source batches commit once and failed batches apply nothing", () => {
  const { history, source, tx, apply, snapshot } = fixture();
  apply({
    kind: "batch",
    operations: [source("batch"), { kind: "set_tint", tint: "green" }],
  });
  assert.equal(history.project.revision, 1);
  assert.equal(history.past.length, 1);
  assert.equal(history.project.materialDraft?.source, "batch");
  assert.equal(history.project.tint, "green");
  apply({ kind: "undo" });
  assert.equal(history.project.materialDraft?.source, "baseline");
  assert.equal(history.project.tint, "warm");
  const before = snapshot();
  assert.throws(() => history.prepare(tx({
    kind: "batch",
    operations: [
      source("partial"),
      { kind: "move_asset", id: "absent", x: 0, z: 0 },
    ],
  })));
  assert.deepEqual(snapshot(), before);
});

test("source edits use the existing bounded history", () => {
  const { history, source, apply } = fixture();
  for (let i = 0; i < 40; i++) apply(source(`edit-${i}`));
  assert.equal(history.past.length, 32);
  for (let i = 0; i < 32; i++) apply({ kind: "undo" });
  assert.equal(history.future.length, 32);
  assert.throws(() => apply({ kind: "undo" }));
  apply(source("new branch"));
  assert.equal(history.future.length, 0);
});
