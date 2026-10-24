import assert from "node:assert/strict";
import { test } from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { normalMapFlags } from "../packages/renderer/normals.ts";
import { validateSourceNormalFrames } from "../packages/renderer/source-normal-profile.ts";

const sha = async (b: ArrayBuffer) =>
  Buffer.from(await crypto.subtle.digest("SHA-256", b)).toString("hex");
async function fixture() {
  const v = new Float32Array([
    0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0,
    1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 1,
  ]);
  const ix = new Uint32Array([0, 1, 2]),
    image = new Uint8Array([3, 4, 5]).buffer;
  const evidence = {
    sourceFactsSha256: "1".repeat(64),
    sourceMeshId: "owned-triangle",
    verticesSha256: await sha(v.buffer),
    indicesSha256: await sha(ix.buffer),
    normalTextureSha256: await sha(image),
  };
  const scene = {
    vertexStride: 48,
    vertexCount: 3,
    indexCount: 3,
    materials: [
      {
        id: "owned-material",
        normalUrl: "owned-normal",
        normalConvention: "opengl",
        normalEncoding: "xy-positive-z",
        uvScale: [1, 1],
        normalFrameProfile: { model: "source-mesh-noscale-v1", evidence },
      },
    ],
    meshes: [
      {
        id: "owned-triangle",
        primitives: [
          {
            material: 0,
            materialSlot: 0,
            baseVertex: 0,
            firstIndex: 0,
            indexCount: 3,
          },
        ],
      },
    ],
    batches: [
      { mesh: 0, materialOverrides: [], firstInstance: 0, instanceCount: 1 },
    ],
    instances: [
      {
        mesh: 0,
        materialOverrides: [],
        model: [2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0.5, 0, 0, 0, 0, 1],
      },
    ],
  } as unknown as CompiledScene;
  return {
    scene,
    vertices: v.buffer,
    indices: ix.buffer,
    read: async () => image,
  };
}
test("native profile binds actual geometry, texture, uses and transforms", async () => {
  const f = await fixture();
  assert.equal(normalMapFlags(f.scene.materials[0]), 6);
  await validateSourceNormalFrames(f.scene, f.vertices, f.indices, f.read);
  const missing = { ...f.scene, materials: [] };
  await validateSourceNormalFrames(missing, f.vertices, f.indices, async () => {
    throw Error("unused");
  });
  for (const mutate of [
    (s: CompiledScene) => (s.materials[0].normalConvention = "directx"),
    (s: CompiledScene) =>
      (s.materials[0].normalFrameProfile!.evidence.verticesSha256 = "2".repeat(
        64,
      )),
    (s: CompiledScene) =>
      (s.materials[0].normalFrameProfile!.evidence.normalTextureSha256 =
        "2".repeat(64)),
    (s: CompiledScene) =>
      (s.materials[0].normalFrameProfile!.evidence.sourceMeshId = "other"),
    (s: CompiledScene) => (s.instances[0].model[0] = NaN),
    (s: CompiledScene) => (s.instances[0].model[0] = 0),
    (s: CompiledScene) => (s.instances[0].model[4] = 1),
    (s: CompiledScene) => (s.instances[0].model[15] = 0),
    (s: CompiledScene) => (s.meshes[0].primitives[0].indexCount = 4),
  ]) {
    const scene = structuredClone(f.scene);
    mutate(scene);
    await assert.rejects(
      validateSourceNormalFrames(scene, f.vertices, f.indices, f.read),
    );
  }
  assert.throws(() =>
    normalMapFlags({ normalFrameProfile: { model: "unknown" } as any }),
  );
  assert.throws(() =>
    normalMapFlags({
      normalFrameProfile: {
        ...f.scene.materials[0].normalFrameProfile,
        extra: 1,
      } as any,
    }),
  );
});
test("native profile rejects invalid indexed attributes even with matching bytes", async () => {
  const f = await fixture(),
    v = new Float32Array(f.vertices);
  v[8] = 0;
  f.scene.materials[0].normalFrameProfile!.evidence.verticesSha256 = await sha(
    f.vertices,
  );
  await assert.rejects(
    validateSourceNormalFrames(f.scene, f.vertices, f.indices, f.read),
    /tangent frame/,
  );
});

