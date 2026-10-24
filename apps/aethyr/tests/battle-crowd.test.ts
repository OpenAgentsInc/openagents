import assert from "node:assert/strict";
import test from "node:test";
import {
  BattleCrowd,
  BATTLE_CROWD_LIMITS,
  battleFrustumPlanes,
  battleRadius,
  battleTerrainHeight,
  createBattleCrowdSeed,
  decodeBattleCounters,
  validateBattleCrowdOptions,
  validateBattleTerrain,
  validateBattleParticipant,
  battleCrowdDeviceRequirements,
  validateBattleCrowdDevice,
  type BattleCrowdTerrain,
} from "../packages/renderer/battle-crowd.ts";
import {
  BATTLE_CLASSES,
  type BattleCrowdClassAssets,
} from "../packages/renderer/battle-assets.ts";
import {
  BATTLE_COMPACTION_WGSL,
  BATTLE_RENDER_WGSL,
  BATTLE_SIMULATION_WGSL,
} from "../packages/renderer/battle-crowd-shader.ts";
import {
  BATTLE_COMBAT_BYTES,
  BATTLE_PROJECTILE_CAPACITY,
  BATTLE_PROJECTILE_OFFSET,
  BATTLE_SQUAD_OFFSET,
} from "../packages/renderer/battle-combat-shader.ts";
import {
  BATTLE_SQUAD_CAPACITY,
  BATTLE_SQUAD_STRIDE,
  BATTLE_SQUAD_INTERVAL,
  battleSquadCount,
  battleSquadId,
} from "../packages/renderer/battle-squad-shader.ts";
import {
  createBattleSiegeGeometry,
  BATTLE_ORDNANCE_COMPACTION_WGSL,
  BATTLE_ORDNANCE_RENDER_WGSL,
} from "../packages/renderer/battle-ordnance.ts";
import {
  BATTLE_PARTICIPANT_CONTACT_BYTES,
  BATTLE_PARTICIPANT_CONTACTS_WGSL,
  decodeBattleParticipantContacts,
} from "../packages/renderer/battle-participant-contacts.ts";
import { MMORPG_FOG_WGSL } from "../packages/renderer/mmorpg-fog.ts";
import {
  BATTLE_GRID,
  BATTLE_BODY_RADII,
} from "../packages/renderer/battle-grid.ts";
import { lookAt, multiply, perspective } from "../packages/renderer/math.ts";

Object.assign(globalThis, {
  GPUBufferUsage: {
    STORAGE: 1,
    COPY_DST: 2,
    COPY_SRC: 4,
    UNIFORM: 8,
    VERTEX: 16,
    INDEX: 32,
    INDIRECT: 64,
    MAP_READ: 128,
  },
  GPUTextureUsage: { TEXTURE_BINDING: 1, COPY_DST: 2 },
  GPUShaderStage: { VERTEX: 1, FRAGMENT: 2, COMPUTE: 4 },
  GPUMapMode: { READ: 1 },
});
const terrain = (): BattleCrowdTerrain => ({
  resolution: 2,
  size: 512,
  center: [0, 0],
  heights: new Float32Array([0, 2, 4, 6]),
});
const frame = () => ({
  viewProjection: multiply(
    perspective(Math.PI / 3, 1.5, 0.1, 2000),
    lookAt([0, 50, 80], [0, 0, 0]),
  ),
  eye: [0, 50, 80],
  time: 0,
});
function assets(): BattleCrowdClassAssets[] {
  return BATTLE_CLASSES.map((kind) => ({
    kind,
    height: 1.8,
    vertices: new ArrayBuffer(48),
    indices: new Uint32Array([0, 1, 2, 0, 1, 2]),
    lods: [
      { firstIndex: 0, indexCount: 3 },
      { firstIndex: 3, indexCount: 3 },
    ],
    vat: {
      width: 4,
      height: 8,
      positions: new Uint16Array(4 * 8 * 4),
      normals: new Uint16Array(4 * 8 * 4),
    },
    materials: {
      alphaMode: "OPAQUE",
      width: 256,
      height: 256,
      layers: 1,
      rgba: new Uint8Array(256 * 256 * 4).fill(255),
    },
    clips: Object.fromEntries(
      ["idle", "walk", "attack", "death"].map((name, index) => [
        name,
        { duration: 2, firstFrame: index * 2, frameCount: 2, loop: index < 2 },
      ]),
    ) as BattleCrowdClassAssets["clips"],
  }));
}
function fakeGpu(failTexture = false) {
  const buffers: any[] = [],
    textures: any[] = [],
    pipelines: any[] = [],
    writes: string[] = [],
    computes: any[] = [],
    draws: number[] = [],
    renderGroups: any[] = [];
  let copies = 0;
  const encode = () => ({
    beginComputePass({ label }: any) {
      const commands: any[] = [];
      computes.push({ label, commands });
      let binding: any;
      return {
        setPipeline(pipeline: any) {
          commands.push({ pipeline: pipeline.entryPoint, binding });
        },
        setBindGroup(_i: number, group: any) {
          binding = group;
        },
        dispatchWorkgroups(count: number) {
          commands.push({ dispatch: count });
        },
        end() {},
      };
    },
    copyBufferToBuffer(
      source: any,
      sourceOffset: number,
      target: any,
      targetOffset: number,
      size: number,
    ) {
      copies++;
      new Uint8Array(target.storage, targetOffset, size).set(
        new Uint8Array(source.storage, sourceOffset, size),
      );
    },
    finish: () => ({}),
  });
  const device = {
    limits: {
      maxBufferSize: 256 * 1024 * 1024,
      maxStorageBufferBindingSize: 128 * 1024 * 1024,
      maxStorageBuffersPerShaderStage: 8,
      maxBindingsPerBindGroup: 1000,
      maxBindGroups: 4,
      maxUniformBufferBindingSize: 65536,
      maxComputeWorkgroupsPerDimension: 65535,
      maxComputeInvocationsPerWorkgroup: 256,
      maxComputeWorkgroupSizeX: 256,
      maxComputeWorkgroupStorageSize: 16384,
    },
    createBuffer(descriptor: any) {
      const buffer = {
        ...descriptor,
        destroyed: 0,
        storage: new ArrayBuffer(descriptor.size),
        mapState: "unmapped",
        async mapAsync() {
          this.mapState = "mapped";
        },
        getMappedRange() {
          return this.storage;
        },
        unmap() {
          this.mapState = "unmapped";
        },
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    createTexture(descriptor: any) {
      const texture = {
        label: descriptor.label,
        width: descriptor.size[0],
        height: descriptor.size[1],
        depthOrArrayLayers: descriptor.size[2],
        format: descriptor.format,
        destroyed: 0,
        createView: (value: any) => ({ texture, value }),
        destroy() {
          this.destroyed++;
        },
      };
      textures.push(texture);
      return texture;
    },
    createBindGroupLayout: (value: any) => value,
    createPipelineLayout: (value: any) => value,
    createShaderModule: (value: any) => value,
    createComputePipelineAsync: async ({ compute }: any) => ({
      entryPoint: compute.entryPoint,
    }),
    createRenderPipelineAsync: async (value: any) => {
      pipelines.push(value);
      return value;
    },
    createBindGroup: (value: any) => value,
    createSampler: () => ({}),
    createCommandEncoder: encode,
    queue: {
      writeBuffer(
        buffer: any,
        offset: number,
        data: any,
        dataOffset = 0,
        size?: number,
      ) {
        const source = ArrayBuffer.isView(data)
          ? new Uint8Array(
              data.buffer,
              data.byteOffset + dataOffset,
              size ?? data.byteLength - dataOffset,
            )
          : new Uint8Array(
              data,
              dataOffset,
              size ?? data.byteLength - dataOffset,
            );
        new Uint8Array(buffer.storage, offset, source.length).set(source);
        writes.push(buffer.label);
      },
      writeTexture() {
        if (failTexture) throw Error("texture upload failed");
      },
      submit() {},
    },
  } as unknown as GPUDevice;
  const pass = {
    setPipeline() {},
    setBindGroup(index: number, group: any) {
      renderGroups[index] = group;
    },
    setVertexBuffer() {},
    setIndexBuffer() {},
    drawIndexedIndirect(_buffer: any, offset: number) {
      draws.push(offset);
    },
  } as unknown as GPURenderPassEncoder;
  return {
    device,
    encoder: encode() as unknown as GPUCommandEncoder,
    pass,
    buffers,
    textures,
    pipelines,
    writes,
    computes,
    draws,
    renderGroups,
    get copies() {
      return copies;
    },
  };
}
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));

