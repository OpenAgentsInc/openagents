import test from "node:test";
import assert from "node:assert/strict";
import {
  prepareCanonicalSourceIndices,
  MAX_CANONICAL_SOURCE_INDICES,
} from "../packages/renderer/canonical-source-indices.ts";
import { STOCK_MATERIAL_FEATURES } from "../packages/renderer/material-specialization.ts";
import {
  STOCK_RASTER_MATERIAL_FLAGS,
  stockRasterPipelineKey,
} from "../packages/renderer/stock-raster-policy.ts";

const draw = (firstIndex = 0, material = 0) => ({
  firstIndex,
  indexCount: 6,
  baseVertex: -2,
  firstInstance: 7,
  count: 3,
  material,
  wind: true,
  sourceDrawIndex: 42,
  rasterOrientation: "negative" as "positive" | "negative" | "degenerate",
});

test("canonical source indices retain the original prefix, source order and all per-draw data", () => {
  const input = new Uint32Array([2, 4, 3, 8, 9, 7]);
  const before = input.slice(),
    negative = draw(),
    positive = { ...draw(), rasterOrientation: "positive" as const };
  const result = prepareCanonicalSourceIndices(
    input,
    new Map([["night", [negative, positive]]]),
    () => 0,
  );
  assert(result);
  assert.deepEqual(input, before);
  assert.deepEqual(
    result.indices,
    new Uint32Array([2, 4, 3, 8, 9, 7, 2, 3, 4, 8, 7, 9]),
  );
  const changed = result.variants.get("night")![0];
  assert.deepEqual(changed, {
    ...negative,
    firstIndex: 6,
    originalFirstIndex: 0,
    rasterOrientation: "positive",
  });
  assert.equal(result.variants.get("night")![1], positive);
  assert.deepEqual(negative, draw());
  assert.equal(result.addedBytes, 24);
});

test("shared negative ranges are copied once across both scenarios and base vertices", () => {
  const a = draw(),
    b = { ...draw(), baseVertex: 1000 };
  const result = prepareCanonicalSourceIndices(
    new Uint32Array([0, 1, 2, 3, 4, 5]),
    new Map([
      ["day", [a]],
      ["night", [b]],
    ]),
    () => 0,
  );
  assert(result);
  assert.equal(result.reversedRanges, 1);
  assert.equal(result.reversedRecords, 2);
  assert.equal(result.addedBytes, 24);
  assert.equal(result.variants.get("night")![0].baseVertex, 1000);
  assert.equal(
    result.variants.get("day")![0].firstIndex,
    result.variants.get("night")![0].firstIndex,
  );
});

test("alpha, vegetation, double-sided, unknown and uncertain-orientation records remain original", () => {
  const all = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => draw(0, i));
  all[4].rasterOrientation = "degenerate";
  all[5].rasterOrientation = "positive";
  const flags = [
    STOCK_RASTER_MATERIAL_FLAGS.masked,
    STOCK_MATERIAL_FEATURES.vegetation,
    STOCK_RASTER_MATERIAL_FLAGS.doubleSided,
    undefined,
    0,
    0,
    STOCK_MATERIAL_FEATURES.glass,
    STOCK_MATERIAL_FEATURES.flame,
  ];
  const before = structuredClone(all);
  const result = prepareCanonicalSourceIndices(
    new Uint32Array([0, 1, 2, 3, 4, 5]),
    new Map([["night", all]]),
    (i) => flags[i],
  );
  assert.equal(result, undefined);
  assert.deepEqual(all, before);
});

test("a rejected range never mutates source records or indices", () => {
  const input = new Uint32Array([1, 2, 3]),
    bad = { ...draw(), firstIndex: 1 },
    before = input.slice();
  assert.throws(
    () =>
      prepareCanonicalSourceIndices(
        input,
        new Map([["night", [bad]]]),
        () => 0,
      ),
    /bounded/,
  );
  assert.deepEqual(input, before);
  assert.equal(bad.firstIndex, 1);
  assert.equal(bad.rasterOrientation, "negative");
});

test("triangle reversal preserves source vertex IDs and world vertices while unifying stock pipeline identity", () => {
  const model = [-2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0.5, 0, 9, 2, 4, 1];
  const points = [
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, 1],
    ],
    source = new Uint32Array([0, 1, 2]);
  const negative = { ...draw(), baseVertex: 0, indexCount: 3 };
  const result = prepareCanonicalSourceIndices(
    source,
    new Map([["night", [negative]]]),
    () => 0,
  );
  assert(result);
  const world = (id: number) =>
    points[id].map(
      (_, axis) =>
        model[axis] * points[id][0] +
        model[axis + 4] * points[id][1] +
        model[axis + 8] * points[id][2] +
        model[axis + 12],
    );
  const original = Array.from(source, (id) => world(id));
  const flipped = Array.from(result.indices.subarray(3), (id) => world(id));
  assert.deepEqual(flipped, [original[0], original[2], original[1]]);
  assert.notEqual(
    stockRasterPipelineKey(0, false, false, "ccw", "negative"),
    stockRasterPipelineKey(0, false, false, "ccw", "positive"),
  );
  assert.equal(
    stockRasterPipelineKey(
      0,
      false,
      false,
      "cw",
      result.variants.get("night")![0].rasterOrientation!,
    ),
    stockRasterPipelineKey(0, false, false, "cw", "positive"),
  );
});

test("valid oversized mirrored geometry keeps all original scene arrays and draws", () => {
  const count = MAX_CANONICAL_SOURCE_INDICES + 2;
  assert.equal(count % 3, 0);
  const indices = new Uint32Array(count);
  indices.set([7, 8, 9]);
  const first = { ...draw(), indexCount: 3 },
    second = { ...draw(), firstIndex: 3, indexCount: count - 3 };
  const variants = new Map([
    ["day", [first]],
    ["night", [first, second]],
  ]);
  const result = prepareCanonicalSourceIndices(indices, variants, () => 0);
  assert.equal(
    result,
    undefined,
    "The optional copy must fall back instead of blocking world loading.",
  );
  assert.deepEqual(indices.subarray(0, 3), new Uint32Array([7, 8, 9]));
  assert.equal(indices.at(-1), 0);
  assert.equal(first.firstIndex, 0);
  assert.equal(first.rasterOrientation, "negative");
  assert.equal(second.firstIndex, 3);
  assert.equal(second.rasterOrientation, "negative");
  assert.equal(variants.get("night")![0], first);
});

test("an oversized candidate still rejects a later invalid selected range without changing inputs", () => {
  const count = MAX_CANONICAL_SOURCE_INDICES + 2;
  const indices = new Uint32Array(count),
    large = { ...draw(), indexCount: count };
  const invalid = { ...draw(), firstIndex: count, indexCount: 3 };
  assert.throws(
    () =>
      prepareCanonicalSourceIndices(
        indices,
        new Map([["night", [large, invalid]]]),
        () => 0,
      ),
    /bounded/,
  );
  assert.equal(large.firstIndex, 0);
  assert.equal(large.rasterOrientation, "negative");
  assert.equal(invalid.firstIndex, count);
  assert.equal(indices.at(-1), 0);
});
