import assert from "node:assert/strict";
import { test } from "node:test";
import type {
  CompiledMaterial,
  CompiledScene,
} from "../packages/assets/src/scene.ts";
import { Renderer } from "../packages/renderer/renderer.ts";
import {
  createMaterialEditor,
  type MaterialEditorHost,
} from "../packages/renderer/material-editor.ts";
import type {
  MaterialBundle,
  MaterialDocument,
} from "../packages/runtime/material-source.ts";
import { RuntimePropTable } from "../packages/runtime/prop-table.ts";

const kind = "custom:haunt:werewolf",
  materialId = kind + ":face";
const material = {
  id: materialId,
  name: "Face",
  baseColorFactor: [1, 1, 1, 1],
  emissiveFactor: [0, 0, 0],
  roughness: 0.7,
  metallic: 0,
  normalScale: 1,
  uvScale: [1, 1],
  alphaMode: "OPAQUE",
  alphaCutoff: 0.5,
  doubleSided: true,
  packedChannels: "metallic-r-roughness-g-occlusion-b",
} as CompiledMaterial;
const scene = {
  materials: [material],
  meshes: [
    {
      primitives: [
        { firstIndex: 0, indexCount: 3, baseVertex: 0, material: 0 },
      ],
    },
  ],
} as unknown as CompiledScene;
function document(entity = "wolf", strength = 0.25): MaterialDocument {
  return {
    schema: "openagents.verse.material.v1",
    target: {
      entity,
      asset: kind,
      surface: "werewolf.face",
      material: materialId,
      uv_set: 0,
      size: [256, 256],
      uv_bounds: [0, 0, 1, 1],
    },
    generated: {
      image: { sha256: "1".repeat(64), bytes: 1, media_type: "image/png" },
      visible: true,
      strength,
    },
    handmade: {
      image: { sha256: "2".repeat(64), bytes: 1, media_type: "image/png" },
      visible: true,
      strength: 0.75,
    },
    recipe: null,
  };
}
function snapshot(documents: MaterialDocument[] = []) {
  return {
    entities: [
      ...new Set(["wolf", ...documents.map((item) => item.target.entity)]),
    ].map((id) => ({
      id,
      kind,
      position: [0, 0, 0],
      yaw: 0,
      scale: 1,
      collidable: false,
    })),
    rooms: [],
    authoring: {
      materialSources: Object.fromEntries(
        documents.map((item, index) => [index, { text: JSON.stringify(item) }]),
      ),
    },
  };
}
function bundle(doc: MaterialDocument): MaterialBundle {
  return {
    document: doc,
    generated: { resource: doc.generated.image!, png_base64: "YQ==" },
    handmade: { resource: doc.handmade.image!, png_base64: "Yg==" },
  };
}
async function images(
  run: (state: {
    drawn: { alpha: number; source: number }[];
    closed: number[];
    wrongSize: boolean;
  }) => Promise<void>,
) {
  const state = {
    drawn: [] as { alpha: number; source: number }[],
    closed: [] as number[],
    wrongSize: false,
  };
  const saved = new Map(
    [
      "OffscreenCanvas",
      "createImageBitmap",
      "GPUTextureUsage",
      "GPUBufferUsage",
    ].map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]),
  );
  Object.assign(globalThis, {
    GPUTextureUsage: { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 },
    GPUBufferUsage: { VERTEX: 1, INDEX: 2, COPY_DST: 4, UNIFORM: 8 },
    OffscreenCanvas: class {
      constructor(width: number, height: number) {
        assert.equal(width, 256);
        assert.equal(height, 256);
      }
      getContext() {
        return {
          globalAlpha: 1,
          drawImage(bitmap: { source: number }, x: number, y: number) {
            assert.equal(x, 0);
            assert.equal(y, 0);
            state.drawn.push({
              alpha: this.globalAlpha,
              source: bitmap.source,
            });
          },
        };
      }
    },
    createImageBitmap: async (blob: Blob) => {
      assert.equal(blob.type, "image/png");
      const source = new Uint8Array(await blob.arrayBuffer())[0];
      return {
        source,
        width: state.wrongSize ? 128 : 256,
        height: 256,
        close() {
          state.closed.push(source);
        },
      };
    },
  });
  try {
    await run(state);
  } finally {
    for (const [name, value] of saved)
      if (value) Object.defineProperty(globalThis, name, value);
      else Reflect.deleteProperty(globalThis, name);
  }
}
function fixture() {
  const textures: any[] = [],
    buffers = new Set<any>(),
    released: any[] = [],
    writes: { buffer: any; values: Float32Array }[] = [];
  const installed: {
    start: number;
    groups: GPUBindGroup[];
    sources: CompiledMaterial[];
  }[] = [];
  const state = {
    fail: "",
    materialCalls: 0,
    fence: Promise.resolve() as Promise<void>,
    suspend: undefined as undefined | (() => Promise<void>),
  };
  const device = {
    createTexture(options: GPUTextureDescriptor) {
      const texture = {
        options,
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
        createView() {
          return {};
        },
      };
      textures.push(texture);
      return texture;
    },
    queue: {
      copyExternalImageToTexture(_from: unknown, to: any, size: unknown) {
        assert.equal(to.premultipliedAlpha, false);
        assert.deepEqual(size, [256, 256]);
        if (state.fail === "copy") throw Error("Copy failed");
      },
      writeBuffer(buffer: any, offset: number, values: Float32Array) {
        assert.equal(offset % 4, 0);
        writes.push({ buffer, values: values.slice() });
      },
      onSubmittedWorkDone() {
        return state.fence;
      },
    },
  } as unknown as GPUDevice;
  const host: MaterialEditorHost = {
    scene,
    device,
    images: bundle,
    count: () => 1,
    async material(_material, _texture, uniform) {
      state.materialCalls++;
      const buffer = {} as GPUBuffer,
        original = new Float32Array(24).fill(0.5);
      buffers.add(buffer);
      uniform(buffer, original);
      await state.suspend?.();
      if (state.fail === "material") throw Error("Material failed");
      return { material: state.materialCalls } as unknown as GPUBindGroup;
    },
    install(start, groups, sources) {
      installed.push({ start, groups, sources });
    },
    release(buffer) {
      if (buffer && buffers.delete(buffer)) released.push(buffer);
    },
  };
  return {
    editor: createMaterialEditor(host),
    host,
    device,
    textures,
    buffers,
    released,
    writes,
    installed,
    state,
  };
}