test("battle configuration bounds population and preserves the supplied center", () => {
  const center: [number, number] = [5, -12],
    config = validateBattleCrowdOptions({ center });
  center[0] = 99;
  assert.deepEqual(config.center, [5, 0, -12]);
  assert.equal(config.count, 10_000);
  assert.equal(config.capacity, 1_000_000);
  assert.equal(validateBattleCrowdOptions({ count: 300_000 }).count, 300_000);
  assert.equal(
    validateBattleCrowdOptions({ count: 10_000, capacity: 10_000 }).capacity,
    10_000,
  );
  // Every storage binding stays below WebGPU's minimum guaranteed 128 MiB limit.
  assert(
    BATTLE_CROWD_LIMITS.count * BATTLE_CROWD_LIMITS.actorBytes <
      128 * 1024 * 1024,
  );
  assert(BATTLE_CROWD_LIMITS.count * 8 * 4 < 128 * 1024 * 1024);
  assert.equal(battleRadius(10_000), 100);
  assert.equal(battleRadius(100_000), 317);
  for (const patch of [
    { count: -1 },
    { count: 1000001 },
    { count: 3.5 },
    { capacity: 0 },
    { capacity: 1000001 },
    { seed: -1 },
    { seed: NaN },
    { center: [Infinity, 0] },
    { radius: 1001 },
    { count: 10000, radius: 55 },
    { radius: 0 },
    { respawnSeconds: 0 },
    { sampleCount: 2 },
    { nearDistance: Infinity },
  ])
    assert.throws(() => validateBattleCrowdOptions(patch as any));
  assert.equal(validateBattleCrowdOptions({ count: 0 }).count, 0);
});
test("terrain sampling and fixed-seed formations retain real human health and all four classes", () => {
  const ground = terrain();
  validateBattleTerrain(ground);
  assert.equal(battleTerrainHeight(ground, 0, 0), 3);
  assert.equal(battleTerrainHeight(ground, -256, -256), 0);
  assert.equal(battleTerrainHeight(ground, 256, 256), 6);
  assert.equal(
    battleTerrainHeight(
      { ...ground, heights: new Float32Array([0, 2, 4, 0]) },
      0,
      0,
    ),
    3,
    "terrain follows the visible triangle diagonal instead of bilinear floating",
  );
  const first = createBattleCrowdSeed(10_000, ground, { seed: 73 }),
    second = createBattleCrowdSeed(10_000, ground, { seed: 73 }),
    changed = createBattleCrowdSeed(10_000, ground, { seed: 74 });
  assert.deepEqual(first, second);
  assert.notDeepEqual(first, changed);
  const words = new Uint32Array(first.buffer),
    classes = new Set<number>();
  let players = 0,
    monsters = 0;
  for (let id = 0; id < 10000; id++) {
    const at = id * 16,
      kind = words[at + 12],
      team = words[at + 13];
    classes.add(kind);
    assert(
      Math.abs(
        first[at + 1] - battleTerrainHeight(ground, first[at], first[at + 2]),
      ) < 1e-6,
    );
    assert.equal(first[at + 6], [200, 100, 20, 1000][kind]);
    assert.equal(words[at + 14], 0xffffffff);
    if (team === 0) {
      players++;
      assert(kind < 2);
    } else {
      monsters++;
      assert(kind >= 2);
    }
  }
  assert.equal(players, 5000);
  assert.equal(monsters, 5000);
  assert.equal(classes.size, 4);
  assert.equal(first.byteLength, 10000 * BATTLE_CROWD_LIMITS.actorBytes);
  assert.throws(() =>
    validateBattleTerrain({
      ...ground,
      heights: new Float32Array([0, 1, NaN, 0]),
    }),
  );
});
test("frustum planes retain world distances for conservative animated limb bounds", () => {
  const planes = battleFrustumPlanes(
    multiply(
      perspective(Math.PI / 3, 1, 0.1, 100),
      lookAt([0, 0, 0], [0, 0, -1]),
    ),
  );
  for (let i = 0; i < 6; i++)
    assert(Math.abs(Math.hypot(...planes.slice(i * 4, i * 4 + 3)) - 1) < 1e-6);
  const inside = (p: number[], radius = 0) =>
    Array.from(
      { length: 6 },
      (_, i) =>
        planes[i * 4] * p[0] +
        planes[i * 4 + 1] * p[1] +
        planes[i * 4 + 2] * p[2] +
        planes[i * 4 + 3],
    ).every((distance) => distance >= -radius);
  assert(inside([0, 0, -10]));
  assert(!inside([20, 0, -10]));
  assert(!inside([0, 0, 1]));
  assert(inside([0, 0, 1], 4.5));
  assert.throws(() => battleFrustumPlanes(new Float32Array(16)));
});
test("the battle encodes bounded simulation, compacts its result, and submits eight indirect source LOD draws", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 64,
      capacity: 128,
    });
  const initialWrites = gpu.writes.length;
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  battle.render(gpu.pass);
  assert.deepEqual(
    gpu.computes.flatMap((stage) =>
      stage.commands
        .filter((value: any) => value.pipeline)
        .map((value: any) => value.pipeline),
    ),
    [
      "clear",
      "scatter",
      "commandSquads",
      "simulate",
      "participants",
      "projectiles",
      "clearDraws",
      "compact",
      "clearOrdnance",
      "compactOrdnance",
    ],
  );
  assert.deepEqual(gpu.draws, [0, 20, 40, 60, 80, 100, 120, 140, 0, 20, 60]);
  assert.equal(
    gpu.renderGroups[0].entries[1].resource.buffer.label,
    "Battle crowd: bounded projectiles and participant",
  );
  assert.deepEqual(gpu.writes.slice(initialWrites), [
    "Battle crowd: frame and simulation parameters",
  ]);
  assert.equal(battle.inspect().simulationSteps, 1);
  assert.equal(battle.inspect().frameDraws, 11);
  const expectedBytes =
    gpu.buffers.reduce((sum, buffer) => sum + buffer.size, 0) +
    gpu.textures.reduce(
      (sum, texture) =>
        sum +
        texture.width *
          texture.height *
          texture.depthOrArrayLayers *
          (texture.format === "rgba16float" ? 8 : 4),
      0,
    );
  assert.equal(battle.bytes, expectedBytes);
  assert.equal(battle.inspect().gpu, null);
  assert.throws(() => battle.prepare(gpu.encoder, frame(), 1 / 60), /Submit/);
  assert.throws(() => battle.setCount(12), /Submit/);
  battle.afterSubmit();
  await flush();
  assert.equal(battle.inspect().sampleSerial, 1);
  battle.dispose();
  battle.dispose();
  assert(gpu.buffers.every((buffer) => buffer.destroyed === 1));
  assert(gpu.textures.every((texture) => texture.destroyed === 1));
  assert.equal(battle.inspect().accountedGpuBytes, 0);
});
test("background elapsed time has a fixed three-step ceiling and reports every dropped second", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 16,
    });
  battle.prepare(gpu.encoder, frame(), 10);
  assert.equal(battle.inspect().simulationSteps, 3);
  assert(Math.abs(battle.inspect().droppedSeconds - 9.95) < 1e-8);
  battle.afterSubmit();
  await flush();
  battle.setCount(0);
  battle.prepare(gpu.encoder, frame(), 0);
  battle.render(gpu.pass);
  battle.afterSubmit();
  assert.equal(battle.inspect().enabled, false);
  assert.equal(battle.inspect().frameDraws, 0);
  assert.equal(battle.inspect().simulationSteps, 0);
  assert.throws(() => battle.setCount(17));
  battle.dispose();
});
test("counters and random sampled actor probes decode GPU buffers without inventing combat events", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 64,
      capacity: 64,
    });
  const counters = new Uint32Array(
    gpu.buffers.find(
      (buffer) => buffer.label === "Battle crowd: actual GPU counters",
    ).storage,
  );
  counters.set([
    30, 20, 10, 4, 83, 17, 14, 2, 3, 12, 28, 77, 0, 11, 9, 2350, 24, 10, 2, 20,
    8, 1700,
  ]);
  counters[24] = 500;
  counters[25] = 18;
  counters[26] = 900;
  counters[27] = 23;
  counters[54] = 81;
  const parsed = decodeBattleCounters(counters);
  assert.equal(parsed.alive, 50);
  assert.equal(parsed.visible, 40);
  assert.equal(parsed.hits, 17);
  assert.equal(parsed.deaths, 14);
  assert.equal(parsed.respawned, 2);
  assert.equal(parsed.gridOverflows, 3);
  assert.equal(parsed.gridOverflowsTotal, 81);
  const probe = await battle.probe();
  assert.equal(probe.count, 64);
  assert.equal(probe.samples.length, 32);
  assert(probe.finite);
  assert.equal(probe.tick, 77);
  assert.equal(probe.gpu.hits, 17);
  assert.equal(probe.sampleSha256.length, 64);
  const again = await battle.probe();
  assert.deepEqual(
    again.samples.map((actor) => actor.id),
    probe.samples.map((actor) => actor.id),
  );
  assert.equal(again.sampleSha256, probe.sampleSha256);
  assert(
    probe.samples.every(
      (actor) =>
        [200, 100, 20, 1000].includes(actor.health) &&
        actor.maxHealth >= actor.health,
    ),
  );
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  battle.afterSubmit();
  battle.setCount(12);
  await flush();
  assert.equal(
    battle.inspect().gpu,
    null,
    "a queued readback from the previous battle cannot relabel the new population",
  );
  battle.dispose();
});
test("a failed animation upload disposes all candidate resources before publishing a battle", async () => {
  const gpu = fakeGpu(true);
  await assert.rejects(
    BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 16,
    }),
    /upload failed/,
  );
  assert(gpu.buffers.length > 0);
  assert(gpu.buffers.every((buffer) => buffer.destroyed === 1));
  assert(gpu.textures.every((texture) => texture.destroyed === 1));
});
test("combat shader uses bounded bins, deferred damage and one animation-timed hit per strike", () => {
  assert(BATTLE_SIMULATION_WGSL.includes("min(population,64u)"));
  assert(BATTLE_SIMULATION_WGSL.includes("damageIn[id]"));
  assert(
    BATTLE_SIMULATION_WGSL.includes("damageActor(enemyId,damage(kind),false)"),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "a.clocks.w<.5&&before<attackTime(kind)&&a.motion.w>=attackTime(kind)",
    ),
  );
  assert(!BATTLE_SIMULATION_WGSL.includes("for(var n=0u;n<frame.counts.x"));
});
test("battle fog shares the source law and only scales the spectator fog ray", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 16,
    });
  const uniform = gpu.buffers.find(
    (buffer) =>
      buffer.label === "Battle crowd: frame and simulation parameters",
  );
  battle.prepare(gpu.encoder, frame(), 0);
  assert.equal(new Float32Array(uniform.storage)[47], 1);
  battle.afterSubmit();
  await flush();
  battle.prepare(gpu.encoder, { ...frame(), sourceFogScale: 10 }, 0);
  assert.equal(new Float32Array(uniform.storage)[47], 10);
  battle.afterSubmit();
  for (const sourceFogScale of [0, 101, NaN])
    assert.throws(() =>
      battle.prepare(gpu.encoder, { ...frame(), sourceFogScale }, 0),
    );
  assert(BATTLE_RENDER_WGSL.includes(MMORPG_FOG_WGSL));
  assert(
    BATTLE_RENDER_WGSL.includes(
      "mmoFog(frame.eyeTime.y/scale,(input.world-frame.eyeTime.xyz)/scale)",
    ),
  );
  assert(!BATTLE_RENDER_WGSL.includes("*.0012,2.0"));
  battle.dispose();
});

