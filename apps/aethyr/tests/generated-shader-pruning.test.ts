import assert from "node:assert/strict";
import test from "node:test";
import { pruneGeneratedShaderFunctions } from "../packages/renderer/generated-shader-pruning.ts";
import { compactGeneratedWGSL } from "../packages/renderer/wgsl-source.ts";
import { buildRendererShaders } from "../packages/renderer/shaders.ts";
import { selectedRendererFeatures } from "../packages/renderer/player-features.ts";

test("generated helper pruning preserves transitive callers and stage entry points", () => {
  const source = `
    fn leaf()->f32{return 1.0;}
    fn used()->f32{return leaf();}
    fn unusedLeaf()->f32{return 2.0;}
    fn unused()->f32{return unusedLeaf();}
    @vertex fn root()->@builtin(position)vec4<f32>{return vec4(used());}
    @compute @workgroup_size(vec3(1,1,1)) fn keepEntry(){}
  `;
  const result = pruneGeneratedShaderFunctions(source, [
    "leaf",
    "used",
    "unusedLeaf",
    "unused",
    "root",
    "keepEntry",
  ]);
  for (const name of ["leaf", "used", "root", "keepEntry"])
    assert.match(result, new RegExp(`\\bfn ${name}\\(`));
  for (const name of ["unusedLeaf", "unused"])
    assert.doesNotMatch(result, new RegExp(`\\bfn ${name}\\(`));
  assert.match(result, /@compute @workgroup_size\(vec3\(1,1,1\)\)fn keepEntry/);
  assert.equal(
    pruneGeneratedShaderFunctions(result, [
      "leaf",
      "used",
      "root",
      "keepEntry",
    ]),
    result,
  );
});

test("nested braces, attributes, comments, and identifier boundaries cannot corrupt generated helpers", () => {
  const source = `
    /* outer { fn fake(){} /* nested } */ */
    @must_use fn unused()->f32{if true{return 2.0;}else{return 3.0;}}
    fn keepLong()->f32{return 4.0;}
    fn keep()->f32{return 5.0;}
    fn caller()->f32{let x=keepLong/* }}} */();return x / 2.0;}
    // fn keep() { missing braces
  `;
  const result = pruneGeneratedShaderFunctions(source, ["unused", "keep"]);
  assert.doesNotMatch(result, /must_use|fn unused\(|fn keep\(/);
  assert.match(result, /fn keepLong\(/);
  assert.match(result, /return x \/ 2.0/);
  assert.throws(
    () => pruneGeneratedShaderFunctions("fn broken(){if true{}", ["broken"]),
    /braces/,
  );
  assert.throws(
    () => pruneGeneratedShaderFunctions("/* unclosed", []),
    /comment/,
  );
  assert.throws(
    () => pruneGeneratedShaderFunctions("fn a(){}fn a(){}", ["a"]),
    /unique/,
  );
  assert.throws(
    () => pruneGeneratedShaderFunctions("fn a(){}", ["a", "a"]),
    /distinct/,
  );
  assert.throws(
    () => pruneGeneratedShaderFunctions("fn a(){}", ["a.*"]),
    /distinct/,
  );
  assert.equal(
    pruneGeneratedShaderFunctions("fn a(){}", ["missing"]),
    compactGeneratedWGSL("fn a(){}"),
  );
});

test("module-level references and non-candidate helpers retain candidate dependencies", () => {
  const source =
    "fn value()->f32{return 1.0;}const x=value();fn neverUsed()->f32{return value();}";
  const result = pruneGeneratedShaderFunctions(source, ["value"]);
  assert.match(result, /fn value\(/);
  assert.match(result, /fn neverUsed\(/);
});

function fields(source: string, name: string) {
  const body = source.match(new RegExp(`struct ${name}\\{([^}]+)\\}`))![1];
  return [
    ...body.matchAll(
      /@location\((\d+)\)(@interpolate\(flat\))?(\w+):([^,}]+)/g,
    ),
  ].map((match) => ({
    location: Number(match[1]),
    interpolation: match[2] ?? "",
    name: match[3],
    type: match[4],
  }));
}

test("stock depth/plain interfaces retain generic custom shader locations and shared deformation/shading", () => {
  const { MATERIAL, COMMON } = buildRendererShaders(selectedRendererFeatures);
  const generic = fields(MATERIAL, "Out"),
    depth = fields(MATERIAL, "DepthOut"),
    plain = fields(MATERIAL, "PlainOut");
  assert.equal(generic.length, 15);
  assert.deepEqual(
    depth.map((field) => field.location),
    [2, 3, 6],
  );
  assert.deepEqual(
    plain.map((field) => field.location),
    [0, 1, 2, 3, 4, 5, 6, 7],
  );
  for (const field of [...depth, ...plain])
    assert.deepEqual(
      field,
      generic.find((candidate) => candidate.location === field.location),
    );
  assert.equal(
    depth.find((field) => field.location === 6)!.interpolation,
    "@interpolate(flat)",
  );
  assert.match(
    MATERIAL,
    /@vertex fn vs\([^\n]+->Out\{return vertexValue\(v,vertex,true\);\}/,
  );
  assert.match(
    MATERIAL,
    /@vertex fn vsPlain\([^\n]+->PlainOut\{let o=vertexValue\(v,vertex,false\);/,
  );
  assert.match(
    MATERIAL,
    /@vertex fn shadowVS\([^\n]+->DepthOut\{let o=vertexValue\(v,vertex,false\);return DepthOut\(frame.shadow\*vec4\(o.world,1.0\),o.rawUV,o.color,o.orientation\);\}/,
  );
  assert.match(MATERIAL, /@fragment fn depthFS\(v:DepthOut,/);
  assert.match(MATERIAL, /@fragment fn shadowFS\(v:DepthOut\)/);
  assert.match(
    MATERIAL,
    /@fragment fn fs\(v:Out,[^\n]+\{return shade\(v,front\);\}/,
  );
  const wrapper = MATERIAL.slice(MATERIAL.indexOf("@fragment fn fsPlain"));
  assert.match(
    wrapper,
    /return shade\(Out\(v.clip,v.world,v.normal,v.rawUV,v.color,v.objectScale,v.uv1,v.orientation,v.undeformedWorld,/,
  );
  assert.equal((wrapper.match(/vec4\(0.0\)/g) ?? []).length, 7);
  assert.match(
    COMMON,
    /fn linearToSrgb\(/,
    "Other generated shader exports keep their helpers.",
  );
  for (const name of [
    "linearToSrgb",
    "distanceMoments",
    "localSkyLighting",
    "localSkyIrradiance",
    "environmentSurfaceWithSky",
    "environmentSurface",
    "clusteredSurface",
  ])
    assert.doesNotMatch(MATERIAL, new RegExp(`\\bfn ${name}\\(`));
  assert.ok(new TextEncoder().encode(MATERIAL).byteLength <= 65536);
  assert.ok(
    new TextEncoder().encode(buildRendererShaders({}).MATERIAL).byteLength <=
      65536,
  );
});
