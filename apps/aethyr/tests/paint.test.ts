import assert from "node:assert/strict";
import test from "node:test";
import type { PaintLayerMaterial } from "../packages/assets/src/scene.ts";
import {
  collectPaintTextures,
  packPaint,
  PAINT_FLOAT_COUNT,
} from "../packages/renderer/paint.ts";

function fixture(): { paint: PaintLayerMaterial } {
  const slot = {
    baseColorUrl: "color",
    normalUrl: "normal",
    mraUrl: "packed",
    colorFactor: [1, 1, 1, 1] as [number, number, number, number],
    emissiveIntensity: 0,
    emissiveColorSpace: "linear" as const,
  };
  return {
    paint: {
      model: "ue-mm-blend-1-v1",
      slots: [{ ...slot }, { ...slot, colorFactor: [0.2, 0.4, 0.6, 1] }],
      uv: { mode: "objectYZ", scale: [1, 1] },
      height: {
        url: "height",
        sourceColorSpace: "linear",
        uvScale: [2, 3],
        contrast: 1,
      },
      overlay: {
        url: "overlay",
        sourceColorSpace: "srgb",
        uvSet: 1,
        useAlpha: true,
        colorIntensity: 1.2,
        roughnessIntensity: -0.5,
      },
      baseRoughnessIntensity: 0.8,
      source: { master: "owned", graphSha256: "owned", parameters: {} },
    },
  };
}

test("paint payload preserves independent UV1 overlay and unclamped source controls", () => {
  const values = packPaint(
    fixture(),
    (url, role) =>
      (
        ({ color: 2, normal: 3, packed: 4, height: 5, overlay: 6 }) as Record<
          string,
          number
        >
      )[url]!,
  );
  assert.equal(values.length, PAINT_FLOAT_COUNT);
  assert.equal(values.byteLength, 9 * 16);
  assert.deepEqual([...values.slice(0, 8)], [1, 2, 1, 1, 1, 1, 2, 3]);
  assert.deepEqual(
    [...values.slice(16, 28)],
    [2, 3, 4, 2, 3, 4, 5, 6, 0, 0, 0, 1],
  );
  assert.equal(values[30], -0.5);
  assert.ok(Math.abs(values[29]! - 1.2) < 1e-6);
});

test("zero emission does not allocate or sample supplemental emission maps", () => {
  const material = fixture();
  material.paint.slots[0].emissiveUrl = "unused";
  assert.ok(
    !collectPaintTextures(material).some((value) => value.url === "unused"),
  );
  packPaint(material, (url) => {
    assert.notEqual(url, "unused");
    return 0;
  });
  material.paint.slots[0].emissiveIntensity = 3;
  assert.ok(
    collectPaintTextures(material).some(
      (value) => value.url === "unused" && value.role === "data",
    ),
  );
});

test("paint collection deduplicates role references while preserving sRGB overlay alpha", () => {
  assert.deepEqual(collectPaintTextures(fixture()), [
    { url: "color", role: "color" },
    { url: "normal", role: "normal" },
    { url: "packed", role: "data" },
    { url: "height", role: "data" },
    { url: "overlay", role: "color" },
  ]);
});

test("disabled paint profile has no texture work and invalid uploads fail", () => {
  assert.ok(
    packPaint({}, () => {
      throw Error("lookup");
    }).every((value) => value === 0),
  );
  assert.throws(() => packPaint(fixture(), () => -0.5), /pool index/);
  const material = fixture();
  material.paint.height.contrast = Infinity;
  assert.throws(() => packPaint(material, () => 0), /finite/);
});