test("all source battle materials are opaque without cutout or coverage while retaining both sides", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 16,
    });
  const pipeline = gpu.pipelines[0];
  assert.equal(pipeline.multisample.alphaToCoverageEnabled, false);
  assert.equal(pipeline.primitive.cullMode, "none");
  assert.equal(pipeline.depthStencil.depthWriteEnabled, true);
  assert.equal(pipeline.fragment.targets[0].blend, undefined);
  const opaqueFragment = BATTLE_RENDER_WGSL.split(
    "@fragment fn fragmentMain",
  )[1].split("struct SpriteOutput")[0];
  assert(!opaqueFragment.includes("discard"));
  assert(!opaqueFragment.includes("texel.a"));
  assert(BATTLE_RENDER_WGSL.includes(",1.0);\n}"));
  battle.dispose();
  const invalid = assets();
  (invalid[2].materials as { alphaMode: string }).alphaMode = "MASK";
  const rejected = fakeGpu();
  await assert.rejects(
    BattleCrowd.create(rejected.device, invalid, terrain(), {
      count: 8,
      capacity: 16,
    }),
    /complete battle/,
  );
  assert.equal(rejected.buffers.length, 0);
});

test("optional distant silhouettes use bounded separate coverage draws and keep mesh materials opaque", async () => {
  const models = assets(),
    model = models[0];
  model.impostor = {
    views: 8,
    tileSize: 32,
    width: 512,
    height: 512,
    frames: 100,
    bounds: { minY: -0.1, maxY: 2.1, radius: 1.4 },
    clips: Object.fromEntries(
      ["idle", "walk", "attack", "death"].map((name, index) => [
        name,
        {
          duration: 2,
          firstFrame: index * 25,
          frameCount: 25,
          loop: index < 2,
        },
      ]),
    ) as typeof model.clips,
    color: new Uint8Array(512 * 512 * 8 * 4),
    normal: new Uint8Array(512 * 512 * 8 * 4),
  };
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, models, terrain(), {
      count: 129,
      capacity: 129,
    });
  const opaque = gpu.pipelines.find(
    (p) => p.vertex.entryPoint === "vertexMain",
  );
  const sprite = gpu.pipelines.find(
    (p) => p.vertex.entryPoint === "vertexImpostor",
  );
  assert.equal(opaque.multisample.alphaToCoverageEnabled, false);
  assert.equal(sprite.multisample.alphaToCoverageEnabled, true);
  assert.equal(sprite.fragment.constants.impostorSampleCount, 4);
  assert.equal(sprite.depthStencil.depthWriteEnabled, true);
  assert.equal(sprite.fragment.targets[0].blend, undefined);
  const metadata = gpu.buffers.find(
    (b) => b.label === "Battle crowd: projected character sizes",
  );
  assert.deepEqual(
    [...new Float32Array(metadata.storage)],
    [1.8, 1, 0, 0, 1.8, 0, 0, 0, 1.8, 0, 0, 0, 1.8, 0, 0, 0].map(Math.fround),
  );
  const visible = gpu.buffers.find(
    (b) => b.label === "Battle crowd: compacted visible actor IDs",
  );
  assert.equal(visible.size, 129 * 12 * 4);
  const args = gpu.buffers.find(
    (b) => b.label === "Battle crowd: indexed indirect arguments",
  );
  assert.deepEqual(
    [...new Uint32Array(args.storage).slice(40, 45)],
    [6, 0, 0, 0, 0],
  );
  battle.prepare(gpu.encoder, { ...frame(), viewportHeight: 1080 }, 1 / 60);
  const uniform = gpu.buffers.find(
    (b) => b.label === "Battle crowd: frame and simulation parameters",
  );
  const camera = frame().viewProjection;
  assert(
    Math.abs(
      new Float32Array(uniform.storage)[35] -
        (1080 * Math.hypot(camera[1], camera[5], camera[9])) / 2,
    ) < 1e-3,
  );
  // Detail's silhouette threshold travels in the frame's free sun.w slot.
  assert.equal(new Float32Array(uniform.storage)[39], 8);
  battle.render(gpu.pass);
  assert.deepEqual(
    gpu.draws.slice(0, 9),
    [0, 20, 40, 60, 80, 100, 120, 140, 160],
  );
  assert.equal(battle.inspect().impostorClasses, 1);
  assert.equal(battle.inspect().frameDraws, 12);
  battle.afterSubmit();
  await flush();
  const probe = await battle.probe();
  assert.equal(probe.draws.length, 12);
  assert.equal(probe.draws[8].kind, "paladin");
  assert.equal(probe.draws[8].lod, "impostor");
  assert.equal(probe.draws[8].indexCount, 6);
  // Lower Detail raises the threshold up to the 16-pixel readability floor.
  for (const bad of [7.9, 16.1, Number.NaN])
    assert.throws(() => battle.setSilhouettePixels(bad));
  battle.setSilhouettePixels(16);
  battle.prepare(gpu.encoder, { ...frame(), viewportHeight: 1080 }, 1 / 60);
  assert.equal(new Float32Array(uniform.storage)[39], 16);
  battle.afterSubmit();
  battle.dispose();
  assert(gpu.buffers.every((b) => b.destroyed));
  assert(gpu.textures.every((t) => t.destroyed));
});

test("single-sample distant silhouettes use opaque cutoff instead of unavailable sample coverage", async () => {
  const gpu=fakeGpu();
  const battle=await BattleCrowd.create(gpu.device,assets(),terrain(),{count:100,capacity:100,sampleCount:1});
  const sprite=gpu.pipelines.find((pipeline)=>pipeline.vertex.entryPoint==="vertexImpostor");
  assert.equal(sprite.multisample.alphaToCoverageEnabled,false);
  assert.equal(sprite.fragment.constants.impostorSampleCount,1);
  assert.equal(sprite.fragment.targets[0].blend,undefined);
  assert.equal(sprite.depthStencil.depthWriteEnabled,true);
  assert(BATTLE_RENDER_WGSL.includes("if albedo.a<select(.5,.01,impostorSampleCount>1u){discard;}"));
  assert(BATTLE_RENDER_WGSL.includes("select(1.0,albedo.a,impostorSampleCount>1u)"));
  battle.dispose();
});

test("partial actor workgroups are dispatched without padding actors or skipping the tail", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 1,
      capacity: 257,
    });
  for (const count of [1, 127, 128, 129, 257]) {
    battle.setCount(count);
    const previous = gpu.computes.length;
    battle.prepare(gpu.encoder, frame(), 1 / 60);
    const stages = gpu.computes.slice(previous),
      simulation = stages[0].commands.filter(
        (command: any) => command.dispatch,
      ),
      compact = stages[1].commands.filter((command: any) => command.dispatch);
    assert.deepEqual(
      simulation.map((command: any) => command.dispatch),
      [
        2048,
        Math.ceil(count / 128),
        Math.ceil(battleSquadCount(count) / 128),
        Math.ceil(count / 128),
        1,
        128,
      ],
    );
    assert.deepEqual(
      compact.map((command: any) => command.dispatch),
      [1, Math.ceil(count / 128)],
    );
    assert.equal(battle.inspect().count, count);
    battle.afterSubmit();
    await flush();
  }
  battle.dispose();
});

