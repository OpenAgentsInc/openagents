import assert from "node:assert/strict";
import test from "node:test";
import {
  deriveStockRasterPolicy,
  deriveStockDepthPolicy,
  stockRasterMaterialFlags,
  stockRasterPipelineKey,
  STOCK_MATERIAL_FEATURE_MASK,
  STOCK_RASTER_DYNAMIC,
  STOCK_RASTER_FLAGS,
  STOCK_RASTER_MATERIAL_FLAGS,
  STOCK_SURFACE_SKY_FLAG,
} from "../packages/renderer/stock-raster-policy.ts";

const orientations = ["positive", "negative", "degenerate"] as const;
const fronts = ["ccw", "cw"] as const;

test("material raster flags preserve independent mask and double-sided facts", () => {
  assert.equal(stockRasterMaterialFlags(false, false), 0);
  assert.equal(
    stockRasterMaterialFlags(true, false),
    STOCK_RASTER_MATERIAL_FLAGS.masked,
  );
  assert.equal(
    stockRasterMaterialFlags(false, true),
    STOCK_RASTER_MATERIAL_FLAGS.doubleSided,
  );
  assert.equal(stockRasterMaterialFlags(true, true), 0x30000);
  assert.equal(
    stockRasterMaterialFlags(true, true) & STOCK_MATERIAL_FEATURE_MASK,
    0,
  );
  assert.throws(() => stockRasterMaterialFlags(1 as never, false));
  assert.throws(() => stockRasterMaterialFlags(false, undefined as never));
});

test("all stock mask/facing/blend/view/orientation combinations use coherent GPU and shader policies", () => {
  let combinations = 0;
  for (const masked of [false, true])
    for (const doubleSided of [false, true])
      for (const blend of [false, true])
        for (const base of fronts)
          for (const orientation of orientations) {
            combinations++;
            const features =
              129 | stockRasterMaterialFlags(masked, doubleSided);
            const actual = deriveStockRasterPolicy(
              features,
              blend,
              base,
              orientation,
            );
            const hardware = !doubleSided && orientation !== "degenerate";
            assert.equal(actual.cullMode, hardware ? "back" : "none");
            assert.equal(
              actual.frontFace,
              hardware && orientation === "negative"
                ? base === "ccw"
                  ? "cw"
                  : "ccw"
                : base,
            );
            assert.equal(
              actual.depthWriteEnabled,
              !blend && (base === "cw" || masked),
            );
            assert.equal(actual.alphaToCoverageEnabled, !blend && masked);
            assert.equal(
              Boolean(actual.rasterFlags & STOCK_RASTER_FLAGS.planarClip),
              base === "cw",
            );
            assert.equal(
              Boolean(actual.rasterFlags & STOCK_RASTER_FLAGS.masked),
              masked,
            );
            assert.equal(
              Boolean(actual.rasterFlags & STOCK_RASTER_FLAGS.doubleSided),
              doubleSided,
            );
            assert.equal(
              Boolean(actual.rasterFlags & STOCK_RASTER_FLAGS.hardwareCulled),
              hardware,
            );
          }
  assert.equal(combinations, 48);
});

test("mirrored hardware culling preserves the previous shader acceptance and never mislabels planar clipping", () => {
  for (const base of fronts)
    for (const orientation of ["positive", "negative"] as const) {
      const policy = deriveStockRasterPolicy(0, false, base, orientation);
      for (const previousRasterFront of [false, true]) {
        const oldSourceFront =
          previousRasterFront === (orientation === "positive");
        const newRasterFront =
          policy.frontFace === base
            ? previousRasterFront
            : !previousRasterFront;
        assert.equal(
          newRasterFront,
          oldSourceFront,
          "Hardware must retain exactly the previous source-facing triangles.",
        );
      }
      // Culled survivors use physicalFront=true in the shader; applying the
      // orientation test a second time would reject mirrored survivors.
      assert.ok(policy.rasterFlags & STOCK_RASTER_FLAGS.hardwareCulled);
      assert.equal(
        Boolean(policy.rasterFlags & STOCK_RASTER_FLAGS.planarClip),
        base === "cw",
      );
    }
});

test("custom pipelines retain conservative depth writes while stock opaque reuses prepass depth", () => {
  for (const blend of [false, true])
    for (const base of fronts)
      for (const orientation of orientations) {
        const custom = deriveStockRasterPolicy(
          undefined,
          blend,
          base,
          orientation,
        );
        assert.deepEqual(custom, {
          rasterFlags: STOCK_RASTER_DYNAMIC,
          cullMode: "none",
          frontFace: base,
          depthWriteEnabled: !blend,
          alphaToCoverageEnabled: !blend,
        });
      }
  const degenerate = deriveStockRasterPolicy(0, false, "ccw", "degenerate");
  assert.equal(degenerate.cullMode, "none");
  assert.equal(degenerate.rasterFlags, 0);
  assert.equal(degenerate.depthWriteEnabled, false);
  const doubleSided = deriveStockRasterPolicy(
    stockRasterMaterialFlags(false, true),
    false,
    "ccw",
    "negative",
  );
  assert.equal(doubleSided.cullMode, "none");
  assert.equal(doubleSided.frontFace, "ccw");
});

