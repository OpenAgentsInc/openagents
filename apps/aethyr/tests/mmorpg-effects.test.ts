import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  MMO_EFFECT_LIMITS,
  MmorpgEffects,
  MmorpgParticleSampler,
  mmorpgParticleProfile,
  validateMmorpgParticleTextures,
  type MmorpgParticleAttachments,
  type MmorpgParticleTextures,
} from "../packages/renderer/mmorpg-effects.ts";
import type { MmorpgActorPose } from "../packages/assets/src/mmorpg.ts";
import type { MmoEvent } from "../packages/runtime/mmorpg-protocol.ts";
import type { CreatureFrame } from "../packages/renderer/creatures.ts";
const actor = (
  id: number,
  kind: MmorpgActorPose["class"] = "paladin",
): MmorpgActorPose => ({
  id,
  class: kind,
  name: kind,
  position: [0, 0, 0],
  yaw: 0,
  state: "idle",
  actionTime: 0,
  animationTime: 0,
  health: 200,
  maxHealth: 200,
  alive: true,
  weapon: null,
});
const anchors: MmorpgParticleAttachments = new Map(
  [1, 2].map((id) => [
    id,
    {
      Head: [0, 1.5, 0],
      Hips: [0, 0.8, 0],
      RightHandIndex1: [-0.3, 1, 0.2],
      LeftHandIndex1: [0.3, 1, 0.2],
    },
  ]),
);
const damage = (id = 1, tick = 0): MmoEvent => ({
  id,
  tick,
  kind: "damage",
  actor: 1,
  target: 2,
  amount: 10,
});
const eye = [0, 2, -5];
function step(sampler: MmorpgParticleSampler, end: number) {
  for (let i = 0; i <= end * 60; i++) sampler.prepare(i / 60, eye, anchors);
}