test("shared squads remain bounded same-team formation tiles and retain the existing combat offsets", () => {
  assert.equal(BATTLE_SQUAD_INTERVAL, 12);
  assert.equal(BATTLE_SQUAD_STRIDE, 64);
  assert.equal(
    BATTLE_SQUAD_OFFSET,
    BATTLE_PROJECTILE_OFFSET + BATTLE_PROJECTILE_CAPACITY * 72,
  );
  assert.equal(
    BATTLE_COMBAT_BYTES - BATTLE_SQUAD_OFFSET,
    BATTLE_SQUAD_CAPACITY * 64,
  );
  assert.equal(battleSquadCount(0), 0);
  for (const count of [1, 31, 32, 64, 129, 10000, 500000, 1000000]) {
    const columns = Math.ceil(Math.sqrt(count));
    assert(battleSquadCount(count) <= BATTLE_SQUAD_CAPACITY);
    for (const id of new Set([0, Math.floor(count / 2), count - 1])) {
      const team = Number(id % columns >= columns / 2);
      const squad = battleSquadId(id, count, team);
      assert.equal(squad % 2, team);
      assert(squad < battleSquadCount(count));
      let members = 0;
      const row = Math.floor(id / columns / 4) * 4;
      const column = Math.floor((id % columns) / 8) * 8;
      for (let z = row; z < row + 4; z++)
        for (let x = column; x < column + 8; x++) {
          const candidate = z * columns + x;
          if (x >= columns || candidate >= count) continue;
          const faction = Number(x >= columns / 2);
          if (battleSquadId(candidate, count, faction) === squad) members++;
        }
      assert(members > 0 && members <= 32);
    }
  }
});

test("fully culled and inactive lanes still reach every workgroup reduction barrier", async () => {
  const compute = (source: string, name: string) =>
    source.slice(source.indexOf(`fn ${name}(`));
  const compact = compute(BATTLE_COMPACTION_WGSL, "compact");
  assert(
    !compact.includes("return;"),
    "culled and padding lanes must not leave before the batch reservation",
  );
  assert.equal(compact.match(/workgroupBarrier\(\)/g)?.length, 3);
  for (const [name, end] of [
    ["scatter", "simulate"],
    ["simulate", undefined],
  ] as const) {
    let stage = compute(BATTLE_SIMULATION_WGSL, name);
    if (end) stage = stage.slice(0, stage.indexOf(`fn ${end}(`));
    assert(
      !stage.includes("return;"),
      `${name} padding/dead lanes must reach the group flush`,
    );
    assert.equal(stage.match(/workgroupBarrier\(\)/g)?.length, 2);
  }
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 129,
      capacity: 129,
    });
  const away = {
    ...frame(),
    viewProjection: multiply(
      perspective(Math.PI / 3, 1.5, 0.1, 2000),
      lookAt([0, 50, 80], [0, 100, 160]),
    ),
  };
  const planes = battleFrustumPlanes(away.viewProjection),
    seed = createBattleCrowdSeed(129, terrain());
  for (let id = 0; id < 129; id++) {
    const p = [seed[id * 16], seed[id * 16 + 1] + 1.3, seed[id * 16 + 2]];
    assert(
      Array.from(
        { length: 6 },
        (_, plane) =>
          planes[plane * 4] * p[0] +
          planes[plane * 4 + 1] * p[1] +
          planes[plane * 4 + 2] * p[2] +
          planes[plane * 4 + 3],
      ).some((distance) => distance < -4.5),
    );
  }
  battle.prepare(gpu.encoder, away, 1 / 60);
  assert.equal(
    gpu.computes[1].commands.filter((command: any) => command.dispatch)[1]
      .dispatch,
    2,
    "all-culled frames still run the partial final group",
  );
  battle.afterSubmit();
  await flush();
  battle.dispose();
});

test("participant updates do not heal; explicit respawn preserves pool cursor and acknowledges a new GPU epoch", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 64,
      capacity: 64,
    });
  const body = gpu.buffers.find(
    (b) => b.label === "Battle crowd: bounded projectiles and participant",
  );
  const u = new Uint32Array(body.storage),
    f = new Float32Array(body.storage);
  const input = {
    active: true,
    firstPerson: true,
    position: [2, 3, 4] as [number, number, number],
    yaw: 0.5,
    team: 0 as const,
    attackSequence: 7,
    attack: "melee" as const,
    aim: [3, 4, 5] as [number, number, number],
  };
  u[8] = 1234;
  u[9] = 37;
  u[11] = 6;
  battle.setParticipant(input);
  assert.equal(u[9], 37);
  battle.setParticipant({ ...input, active: false });
  assert.equal(u[9], 37);
  const epoch = battle.resetParticipant();
  assert.equal(epoch, 1);
  assert.equal(
    u[8],
    1234,
    "respawn must not rewind allocation over living projectiles",
  );
  assert.equal(u[9], 200);
  assert.equal(u[11], 7, "held action cannot replay on respawn");
  assert.equal(u[20], 7, "the received edge baseline also survives respawn");
  assert.equal(u[19], epoch);
  assert.equal(battle.inspect().participantResetEpoch, 1);
  battle.setParticipant(input);
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  battle.render(gpu.pass);
  assert.equal(battle.inspect().frameDraws, 12);
  assert.equal(gpu.draws.at(-2), 40);
  assert.equal(gpu.draws.at(-1), 60);
  assert.throws(() => battle.resetParticipant(), /Submit/);
  battle.afterSubmit();
  await flush();
  // A CPU queue write is not a GPU processing acknowledgement.
  assert.equal(battle.inspect().gpu?.participantEpoch, 0);
  f.set([2, 3, 4, 0.5], 0);
  u[9] = 0;
  const probe = await battle.probe();
  assert.equal(probe.participant.health, 0);
  assert(probe.participant.dead);
  assert.equal(probe.participant.resetEpoch, 1);
  assert.deepEqual(probe.participant.position, [2, 3, 4]);
  assert.equal(
    probe.gpu.alive + probe.gpu.dead,
    0,
    "mock GPU does not fabricate actor execution",
  );
  battle.dispose();
});
test("participant input and battle resource bindings stay bounded", async () => {
  const good = {
    active: true,
    position: [0, 0, 0],
    yaw: 0,
    team: 0,
    attackSequence: 1,
    attack: "ranged",
    aim: [0, 1, 10],
  } as const;
  assert.deepEqual(validateBattleParticipant(good), good);
  for (const bad of [
    { team: 2 },
    { attackSequence: -1 },
    { attackSequence: 2 ** 32 },
    { position: [0, Infinity, 0] },
    { aim: [] },
    { firstPerson: 1 },
    { attack: "global-kill" },
  ])
    assert.throws(() => validateBattleParticipant({ ...good, ...bad } as any));
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 129,
      capacity: 129,
    });
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  const group = gpu.computes[0].commands.find((x: any) => x.pipeline).binding;
  assert.equal(
    group.layout.entries.filter((x: any) => x.buffer.type !== "uniform").length,
    8,
  );
  assert.equal(group.entries[2].resource.buffer.size, 512 * 512 * 268);
  assert.equal(group.entries[3].resource.buffer.size, BATTLE_COMBAT_BYTES);
  assert.equal(group.entries[4].resource.buffer.size, 129 * 8);
  assert.equal(battle.inspect().projectileCapacity, 16384);
  assert.equal(battle.inspect().siegeCount, 8);
  battle.afterSubmit();
  await flush();
  battle.dispose();
});
test("explicit probes retain real projectile words, siege factions and additional counters", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 64,
      capacity: 64,
    });
  const arena = gpu.buffers.find(
    (b) => b.label === "Battle crowd: bounded projectiles and participant",
  ).storage;
  const p = new Float32Array(arena, BATTLE_PROJECTILE_OFFSET + 5 * 64, 16),
    w = new Uint32Array(p.buffer, p.byteOffset, 16);
  p.set([1, 2, 3, 0.5, 4, 5, 6, 2, 10, 0, 20, 8]);
  w.set([1, 180, 1, 31], 12);
  new Uint32Array(
    arena,
    BATTLE_PROJECTILE_OFFSET +
      BATTLE_PROJECTILE_CAPACITY * 64 +
      BATTLE_PROJECTILE_CAPACITY * 0,
    BATTLE_PROJECTILE_CAPACITY,
  )[5] = 1;
  const c = new Uint32Array(
    gpu.buffers.find((b) => b.label === "Battle crowd: actual GPU counters")
      .storage,
  );
  c[34] = 12;
  c[35] = 1;
  c[36] = 9;
  c[37] = 8;
  c[38] = 2;
  c[39] = 4;
  c[40] = 5;
  c[47] = 300;
  c[52] = 3;
  new Uint32Array(arena)[18] = 78;
  new Float32Array(arena).set([2, 0.75, 3], 21);
  const probe = await battle.probe();
  assert.deepEqual(probe.participant.lastImpact, {
    position: [2, 0.75, 3],
    tick: 77,
  });
  assert.equal(probe.projectiles.length, 1);
  assert.deepEqual(probe.projectiles[0].position, [1, 2, 3]);
  assert.equal(probe.projectiles[0].damage, 180);
  assert.equal(probe.projectiles[0].kind, 1);
  assert.equal(probe.gpu.projectileOverflows, 2);
  assert.equal(probe.gpu.siegeHits, 5);
  assert.equal(probe.gpu.participantDamage, 300);
  assert.equal(probe.gpu.participantEpoch, 3);
  assert.equal(probe.sieges.length, 8);
  assert.deepEqual(
    probe.sieges.map((s) => s.team),
    [0, 1, 0, 1, 0, 1, 0, 1],
  );
  assert(
    probe.sieges.every(
      (s) => s.health === 750 && s.position.every(Number.isFinite),
    ),
  );
  battle.dispose();
});
test("procedural trebuchets have separate arm geometry, actual triangles and finite normals", () => {
  const mesh = createBattleSiegeGeometry(),
    v = new Float32Array(mesh.vertices),
    u = new Uint32Array(mesh.vertices);
  assert.equal(mesh.projectile.indexCount, 24);
  assert(mesh.siege.indexCount >= 100);
  assert.equal(mesh.weapon.indexCount, 132);
  assert.equal(
    mesh.indices.length,
    mesh.meteorFx.firstIndex + mesh.meteorFx.indexCount,
  );
  assert(mesh.indices.every((i) => i < v.length / 8));
  const parts = new Set();
  for (let i = 0; i < v.length / 8; i++) {
    assert(Array.from(v.slice(i * 8, i * 8 + 6)).every(Number.isFinite));
    assert(Math.abs(Math.hypot(...v.slice(i * 8 + 3, i * 8 + 6)) - 1) < 1e-6);
    parts.add(u[i * 8 + 6]);
  }
  assert(parts.has(1));
  assert(parts.has(2));
  assert(parts.has(3));
  assert(parts.has(4), "the ranged focus has an actual orb mesh");
});
test("ranged and participant shader contracts keep ballistics, delayed AoE and one protected sequence-bound strike", () => {
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "atomicStore(&combat.data[9],select(92u,252u,frame.participant.w==2u))",
    ),
  );
  assert(!BATTLE_SIMULATION_WGSL.includes("select(92u,198u"));
  const receive = BATTLE_SIMULATION_WGSL.indexOf(
    "atomicStore(&combat.data[12],frame.participant.z)",
  );
  const gate = BATTLE_SIMULATION_WGSL.indexOf("if cooldown==0u&&");
  assert(
    receive >= 0 && receive < gate,
    "a rejected edge must be consumed before cooldown gating",
  );

  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "frame.participant.z>atomicLoad(&combat.data[12])",
    ),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("select(21u,42u,kind==1u)"));
  assert(BATTLE_SIMULATION_WGSL.includes("atomicStore(&combat.data[8],120u)"));
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "atomicStore(&stats[52],atomicLoad(&combat.data[11]))",
    ),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "atomicCompareExchangeWeak(&combat.occupied[slot],0u,1u)",
    ),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes("for(var attempt=0u;attempt<8u;attempt++)"),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("-9.0*dt*dt"));
  assert(BATTLE_SIMULATION_WGSL.includes("applyBlast("));
  assert(BATTLE_SIMULATION_WGSL.includes("damageOut[id].local"));
  assert(BATTLE_SIMULATION_WGSL.includes("distance-bodyRadius(kind)-.4"));
  assert(BATTLE_ORDNANCE_COMPACTION_WGSL.includes("16384u"));
  assert(BATTLE_ORDNANCE_RENDER_WGSL.includes("@vertex fn weaponVS"));
  assert(!BATTLE_ORDNANCE_RENDER_WGSL.includes("discard;"));
});

