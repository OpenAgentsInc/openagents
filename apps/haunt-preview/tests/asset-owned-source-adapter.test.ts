import assert from "node:assert/strict";
import { test } from "node:test";
import { describeSource } from "../src/asset-compiler.ts";
import { compileOwnedSource, MAX_OWNED_RECIPE_BYTES, MAX_PALETTE_REPLACEMENTS, OWNED_SOURCE_EXTENSION } from "../src/asset-owned-source-adapter.ts";
import { descriptionBytes, MAX_OUTPUT_BYTES, validateImportedAsset } from "../src/asset-import-protocol.ts";
import type { AssetConverter, SourceBytes } from "../src/asset-import-protocol.ts";
import { initialProject } from "../src/project.ts";
import { houseGeometry } from "../src/scene.ts";

const encode = (value: unknown) => new TextEncoder().encode(JSON.stringify(value));
const unused: AssetConverter = async () => { throw new Error("core converter should not run"); };
const wrapper = (recipe: unknown) => ({ asset: { version: "2.0" }, extensionsUsed: [OWNED_SOURCE_EXTENSION], extensionsRequired: [OWNED_SOURCE_EXTENSION], extensions: { [OWNED_SOURCE_EXTENSION]: recipe } });
async function source(value: unknown, id = "house", path = "recipes/house.gltf"): Promise<SourceBytes> {
  const bytes = encode(value);
  return { record: await describeSource(id, "gltf", path, bytes), bytes };
}
async function owned(recipe: unknown) {
  const input = await source(wrapper(recipe));
  return (await compileOwnedSource(input.record, input.bytes, [], unused))!;
}

test("geometry uses the real house generator and preserves every packed attribute", async () => {
  const project = initialProject(), original = houseGeometry(project.entities, undefined, project.layout);
  const result = await owned({ version: 1, kind: "geometry", project });
  let vertex = 0;
  for (const mesh of result.meshes) {
    assert.ok(mesh.vertexCount <= 65_535);
    assert.equal(mesh.indexCount, mesh.vertexCount);
    for (let at = 0; at < mesh.vertexCount; at++, vertex++) {
      const start = vertex * 10;
      assert.deepEqual(mesh.positions.subarray(at * 3, at * 3 + 3), original.subarray(start, start + 3));
      assert.deepEqual(mesh.normals!.subarray(at * 3, at * 3 + 3), original.subarray(start + 3, start + 6));
      assert.deepEqual(mesh.colors!.subarray(at * 3, at * 3 + 3), original.subarray(start + 6, start + 9));
      assert.equal(mesh.glow![at], original[start + 9]);
      assert.equal(mesh.indices[at], at);
    }
  }
  assert.equal(vertex, original.length / 10);
  assert.ok(descriptionBytes(result) <= MAX_OUTPUT_BYTES);
  assert.equal(result.sources.length, 1);
  assert.ok(result.meshes.some(mesh => mesh.glow!.some(value => value > 0)));
});

test("palette decimals match Float32 house colors and only actual dependencies are returned", async () => {
  const project = initialProject(), baseline = houseGeometry(project.entities, undefined, project.layout);
  const from = [0.29, 0.23, 0.22], to = [0.1, 0.8, 0.2];
  const palette = await source({ version: 1, replacements: [{ from, to }] }, "palette", "palettes/house.json");
  const other = await source({ unused: true }, "other", "palettes/unused.json");
  const input = await source(wrapper({ version: 1, kind: "geometry", project, palette: { source: "../palettes/house.json" } }));
  const result = (await compileOwnedSource(input.record, input.bytes, [palette, other], unused))!;
  assert.deepEqual(result.sources, [input.record, palette.record]);
  let vertex = 0, replacements = 0;
  for (const mesh of result.meshes) for (let at = 0; at < mesh.vertexCount; at++, vertex++) {
    const original = baseline.subarray(vertex * 10 + 6, vertex * 10 + 9);
    const matches = from.every((value, axis) => Math.fround(value) === original[axis]);
    if (matches) replacements++;
    assert.deepEqual([...mesh.colors!.subarray(at * 3, at * 3 + 3)], matches ? to.map(Math.fround) : [...original]);
  }
  assert.ok(replacements > 0);
  const inline = await owned({ version: 1, kind: "geometry", project, palette: { replacements: [{ from, to }] } });
  assert.deepEqual(inline.meshes.map(mesh => mesh.colors), result.meshes.map(mesh => mesh.colors));
  assert.equal(inline.sources.length, 1);
});

