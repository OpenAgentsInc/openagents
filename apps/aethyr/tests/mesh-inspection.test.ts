import assert from "node:assert/strict";
import test from "node:test";
import { resolve } from "node:path";
import { build } from "esbuild";
import { Renderer } from "../packages/renderer/renderer.ts";
import { createMeshInspection } from "../packages/renderer/mesh-inspection.ts";
import { MESH_FRAGMENT } from "../packages/renderer/mesh-view.ts";

function fixture() {
  const source = new Uint32Array([4, 5, 6, 7, 8, 9]);
  const events: string[] = [],
    uploads: { label: string; indices: Uint32Array }[] = [];
  const owned = new Set<any>();
  const state = { fail: "", uploadCount: 0, bytes: 0 };
  const pipeline = {},
    module = {};
  let descriptor: any,
    shaderSource = "";
  const device = {
    pushErrorScope(scope: string) {
      events.push("push:" + scope);
    },
    createShaderModule(options: any) {
      events.push("module");
      shaderSource = options.code;
      if (state.fail === "module") throw Error("Module failed");
      return module;
    },
    async createRenderPipelineAsync(value: any) {
      events.push("pipeline");
      descriptor = value;
      if (state.fail === "pipeline") throw Error("Pipeline failed");
      return pipeline;
    },
    async popErrorScope() {
      events.push("pop");
      return state.fail === "validation"
        ? { message: "Validation failed" }
        : null;
    },
  };
  const renderer: any = Object.assign(Object.create(Renderer.prototype), {
    device,
    shaders: { MATERIAL: "owned-stock-source" },
    features: { createMeshInspection },
    meshSourceIndices: source,
    meshEnabled: false,
    disposed: false,
    pipelineDescriptor(value: any) {
      assert.equal(value, module);
      if (state.fail === "descriptor") throw Error("Descriptor failed");
      return {
        vertex: { module, entryPoint: "vs" },
        fragment: {
          module,
          entryPoint: "fs",
          targets: [{ format: "rgba16float" }],
        },
        primitive: { topology: "triangle-list", cullMode: "back" },
        depthStencil: {
          format: "depth24plus",
          depthWriteEnabled: true,
          depthCompare: "less",
        },
        multisample: { count: 4 },
      };
    },
    buffer(data: ArrayBuffer, usage: number, label: string) {
      assert.equal(usage, 16);
      state.uploadCount++;
      if (state.fail === "second-upload" && state.uploadCount === 2)
        throw Error("Allocation failed");
      const buffer = { size: data.byteLength, label };
      owned.add(buffer);
      state.bytes += buffer.size;
      uploads.push({ label, indices: new Uint32Array(data).slice() });
      return buffer;
    },
    releaseBuffer(buffer: any) {
      if (buffer && owned.delete(buffer)) state.bytes -= buffer.size;
    },
  });
  return {
    renderer,
    source,
    events,
    uploads,
    owned,
    state,
    pipeline,
    get descriptor() {
      return descriptor;
    },
    get shaderSource() {
      return shaderSource;
    },
  };
}

async function withUsage(run: () => Promise<void>) {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { INDEX: 16 },
  });
  try {
    await run();
  } finally {
    if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
}

test("editor mesh overlay preserves shader, pipeline state, exact indices and tracked upload sizes", () =>
  withUsage(async () => {
    const f = fixture(),
      before = f.source.slice();
    await f.renderer.meshView(true);
    assert.equal(f.shaderSource, "owned-stock-source" + MESH_FRAGMENT);
    assert.deepEqual(f.events, [
      "push:validation",
      "module",
      "pipeline",
      "pop",
    ]);
    assert.equal(f.descriptor.vertex.entryPoint, "vs");
    assert.equal(f.descriptor.fragment.entryPoint, "meshFS");
    assert.deepEqual(f.descriptor.fragment.targets, [
      { format: "rgba16float" },
    ]);
    assert.deepEqual(f.descriptor.primitive, {
      topology: "line-list",
      cullMode: "none",
    });
    assert.deepEqual(f.descriptor.depthStencil, {
      format: "depth24plus",
      depthWriteEnabled: false,
      depthCompare: "less-equal",
    });
    assert.equal(f.descriptor.multisample.count, 4);
    assert.deepEqual(
      [...f.uploads[0].indices],
      [4, 5, 5, 6, 6, 4, 7, 8, 8, 9, 9, 7],
    );
    assert.deepEqual([...f.uploads[1].indices].slice(0, 6), [0, 1, 1, 2, 2, 0]);
    assert.deepEqual(
      [...f.uploads[1].indices].slice(-6),
      [33, 34, 34, 35, 35, 33],
    );
    assert.deepEqual(
      f.uploads.map((x) => x.label),
      ["Mesh inspection edges", "Room inspection edges"],
    );
    assert.equal(f.state.bytes, 48 + 288);
    assert.equal(f.renderer.meshPipeline, f.pipeline);
    assert.equal(f.renderer.meshEnabled, true);
    await f.renderer.meshView(false);
    await f.renderer.meshView(true);
    assert.equal(f.events.length, 4);
    assert.equal(f.state.uploadCount, 2);
    assert.equal(f.owned.size, 2);
    assert.deepEqual(f.source, before);
  }));