test("local magic bolts sweep bounded enemy capsules and source terrain before their far aim endpoint", () => {
  assert(
    BATTLE_SIMULATION_WGSL.includes("let gravity=select(18.0,0.0,kind==2u)"),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("if local{"));
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "localProjectileHit(before,delta,p.info.x)",
    ),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("for(var z=0;z<3;z++)"));
  assert(BATTLE_SIMULATION_WGSL.includes("for(var x=0;x<3;x++)"));
  assert(
    BATTLE_SIMULATION_WGSL.includes("min(atomicLoad(&cells[cell].count),64u)"),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("a.motion.z<=0.0||a.info.y==team"));
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "capsuleSegmentHit(origin,delta,a.pose.xyz,height,bodyRadius(a.info.x))",
    ),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes("var best=groundSegmentHit(origin,delta)"),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "triangleSegmentHit(origin,delta,p00,p10,p01)",
    ),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("let point=before+delta*min(t,1.0)"));
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "atomicStore(&combat.data[13],bitcast<u32>(point.x))",
    ),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes("if destination<frame.counts.x"),
    "NPC targeted collision path is retained",
  );
  assert(BATTLE_ORDNANCE_RENDER_WGSL.includes("ranged&&v.part!=4u"));
  assert(BATTLE_ORDNANCE_RENDER_WGSL.includes("!ranged&&v.part==4u"));
});

test("formations fit measured bodies inside the shared four-metre grid without compressing large armies", () => {
  assert.equal(BATTLE_CROWD_LIMITS.count, 1_000_000);
  assert.equal(BATTLE_CROWD_LIMITS.gridDimension, BATTLE_GRID.dimension);
  assert.equal(BATTLE_GRID.cellSize, 4);
  assert.equal(BATTLE_GRID.halfExtent, 1024);
  for (const count of [0, 1, 10000, 100000, 300000, 500000, 1000000]) {
    const radius = battleRadius(count);
    assert.equal(radius, Math.max(12, Math.ceil(Math.sqrt(count))));
    assert(radius <= 1000 && radius < BATTLE_GRID.halfExtent);
  }
  assert.throws(() =>
    validateBattleCrowdOptions({ count: 1000000, radius: 450 }),
  );
  assert.equal(
    validateBattleCrowdOptions({ count: 1000000, radius: 1000 }).radius,
    1000,
  );
  const source = BATTLE_SIMULATION_WGSL;
  assert(source.includes("battleGridPosition(position,frame.battle.xy)"));
  assert(source.includes("id<battleGridCells"));
  assert(source.includes("coordinate>=vec2i(battleGridDimension)"));
  assert(source.includes("ceil(radius/battleCellSize)"));
  assert(source.includes("q>=vec2i(battleGridDimension)"));
  assert(
    source.includes("vec2(256)"),
    "the independent native257 terrain is unchanged",
  );
});

test("every seeded neighbor clears actual class core and feet, including staggered Warroks at one million", () => {
  for (const count of [129, 10000, 1000000]) {
    const data = createBattleCrowdSeed(count, terrain(), { seed: 1 }),
      words = new Uint32Array(data.buffer),
      columns = Math.ceil(Math.sqrt(count));
    let warroks = 0,
      checked = 0,
      minimumGap = Infinity;
    for (let id = 0; id < count; id++) {
      const kind = words[id * 16 + 12];
      if (kind === 3) warroks++;
      const column = id % columns,
        row = Math.floor(id / columns);
      for (let dz = 0; dz <= 1; dz++)
        for (let dx = -1; dx <= 1; dx++) {
          if (dz === 0 && dx <= 0) continue;
          const x = column + dx,
            other = (row + dz) * columns + x;
          if (x < 0 || x >= columns || other >= count) continue;
          const d = Math.hypot(
              data[id * 16] - data[other * 16],
              data[id * 16 + 2] - data[other * 16 + 2],
            ),
            gap =
              d -
              BATTLE_BODY_RADII[kind] -
              BATTLE_BODY_RADII[words[other * 16 + 12]];
          minimumGap = Math.min(minimumGap, gap);
          checked++;
          if (gap < 0.0998)
            assert.fail(
              `Overlapping source bodies count=${count} ids=${id}/${other} gap=${gap}`,
            );
        }
      assert(
        Math.abs(data[id * 16]) < BATTLE_GRID.halfExtent &&
          Math.abs(data[id * 16 + 2]) < BATTLE_GRID.halfExtent,
      );
    }
    assert(checked > count);
    assert(warroks > 0);
    assert(minimumGap >= 0.0998);
    if (count >= 10000)
      assert(
        Math.abs(warroks / count - 0.05) < 0.002,
        "source monster mix retains approximately ten percent Warroks",
      );
  }
});

test("contact handling retains deepest stable contacts and never turns from positional repair", () => {
  const source = BATTLE_SIMULATION_WGSL;
  assert(source.includes("var constraints:array<vec4f,16>"));
  assert(source.includes("var followingSpeed:array<f32,16>"));
  assert(source.includes("followingSpeed[n]=followingSpeed[n-1u]"));
  assert(source.includes("gap<c.z||(gap==c.z&&candidate<u32(c.w))"));
  assert(source.includes("candidateDistance<retainedDistance*.75"));
  assert(source.includes("min(acos(alignment),4.0*dt)"));
  assert(source.includes("if alignment>.999999{return yaw;}"));
  assert(source.includes("abs(cross)<.00001&&alignment<0.0"));
  assert(!source.includes("atan2(goal.x,-goal.y)"));
  assert(source.includes("clearSpawn(location,kind,id)"));
  assert(source.includes(".6*dt/max(length(correction),.0001)"));
  assert(!source.includes("atan2(a.motion.x,-a.motion.y)"));
  assert(!source.includes(".08/max(length(correction)"));
  assert(!source.includes("population>10u"));
});

