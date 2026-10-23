import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { compileAsset, MAX_ASSET_BYTES, type SourceRecord } from "../src/asset-compiler.ts";

const GLB_MAGIC = 0x46546c67;
const JSON_CHUNK = 0x4e4f534a;
const BIN_CHUNK = 0x004e4942;

function align4(n: number): number { return (n + 3) & ~3; }

function glb(json: unknown, bin: Uint8Array): Uint8Array {
  const jsonBytes = new TextEncoder().encode(JSON.stringify(json));
  const jsonLength = align4(jsonBytes.byteLength);
  const binLength = align4(bin.byteLength);
  const total = 12 + 8 + jsonLength + 8 + binLength;
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  view.setUint32(0, GLB_MAGIC, true);
  view.setUint32(4, 2, true);
  view.setUint32(8, total, true);
  view.setUint32(12, jsonLength, true);
  view.setUint32(16, JSON_CHUNK, true);
  out.set(jsonBytes, 20);
  for (let i = jsonBytes.byteLength; i < jsonLength; i++) out[20 + i] = 0x20;
  const binStart = 20 + jsonLength;
  view.setUint32(binStart, binLength, true);
  view.setUint32(binStart + 4, BIN_CHUNK, true);
  out.set(bin, binStart + 8);
  return out;
}

function record(bytes: Uint8Array): SourceRecord {
  return { id: "root", kind: "glb", path: "root.glb", bytes: bytes.byteLength, hash: createHash("sha256").update(bytes).digest("hex") };
}

function triangleJson(accessorOverrides: Record<string, unknown> = {}, mode = 4): unknown {
  return {
    asset: { version: "2.0" },
    buffers: [{ byteLength: 42 }],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: 36 },
      { buffer: 0, byteOffset: 36, byteLength: 6 },
    ],
    accessors: [
      { bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0], ...accessorOverrides },
      { bufferView: 1, componentType: 5123, count: 3, type: "SCALAR" },
    ],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, mode }] }],
  };
}

function triangleBin(): Uint8Array {
  const bin = new Uint8Array(42);
  const view = new DataView(bin.buffer);
  const positions = [0, 0, 0, 1, 0, 0, 0, 1, 0];
  positions.forEach((v, i) => view.setFloat32(i * 4, v, true));
  [0, 1, 2].forEach((v, i) => view.setUint16(36 + i * 2, v, true));
  return bin;
}

function filesFor(bytes: Uint8Array): { record: SourceRecord; bytes: Uint8Array }[] {
  return [{ record: record(bytes), bytes }];
}

describe("compileAsset", () => {
  it("compiles one valid triangle", async () => {
    const bytes = glb(triangleJson(), triangleBin());
    const asset = await compileAsset(record(bytes), bytes, filesFor(bytes));
    assert.equal(asset.id, "root");
    assert.deepEqual(asset.sources, [record(bytes)]);
    assert.deepEqual(asset.images, []);
    assert.deepEqual(asset.diagnostics, []);
    assert.equal(asset.meshes.length, 1);
    assert.equal(asset.meshes[0].source, "root");
    assert.deepEqual(Array.from(asset.meshes[0].positions), [0, 0, 0, 1, 0, 0, 0, 1, 0]);
    assert.deepEqual(Array.from(asset.meshes[0].indices), [0, 1, 2]);
  });

  it("honors bufferView offsets and accessor byteOffset/stride", async () => {
    const bin = new Uint8Array(64);
    const view = new DataView(bin.buffer);
    const positions = [0, 0, 0, 1, 0, 0, 0, 1, 0];
    positions.forEach((v, i) => view.setFloat32(8 + i * 4, v, true));
    [0, 1, 2].forEach((v, i) => view.setUint32(48 + i * 4, v, true));
    const json = {
      asset: { version: "2.0" },
      buffers: [{ byteLength: 64 }],
      bufferViews: [
        { buffer: 0, byteOffset: 4, byteLength: 44 },
        { buffer: 0, byteOffset: 48, byteLength: 12 },
      ],
      accessors: [
        { bufferView: 0, byteOffset: 4, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0] },
        { bufferView: 1, componentType: 5125, count: 3, type: "SCALAR" },
      ],
      meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, mode: 4 }] }],
    };
    const bytes = glb(json, bin);
    const asset = await compileAsset(record(bytes), bytes, filesFor(bytes));
    assert.deepEqual(asset.diagnostics, []);
    assert.deepEqual(Array.from(asset.meshes[0].positions), [0, 0, 0, 1, 0, 0, 0, 1, 0]);
    assert.deepEqual(Array.from(asset.meshes[0].indices), [0, 1, 2]);
  });

  it("rejects an out-of-range accessor", async () => {
    const bytes = glb(triangleJson({ count: 4 }), triangleBin());
    const asset = await compileAsset(record(bytes), bytes, filesFor(bytes));
    assert.deepEqual(asset.meshes, []);
    assert.ok(asset.diagnostics.some((d) => d.severity === "error" && d.code === "mesh-invalid"));
  });

  it("rejects unsupported primitive mode", async () => {
    const bytes = glb(triangleJson({}, 1), triangleBin());
    const asset = await compileAsset(record(bytes), bytes, filesFor(bytes));
    assert.deepEqual(asset.meshes, []);
    assert.ok(asset.diagnostics.some((d) => d.severity === "error" && d.code === "mesh-invalid"));
  });

  it("rejects oversized bytes before parsing", async () => {
    const bytes = new Uint8Array(MAX_ASSET_BYTES + 1);
    const asset = await compileAsset({ ...record(bytes), bytes: bytes.byteLength }, bytes, []);
    assert.deepEqual(asset.meshes, []);
    assert.ok(asset.diagnostics.some((d) => d.code === "asset-limit"));
  });
});