test("masked alpha-to-coverage fringes keep depth writes below the prepass cutoff", () => {
  const masked = deriveStockRasterPolicy(
    stockRasterMaterialFlags(true, true),
    false,
    "ccw",
    "positive",
  );
  assert.equal(masked.alphaToCoverageEnabled, true);
  assert.equal(masked.depthWriteEnabled, true);
  const blended = deriveStockRasterPolicy(
    stockRasterMaterialFlags(true, true),
    true,
    "ccw",
    "positive",
  );
  assert.equal(blended.alphaToCoverageEnabled, false);
  assert.equal(blended.depthWriteEnabled, false);
});

test("surface-sky specialization is independent of material and raster policy flags", () => {
  for (const masked of [false, true])
    for (const doubleSided of [false, true])
      for (const base of fronts)
        for (const orientation of orientations) {
          const features = 129 | stockRasterMaterialFlags(masked, doubleSided);
          assert.deepEqual(
            deriveStockRasterPolicy(
              features | STOCK_SURFACE_SKY_FLAG,
              false,
              base,
              orientation,
            ),
            deriveStockRasterPolicy(features, false, base, orientation),
          );
          assert.equal(
            (features | STOCK_SURFACE_SKY_FLAG) & STOCK_MATERIAL_FEATURE_MASK,
            129,
          );
        }
  assert.equal(STOCK_SURFACE_SKY_FLAG, 0x40000);
});

test("invalid packed flags or raster inputs cannot silently choose a pipeline", () => {
  for (const features of [
    -1,
    NaN,
    Infinity,
    0.5,
    STOCK_RASTER_DYNAMIC,
    0x80000,
  ])
    assert.throws(() =>
      deriveStockRasterPolicy(features, false, "ccw", "positive"),
    );
  assert.throws(() =>
    deriveStockRasterPolicy(0, 1 as never, "ccw", "positive"),
  );
  assert.throws(() =>
    deriveStockRasterPolicy(0, false, "other" as never, "positive"),
  );
  assert.throws(() =>
    deriveStockRasterPolicy(0, false, "ccw", "other" as never),
  );
});

test("pipeline reuse merges only identical stock shader and raster descriptors", () => {
  const descriptors = new Map<string, string>();
  for (const material of [0, 8, 129, 511])
    for (const masked of [false, true])
      for (const doubleSided of [false, true])
        for (const surfaceSky of [0, STOCK_SURFACE_SKY_FLAG])
          for (const blend of [false, true])
            for (const additive of [false, true])
              for (const front of fronts)
                for (const orientation of orientations) {
                  const features =
                    material |
                    stockRasterMaterialFlags(masked, doubleSided) |
                    surfaceSky;
                  const key = stockRasterPipelineKey(
                    features,
                    blend,
                    additive,
                    front,
                    orientation,
                  );
                  const descriptor = JSON.stringify([
                    features,
                    blend,
                    additive,
                    deriveStockRasterPolicy(
                      features,
                      blend,
                      front,
                      orientation,
                    ),
                  ]);
                  if (descriptors.has(key))
                    assert.equal(descriptors.get(key), descriptor);
                  descriptors.set(key, descriptor);
                }
  const sided = stockRasterMaterialFlags(false, true);
  assert.equal(
    new Set(
      orientations.map((orientation) =>
        stockRasterPipelineKey(sided, false, false, "ccw", orientation),
      ),
    ).size,
    1,
  );
  assert.equal(
    new Set(
      orientations.map((orientation) =>
        stockRasterPipelineKey(0, false, false, "ccw", orientation),
      ),
    ).size,
    3,
  );
});

test("depth skips fragment processing only when original alpha and facing tests cannot reject survivors", () => {
  for (const orientation of orientations)
    for (const masked of [false, true])
      for (const vegetation of [0, 8])
        for (const double of [false, true]) {
          const features =
            vegetation | stockRasterMaterialFlags(masked, double);
          const policy = deriveStockDepthPolicy(features, orientation);
          assert.equal(
            policy.fragmentRequired,
            masked ||
              vegetation !== 0 ||
              (!double && orientation === "degenerate"),
          );
          if (policy.fragmentRequired)
            assert.deepEqual(policy, {
              fragmentRequired: true,
              cullMode: "none",
              frontFace: "ccw",
            });
          else {
            assert.equal(policy.cullMode, double ? "none" : "back");
            assert.equal(
              policy.frontFace,
              !double && orientation === "negative" ? "cw" : "ccw",
            );
          }
        }
  assert.equal(
    deriveStockDepthPolicy(undefined, "positive").fragmentRequired,
    true,
  );
  assert.throws(() => deriveStockDepthPolicy(0x80000000, "positive"));
});