test("face staging preserves layer order/alpha, byte layout, per-object bindings, and reuse", () =>
  images(async (image) => {
    const f = fixture(),
      doc = document();
    const plan = await f.editor.prepare(snapshot([doc]));
    assert.equal(plan.ready(), true);
    assert.equal(f.installed.length, 0);
    assert.deepEqual(image.drawn, [
      { alpha: 0.25, source: 97 },
      { alpha: 0.75, source: 98 },
    ]);
    assert.deepEqual(image.closed, [97, 98]);
    assert.equal(f.textures[0].options.format, "rgba8unorm-srgb");
    assert.deepEqual(f.textures[0].options.size, [256, 256]);
    assert.equal(f.textures[0].options.usage, 7);
    assert.equal(plan.remap("wolf", 0), 1);
    assert.equal(plan.remap("other", 0), 0);
    plan.activate();
    assert.equal(plan.ready(), false);
    assert.deepEqual(f.installed[0].sources, [material]);
    assert.equal(f.editor.remap("wolf", 0), 1);
    const reused = await f.editor.prepare(snapshot([doc]));
    assert.equal(f.state.materialCalls, 1);
    assert.equal(f.textures.length, 1);
    reused.activate();
    await Promise.resolve();
    assert.equal(f.textures[0].destroyed, 0);
    f.editor.dispose();
    f.editor.dispose();
    assert.equal(f.textures[0].destroyed, 1);
  }));

test("material parameter previews use retained originals for base and per-object uniforms", () =>
  images(async () => {
    const f = fixture(),
      base = {} as GPUBuffer,
      original = new Float32Array(24).map((_, index) => index);
    f.editor.track(materialId, base, original);
    original.fill(99);
    const plan = await f.editor.prepare(snapshot([document()]));
    plan.activate();
    const values = {
      [materialId]: {
        red: 0.25,
        green: 0.5,
        blue: 0.75,
        roughness: 0.2,
        metallic: 0.3,
        normal: 0.4,
      },
    };
    f.editor.applyValues(values);
    assert.equal(f.writes.length, 2);
    for (const { values: data } of f.writes) {
      assert.equal(data[0], Math.fround(0.25));
      assert.equal(data[1], Math.fround(0.5));
      assert.equal(data[2], Math.fround(0.75));
      assert.equal(data[8], Math.fround(0.2));
      assert.equal(data[9], Math.fround(0.3));
      assert.equal(data[10], Math.fround(0.4));
    }
    assert.equal(f.writes[0].values[3], 3);
    assert.equal(f.writes[1].values[3], 0.5);
    f.editor.applyValues({});
    assert.equal(f.writes[2].values[0], 0);
    assert.equal(f.writes[3].values[0], 0.5);
    const bindings = f.editor.bindings(
      {
        entities: [
          { id: "one", kind },
          { id: "two", kind },
          { id: "missing", kind: "absent" },
        ],
      },
      new Map([[kind, 0]]),
    );
    assert.deepEqual(bindings, [{ id: materialId, objects: ["one", "two"] }]);
    f.editor.dispose();
  }));

