import test from "node:test";
import assert from "node:assert/strict";
import {
  STOCK_MATERIAL_FEATURES,
  STOCK_MATERIAL_DYNAMIC,
  stockMaterialConstants,
  type StockMaterialFeatures,
} from "../packages/renderer/material-specialization.ts";
import { buildRendererShaders } from "../packages/renderer/shaders.ts";
import { surfaceSkyWGSL } from "../packages/renderer/surface-sky.ts";

const disabled = (): StockMaterialFeatures => ({
  layered: false,
  paint: false,
  terrain: false,
  vegetation: false,
  glass: false,
  flame: false,
  creator: false,
  normal: false,
  mra: false,
});

test("stock material flags preserve independent overlapping families and the zero-feature pipeline", () => {
  assert.deepEqual(stockMaterialConstants(disabled()), {
    AF_MATERIAL_FEATURES: 0,
  });
  const overlap = {
    ...disabled(),
    layered: true,
    paint: true,
    vegetation: true,
    normal: true,
  };
  assert.equal(
    stockMaterialConstants(overlap).AF_MATERIAL_FEATURES,
    1 | 2 | 8 | 128,
  );
  const names = Object.keys(
    STOCK_MATERIAL_FEATURES,
  ) as (keyof StockMaterialFeatures)[];
  const masks = new Set<number>();
  for (let combination = 0; combination < 1 << names.length; combination++) {
    const flags = disabled();
    names.forEach((name, index) => {
      flags[name] = !!(combination & (1 << index));
    });
    const mask = stockMaterialConstants(flags).AF_MATERIAL_FEATURES;
    assert.equal(mask, combination);
    assert.notEqual(mask, STOCK_MATERIAL_DYNAMIC);
    masks.add(mask);
  }
  assert.equal(masks.size, 512);
  assert.deepEqual(overlap, {
    ...disabled(),
    layered: true,
    paint: true,
    vegetation: true,
    normal: true,
  });
});

test("a missing or malformed feature flag cannot silently disable a material family", () => {
  for (const value of [
    {},
    { ...disabled(), layered: 1 },
    { ...disabled(), extra: false },
  ])
    assert.throws(
      () => stockMaterialConstants(value as StockMaterialFeatures),
      /every stock material/,
    );
  const missing: Partial<StockMaterialFeatures> = disabled();
  delete missing.glass;
  assert.throws(() => stockMaterialConstants(missing as StockMaterialFeatures));
});

test("generic and custom-compatible shader defaults retain dynamic material flags", () => {
  const { MATERIAL } = buildRendererShaders({});
  assert.match(MATERIAL, /override AF_MATERIAL_FEATURES:u32 = 4294967295u/);
  assert.match(
    MATERIAL,
    /if AF_MATERIAL_FEATURES==4294967295u\{return dynamic;\}/,
  );
  for (const bit of Object.values(STOCK_MATERIAL_FEATURES))
    assert.ok(
      MATERIAL.includes(`stockMaterialFeature(${bit}u,`),
      `Feature ${bit} has no stock shader guard.`,
    );
  // The generic sentinel still samples original maps even if an unusual dynamic
  // uniform uses a fractional map weight; only explicit stock flags remove them.
  assert.ok(
    MATERIAL.includes("stockMaterialFeature(256u,true){mra=textureSample("),
  );
  assert.ok(MATERIAL.includes("ao=mix(1.0,mra.b,material.maps.y)"));
});

