import assert from "node:assert/strict";
import { test } from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { MATERIAL } from "../packages/renderer/shaders.ts";
import {
  prepareSourceWind,
  usesOwnedWindVertex,
  usesOwnedWindShadow,
  type SourceWindManifest,
} from "../packages/renderer/wind-source.ts";
const model = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 3, 0, 0, 1];
function fixture() {
  const scene = {
    scenario: "day",
    vertices: "v",
    indices: "i",
    vertexColors: "c",
    vertexCount: 3,
    indexCount: 6,
    materials: [{ id: "cloth" }, { id: "rope" }],
    meshes: [{ bounds: { min: [0, 0, 0], max: [1, 2, 0] }, primitives: [] }],
    instances: [
      {
        actor: "tent",
        component: "cloth",
        mesh: 0,
        model,
        materialOverrides: [],
      },
      {
        actor: "wall",
        component: "fixed",
        mesh: 0,
        model: [...model.slice(0, 12), 5, 0, 0, 1],
        materialOverrides: [],
      },
    ],
  } as unknown as CompiledScene;
  const vertices = new Float32Array(36);
  vertices[12] = 1;
  vertices[25] = 2;
  const colors = new Uint8Array([
    255, 0, 0, 255, 255, 255, 0, 255, 255, 128, 0, 255,
  ]);
  const indices = new Uint32Array([0, 1, 2, 0, 0, 0]);
  const draws = [
    {
      material: 0,
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      firstInstance: 0,
      count: 2,
    },
    {
      material: 1,
      firstIndex: 3,
      indexCount: 3,
      baseVertex: 0,
      firstInstance: 0,
      count: 2,
    },
  ];
  const manifest: SourceWindManifest = {
    schema: "aethyrforge.source-wind-sway.v1",
    sourceBoundsSha256: "a".repeat(64),
    scenario: "day",
    source: {
      vertices: "v",
      indices: "i",
      vertexColors: "c",
      vertexCount: 3,
      indexCount: 6,
      instances: 2,
    },
    records: [
      {
        instance: 0,
        actor: "tent",
        component: "cloth",
        model: [...model],
        pivot: [3, 0, 0],
        worldUp: [0, 1, 0],
        radiusMeters: 2,
        materials: [
          { id: "cloth", windSpeed: 0.5, swaySpeed: 0.2, swayAmount: 0.005 },
        ],
      },
    ],
  };
  return { scene, vertices, indices, colors, draws, manifest };
}
test("wind partitions primitive uses without moving the fixed material on the same component", () => {
  const f = fixture(),
    p = prepareSourceWind(
      f.scene,
      f.draws,
      f.vertices,
      f.indices,
      f.colors,
      f.manifest,
    );
  assert.deepEqual(
    p.draws.map((d) => [d.material, d.firstInstance, d.count, !!d.wind]),
    [
      [0, 0, 1, true],
      [0, 1, 1, false],
      [1, 0, 2, false],
    ],
  );
  assert.deepEqual([...p.moving], [1, 0]);
  assert.equal(p.sourceCount, 1);
  assert.ok(p.padding[0] > 0);
  assert.equal(p.padding[1], 0);
  assert.equal(p.bounds.aabbs[0], 3 - p.padding[0]);
  assert.equal(p.parameters[3], 2);
});
test("wind rejects stale geometry, duplicate identity, changed matrix/up, absent material and unbounded source radius", () => {
  for (const change of [
    (f: ReturnType<typeof fixture>) => {
      f.manifest.source.vertices = "other";
    },
    (f: ReturnType<typeof fixture>) => {
      f.manifest.records.push(f.manifest.records[0]);
    },
    (f: ReturnType<typeof fixture>) => {
      f.manifest.records[0].model[12] = 9;
    },
    (f: ReturnType<typeof fixture>) => {
      f.manifest.records[0].worldUp = [1, 0, 0];
    },
    (f: ReturnType<typeof fixture>) => {
      f.manifest.records[0].materials[0].id = "missing";
    },
    (f: ReturnType<typeof fixture>) => {
      f.manifest.records[0].radiusMeters = 0;
    },
  ]) {
    const f = fixture();
    change(f);
    assert.throws(() =>
      prepareSourceWind(
        f.scene,
        f.draws,
        f.vertices,
        f.indices,
        f.colors,
        f.manifest,
      ),
    );
  }
});
test("legacy worlds stay static, green-zero selected vertices are pinned, absent primitive fails safely", () => {
  const f = fixture(),
    legacy = prepareSourceWind(
      f.scene,
      f.draws,
      f.vertices,
      f.indices,
      f.colors,
    );
  assert.equal(legacy.sourceCount, 0);
  assert.ok(legacy.parameters.every((n) => n === 0));
  f.colors.fill(0);
  const pinned = prepareSourceWind(
    f.scene,
    f.draws,
    f.vertices,
    f.indices,
    f.colors,
    f.manifest,
  );
  assert.ok(pinned.moving.every((n) => n === 0));
  assert.ok(pinned.padding.every((n) => n === 0));
  assert.throws(() =>
    prepareSourceWind(
      f.scene,
      f.draws.filter((d) => d.material !== 0),
      f.vertices,
      f.indices,
      f.colors,
      f.manifest,
    ),
  );
});