test("larger allowed houses split triangles into bounded independent meshes", async () => {
  const project = initialProject();
  project.layout = { removedWalls: [], rooms: [-8, 8].flatMap((x, a) => [-8, 8].map((z, b) => ({ id: `room-${a}-${b}`, name: "Room", x, z, width: 16, depth: 16 }))) };
  project.entities = Array.from({ length: 64 }, (_, i) => ({ id: `pumpkin-${i}`, kind: "pumpkin", x: -8, z: -8, scale: 1 }));
  const input = await source(wrapper({ version: 1, kind: "geometry", project }));
  const result = (await compileOwnedSource(input.record, input.bytes, [], unused))!;
  assert.ok(result.meshes.length > 1);
  for (const mesh of result.meshes) {
    assert.ok(mesh.vertexCount <= 65_535);
    assert.equal(mesh.vertexCount % 3, 0);
    assert.equal(mesh.indices[0], 0);
    assert.equal(mesh.indices.at(-1), mesh.vertexCount - 1);
  }
  validateImportedAsset(result, input.record, [input.record]);
});

test("card conversion uses one local core-decoded image and binds its real image id", async () => {
  const imageBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
  const image = { record: await describeSource("image", "image", "images/card.png", imageBytes), bytes: imageBytes };
  const other = await source({ unused: true }, "unused", "unrelated.gltf");
  const input = await source(wrapper({ version: 1, kind: "card", image: "../images/card.png", width: 2, height: 3 }), "poster");
  let calls = 0;
  const converter: AssetConverter = async (record, bytes, files) => {
    calls++; assert.deepEqual(record, image.record); assert.deepEqual(bytes, imageBytes); assert.deepEqual(files, []);
    bytes.fill(0); // The converter owns a copy.
    return { id: record.id, sources: [record], meshes: [], diagnostics: [], images: [{ id: "poster-card-material", source: record.id,
      mime: "image/png", width: 1, height: 1, bytes: new Uint8Array([255, 0, 0, 255]) }] };
  };
  const result = (await compileOwnedSource(input.record, input.bytes, [image, other], converter))!;
  assert.equal(calls, 1);
  assert.deepEqual(imageBytes, new Uint8Array([0x89, 0x50, 0x4e, 0x47]));
  assert.deepEqual(result.sources, [input.record, image.record]);
  assert.deepEqual(result.meshes[0].uvs, new Float32Array([0, 1, 1, 1, 1, 0, 0, 0]));
  assert.deepEqual(result.meshes[0].bounds, { min: [-1, -1.5, 0], max: [1, 1.5, 0] });
  assert.equal(result.materials![0].baseColorTexture, result.images[0].id);
  assert.equal(result.meshes[0].material, result.materials![0].id);
  assert.equal(result.materials![0].doubleSided, true);
  assert.equal(result.materials![0].unlit, true);
  validateImportedAsset(result, input.record, [input.record, image.record, other.record]);
});

test("ordinary formats delegate without calling a second implementation", async () => {
  const input = await source({ asset: { version: "2.0" }, meshes: [] });
  assert.equal(await compileOwnedSource(input.record, input.bytes, [], unused), undefined);
  assert.equal(await compileOwnedSource({ ...input.record, kind: "glb" }, input.bytes, [], unused), undefined);
});

