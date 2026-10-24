import assert from "node:assert/strict";
import test, { after } from "node:test";
import {
  MmorpgNames,
  MMORPG_NAME_TEXTURE_BYTES,
  MMORPG_NAME_HEIGHT,
  MMORPG_NAME_SIZE,
  mmorpgNameAxes,
  paintMmorpgName,
  MMORPG_NAME_WGSL,
} from "../packages/renderer/mmorpg-names.ts";
import {
  lookAt,
  multiply,
  perspective,
  type V3,
} from "../packages/renderer/math.ts";
import type { MmorpgActorPose } from "../packages/assets/src/mmorpg.ts";
import type { CreatureFrame } from "../packages/renderer/creatures.ts";

const previous = ["GPUTextureUsage", "GPUBufferUsage"].map(
  (name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)] as const,
);
Object.defineProperty(globalThis, "GPUTextureUsage", {
  configurable: true,
  value: { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 },
});
Object.defineProperty(globalThis, "GPUBufferUsage", {
  configurable: true,
  value: { UNIFORM: 1, COPY_DST: 2, STORAGE: 4 },
});
after(() => {
  for (const [name, descriptor] of previous) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else delete (globalThis as any)[name];
  }
});
const actor = (id: number, name = "Zombie"): MmorpgActorPose => ({
  id,
  name,
  class: "zombie",
  position: [0, 0, 5],
  yaw: 0,
  state: "idle",
  actionTime: 0,
  animationTime: 0,
  health: 20,
  maxHealth: 50,
  alive: true,
  weapon: null,
});
const camera = (
  eye: V3 = [0, 3, -5],
  target: V3 = [0, 1, 5],
): CreatureFrame => ({
  mode: "day",
  time: 0,
  viewProjection: multiply(
    perspective(Math.PI / 3, 16 / 9, 1 / 6, 2000),
    lookAt(eye, target),
  ),
  eye,
  sunDirection: [0, 1, 0],
  sunIlluminance: [1, 1, 1],
  diffuseRotation: new Float32Array(16),
  diffuseSH: new Float32Array(36),
});
function gpu() {
  const pipelines: any[] = [],
    textures: any[] = [],
    buffers: any[] = [],
    copies: any[] = [],
    writes: { buffer: any; values: Float32Array }[] = [],
    mips: any[] = [],
    draws: number[][] = [],
    rasterized: string[] = [];
  let failGroup = false;
  const device = {
    createShaderModule: (descriptor: any) => descriptor,
    async createRenderPipelineAsync(descriptor: any) {
      pipelines.push(descriptor);
      return { getBindGroupLayout: () => ({}) };
    },
    createTexture(descriptor: any) {
      const texture: any = {
        descriptor,
        destroyed: 0,
        createView: (view: any = {}) => ({ view, texture }),
        destroy() {
          this.destroyed++;
        },
      };
      textures.push(texture);
      return texture;
    },
    createBuffer(descriptor: any) {
      const buffer = {
        ...descriptor,
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    createSampler: (descriptor: any) => descriptor,
    createBindGroup(descriptor: any) {
      if (failGroup) throw Error("Test group failure");
      return descriptor;
    },
    queue: {
      copyExternalImageToTexture(source: any, target: any, size: any) {
        copies.push({ source, target, size });
      },
      writeBuffer(buffer: any, _offset: number, data: Float32Array) {
        writes.push({ buffer, values: new Float32Array(data) });
      },
    },
  } as unknown as GPUDevice;
  const encoder = {
    beginRenderPass(descriptor: any) {
      const mip: any = { descriptor, draws: [] };
      mips.push(mip);
      return {
        setPipeline() {},
        setBindGroup() {},
        draw(...args: number[]) {
          mip.draws.push(args);
        },
        end() {},
      };
    },
  } as unknown as GPUCommandEncoder;
  const pass = {
    setPipeline() {},
    setBindGroup() {},
    draw(...args: number[]) {
      draws.push(args);
    },
  } as unknown as GPURenderPassEncoder;
  return {
    device,
    encoder,
    pass,
    pipelines,
    textures,
    buffers,
    copies,
    writes,
    mips,
    draws,
    rasterized,
    rasterize: (name: string) => {
      rasterized.push(name);
      return { width: 256, height: 128 } as OffscreenCanvas;
    },
    failGroups() {
      failGroup = true;
    },
  };
}

test("name artwork retains exact source canvas typography, size and class offsets", () => {
  const calls: unknown[] = [],
    context: any = {
      clearRect: (...args: any[]) => calls.push(args),
      fillText: (...args: any[]) => calls.push(args),
    };
  paintMmorpgName(context, "Knight <friend>");
  assert.deepEqual(calls, [
    [0, 0, 256, 128],
    ["Knight <friend>", 128, 64],
  ]);
  assert.equal(context.font, "18pt Helvetica");
  assert.equal(context.fillStyle, "#FFF");
  assert.equal(context.textAlign, "center");
  assert.equal(context.shadowOffsetX, 3);
  assert.equal(context.shadowOffsetY, 3);
  assert.equal(context.shadowBlur, 4);
  assert.equal(context.shadowColor, "rgba(0,0,0,0.3)");
  assert.deepEqual(MMORPG_NAME_SIZE, [10 / 6, 5 / 6]);
  assert.deepEqual(MMORPG_NAME_HEIGHT, {
    paladin: 11 / 6,
    sorceror: 10 / 6,
    zombie: 8 / 6,
    warrok: 16 / 6,
  });
});

test("billboard camera axes follow the actual pitched and off-axis projection", () => {
  const view = lookAt([12, 6, -5], [3, 1, 7]),
    projection = perspective(0.9, 1.7, 0.1, 2000);
  projection[8] = 0.2;
  projection[9] = -0.15;
  const axes = mmorpgNameAxes(multiply(projection, view));
  for (const [axis, row] of [
    [axes.right, 0],
    [axes.up, 1],
  ] as const)
    for (let i = 0; i < 3; i++)
      assert(Math.abs(axis[i] - view[row + i * 4]) < 0.000005);
  assert.throws(() => mmorpgNameAxes(new Float32Array(16)), /singular/);
  assert.throws(
    () => mmorpgNameAxes(new Float32Array(16).fill(NaN)),
    /complete/,
  );
});

test("names share one depth-tested draw and cache rasterization and all real mips", async () => {
  const g = gpu(),
    names = await MmorpgNames.create(g.device, { rasterize: g.rasterize });
  const actors = [
    actor(1, "Local"),
    actor(2),
    actor(3),
    { ...actor(4), alive: false, state: "death" as const, health: 0 },
  ];
  names.prepare(g.encoder, camera(), actors, 1);
  assert.equal(names.stats.nameLabels, 0);
  names.render(g.pass);
  assert.deepEqual(g.draws, [[6, 2]]);
  assert.deepEqual(g.rasterized, ["Zombie"]);
  assert.equal(g.copies.length, 1);
  assert.equal(g.mips.length, 8);
  assert(g.mips.every((m) => m.draws[0][0] === 3));
  assert.equal(g.pipelines[0].depthStencil.depthWriteEnabled, false);
  assert.equal(g.pipelines[0].depthStencil.depthCompare, "less-equal");
  assert.equal(g.pipelines[0].multisample.count, 4);
  assert.equal(g.pipelines[0].fragment.targets[0].blend.color.srcFactor, "one");
  assert.deepEqual(g.textures[0].descriptor.size, [256, 128, 64]);
  assert.equal(g.textures[0].descriptor.mipLevelCount, 9);
  assert.equal(names.stats.nameTriangles, 4);
  assert.equal(names.stats.nameDrawCalls, 1);
  assert.equal(names.stats.nameTextureBytes, MMORPG_NAME_TEXTURE_BYTES);
  assert(
    MMORPG_NAME_TEXTURE_BYTES > 8 * 1024 * 1024 &&
      MMORPG_NAME_TEXTURE_BYTES < 11 * 1024 * 1024,
  );
  assert.equal(names.bytes, MMORPG_NAME_TEXTURE_BYTES + 96 + 64 * 16);
  const before = new Float32Array(g.writes.at(-2)!.values);
  names.prepare(g.encoder, camera([4, 3, -5]), actors, 1);
  names.render(g.pass);
  assert.equal(g.copies.length, 1);
  assert.equal(g.mips.length, 8);
  assert.notDeepEqual(g.writes.at(-2)!.values, before);
  assert.equal(g.writes.at(-1)!.values[1], Math.fround(8 / 6));
  names.dispose();
  names.dispose();
  assert(g.buffers.every((b) => b.destroyed === 1));
  assert.equal(g.textures[0].destroyed, 1);
  assert.equal(names.stats.nameTextureBytes, 0);
  names.render(g.pass);
  assert.equal(names.stats.nameLabels, 0);
});

test("full name cache remains bounded and reuses a retired texture layer", async () => {
  const g = gpu(),
    names = await MmorpgNames.create(g.device, { rasterize: g.rasterize }),
    actors = Array.from({ length: 64 }, (_, i) => actor(i, "N" + i));
  names.prepare(g.encoder, camera(), actors);
  names.render(g.pass);
  assert.equal(g.copies.length, 64);
  assert.equal(names.stats.nameLabels, 64);
  assert.equal(names.stats.nameTriangles, 128);
  assert.equal(names.stats.nameTextureLayers, 64);
  actors[0] = actor(0, "N64");
  names.prepare(g.encoder, camera(), actors);
  names.render(g.pass);
  assert.equal(names.stats.nameTextureLayers, 64);
  assert.equal(g.copies.length, 65);
  assert.equal(g.copies[64].target.origin[2], g.copies[0].target.origin[2]);
  names.prepare(g.encoder, camera(), actors);
  assert.equal(g.copies.length, 65);
  names.dispose();
});

test("invalid submissions do not upload art and dead, local, near-plane or rear labels do not draw", async () => {
  const g = gpu(),
    names = await MmorpgNames.create(g.device, { rasterize: g.rasterize });
  assert.throws(() => names.prepare(g.encoder, camera(), [actor(1), actor(1)]));
  assert.equal(g.copies.length, 0);
  assert.equal(g.writes.length, 0);
  const rear = { ...actor(2), position: [0, 0, -20] as V3 };
  names.prepare(g.encoder, camera(), [actor(1), rear], 1);
  names.render(g.pass);
  assert.equal(names.stats.nameLabels, 0);
  names.prepare(g.encoder, { ...camera(), enabled: false }, [actor(1)]);
  names.render(g.pass);
  assert.equal(names.stats.nameLabels, 0);
  assert(
    MMORPG_NAME_WGSL.includes(
      "anchor.w<=.0001||anchor.z<0.0||anchor.z>anchor.w",
    ),
  );
  assert(!MMORPG_NAME_WGSL.includes("fog"));
  names.dispose();
});

test("name billboards sort from far to near and release failed GPU candidates", async () => {
  const g = gpu(),
    names = await MmorpgNames.create(g.device, { rasterize: g.rasterize });
  names.prepare(g.encoder, camera(), [
    actor(1),
    { ...actor(2), position: [0, 0, 20] },
  ]);
  names.render(g.pass);
  assert.equal(g.writes.at(-1)!.values[2], 20);
  assert.equal(g.writes.at(-1)!.values[6], 5);
  names.dispose();
  const failed = gpu();
  failed.failGroups();
  await assert.rejects(MmorpgNames.create(failed.device), /Test group/);
  assert(failed.buffers.every((b) => b.destroyed === 1));
  assert(failed.textures.every((t) => t.destroyed === 1));
});