test("mesh setup balances error scopes and rejects oversized or incomplete source indices", () =>
  withUsage(async () => {
    for (const fail of ["module", "descriptor", "pipeline", "validation"]) {
      const f = fixture();
      f.state.fail = fail;
      await assert.rejects(f.renderer.meshView(true), /failed/i);
      assert.equal(f.events.filter((x) => x === "pop").length, 1);
      assert.equal(f.state.bytes, 0);
      assert.equal(f.state.uploadCount, 0);
      assert.equal(f.renderer.meshEnabled, false);
      assert.equal(f.renderer.meshPipeline, undefined);
    }
    for (const indices of [
      new Uint32Array([1, 2]),
      new Uint32Array(4_194_306),
    ]) {
      const f = fixture();
      f.renderer.meshSourceIndices = indices;
      await assert.rejects(f.renderer.meshView(true), /too large/);
      assert.equal(f.state.uploadCount, 0);
      assert.equal(f.state.bytes, 0);
    }
  }));

test("partial mesh allocations release tracked buffers and allow a clean retry", () =>
  withUsage(async () => {
    const f = fixture();
    f.state.fail = "second-upload";
    await assert.rejects(f.renderer.meshView(true), /Allocation failed/);
    assert.equal(f.state.bytes, 0);
    assert.equal(f.owned.size, 0);
    assert.equal(f.renderer.meshIndices, undefined);
    assert.equal(f.renderer.meshRoomIndices, undefined);
    assert.equal(f.renderer.meshPipeline, undefined);
    assert.equal(f.renderer.meshEnabled, false);
    f.state.fail = "";
    await f.renderer.meshView(true);
    assert.equal(f.state.bytes, 336);
    assert.equal(f.owned.size, 2);
    assert.equal(f.renderer.meshEnabled, true);
  }));

test("players omit the editor overlay and closing during setup cannot retain new buffers", () =>
  withUsage(async () => {
    const absent = fixture();
    absent.renderer.features = {};
    await absent.renderer.meshView(false);
    await assert.rejects(absent.renderer.meshView(true), /not available/);
    assert.equal(absent.events.length, 0);
    const closing = fixture();
    closing.renderer.device.createRenderPipelineAsync = async () => {
      closing.renderer.disposed = true;
      return closing.pipeline;
    };
    await assert.rejects(closing.renderer.meshView(true), /world closed/);
    assert.equal(closing.state.uploadCount, 0);
    assert.equal(closing.state.bytes, 0);
    assert.equal(closing.events.filter((x) => x === "pop").length, 1);
    const late = fixture();
    late.renderer.features.createMeshInspection = async (
      ...args: Parameters<typeof createMeshInspection>
    ) => {
      const resources = await createMeshInspection(...args);
      late.renderer.disposed = true;
      return resources;
    };
    await assert.rejects(late.renderer.meshView(true), /world closed/);
    assert.equal(late.state.bytes, 0);
    assert.equal(late.owned.size, 0);
  }));

test("published graphs exclude editor mesh and material tools while the editor graph includes them", async () => {
  const original = resolve("packages/renderer/features.ts");
  for (const profile of ["editor", "room", "player"]) {
    const result = await build({
      entryPoints: ["packages/runtime/published-world.ts"],
      bundle: true,
      write: false,
      metafile: true,
      platform: "browser",
      format: "esm",
      logLevel: "silent",
      define: {
        "import.meta.env": JSON.stringify({
          VITE_C_TOWN_BACKEND: "baseline",
          VITE_C_TOWN_BENCHMARK: "0",
        }),
      },
      plugins:
        profile === "editor"
          ? []
          : [
              {
                name: "player-feature-test",
                setup(builder) {
                  builder.onResolve(
                    { filter: /(?:^|\/)features\.ts$/ },
                    (args) =>
                      resolve(args.resolveDir, args.path) === original
                        ? {
                            path: resolve(
                              `packages/renderer/${profile}-features.ts`,
                            ),
                          }
                        : undefined,
                  );
                },
              },
            ],
    });
    const inputs = Object.keys(result.metafile!.inputs);
    for (const name of [
      "mesh-inspection.ts",
      "mesh-view.ts",
      "material-editor.ts",
    ])
      assert.equal(
        inputs.some((path) => path.endsWith("/" + name)),
        profile === "editor",
        `${profile}: ${name}`,
      );
    assert.equal(
      result.outputFiles!.some((file) =>
        file.text.includes("Select a werewolf face in this world."),
      ),
      profile === "editor",
    );
  }
});