test("discard/failure cannot replace a working face; retired uniforms wait for the queue", () =>
  images(async () => {
    const f = fixture(),
      initial = await f.editor.prepare(snapshot([document()]));
    initial.activate();
    const candidate = await f.editor.prepare(snapshot([document("wolf", 0.5)]));
    candidate.discard();
    candidate.discard();
    assert.equal(f.textures[1].destroyed, 1);
    assert.equal(f.textures[0].destroyed, 0);
    assert.equal(f.installed.length, 1);
    assert.equal(f.editor.remap("wolf", 0), 1);
    let finish!: () => void;
    f.state.fence = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const replacement = await f.editor.prepare(snapshot([document("other")]));
    replacement.activate();
    assert.equal(f.editor.remap("wolf", 0), 0);
    assert.equal(f.editor.remap("other", 0), 1);
    assert.equal(f.textures[0].destroyed, 0);
    finish();
    await Promise.resolve();
    assert.equal(f.textures[0].destroyed, 1);
    assert.equal(f.released.length, 2);
    f.editor.dispose();
    assert.equal(f.textures[2].destroyed, 1);
  }));

test("image, copy, material, and partial multi-face failures release only newly created resources", () =>
  images(async (image) => {
    for (const fail of ["size", "copy", "material"]) {
      const f = fixture();
      image.wrongSize = fail === "size";
      f.state.fail = fail;
      await assert.rejects(
        f.editor.prepare(snapshot([document()])),
        /256|failed/i,
      );
      assert.equal(f.installed.length, 0);
      assert.equal(f.buffers.size, 0);
      for (const texture of f.textures) assert.equal(texture.destroyed, 1);
      f.editor.dispose();
    }
    image.wrongSize = false;
    const f = fixture(),
      original = f.host.images;
    f.host.images = (doc) =>
      doc.target.entity === "other" ? undefined : original(doc);
    await assert.rejects(
      f.editor.prepare(snapshot([document(), document("other")])),
      /source images/,
    );
    assert.equal(f.textures[0].destroyed, 1);
    assert.equal(f.buffers.size, 0);
  }));

test("closing pending and late material work invalidates plans and releases resources once", () =>
  images(async () => {
    const pending = fixture(),
      plan = await pending.editor.prepare(snapshot([document()]));
    pending.editor.dispose();
    assert.equal(plan.ready(), false);
    assert.throws(() => plan.activate(), /closed/);
    plan.discard();
    assert.equal(pending.textures[0].destroyed, 1);
    assert.equal(pending.buffers.size, 0);
    await assert.rejects(pending.editor.prepare(snapshot()), /closed/);
    const late = fixture();
    let release!: () => void;
    let entered = false;
    late.state.suspend = () => {
      entered = true;
      return new Promise<void>((resolve) => {
        release = resolve;
      });
    };
    const work = late.editor.prepare(snapshot([document()]));
    for (let index = 0; index < 30 && !entered; index++)
      await Promise.resolve();
    assert(entered);
    late.editor.dispose();
    release();
    await assert.rejects(work, /closed/);
    assert.equal(late.textures[0].destroyed, 1);
    assert.equal(late.buffers.size, 0);
  }));