test("team-mask and squared rejection preserve nearest-target ties and every physical contact", () => {
  const predicateBody = BATTLE_SIMULATION_WGSL.match(
    /fn withinDistanceSquared\([^]*?\)->bool\s*\{([^]*?)\n\}/,
  );
  const maskBody = BATTLE_SIMULATION_WGSL.match(
    /fn cellHasTeam\([^]*?\)->bool\s*\{([^]*?)\}/,
  );
  assert(predicateBody && maskBody);
  const predicate = new Function("squared", "limit", "max", predicateBody[1]);
  const includes = (squared: number, limit: number) =>
    Boolean(predicate(squared, limit, Math.max));
  const maskFunction = new Function(
    "mask",
    "team",
    "i32",
    maskBody[1]
      .replace("atomicLoad(&cells[cell].x)", "mask")
      .replace(/(\d+)u/g, "$1"),
  );
  const hasTeam = (mask: number, team: number) =>
    Boolean(maskFunction(mask, team, (n: number) => n | 0));
  type Candidate = { id: number; team: number; squared: number };
  const nearest = (groups: Candidate[][], team: number, optimized: boolean) => {
    let best = 0xffffffff,
      distance = 1e9;
    for (const group of groups) {
      const mask = group.reduce((bits, a) => bits | (1 << a.team), 0);
      if (optimized && !hasTeam(mask, 1 - team)) continue;
      for (const a of group) {
        if (a.team === team || (optimized && !includes(a.squared, distance)))
          continue;
        const d = Math.fround(Math.sqrt(a.squared));
        if (d < distance || (d === distance && a.id < best)) {
          distance = d;
          best = a.id;
        }
      }
    }
    return best;
  };
  const roundedTie = [
    { id: 9, team: 1, squared: 1 },
    { id: 2, team: 1, squared: 1 + 2 ** -23 },
  ];
  assert.equal(Math.fround(Math.sqrt(roundedTie[1].squared)), 1);
  assert.equal(
    nearest([roundedTie], 0, true),
    2,
    "float-equal lengths must still use the stable ID tie",
  );
  assert.equal(hasTeam(0, 1), false);
  assert.equal(hasTeam(1, 1), false);
  assert.equal(hasTeam(2, 0), false);
  assert.equal(hasTeam(3, 0) && hasTeam(3, 1), true);
  let seed = 0x51ce;
  const random = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 2 ** 32;
  };
  for (let trial = 0; trial < 1000; trial++) {
    const groups = Array.from({ length: 9 }, () =>
      Array.from({ length: 16 }, (_, id) => {
        const x = Math.fround((random() - 0.5) * 4000),
          z = Math.fround((random() - 0.5) * 4000);
        return {
          id: id + Math.floor(random() * 1000),
          team: Number(random() > 0.5),
          squared: Math.fround(Math.fround(x * x) + Math.fround(z * z)),
        };
      }),
    );
    for (const team of [0, 1])
      assert.equal(nearest(groups, team, true), nearest(groups, team, false));
  }
  for (const left of BATTLE_BODY_RADII)
    for (const right of BATTLE_BODY_RADII) {
      const limit = left + right + 0.6;
      for (let index = -100; index <= 100; index++) {
        const d = Math.fround(limit + index * 1e-7),
          squared = Math.fround(d * d);
        const distance = Math.fround(Math.sqrt(squared));
        const gap = Math.fround(
          Math.fround(
            Math.fround(distance - Math.fround(left)) - Math.fround(right),
          ) - Math.fround(0.1),
        );
        if (gap < 0.5)
          assert(
            includes(squared, limit),
            "broad rejection must not remove a body contact at a rounding boundary",
          );
      }
    }
});