test("blood follows source decaying emission, lifetime and Head/Hips anchors only once per event", () => {
  const sampler = new MmorpgParticleSampler();
  const actors = [actor(1), actor(2, "zombie")];
  sampler.setMmorpgActors(actors, [damage()], 0);
  step(sampler, 1.1);
  assert.equal(sampler.stats.emitted, 24);
  assert.equal(sampler.stats.emitters, 0);
  assert.equal(sampler.stats.particles, 0);
  sampler.setMmorpgActors(actors, [damage()], 60);
  sampler.prepare(1.2, eye, anchors);
  assert.equal(sampler.stats.emitted, 24);
});
test("Sorceror hands preserve four authored fire/smoke emitters without replay on static snapshots", () => {
  const sampler = new MmorpgParticleSampler();
  const sorceror = { ...actor(1, "sorceror"), state: "attack" as const };
  const attack: MmoEvent = { id: 1, tick: 0, kind: "attack", actor: 1 };
  sampler.setMmorpgActors([sorceror], [attack], 0);
  sampler.prepare(0, eye, anchors);
  assert.equal(sampler.stats.emitters, 4);
  for (let i = 1; i <= 240; i++) {
    sampler.setMmorpgActors([sorceror], [attack], i);
    sampler.prepare(i / 60, eye, anchors);
  }
  assert.equal(sampler.stats.emitted, 1248);
  assert.equal(sampler.stats.emitters, 0);
  assert.equal(sampler.stats.particles, 0);
});
test("Sorceror hits emit the exact 400 fire and 200 smoke sprites at animated target bones", () => {
  const sampler = new MmorpgParticleSampler();
  sampler.setMmorpgActors(
    [actor(1, "sorceror"), actor(2, "zombie")],
    [damage()],
    0,
  );
  const first = sampler.prepare(0, eye, anchors);
  assert.equal(first.count, 600);
  assert.equal(first.ranges[1].count, 600);
  let fire = 0,
    smoke = 0;
  for (let p = 0; p < first.data.length; p += 12) {
    if (first.data[p + 10] === 3) fire++;
    else if (first.data[p + 10] === 4) smoke++;
    const nearHead = Math.abs(first.data[p + 1] - 1.5) <= 1 / 6 + 1e-6;
    const nearHips = Math.abs(first.data[p + 1] - 0.8) <= 1 / 6 + 1e-6;
    assert.ok(nearHead || nearHips);
    assert.ok(Math.abs(first.data[p]) <= 1 / 6 + 1e-6);
  }
  assert.equal(fire, 400);
  assert.equal(smoke, 200);
  assert.equal(sampler.prepare(1.6, eye, anchors).count, 0);
});
test("particle curves preserve source alpha, size, smoke absorption and additive fire", () => {
  assert.equal(mmorpgParticleProfile(0, 0).alpha, 0);
  assert.equal(mmorpgParticleProfile(0, 0.7).alpha, 1);
  assert.equal(mmorpgParticleProfile(0, 0.5).size, 1);
  assert.equal(mmorpgParticleProfile(1, 0.5).size, 2.5);
  assert.equal(mmorpgParticleProfile(3, 0.5).size, 3);
  assert.equal(mmorpgParticleProfile(4, 1).size, 10);
  assert.equal(mmorpgParticleProfile(3, 0.5).blend, 0);
  assert.equal(mmorpgParticleProfile(4, 0.5).blend, 1);
  assert.deepEqual(mmorpgParticleProfile(4, 0.5).color, [0, 0, 0]);
  assert.equal(mmorpgParticleProfile(1, 1).alpha, 0);
  assert.throws(() => mmorpgParticleProfile(6, 0.5));
});
test("the fixed slab absorbs combat bursts, exposes drops and reuses its storage", () => {
  const sampler = new MmorpgParticleSampler();
  sampler.setMmorpgActors(
    [actor(1, "sorceror"), actor(2, "zombie")],
    Array.from({ length: 9 }, (_, i) => damage(i + 1)),
    0,
  );
  const sample = sampler.prepare(0, eye, anchors);
  assert.equal(sample.count, MMO_EFFECT_LIMITS.particles);
  assert.equal(sample.data.buffer, sampler.data.buffer);
  assert.equal(sampler.stats.droppedParticles, 9 * 600 - 4096);
  assert.ok(sampler.cpuBytes < 1024 * 1024);
  assert.equal(sampler.prepare(2, eye, anchors).count, 0);
  assert.equal(
    sampler.prepare(2.1, eye, anchors).data.buffer,
    sample.data.buffer,
  );
});
test("stale history, missing loaded bones and backward clocks do not replay old damage", () => {
  const sampler = new MmorpgParticleSampler();
  const actors = [actor(1), actor(2, "zombie")];
  sampler.setMmorpgActors(actors, [damage()], 181);
  sampler.prepare(0, eye, anchors);
  assert.equal(sampler.stats.emitters, 0);
  sampler.setMmorpgActors(actors, [damage(2, 181)], 181);
  sampler.prepare(0.1, eye, new Map());
  assert.equal(sampler.stats.emitters, 0);
  sampler.setMmorpgActors(actors, [damage(3, 182)], 182);
  sampler.prepare(0.2, eye, anchors);
  sampler.prepare(0.3, eye, anchors);
  assert.ok(sampler.count > 0);
  sampler.prepare(0, eye, anchors);
  assert.equal(sampler.count, 0);
  sampler.setMmorpgActors(actors, [damage(3, 182)], 182);
  sampler.prepare(0.1, eye, anchors);
  assert.equal(sampler.count, 0);
});
test("the dormant level effect requires its explicit deduplicated award hook", () => {
  const sampler = new MmorpgParticleSampler();
  sampler.setMmorpgActors([actor(1)], [], 0);
  assert.equal(sampler.prepare(0, eye, anchors).count, 0);
  sampler.emitLevelUp(1, 100);
  assert.equal(sampler.prepare(0.1, eye, anchors).count, 300);
  sampler.emitLevelUp(1, 100);
  sampler.prepare(0.2, eye, anchors);
  assert.equal(sampler.stats.emitted, 300);
  assert.equal(sampler.prepare(6, eye, anchors).count, 0);
});