function rendererFixture(editor: boolean) {
  const f = fixture(),
    r = Object.create(Renderer.prototype) as any;
  const allocated: any[] = [],
    released: any[] = [];
  Object.assign(r, {
    scene,
    device: f.device,
    features: editor ? { createMaterialEditor } : {},
    materials: [{}],
    renderMaterialInfo: [material],
    creatorImageProvider: bundle,
    materialFeatures: new WeakMap(),
    sampler: {},
    materialLayout: {},
    layerPools: {
      forMaterial() {
        return Object.fromEntries(
          ["color", "normal", "data"].map((role) => [
            role,
            {
              index: () => 0,
              texture: {
                createView() {
                  return {};
                },
              },
            },
          ]),
        );
      },
    },
    texture: async () => ({
      createView() {
        return {};
      },
    }),
    prepareStockPipelines: async () => {},
    propTable: new RuntimePropTable({ initialCapacity: 32 }),
    authoredKinds: new Map(),
    authoredEntityDraws: new Map(),
    authoredFullRebuilds: 0,
    authoredIncrementalUpdates: 0,
    authoredBytesUploaded: 0,
    roomVertices: {},
    roomMaterial: {},
    geometryVersion: 0,
    buffer(data: ArrayBuffer | ArrayBufferView) {
      const buffer = {
        size: data.byteLength,
        values: new Uint8Array("buffer" in data ? data.buffer : data).slice(),
      };
      allocated.push(buffer);
      return buffer;
    },
    releaseBuffer(buffer: unknown) {
      if (buffer) released.push(buffer);
    },
  });
  (r.device as any).pushErrorScope = () => {};
  (r.device as any).popErrorScope = async () => null;
  (r.device as any).createBindGroup = () => ({});
  return { r, f, allocated, released };
}
test("ordinary published props/rooms retain identity materials; unsupported face data fails explicitly", () =>
  images(async () => {
    const { r } = rendererFixture(false),
      source = snapshot();
    (source.rooms as any[]).push({
      id: "room",
      center: [0, 0],
      size: [4, 4],
      floorY: 0,
    });
    const plan = await r.prepareAuthored(source, new Map([[kind, 0]]));
    assert(plan.ready());
    plan.activate();
    assert.equal(r.authoredDraws[0].material, 0);
    assert.equal(r.roomCount, 1);
    assert.equal(r.materialEditor, undefined);
    const live = r.authoredInstances;
    await assert.rejects(
      r.prepareAuthored(snapshot([document()]), new Map([[kind, 0]])),
      /editor.*face materials/,
    );
    assert.equal(r.authoredInstances, live);
    assert.equal(r.authoredDraws[0].material, 0);
    await r.makeMaterial(material);
    assert.equal(r.materialEditor, undefined);
    assert.throws(
      () => r.applyMaterialValues({ [materialId]: { red: 0.5 } }),
      /editor/,
    );
  }));

test("actual editor bridge retains base previews, face remaps and incremental prop updates", () =>
  images(async () => {
    const { r, f } = rendererFixture(true);
    await r.makeMaterial(material);
    assert(r.materialEditor);
    const source = snapshot([document()]),
      meshes = new Map([[kind, 0]]);
    const first = await r.prepareAuthored(source, meshes);
    first.activate();
    assert.equal(r.authoredDraws[0].material, 1);
    const next = structuredClone(source);
    next.entities[0].position = [2, 0, 0];
    next.entities.push({
      id: "new-wolf",
      kind,
      position: [4, 0, 0],
      yaw: 0,
      scale: 1,
      collidable: false,
    });
    const second = await r.prepareAuthored(next, meshes);
    second.activate();
    assert.equal(r.authoredIncrementalUpdates, 1);
    assert.deepEqual(
      r.authoredDraws.map((draw: any) => draw.material),
      [1, 0],
    );
    r.applyMaterialValues({ [materialId]: { red: 0.25 } });
    assert.equal(f.writes.length, 4);
    assert.deepEqual(r.materialBindings(next, meshes), [
      { id: materialId, objects: ["wolf", "new-wolf"] },
    ]);
    r.materialEditor.dispose();
  }));

test("late base-material completion cannot reopen editor state or admit a closed-world identity plan", () =>
  images(async () => {
    const { r } = rendererFixture(true);
    let created = 0;
    r.features.createMaterialEditor = (host: MaterialEditorHost) => {
      created++;
      return createMaterialEditor(host);
    };
    let release!: () => void;
    const waiting = new Promise<void>((resolve) => {
      release = resolve;
    });
    r.texture = async () => {
      await waiting;
      return {
        createView() {
          return {};
        },
      };
    };
    const material = r.makeMaterial(scene.materials[0]);
    r.disposed = true;
    r.materialEditor = undefined;
    release();
    await material;
    assert.equal(created, 0);
    assert.equal(r.materialEditor, undefined);
    await assert.rejects(r.prepareCreatorMaterials(snapshot()), /closed/);
    await assert.rejects(
      r.prepareCreatorMaterials(snapshot([document()])),
      /closed/,
    );
    assert.equal(created, 0);
  }));