test("production turn math preserves exact signed-zero axis goals and converges from every quadrant", () => {
  const match = BATTLE_SIMULATION_WGSL.match(
    /fn turnToward\([^]*?\)\s*->f32\s*\{([^]*?)\n\}/,
  );
  assert(match);
  const body = match[1].replace(/if ([^{]+)\{/g, "if ($1){");
  const vector = (x: number, y: number) => ({ x, y });
  const turn = new Function(
    "yaw",
    "goal",
    "dt",
    "vec2",
    "safeNormal",
    "sin",
    "cos",
    "dot",
    "clamp",
    "abs",
    "acos",
    "min",
    "select",
    body,
  );
  const apply = (yaw: number, goal: { x: number; y: number }) =>
    turn(
      yaw,
      goal,
      1 / 60,
      vector,
      (v: { x: number; y: number }) => {
        const n = Math.hypot(v.x, v.y);
        return vector(v.x / n, v.y / n);
      },
      Math.sin,
      Math.cos,
      (a: { x: number; y: number }, b: { x: number; y: number }) =>
        a.x * b.x + a.y * b.y,
      (n: number, a: number, b: number) => Math.min(b, Math.max(a, n)),
      Math.abs,
      Math.acos,
      Math.min,
      (a: number, b: number, c: boolean) => (c ? b : a),
    ) as number;
  for (const zero of [0, -0]) {
    assert.equal(
      apply(Math.PI / 2, vector(100, zero)),
      Math.PI / 2,
      "blue rear must keep facing +X",
    );
    assert.equal(
      apply(-Math.PI / 2, vector(-100, zero)),
      -Math.PI / 2,
      "red rear must keep facing -X",
    );
  }
  for (const start of [-Math.PI, -Math.PI / 2, 0, Math.PI / 2, Math.PI])
    for (const goal of [
      vector(1, 0),
      vector(-1, 0),
      vector(0, 1),
      vector(0, -1),
      vector(1, 1),
    ]) {
      let yaw = start;
      for (let tick = 0; tick < 120; tick++) {
        const next = apply(yaw, goal);
        assert(Number.isFinite(next));
        assert(Math.abs(next - yaw) <= 4 / 60 + 1e-12);
        yaw = next;
      }
      assert(
        (Math.sin(yaw) * goal.x - Math.cos(yaw) * goal.y) /
          Math.hypot(goal.x, goal.y) >
          0.99999,
      );
    }
});

// Execute the scalar production WGSL expressions directly; trajectories below are independent fixtures.
function contactScalarExpressions() {
  const match = BATTLE_SIMULATION_WGSL.match(
    /let limit=([^;]+);let inward=dot\(velocity,contact\.xy\)/,
  );
  assert(match);
  const expression = match[1]
    .replaceAll("contact.z", "gap")
    .replaceAll("followingSpeed[n]", "following");
  return new Function(
    "gap",
    "following",
    "dt",
    "max",
    `return ${expression}`,
  ) as (
    gap: number,
    following: number,
    dt: number,
    max: typeof Math.max,
  ) => number;
}

test("actual contact scalar permits steady convoys, safely brakes behind a stopping leader and stops head-on traffic", () => {
  const scalar = contactScalarExpressions(),
    dt = 1 / 60,
    core = BATTLE_BODY_RADII[0] * 2;
  const limit = (gap: number, leaderOutward: number) =>
    scalar(gap, Math.max(0, leaderOutward), dt, Math.max);
  assert.equal(limit(0, 3.4), 3.4);
  assert.equal(limit(0, 0), 0);
  assert.equal(
    limit(0, -3.4),
    0,
    "a head-on neighbor grants no receding allowance",
  );
  const count = 32,
    positions = Array.from({ length: count }, (_, id) => id * (core + 0.11)),
    velocity = Array(count).fill(0);
  const start = positions[0];
  let minimumGap = Infinity,
    progressAtThree = 0;
  for (let tick = 0; tick < 420; tick++) {
    const next = velocity.map((v) => Math.min(3.4, v + 8 * dt));
    if (tick >= 180) next[count - 1] = 0;
    for (let id = 0; id < count - 1; id++) {
      const gap = positions[id + 1] - positions[id] - core - 0.1;
      next[id] = Math.min(next[id], limit(gap, velocity[id + 1]));
    }
    for (let id = 0; id < count; id++) {
      positions[id] += next[id] * dt;
      velocity[id] = next[id];
    }
    for (let id = 0; id < count - 1; id++)
      minimumGap = Math.min(
        minimumGap,
        positions[id + 1] - positions[id] - core,
      );
    if (tick === 179) progressAtThree = positions[0] - start;
  }
  assert(
    progressAtThree > 6,
    "the rearmost fighter must follow the moving convoy rather than park",
  );
  assert(
    minimumGap >= -1e-8,
    "a leader's sudden attack stop must not create body penetration",
  );
  assert(velocity[0] < 0.01, "stopped leader eventually stops its followers");
  let left = -3,
    right = 3;
  for (let tick = 0; tick < 180; tick++) {
    const allowed = limit(right - left - core - 0.1, -3.4),
      speed = Math.min(3.4, allowed);
    left += speed * dt;
    right -= speed * dt;
    assert(
      right - left >= core + 0.099999,
      "head-on actors preserve the original half-gap stop",
    );
  }
});

test("melee reaches the surface of large bodies without compressing them and projectile queries cover their radius", () => {
  const base = [1.666667, 6.666667, 1.666667, 2.5];
  for (let attacker = 0; attacker < 4; attacker++)
    for (let target = 0; target < 4; target++) {
      const required =
        BATTLE_BODY_RADII[attacker] + BATTLE_BODY_RADII[target] + 0.1;
      const reach =
        base[attacker] + Math.max(0, BATTLE_BODY_RADII[target] - 0.64);
      assert(
        reach * 0.95 >= required,
        `class ${attacker} cannot melee class ${target} at physical clearance`,
      );
    }
  assert(
    BATTLE_SIMULATION_WGSL.includes(
      "attackReach+=max(0.0,bodyRadius(source[enemyId].info.x)-.64)",
    ),
  );
  assert(
    BATTLE_SIMULATION_WGSL.includes("d<1.8+max(0.0,bodyRadius(a.info.x)-.64)"),
  );
  assert(BATTLE_SIMULATION_WGSL.includes("vec2(battleMaxBodyRadius)"));
  assert(
    Math.ceil(
      (0.7 + 2 * Math.max(...BATTLE_BODY_RADII)) / BATTLE_GRID.cellSize,
    ) +
      1 <=
      3,
  );
});

function contactFixture(epoch = 1, tick = 60) {
  const raw = new ArrayBuffer(BATTLE_PARTICIPANT_CONTACT_BYTES),
    words = new Uint32Array(raw),
    floats = new Float32Array(raw);
  words.set([epoch, tick, 1, 0]);
  floats.set([1, 2, 3, 1], 4);
  floats.set([2, 2, 3, 1.33, 0.5, -0.25], 8);
  words[14] = 3;
  floats[15] = 1;
  return raw;
}

test("body-contact decoding preserves real radii, velocities and overflow, and refuses malformed geometry", () => {
  const raw = contactFixture(),
    sample = decodeBattleParticipantContacts(raw, 8);
  assert.deepEqual(sample.position, [1, 2, 3]);
  assert.equal(sample.epoch, 1);
  assert.equal(sample.tick, 60);
  assert.equal(sample.contacts[0].id, 3);
  assert(Math.abs(sample.contacts[0].radius - 1.33) < 1e-6);
  assert.deepEqual(sample.contacts[0].velocity, [0.5, -0.25]);
  assert(Object.isFrozen(sample.contacts[0]));
  const over = raw.slice(0);
  new Uint32Array(over)[3] = 7;
  assert.equal(decodeBattleParticipantContacts(over, 8).overflow, 7);
  assert.throws(() => decodeBattleParticipantContacts(raw.slice(0, -4), 8));
  for (const [word, value] of [
    [2, 65],
    [14, 8],
  ] as const) {
    const bad = raw.slice(0);
    new Uint32Array(bad)[word] = value;
    assert.throws(() => decodeBattleParticipantContacts(bad, 8));
  }
  for (const [word, value] of [
    [4, NaN],
    [11, -1],
    [12, Infinity],
  ] as const) {
    const bad = raw.slice(0);
    new Float32Array(bad)[word] = value;
    assert.throws(() => decodeBattleParticipantContacts(bad, 8));
  }
  const duplicate = raw.slice(0),
    w = new Uint32Array(duplicate);
  w[2] = 2;
  new Uint8Array(duplicate, 64, 32).set(new Uint8Array(raw, 32, 32));
  assert.throws(() => decodeBattleParticipantContacts(duplicate, 8));
  assert(BATTLE_PARTICIPANT_CONTACTS_WGSL.includes("min(population,64u)"));
  assert(
    BATTLE_PARTICIPANT_CONTACTS_WGSL.includes(
      "distance<other.distance||(distance==other.distance&&id<other.id)",
    ),
  );
  assert(!BATTLE_PARTICIPANT_CONTACTS_WGSL.includes("n<frame.counts.x"));
});

test("participant contacts are asynchronously bounded, life-scoped and serialled even at the same GPU tick", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 8,
    });
  const input = {
    active: true,
    position: [1, 2, 3] as [number, number, number],
    yaw: 0,
    team: 0 as const,
    attackSequence: 0,
    attack: "melee" as const,
    aim: [0, 0, 0] as [number, number, number],
  };
  battle.setParticipant(input);
  const epoch = battle.resetParticipant();
  assert.equal(battle.latestParticipantContacts(), null);
  const output = gpu.buffers.find(
    (b) => b.label === "Battle crowd: nearby participant bodies",
  );
  const stages = gpu.buffers.filter(
    (b) => b.label === "Battle crowd: asynchronous body contact readback",
  );
  assert.equal(stages.length, 3);
  assert.equal(output.size + stages.reduce((n, b) => n + b.size, 0), 8320);
  new Uint8Array(output.storage).set(new Uint8Array(contactFixture(epoch)));
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  assert.equal(battle.latestParticipantContacts(), null);
  battle.afterSubmit();
  await flush();
  const first = battle.latestParticipantContacts();
  assert(first);
  assert.equal(first.epoch, epoch);
  assert.equal(
    battle.latestParticipantContacts(),
    first,
    "getters cannot relabel cached data as a new sample",
  );
  const count = gpu.buffers.length;
  const next = contactFixture(epoch);
  new Float32Array(next)[4] = 1.5;
  new Uint8Array(output.storage).set(new Uint8Array(next));
  battle.prepare(gpu.encoder, frame(), 0);
  battle.afterSubmit();
  await flush();
  const second = battle.latestParticipantContacts();
  assert(second);
  assert(second.serial > first.serial);
  assert.equal(second.tick, first.tick);
  assert.equal(second.position[0], 1.5);
  assert.equal(gpu.buffers.length, count);
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  battle.afterSubmit();
  const newEpoch = battle.resetParticipant();
  assert.equal(battle.latestParticipantContacts(), null);
  await flush();
  assert.equal(
    battle.latestParticipantContacts(),
    null,
    "prior-life pending maps cannot publish",
  );
  new Uint8Array(output.storage).set(new Uint8Array(contactFixture(newEpoch)));
  battle.prepare(gpu.encoder, frame(), 1 / 60);
  battle.afterSubmit();
  await flush();
  assert.equal(battle.latestParticipantContacts()?.epoch, newEpoch);
  battle.setParticipant({ ...input, active: false });
  assert.equal(battle.latestParticipantContacts(), null);
  battle.dispose();
  assert.equal(battle.latestParticipantContacts(), null);
  assert(gpu.buffers.every((b) => b.destroyed === 1));
});

test("late maps cannot replace newer participant contacts, and three busy slots bound requests", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 8,
    });
  battle.setParticipant({
    active: true,
    position: [1, 2, 3],
    yaw: 0,
    team: 0,
    attackSequence: 0,
    attack: "melee",
    aim: [0, 0, 0],
  });
  const epoch = battle.resetParticipant(),
    maps: { release: () => void }[] = [];
  const output = gpu.buffers.find(
    (b) => b.label === "Battle crowd: nearby participant bodies",
  );
  for (const slot of gpu.buffers.filter(
    (b) => b.label === "Battle crowd: asynchronous body contact readback",
  ))
    slot.mapAsync = () =>
      new Promise<void>((resolve) => maps.push({ release: resolve }));
  const allocated = gpu.buffers.length;
  const encodedBefore = performance.now();
  for (let i = 1; i <= 4; i++) {
    const raw = contactFixture(epoch);
    new Float32Array(raw)[4] = i;
    new Uint8Array(output.storage).set(new Uint8Array(raw));
    battle.prepare(gpu.encoder, frame(), 1 / 60);
    battle.afterSubmit();
  }
  const encodedAfter = performance.now();
  assert.equal(
    maps.length,
    3,
    "a fourth outstanding map must not allocate or enqueue more work",
  );
  assert.equal(gpu.buffers.length, allocated);
  await new Promise((resolve) => setTimeout(resolve, 10));
  maps[2].release();
  await flush();
  const newest = battle.latestParticipantContacts();
  assert(newest);
  assert.equal(newest.position[0], 3);
  assert(
    newest.sampledAt >= encodedBefore && newest.sampledAt <= encodedAfter,
    "encoding time must survive delayed map receipt",
  );
  assert(
    performance.now() - newest.sampledAt >= 5,
    "cache age includes GPU queue and map delay",
  );
  maps[0].release();
  maps[1].release();
  await flush();
  assert.equal(
    battle.latestParticipantContacts(),
    newest,
    "late same-tick maps must not refresh stale origins",
  );
  battle.dispose();
});