test("decimal card dimensions share Float32 vertex and bounds coordinates", async () => {
  const bytes = new Uint8Array([1]);
  const image = { record: await describeSource("image", "image", "recipes/card.png", bytes), bytes };
  const converter: AssetConverter = async record => ({ id: record.id, sources: [record], meshes: [], diagnostics: [],
    images: [{ id: "decoded", source: record.id, mime: "image/png", width: 1, height: 1, bytes: new Uint8Array([255, 0, 0, 255]) }] });
  for (const width of [0.1, 1.2]) {
    const input = await source(wrapper({ version: 1, kind: "card", image: "card.png", width, height: 0.1 }));
    const result = (await compileOwnedSource(input.record, input.bytes, [image], converter))!;
    const mesh = result.meshes[0];
    assert.deepEqual(mesh.bounds, { min: [-Math.fround(width / 2), -Math.fround(0.05), 0], max: [Math.fround(width / 2), Math.fround(0.05), 0] });
    assert.equal(mesh.positions[0], mesh.bounds.min[0]);
    assert.equal(mesh.positions[7], mesh.bounds.max[1]);
    validateImportedAsset(result, input.record, [input.record, image.record]);
  }
});

test("owned versions, project identity, fields, counts, and recipe byte bounds refuse before conversion", async () => {
  const project = initialProject();
  for (const recipe of [
    { version: 2, kind: "geometry", project },
    { version: 1, kind: "unknown" },
    { version: 1, kind: "geometry", project: { ...project, id: "medieval-town-v1" } },
    { version: 1, kind: "geometry", project: { ...project, entities: Array(65).fill(project.entities[0]) } },
    { version: 1, kind: "geometry", project, unsupported: true },
    { version: 1, kind: "geometry", project, palette: { replacements: Array(MAX_PALETTE_REPLACEMENTS + 1).fill({ from: [0, 0, 0], to: [1, 1, 1] }) } },
    { version: 1, kind: "geometry", project, palette: { replacements: [{ from: [0, 0, 0], to: [2, 0, 0] }] } },
    { version: 1, kind: "geometry", project, palette: { replacements: [{ from: [0, 0, 0], to: [1, 1, 1] }, { from: [0, 0, 0], to: [0, 1, 0] }] } },
  ]) await assert.rejects(owned(recipe));
  const oversized = await source({ ...wrapper({ version: 1, kind: "geometry", project }), extras: "x".repeat(MAX_OWNED_RECIPE_BYTES) });
  await assert.rejects(compileOwnedSource(oversized.record, oversized.bytes, [], unused), /too large/);
  const missingRequired = await source({ ...wrapper({ version: 1, kind: "geometry", project }), extensionsRequired: [] });
  await assert.rejects(compileOwnedSource(missingRequired.record, missingRequired.bytes, [], unused), /required/);
  const missingUsed = await source({ ...wrapper({ version: 1, kind: "geometry", project }), extensionsUsed: [] });
  await assert.rejects(compileOwnedSource(missingUsed.record, missingUsed.bytes, [], unused), /required/);
});

test("cards refuse external, escaping, ambiguous, missing, invalid images and dimensions", async () => {
  for (const image of ["https://example.com/image.png", "/image.png", "../../image.png", "missing.png"]) {
    await assert.rejects(owned({ version: 1, kind: "card", image, width: 1, height: 1 }));
  }
  for (const width of [0, -1, 65, null, 1e-50]) await assert.rejects(owned({ version: 1, kind: "card", image: "local.png", width, height: 1 }));
  const bytes = new Uint8Array([1]), image = { record: await describeSource("image", "image", "recipes/local.png", bytes), bytes };
  const input = await source(wrapper({ version: 1, kind: "card", image: "local.png", width: 1, height: 1 }));
  const duplicate = { ...image, record: { ...image.record, id: "second" } };
  await assert.rejects(compileOwnedSource(input.record, input.bytes, [image, duplicate], unused), /choose one/);
  const badImage: AssetConverter = async record => ({ id: record.id, sources: [record], meshes: [], diagnostics: [],
    images: [{ id: "broken", source: record.id, mime: "image/png", width: 2, height: 2, bytes: new Uint8Array(1) }] });
  await assert.rejects(compileOwnedSource(input.record, input.bytes, [image], badImage), /invalid/);
});