test("surface-sky specialization keeps generic defaults and guards both shader stages", () => {
  const { MATERIAL } = buildRendererShaders({});
  assert.match(MATERIAL, /override AF_SURFACE_SKY:bool=true/);
  assert.match(
    MATERIAL,
    /surfaceSkyVertex\(material\.vegetationFlags\.w,vertex,frame\.skyLayers\.x,receiveSky&&AF_SURFACE_SKY&&frame\.passSettings\.z>0\.5\)/,
  );
  const fragment = MATERIAL.slice(
    MATERIAL.indexOf("fn shade(v:Out"),
    MATERIAL.indexOf("@fragment fn fs(v:Out"),
  );
  const start = fragment.indexOf("if AF_SURFACE_SKY{");
  assert.ok(start >= 0);
  let depth = 1,
    end = start + "if AF_SURFACE_SKY{".length;
  for (; end < fragment.length && depth; end++) {
    if (fragment[end] === "{") depth++;
    if (fragment[end] === "}") depth--;
  }
  assert.equal(depth, 0);
  const guarded = fragment.slice(start, end);
  for (const term of [
    "SurfaceSkyVertex(",
    "surfaceSkyIrradiance(",
    "refinedSkyReflection(",
  ])
    assert.ok(guarded.includes(term));
  for (const field of "ABCDEFG")
    assert.ok(guarded.includes(`v.surfaceSky${field}`));
  const outside = fragment.slice(0, start) + fragment.slice(end);
  assert.ok(!outside.includes("v.surfaceSky"));
  assert.ok(outside.includes("environmentSurfaceWithReflections("));
  assert.match(
    fragment.slice(0, start),
    /var r=vec4\(0\.0\);var s=vec4\(0\.0\)/,
  );
  assert.match(fragment.slice(end), /surfaceSpecular,r,s\)/);
});

test("disabling unselected surface sky preserves its existing zero fallback", () => {
  const source = surfaceSkyWGSL();
  // The old vertex path exits before any storage access when selection is zero.
  // Its zero-initialized output makes both downstream refinement weights zero.
  assert.match(
    source,
    /var out:SurfaceSkyVertex;if !enabled\|\|selection<1\.0\{return out;\}/,
  );
  assert.match(
    source,
    /if abs\(coefficients\.g\.w\)<0\.9999\{return vec4\(0\.0\);\}/,
  );
  const { MATERIAL } = buildRendererShaders({});
  assert.match(
    MATERIAL,
    /if coefficients\.g\.w> -0\.9999\{return vec4\(0\.0\);\}/,
  );
  // The replacement zeros enter the same environment/bounce/reflection wrapper;
  // no light gain, normal, coverage, or material result changes with this flag.
  assert.match(
    MATERIAL,
    /environmentSurfaceWithReflections\(v\.world,geometricNormal,n,view,base,rough,metal,ao\*contact,surfaceSpecular,r,s\)/,
  );
});

test("clipping and backface rejection retain derivatives but precede expensive lighting", () => {
  const { MATERIAL } = buildRendererShaders({});
  const fragment = MATERIAL.slice(
    MATERIAL.indexOf("fn shade(v:Out"),
    MATERIAL.indexOf("@fragment fn fs(v:Out"),
  );
  const rejection = fragment.indexOf(
    "if clipped||(!stockRasterFlag(4u,material.uv.w>=0.5)&&!physicalFront){discard;}",
    fragment.indexOf("var coverageWidth"),
  );
  assert.ok(rejection > 0);
  for (const operation of ["dpdx(", "dpdy(", "fwidth(", "textureSample("])
    assert.ok(
      fragment.lastIndexOf(operation) < rejection,
      `${operation} moved after nonuniform rejection.`,
    );
  for (const operation of [
    "let vis=shadowVisibility",
    "environmentSurfaceWithReflections(",
    "clusteredSurfaceTransmission(",
    "let fog=surfaceFog(",
  ])
    assert.ok(
      fragment.indexOf(operation) > rejection,
      `${operation} runs before rejected fragments leave.`,
    );
  assert.match(
    MATERIAL,
    /@vertex fn depthVS\([^\n]+->DepthOut\{let o=vertexValue\(v,vertex,false\);return DepthOut\(o.clip,o.rawUV,o.color,o.orientation\);\}/,
  );
  assert.match(
    MATERIAL,
    /@vertex fn vs\([^\n]+\{return vertexValue\(v,vertex,true\);\}/,
  );
});