test("meteor input packs explicit target validity separately from first person and reports real cooldown counters", async () => {
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 8,
    });
  const input = {
    active: true,
    position: [0, 0, 0] as [number, number, number],
    yaw: 0,
    team: 0 as const,
    attackSequence: 1,
    attack: "meteor_shower" as const,
    aim: [0, 0, 20] as [number, number, number],
  };
  assert.equal(validateBattleParticipant(input).attack, "meteor_shower");
  assert.throws(() =>
    validateBattleParticipant({ ...input, meteorTargetValid: "true" as any }),
  );
  const uniform = gpu.buffers.find(
    (b) => b.label === "Battle crowd: frame and simulation parameters",
  );
  for (const firstPerson of [false, true])
    for (const meteorTargetValid of [false, true]) {
      battle.setParticipant({ ...input, firstPerson, meteorTargetValid });
      battle.prepare(gpu.encoder, frame(), 0);
      assert.equal(new Uint32Array(uniform.storage)[83], 2);
      assert.equal(
        new Float32Array(uniform.storage)[79],
        Number(firstPerson) + Number(meteorTargetValid) * 2,
      );
      battle.afterSubmit();
      await flush();
    }
  const counters = new Uint32Array(64);
  counters.set([2, 6, 31, 4, 9], 55);
  const decoded = decodeBattleCounters(counters);
  assert.equal(decoded.meteorCasts, 2);
  assert.equal(decoded.meteorImpacts, 6);
  assert.equal(decoded.meteorCooldownTicks, 31);
  assert.equal(decoded.meteorsActive, 4);
  assert.equal(decoded.meteorHits, 9);
  assert(
    gpu.buffers
      .filter((b) => b.label.startsWith("Battle crowd: delayed damage"))
      .every((b) => (b.usage & GPUBufferUsage.COPY_SRC) !== 0),
  );
  battle.dispose();
});

test("meteor scheduling reserves a complete frozen cast and retains the source six staggered one-second falls", () => {
  const source = BATTLE_SIMULATION_WGSL;
  assert(source.includes("(u32(frame.participantAim.w)&2u)==0u"));
  assert(source.includes("aimedDistance>=.01&&aimedDistance<=80.001"));
  assert(
    source.includes(
      "for(var j=0u;j<reserved;j++){atomicStore(&combat.occupied[slots[j]],0u);}",
    ),
  );
  assert(source.includes("for(var die=0u;die<40u;die++)"));
  assert(source.includes("vec4u(frame.participant.x,roll,3u,epoch)"));
  assert(source.includes("p.info.w!=atomicLoad(&combat.data[11])"));
  assert(source.includes("damageActor(id,value,kind==2u||kind==3u)"));
  assert(source.includes("p.info.z=4u"));
  const impacts: number[] = [];
  for (let meteor = 0; meteor < 6; meteor++) {
    const delay = 150 + Math.round(meteor * 0.13 * 60);
    let age = Math.fround(-(delay + 1) / 60),
      launched = false;
    for (let tick = 0; tick < 300; tick++) {
      if (age < 0) {
        age = Math.fround(age + Math.fround(1 / 60));
        if (age >= -0.0001) {
          age = 0;
          launched = true;
          assert.equal(tick, delay);
        }
        continue;
      }
      assert(launched);
      age = Math.min(1, Math.fround(age + Math.fround(1 / 60)));
      if (age >= 0.9999) {
        impacts.push(tick);
        break;
      }
    }
  }
  assert.deepEqual(impacts, [210, 218, 226, 233, 241, 249]);
  assert(impacts[5] < 252, "cooldown covers the last scheduled impact");
});

test("meteor impacts use the full shared roll throughout the blast while other projectiles keep falloff", () => {
  const match = BATTLE_SIMULATION_WGSL.match(/let value=([^;]+);damageActor/);
  assert(match);
  const expression = match[1].replaceAll(/([0-9]+)u\b/g, "$1");
  const value = new Function(
    "amount",
    "d",
    "radius",
    "kind",
    "select",
    "max",
    "u32",
    "f32",
    `return ${expression}`,
  );
  const evaluate = (amount: number, d: number, kind: number) =>
    value(
      amount,
      d,
      4,
      kind,
      (a: number, b: number, c: boolean) => (c ? b : a),
      Math.max,
      Math.trunc,
      (n: number) => n,
    ) as number;
  for (const amount of [40, 140, 240])
    for (const d of [0, 1, 2, 3.9, 3.999])
      assert.equal(evaluate(amount, d, 3), amount);
  assert.equal(evaluate(300, 2, 2), 150);
  assert.equal(evaluate(180, 2, 1), 90);
  assert.equal(evaluate(20, 2, 0), 10);
});

test("meteor soft fire uses a bounded additive billboard draw while ordinary heads remain opaque", async () => {
  const mesh = createBattleSiegeGeometry(),
    words = new Uint32Array(mesh.vertices),
    parts = new Map<number, number>();
  assert.equal(mesh.projectile.indexCount, 24);
  for (
    let index = mesh.meteorFx.firstIndex;
    index < mesh.meteorFx.firstIndex + mesh.meteorFx.indexCount;
    index += 3
  ) {
    const vertex = mesh.indices[index],
      part = words[vertex * 8 + 6];
    parts.set(part, (parts.get(part) ?? 0) + 1);
  }
  assert.equal(mesh.meteorFx.indexCount, 19 * 6);
  assert.equal(parts.size, 19);
  assert([...parts.values()].every((count) => count === 2));
  assert(BATTLE_ORDNANCE_COMPACTION_WGSL.includes("if a.pose.w<0.0{return;}"));
  assert(
    BATTLE_ORDNANCE_COMPACTION_WGSL.includes(
      "atomicMax(&draws[3].instanceCount,offset+1u)",
    ),
  );
  assert(
    BATTLE_ORDNANCE_COMPACTION_WGSL.includes(
      "(u32(frame.participantAim.w)&1u)!=0u",
    ),
  );
  assert(BATTLE_ORDNANCE_RENDER_WGSL.includes(".25+(1.0-j/8.0)*.8"));
  assert(BATTLE_ORDNANCE_RENDER_WGSL.includes("pow(max(0.0,1.0-r),2.0)"));
  assert(BATTLE_ORDNANCE_RENDER_WGSL.includes("color=vec3(.22,.095,.035)"));
  const gpu = fakeGpu(),
    battle = await BattleCrowd.create(gpu.device, assets(), terrain(), {
      count: 8,
      capacity: 8,
    });
  const fx = gpu.pipelines.find((p) => p.vertex.entryPoint === "meteorFxVS"),
    head = gpu.pipelines.find((p) => p.vertex.entryPoint === "projectileVS");
  assert.equal(fx.fragment.entryPoint, "meteorFxFS");
  assert.equal(fx.depthStencil.depthWriteEnabled, false);
  assert.equal(fx.depthStencil.depthCompare, "less-equal");
  assert.deepEqual(fx.fragment.targets[0].blend, {
    color: { srcFactor: "one", dstFactor: "one" },
    alpha: { srcFactor: "zero", dstFactor: "one" },
  });
  assert.equal(head.depthStencil.depthWriteEnabled, true);
  assert.equal(head.fragment.targets[0].blend, undefined);
  assert.equal(head.fragment.entryPoint, "fs");
  assert.equal(
    gpu.buffers.find(
      (b) => b.label === "Battle crowd: ordnance indirect arguments",
    ).size,
    80,
  );
  battle.dispose();
});

test("device preflight rejects buffer, binding and dispatch failures before allocating any battle resource", async () => {
  const large = battleCrowdDeviceRequirements(1_000_000, terrain(), assets());
  assert.equal(large.gridBytes, 512 * 512 * 268);
  assert.equal(large.actorBytes, 64_000_000);
  assert.equal(large.damageBytes, 8_000_000);
  assert.equal(large.visibleBytes, 48_000_000);
  assert.equal(large.actorWorkgroups, 7813);
  assert.equal(large.clearWorkgroups, 7813);
  assert.equal(large.largestStorageBytes, 70_254_592);
  const gpu = fakeGpu();
  assert.equal(
    validateBattleCrowdDevice(gpu.device.limits, 1_000_000, terrain(), assets())
      .largestBufferBytes,
    70_254_592,
  );
  for (const [name, minimum] of Object.entries(large.limits)) {
    const limits = { ...gpu.device.limits, [name]: minimum - 1 };
    assert.throws(
      () =>
        validateBattleCrowdDevice(
          limits as GPUSupportedLimits,
          1_000_000,
          terrain(),
          assets(),
        ),
      new RegExp(name),
    );
  }
  for (const name of [
    "maxBufferSize",
    "maxStorageBufferBindingSize",
    "maxStorageBuffersPerShaderStage",
    "maxBindingsPerBindGroup",
    "maxComputeWorkgroupsPerDimension",
  ]) {
    const bad = fakeGpu(),
      requirements = battleCrowdDeviceRequirements(129, terrain(), assets());
    (bad.device.limits as any)[name] =
      requirements.limits[name as keyof typeof requirements.limits] - 1;
    await assert.rejects(
      BattleCrowd.create(bad.device, assets(), terrain(), {
        count: 129,
        capacity: 129,
      }),
      new RegExp(name),
    );
    assert.equal(bad.buffers.length, 0);
    assert.equal(bad.textures.length, 0);
    assert.equal(bad.pipelines.length, 0);
  }
});