function imageFixture(): {
  descriptions: MmorpgParticleTextures;
  bytes: ArrayBuffer;
} {
  const bytes = new ArrayBuffer(33),
    data = new Uint8Array(bytes),
    view = new DataView(bytes);
  data.set([137, 80, 78, 71, 13, 10, 26, 10]);
  view.setUint32(8, 13);
  view.setUint32(12, 0x49484452);
  view.setUint32(16, 2);
  view.setUint32(20, 2);
  const sha256 = createHash("sha256").update(data).digest("hex");
  const image = {
    url: "/private-assets/source-particle.png",
    sha256,
    byteLength: 33,
    width: 2,
    height: 2,
  };
  return { bytes, descriptions: { blood: image, fire: image, levelUp: image } };
}
function fakeDevice() {
  const resources: { destroyed: boolean; destroy(): void }[] = [],
    writes: number[] = [],
    draws: number[][] = [],
    pipelines: GPURenderPipelineDescriptor[] = [];
  const resource = () => {
    const r = {
      destroyed: false,
      destroy() {
        this.destroyed = true;
      },
      createView() {
        return {};
      },
    };
    resources.push(r);
    return r;
  };
  const pass = {
    setPipeline() {},
    setBindGroup() {},
    setVertexBuffer() {},
    draw(...args: number[]) {
      draws.push(args);
    },
    end() {},
  };
  const device = {
    createBuffer: resource,
    createTexture: resource,
    createShaderModule() {
      return {};
    },
    createBindGroupLayout() {
      return {};
    },
    createPipelineLayout() {
      return {};
    },
    createSampler() {
      return {};
    },
    createBindGroup() {
      return {};
    },
    async createRenderPipelineAsync(d: GPURenderPipelineDescriptor) {
      pipelines.push(d);
      return {
        getBindGroupLayout() {
          return {};
        },
      };
    },
    createCommandEncoder() {
      return {
        beginRenderPass() {
          return pass;
        },
        finish() {
          return {};
        },
      };
    },
    queue: {
      writeBuffer(_buffer: unknown, _offset: number, data: Float32Array) {
        writes.push(data.byteLength);
      },
      copyExternalImageToTexture() {},
      submit() {},
    },
  };
  return {
    device: device as unknown as GPUDevice,
    resources,
    writes,
    draws,
    pipelines,
    pass: pass as unknown as GPURenderPassEncoder,
  };
}
const frame = (): CreatureFrame => ({
  mode: "day",
  time: 0,
  viewProjection: new Float32Array([
    1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]),
  eye,
  sunDirection: [0, 1, 0],
  sunIlluminance: [1, 1, 1],
  diffuseRotation: new Float32Array(16),
  diffuseSH: new Float32Array(36),
});
test("GPU submission is bounded and disposal/failed loading release every owned resource", async () => {
  const previous = {
    buffer: globalThis.GPUBufferUsage,
    shader: globalThis.GPUShaderStage,
    texture: globalThis.GPUTextureUsage,
    image: globalThis.createImageBitmap,
  };
  Object.assign(globalThis, {
    GPUBufferUsage: { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 },
    GPUShaderStage: { VERTEX: 1, FRAGMENT: 2 },
    GPUTextureUsage: { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 },
    createImageBitmap: async () => ({ width: 2, height: 2, close() {} }),
  });
  try {
    const { descriptions, bytes } = imageFixture(),
      gpu = fakeDevice();
    const effects = await MmorpgEffects.create(
      gpu.device,
      descriptions,
      1 / 6,
      async () => bytes,
    );
    gpu.draws.length = 0;
    effects.setMmorpgActors(
      [actor(1, "sorceror"), actor(2, "zombie")],
      [damage()],
      0,
    );
    effects.prepare(frame(), anchors);
    effects.render(gpu.pass);
    assert.equal(effects.stats.particles, 600);
    assert.equal(effects.stats.drawCalls, 1);
    assert.equal(effects.stats.triangles, 1200);
    assert.deepEqual(gpu.draws, [[6, 600, 0, 0]]);
    assert.ok(gpu.writes.every((n) => n <= 4096 * 48));
    assert.equal(
      gpu.pipelines[0].fragment!.targets![0]!.blend!.color!.dstFactor,
      "one-minus-src-alpha",
    );
    effects.dispose();
    effects.dispose();
    assert.equal(gpu.resources.length, 5);
    assert.ok(gpu.resources.every((r) => r.destroyed));
    const failed = fakeDevice();
    let reads = 0;
    await assert.rejects(
      MmorpgEffects.create(failed.device, descriptions, 1 / 6, async () => {
        if (++reads === 2) throw Error("download failed");
        return bytes;
      }),
      /download failed/,
    );
    assert.ok(failed.resources.every((r) => r.destroyed));
  } finally {
    Object.assign(globalThis, {
      GPUBufferUsage: previous.buffer,
      GPUShaderStage: previous.shader,
      GPUTextureUsage: previous.texture,
      createImageBitmap: previous.image,
    });
  }
});
test("sprite manifest admission checks source extents and total mip storage", () => {
  const { descriptions } = imageFixture();
  assert.equal(validateMmorpgParticleTextures(descriptions), 60);
  assert.throws(() =>
    validateMmorpgParticleTextures({
      ...descriptions,
      blood: { ...descriptions.blood, url: "https://example.test/blood.png" },
    }),
  );
  assert.throws(() =>
    validateMmorpgParticleTextures({
      ...descriptions,
      blood: { ...descriptions.blood, width: 2048 },
    }),
  );
  assert.throws(() =>
    validateMmorpgParticleTextures(
      Object.fromEntries(
        Object.keys(descriptions).map((key) => [
          key,
          { ...descriptions.blood, width: 1024, height: 1024 },
        ]),
      ) as MmorpgParticleTextures,
    ),
  );
});