test("fragment-only edits retain the exact bounded vertex contract; changed dependencies do not", () => {
  const owned =
    "fn vertexDependency(){}\nfn vertexValue(){}\nfn sourceFrontFacing(){}\nfragment body";
  assert.equal(
    usesOwnedWindVertex(owned + "\n// accepted fragment edit", owned),
    true,
  );
  assert.equal(
    usesOwnedWindVertex(
      owned.replace("fragment body", "changed fragment"),
      owned,
    ),
    true,
  );
  assert.equal(
    usesOwnedWindVertex(
      owned.replace("vertexDependency", "unknownDependency"),
      owned,
    ),
    false,
  );
  assert.equal(
    usesOwnedWindVertex(owned.replace("vertexValue", "unknownVertex"), owned),
    false,
  );
  assert.equal(usesOwnedWindVertex("unrelated", "no anchor"), false);
});

test("changing shadow coverage disables static reuse even when the vertex prefix is unchanged", () => {
  const owned =
    "owned vertex\nfn sourceFrontFacing(){}\n@fragment fn shadowFS(){source alpha}\nfn shadowDepthBias(){}\nmain fragment";
  assert.equal(
    usesOwnedWindShadow(owned + "// ordinary main edit", owned),
    true,
  );
  assert.equal(
    usesOwnedWindShadow(
      owned.replace("main fragment", "another main fragment"),
      owned,
    ),
    true,
  );
  assert.equal(
    usesOwnedWindShadow(owned.replace("source alpha", "animated alpha"), owned),
    false,
  );
  assert.equal(
    usesOwnedWindShadow(owned.replace("shadowFS", "newEntry"), owned),
    false,
  );
});

test("actual flutter dependencies participate in bounded culling and shadow reuse", () => {
  assert.equal(
    usesOwnedWindVertex(MATERIAL + "\n// fragment comment", MATERIAL),
    true,
  );
  assert.equal(
    usesOwnedWindShadow(MATERIAL + "\n// fragment comment", MATERIAL),
    true,
  );
  for (const [before, after] of [
    ["fn windFlutterOffset", "fn modifiedFlutterOffset"],
    ["frame.passSettings.w,additional", "frame.eye.w,additional"],
    [
      "v.uv,i32(material.vegetationMaps.x),0.0",
      "v.uv,i32(material.vegetationMaps.x),1.0",
    ],
  ]) {
    assert.ok(MATERIAL.includes(before));
    const modified = MATERIAL.replace(before, after);
    assert.equal(usesOwnedWindVertex(modified, MATERIAL), false);
    assert.equal(usesOwnedWindShadow(modified, MATERIAL), false);
  }
});

test("the accepted fragment tint smoke retains owned vertex and shadow dependencies", () => {
  const needle = "return vec4(max(color,vec3(0.0)),alpha);";
  const edited = MATERIAL.replace(
    needle,
    "return vec4(max(color,vec3(0.0))*vec3(0.35,0.7,1.0),alpha);",
  );
  assert.notEqual(edited, MATERIAL);
  assert(Buffer.byteLength(edited) <= 65_536);
  const boundary = MATERIAL.indexOf("fn shade(");
  assert(boundary > 0 && MATERIAL.indexOf(needle) > boundary);
  assert.equal(edited.slice(0, boundary), MATERIAL.slice(0, boundary));
  assert.equal(usesOwnedWindVertex(edited, MATERIAL), true);
  assert.equal(usesOwnedWindShadow(edited, MATERIAL), true);
  assert.match(edited, /@vertex fn depthVS[^]*?->DepthOut/);
  assert.match(edited, /@vertex fn vsPlain[^]*?->PlainOut/);
});