async function vegetationFixture() {
  const f = await fixture();
  const original = structuredClone(f.scene.materials[0]);
  delete original.normalFrameProfile;
  original.normalConvention = "directx";
  original.vegetation = {
    model: "ue-mm-vegetation-1-v1",
    normalUrl: "owned-layer-normal",
  } as any;
  f.scene.materials.push(original);
  const clone = f.scene.materials[0];
  clone.id = "owned-clone";
  clone.vegetation = structuredClone(original.vegetation);
  clone.normalFrameProfile!.model = "source-vegetation-noscale-v1";
  Object.assign(clone.normalFrameProfile!.evidence, {
    sourceMaterialId: original.id,
    sourceMaterialsSha256: "3".repeat(64),
    sourceNormalMetadataSha256: "4".repeat(64),
  });
  return f;
}

test("native vegetation requires an isolated unchanged material clone and its actual normal image", async () => {
  const f = await vegetationFixture();
  const stale = structuredClone(f.scene);
  stale.batches[0].materialOverrides = [1];
  await assert.rejects(
    validateSourceNormalFrames(stale, f.vertices, f.indices, f.read),
    /render batches/,
  );
  for (const batches of [[], [...f.scene.batches, ...f.scene.batches]]) {
    await assert.rejects(
      validateSourceNormalFrames(
        { ...f.scene, batches },
        f.vertices,
        f.indices,
        f.read,
      ),
      /render batch/,
    );
  }
  const requests: string[] = [];
  await validateSourceNormalFrames(
    f.scene,
    f.vertices,
    f.indices,
    async (url) => {
      requests.push(url);
      return f.read();
    },
  );
  assert.deepEqual(requests, ["owned-layer-normal"]);
  for (const mutate of [
    (s: CompiledScene) =>
      (s.materials[0].vegetation!.normalUrl = "different-image"),
    (s: CompiledScene) => (s.materials[0].roughness = 0.8),
    (s: CompiledScene) =>
      (s.materials[0].normalFrameProfile!.evidence.sourceMaterialId =
        "missing"),
    (s: CompiledScene) =>
      delete s.materials[0].normalFrameProfile!.evidence.sourceMaterialsSha256,
    (s: CompiledScene) =>
      (s.materials[0].normalFrameProfile!.model = "source-mesh-noscale-v1"),
    (s: CompiledScene) => {
      s.meshes.push({ ...s.meshes[0], id: "another-mesh" });
      s.instances.push({ ...s.instances[0], mesh: 1 });
    },
  ]) {
    const scene = structuredClone(f.scene);
    mutate(scene);
    await assert.rejects(
      validateSourceNormalFrames(scene, f.vertices, f.indices, f.read),
    );
  }
});

test("native clones allow only validated owned transmission while preserving the original recipe", async () => {
  const f = await vegetationFixture();
  for (const anisotropy of [0, 0.5, 0.8]) {
    const scene = structuredClone(f.scene);
    scene.materials[0].vegetation!.transmission = {
      model: "bounded-forward-sheet-v1",
      anisotropy,
    };
    const before = structuredClone(scene);
    await validateSourceNormalFrames(scene, f.vertices, f.indices, f.read);
    assert.deepEqual(scene, before);
    assert.equal(scene.materials[1].vegetation!.transmission, undefined);
  }
  for (const mutate of [
    (s: CompiledScene) =>
      (s.materials[0].vegetation!.transmission = {
        model: "unknown",
        anisotropy: 0.5,
      } as any),
    (s: CompiledScene) =>
      (s.materials[0].vegetation!.transmission = {
        model: "bounded-forward-sheet-v1",
        anisotropy: 0.81,
      }),
    (s: CompiledScene) =>
      (s.materials[0].vegetation!.transmission = {
        model: "bounded-forward-sheet-v1",
        anisotropy: NaN,
      }),
    (s: CompiledScene) =>
      (s.materials[0].vegetation!.transmission = {
        model: "bounded-forward-sheet-v1",
        anisotropy: 0.5,
        sourceMeasured: true,
      } as any),
    (s: CompiledScene) =>
      (s.materials[1].vegetation!.transmission = {
        model: "bounded-forward-sheet-v1",
        anisotropy: 0.5,
      }),
    (s: CompiledScene) => (s.materials[0].vegetation!.normalFlatness = 0.4),
    (s: CompiledScene) =>
      ((s.materials[0].vegetation as any).sourceParameters = "changed"),
    (s: CompiledScene) => (s.materials[0].roughness = 0.7),
  ]) {
    const scene = structuredClone(f.scene);
    scene.materials[0].vegetation!.transmission = {
      model: "bounded-forward-sheet-v1",
      anisotropy: 0.5,
    };
    mutate(scene);
    await assert.rejects(
      validateSourceNormalFrames(scene, f.vertices, f.indices, f.read),
    );
  }
});
