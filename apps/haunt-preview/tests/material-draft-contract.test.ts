import assert from "node:assert/strict";
import { test } from "node:test";
import {
  initialProject,
  readProject,
  MAX_SCENE_BYTES,
  MAX_PROJECT_BYTES,
} from "../src/project.ts";
import { MAX_SOURCE_BYTES } from "../src/protocol.ts";

test("legacy projects and invalid or empty shader drafts round trip without runtime fields", () => {
  const project = initialProject();
  assert.deepEqual(readProject(project), project);
  assert.equal("materialDraft" in readProject(project), false);
  for (const source of ["", "invalid wgsl {"]) {
    const draft = { asset: project.material, source, device: { private: true } };
    const input = { ...project, materialDraft: draft };
    const before = structuredClone(input);
    const saved = readProject(input);
    assert.deepEqual(saved.materialDraft, { asset: project.material, source });
    assert.notEqual(saved.materialDraft, draft);
    assert.deepEqual(saved.entities, project.entities);
    assert.deepEqual(saved.layout, project.layout);
    assert.deepEqual(input, before);
  }
});

test("raw shader bytes and JSON escaping have separate size allowances", () => {
  const project = initialProject();
  for (const source of [
    "a".repeat(MAX_SOURCE_BYTES),
    "\u0000".repeat(MAX_SOURCE_BYTES),
    "😀".repeat(MAX_SOURCE_BYTES / 4),
  ]) {
    const saved = readProject({
      ...project,
      materialDraft: { asset: project.material, source },
    });
    assert.equal(saved.materialDraft?.source, source);
    assert.ok(Buffer.byteLength(JSON.stringify(saved)) <= MAX_PROJECT_BYTES);
  }
  assert.equal(MAX_PROJECT_BYTES, MAX_SCENE_BYTES + 6 * MAX_SOURCE_BYTES + 256);
});

test("malformed, foreign, and oversized drafts fail without changing their scene", () => {
  const project = initialProject();
  const before = structuredClone(project);
  for (const materialDraft of [
    null,
    [],
    {},
    "source",
    { asset: "../other.wgsl", source: "" },
    { asset: project.material, source: 17 },
    { asset: project.material, source: "a".repeat(MAX_SOURCE_BYTES + 1) },
    { asset: project.material, source: "😀".repeat(MAX_SOURCE_BYTES / 4 + 1) },
  ]) {
    assert.throws(() => readProject({ ...project, materialDraft }));
    assert.deepEqual(project, before);
  }
});
