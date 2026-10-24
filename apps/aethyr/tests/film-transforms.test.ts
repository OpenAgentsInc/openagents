import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import {
  prepareFilmTransforms,
  rotateSourceModel,
  sourceModelSha256,
  validateFilmTransforms,
  type FilmSourceTransform,
} from "../packages/renderer/film-transforms.ts";
import { modelMatrix, transformPoint } from "../packages/renderer/math.ts";
import { validateFrameCaptureOptions } from "../packages/renderer/film-frame.ts";

function scene(): CompiledScene {
  return {
    scenario: "day",
    meshes: [
      { id: "owned-wheel", bounds: { min: [-2, -1, -0.1], max: [2, 1, 0.1] } },
    ],
    instances: [
      { mesh: 0, model: modelMatrix([10, 4, -2]) },
      { mesh: 0, model: modelMatrix([20, 4, -2]) },
    ],
  } as CompiledScene;
}
async function rotation(s = scene()): Promise<FilmSourceTransform> {
  return {
    mode: "day",
    instanceIndex: 0,
    meshId: "owned-wheel",
    baseModelSha256: await sourceModelSha256(s.instances[0].model),
    pivot: [0, 0, 0],
    axis: [0, 0, 1],
    angleRadians: Math.PI / 2,
  };
}
test("source selectors hash exact Float64 LE values and validate mode, mesh and saved model before any change", async () => {
  const s = scene(),
    before = structuredClone(s),
    r = await rotation(s);
  const bytes = Buffer.alloc(128);
  s.instances[0].model.forEach((n, i) => bytes.writeDoubleLE(n, i * 8));
  assert.equal(
    r.baseModelSha256,
    createHash("sha256").update(bytes).digest("hex"),
  );
  const [prepared] = await prepareFilmTransforms(s, [r], "day");
  assert.equal(prepared.instanceIndex, 0);
  assert.deepEqual(
    Array.from(prepared.bounds.aabbs).map((n) => Math.round(n * 10) / 10),
    [9, 2, -2.1, 11, 6, -1.9],
  );
  assert.deepEqual(s, before);
  for (const bad of [
    { ...r, mode: "night" },
    { ...r, instanceIndex: 2 },
    { ...r, meshId: "other" },
    { ...r, baseModelSha256: "0".repeat(64) },
  ])
    await assert.rejects(prepareFilmTransforms(s, [bad], "day"), /changed/);
  s.instances[0].model[12] += 0.0000000001;
  await assert.rejects(prepareFilmTransforms(s, [r], "day"), /changed/);
});
test("local hinge rotation fixes its pivot and random-access reverse samples have no accumulated drift", async () => {
  const base = modelMatrix([3, 4, 5], 0.3, 2);
  base[8] *= -1;
  base[9] *= -1;
  base[10] *= -1;
  const pivot: [number, number, number] = [0.2, 0.4, -0.1],
    axis: [number, number, number] = [0, 3, 0];
  const original = base.slice(),
    fixed = transformPoint(base, pivot);
  const samples = [0, 3, 0, 86400, 0].map((time) =>
    rotateSourceModel(base, { pivot, axis, angleRadians: time * 0.23 }),
  );
  assert.deepEqual(samples[0], samples[2]);
  assert.deepEqual(samples[0], samples[4]);
  for (const m of samples)
    transformPoint(m, pivot).forEach((n, i) =>
      assert.ok(Math.abs(n - fixed[i]) < 1e-6),
    );
  const quarter = rotateSourceModel(modelMatrix([0, 0, 0]), {
    pivot: [0, 0, 0],
    axis: [0, 0, 1],
    angleRadians: Math.PI / 2,
  });
  transformPoint(quarter, [2, 0, 0]).forEach((n, i) =>
    assert.ok(Math.abs(n - [0, 2, 0][i]) < 1e-6),
  );
  assert.deepEqual(base, original);
  const s = scene();
  s.instances[0].model = base;
  assert.equal(
    (await prepareFilmTransforms(s, [await rotation(s)], "day")).length,
    1,
  );
  s.instances[0].model[0] *= 2;
  await assert.rejects(
    prepareFilmTransforms(s, [await rotation(s)], "day"),
    /uniform scale/,
  );
});
test("rotation input is finite, closed, unique and bounded to sixteen objects", async () => {
  const r = await rotation();
  assert.equal(
    validateFrameCaptureOptions({ transforms: [r] }).transforms?.length,
    1,
  );
  assert.deepEqual(validateFilmTransforms([]), []);
  for (const bad of [
    null,
    {},
    [r, r],
    Array.from({ length: 17 }, (_, i) => ({ ...r, instanceIndex: i })),
    [{ ...r, instanceIndex: -1 }],
    [{ ...r, instanceIndex: 65536 }],
    [{ ...r, pivot: [Infinity, 0, 0] }],
    [{ ...r, pivot: [1001, 0, 0] }],
    [{ ...r, axis: [0, 0, 0] }],
    [{ ...r, angleRadians: NaN }],
    [{ ...r, angleRadians: 1000001 }],
    [{ ...r, extra: 1 }],
    [{ ...r, baseModelSha256: "aa" }],
  ])
    assert.throws(() => validateFilmTransforms(bad));
});
