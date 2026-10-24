import assert from "node:assert/strict";
import { createHash, webcrypto } from "node:crypto";
import test from "node:test";
import {
  EnemyCharacters,
  enemyHealthBarScale,
  type EnemyActorPose,
} from "../packages/renderer/enemy-characters.ts";
import {
  CharacterRenderer,
  CHARACTER_WGSL,
} from "../packages/renderer/character-renderer.ts";
import {
  CHARACTER_MOTIONS,
  sampleCharacterLocals,
  validateCharacter,
  type CharacterManifest,
} from "../packages/renderer/character.ts";
import { MmorpgCharacters } from "../packages/renderer/mmorpg-characters.ts";
import type {
  MmorpgActorPose,
  MmorpgClass,
} from "../packages/assets/src/mmorpg.ts";
import type { CreatureFrame } from "../packages/renderer/creatures.ts";
import { validEnemySnapshot } from "../packages/runtime/enemies.ts";

const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
function fixture() {
  const vertices = new Float32Array(48);
  for (let i = 0; i < 3; i++) {
    vertices.set(
      [
        i === 1 ? 1 : 0,
        i === 2 ? 1 : 0,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
      ],
      i * 16,
    );
  }
  const indices = new Uint32Array([0, 1, 2]);
  const file = (data: ArrayBufferLike, name: string) => ({
    url: `/private-assets/${name}`,
    sha256: createHash("sha256").update(new Uint8Array(data)).digest("hex"),
    byteLength: data.byteLength,
  });
  const manifest: CharacterManifest = {
    schema: "aethyrforge.character.v1",
    name: "test/knight",
    geometry: {
      vertices: file(vertices.buffer, "knight.vertices.bin"),
      indices: file(indices.buffer, "knight.indices.bin"),
      vertexCount: 3,
      indexCount: 3,
    },
    joints: [
      {
        parent: -1,
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
        inverseBind: identity,
      },
    ],
    clips: [...CHARACTER_MOTIONS, "death" as const].map((name) => ({
      name,
      duration: 1,
      distance: name === "run" ? 3.4 : 0,
      tracks: [
        {
          joint: 0,
          translation: [
            { time: 0, value: [0, 0, 0] },
            { time: 1, value: name === "death" ? [0, -0.8, 0] : [0, 0, 1] },
          ],
          rotation: [],
          scale: [],
        },
      ],
    })),
    materials: [
      { name: "armor", color: [1, 1, 1, 1], alphaCutoff: 0, doubleSided: true },
    ],
    draws: [{ material: 0, firstIndex: 0, indexCount: 3 }],
  };
  const data = new Map([
    [manifest.geometry.vertices.url, vertices.buffer],
    [manifest.geometry.indices.url, indices.buffer],
  ]);
  return {
    manifest,
    load: async (url: string) => {
      const result = data.get(url);
      if (!result) throw Error("Unexpected asset load");
      return result.slice(0);
    },
  };
}
function fakeGPU() {
  const buffers: { size: number; destroyed: number }[] = [],
    textures: {
      width: number;
      height: number;
      mipLevelCount: number;
      format: string;
      destroyed: number;
    }[] = [],
    writes: { buffer: unknown; bytes: Uint8Array }[] = [],
    draws: number[][] = [],
    shaders: string[] = [],
    groups: GPUBindGroupDescriptor[] = [],
    boundGroups: number[] = [],
    mipDraws: number[] = [];
  const barDraws: number[][] = [];
  let submits = 0;
  let pipelines = 0,
    shader = "";
  const device = {
    createShaderModule: (value: { code: string }) => {
      shader = value.code;
      shaders.push(value.code);
      return {};
    },
    createRenderPipelineAsync: async () => {
      pipelines++;
      return { getBindGroupLayout: () => ({}) };
    },
    createBuffer: ({ size }: { size: number }) => {
      const b = {
        size,
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(b);
      return b;
    },
    createTexture: ({
      size,
      mipLevelCount = 1,
      format,
    }: {
      size: number[];
      mipLevelCount?: number;
      format: string;
    }) => {
      const t = {
        width: size[0],
        height: size[1],
        mipLevelCount,
        format,
        destroyed: 0,
        createView: () => ({}),
        destroy() {
          this.destroyed++;
        },
      };
      textures.push(t);
      return t;
    },
    createBindGroup: (description: GPUBindGroupDescriptor) => {
      groups.push(description);
      return {};
    },
    createSampler: () => ({}),
    createCommandEncoder: () => ({
      beginRenderPass: () => ({
        setPipeline: () => {},
        setBindGroup: () => {},
        draw: (n: number) => mipDraws.push(n),
        end: () => {},
      }),
      finish: () => ({}),
    }),
    queue: {
      writeBuffer: (
        buffer: unknown,
        _at: number,
        data: ArrayBuffer | ArrayBufferView,
      ) =>
        writes.push({
          buffer,
          bytes:
            data instanceof ArrayBuffer
              ? new Uint8Array(data.slice(0))
              : new Uint8Array(
                  data.buffer.slice(
                    data.byteOffset,
                    data.byteOffset + data.byteLength,
                  ),
                ),
        }),
      writeTexture: () => {},
      copyExternalImageToTexture: () => {},
      submit: () => {
        submits++;
      },
    },
  } as unknown as GPUDevice;
  const pass = {
    setPipeline: () => {},
    setBindGroup: (index: number) => {
      boundGroups.push(index);
    },
    setVertexBuffer: () => {},
    setIndexBuffer: () => {},
    drawIndexed: (...args: number[]) => draws.push(args),
    draw: (...args: number[]) => barDraws.push(args),
  } as unknown as GPURenderPassEncoder;
  return {
    device,
    pass,
    buffers,
    textures,
    writes,
    draws,
    groups,
    boundGroups,
    shaders,
    mipDraws,
    barDraws,
    get submits() {
      return submits;
    },
    get pipelines() {
      return pipelines;
    },
    get shader() {
      return shader;
    },
  };
}
const frame: CreatureFrame = {
  mode: "day",
  viewProjection: new Float32Array(identity),
  diffuseRotation: new Float32Array(identity),
  diffuseSH: new Float32Array(36),
  eye: [0, 1, 0],
  sunDirection: [0, 1, 0],
  sunIlluminance: [1, 1, 1],
  time: 0,
  enabled: true,
};
const actor = (id: number): EnemyActorPose => ({
  id,
  position: [id, 0, 0],
  yaw: 0,
  alive: true,
  deathSeconds: 0,
  animationTime: 0.25,
});
function globals() {
  Object.defineProperty(globalThis, "crypto", {
    value: webcrypto,
    configurable: true,
  });
  Object.assign(globalThis, {
    GPUBufferUsage: {
      COPY_DST: 1,
      VERTEX: 2,
      INDEX: 4,
      UNIFORM: 8,
      STORAGE: 16,
    },
    GPUTextureUsage: { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 },
  });
}

test("death clamps to its last authored pose while running loops", () => {
  const { manifest } = fixture();
  validateCharacter(manifest);
  assert.deepEqual(
    sampleCharacterLocals(manifest, "death", 99),
    sampleCharacterLocals(manifest, "death", 1),
  );
  assert.equal(
    sampleCharacterLocals(manifest, "death", 99)[0].translation[1],
    -0.8,
  );
  assert.equal(
    sampleCharacterLocals(manifest, "death", -1)[0].translation[1],
    0,
  );
  assert.deepEqual(
    sampleCharacterLocals(manifest, "run", 1.25),
    sampleCharacterLocals(manifest, "run", 0.25),
  );
  assert.throws(() =>
    validateCharacter({
      ...manifest,
      clips: manifest.clips.filter((x) => x.name !== "walk"),
    }),
  );
  const original = {
    ...manifest,
    clips: manifest.clips.filter((x) => x.name !== "death"),
  };
  assert.doesNotThrow(() => validateCharacter(original));
});

test("16 knights share a pipeline, geometry and material with one instanced draw", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU();
  let loads = 0;
  const enemy = await EnemyCharacters.create(
    gpu.device,
    f.manifest,
    async (u) => {
      loads++;
      return f.load(u);
    },
  );
  assert.equal(loads, 2);
  assert.equal(gpu.pipelines, 1);
  assert.equal(gpu.textures.length, 2);
  assert.equal(gpu.buffers.length, 6);
  const actors = Array.from({ length: 16 }, (_, id) => actor(id));
  enemy.render(gpu.pass, frame, actors);
  assert.deepEqual(gpu.draws, [[3, 16, 0]]);
  assert.deepEqual(enemy.stats, {
    submittedActors: 16,
    submittedTriangles: 16,
    submittedHealthBars: 0,
    submittedHealthBarTriangles: 0,
  });
  const models = gpu.writes.at(-1)!.bytes,
    view = new DataView(models.buffer);
  assert.equal(models.byteLength, 16 * 80);
  assert.equal(view.getFloat32(15 * 80 + 48, true), 15);
  assert.equal(view.getUint32(15 * 80 + 64, true), 15);
  assert.ok(gpu.shader.includes("@builtin(instance_index)"));
  assert.ok(gpu.shader.includes("heightFogComposition"));
  assert.equal(gpu.shader, CHARACTER_WGSL);
  assert.equal(enemy.bytes, 192 + 12 + 464 + 16 * (128 + 80) + 64 + 8);
  enemy.dispose();
  enemy.dispose();
  assert.ok(gpu.buffers.every((b) => b.destroyed === 1));
  assert.ok(gpu.textures.every((t) => t.destroyed === 1));
  enemy.render(gpu.pass, frame, actors);
  assert.equal(gpu.draws.length, 1);
  assert.equal(enemy.stats.submittedActors, 0);
});

test("dead knights stay down after a one-shot and retain poses when the render clock changes", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  const live = actor(7);
  enemy.render(gpu.pass, frame, [live]);
  const dead = { ...live, alive: false, deathSeconds: 1 };
  enemy.render(gpu.pass, frame, [dead]);
  enemy.render(gpu.pass, { ...frame, time: 999 }, [
    { ...dead, deathSeconds: 2 },
  ]);
  const final = gpu.writes.at(-2)!.bytes.slice();
  assert.ok(Math.abs(new Float32Array(final.buffer)[13] + 0.8) < 1e-6);
  enemy.render(gpu.pass, { ...frame, time: 1999 }, [
    { ...dead, deathSeconds: 200 },
  ]);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, final);
  // Same simulation snapshot does not advance on a render-only frame.
  enemy.render(gpu.pass, { ...frame, time: 9999 }, [
    { ...dead, deathSeconds: 200 },
  ]);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, final);
  enemy.dispose();
});

test("enemy admission rejects duplicate or unbounded actors before changing GPU data", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  const before = gpu.writes.length;
  for (const values of [
    [actor(0), actor(0)],
    Array.from({ length: 17 }, (_, i) => actor(i)),
    [{ ...actor(0), id: -1 }],
    [{ ...actor(0), deathSeconds: NaN }],
    [{ ...actor(0), position: [0, Infinity, 0] }],
  ] as EnemyActorPose[][])
    assert.throws(() => enemy.render(gpu.pass, frame, values));
  assert.equal(gpu.writes.length, before);
  assert.equal(gpu.draws.length, 0);
  await assert.rejects(
    EnemyCharacters.create(
      gpu.device,
      {
        ...f.manifest,
        clips: f.manifest.clips.filter((c) => c.name !== "death"),
      },
      f.load,
    ),
    /death animation/,
  );
  await assert.rejects(
    CharacterRenderer.create(gpu.device, f.manifest, f.load, 65),
    /64 actors/,
  );
  enemy.dispose();
});

test("a failed shared texture load destroys every candidate resource", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU();
  f.manifest.materials[0].texture = {
    url: "/private-assets/knight.png",
    sha256: "0".repeat(64),
    byteLength: 33,
    width: 1,
    height: 1,
  };
  await assert.rejects(
    EnemyCharacters.create(gpu.device, f.manifest, async (u) => {
      if (u.endsWith(".png")) throw Error("Disconnected during texture load");
      return f.load(u);
    }),
    /Disconnected/,
  );
  assert.equal(gpu.textures.length, 1);
  assert.ok(
    gpu.buffers.length > 0 && gpu.buffers.every((b) => b.destroyed === 1),
  );
  assert.ok(gpu.textures.every((t) => t.destroyed === 1));
});

test("a skipped death frame samples its elapsed pose and actor ID reuse clears old animation", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  enemy.render(gpu.pass, frame, [actor(0)]);
  enemy.render(gpu.pass, frame, [
    { ...actor(0), alive: false, deathSeconds: 200 },
  ]);
  assert.ok(
    Math.abs(new Float32Array(gpu.writes.at(-2)!.bytes.buffer)[13] + 0.8) <
      1e-6,
  );
  enemy.render(gpu.pass, frame, []);
  enemy.render(gpu.pass, frame, [actor(0)]);
  assert.equal(new Float32Array(gpu.writes.at(-2)!.bytes.buffer)[13], 0);
  assert.equal(enemy.stats.submittedActors, 1);
  enemy.dispose();
});

test("run phase follows collision-admitted distance and the prepared stride", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU();
  f.manifest.clips.find((c) => c.name === "run")!.distance = 1.7;
  const enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  enemy.render(gpu.pass, frame, [actor(0)]);
  const first = gpu.writes.at(-2)!.bytes;
  assert.equal(new Float32Array(first.buffer)[14], 0.5);
  enemy.render(gpu.pass, { ...frame, time: 100 }, [actor(0)]);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, first);
  enemy.render(gpu.pass, frame, [{ ...actor(0), animationTime: 0.375 }]);
  assert.equal(new Float32Array(gpu.writes.at(-2)!.bytes.buffer)[14], 0.75);
  enemy.dispose();
});

test("enemy capture pins own submitted palettes and poses without changing the last live frame", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  assert.throws(() => enemy.pinCaptureReference(), /Render the knights/);
  const live = actor(7);
  enemy.render(gpu.pass, frame, [live]);
  const submittedJoints = gpu.writes.at(-2)!.bytes.slice(),
    submittedModels = gpu.writes.at(-1)!.bytes.slice(),
    reference = enemy.pinCaptureReference(),
    identity = await enemy.captureReferenceIdentity(reference);
  assert.match(reference.modelIdentity, /^[a-f0-9]{64}$/);
  assert.equal(identity.modelIdentity, reference.modelIdentity);
  assert.match(identity.poseSha256, /^[a-f0-9]{64}$/);
  live.position[0] = -99;
  live.animationTime = 99;
  assert.deepEqual(reference.actors[0].position, [7, 0, 0]);
  assert.equal(reference.actors[0].animationTime, 0.25);
  assert.throws(() => (reference.actorIds as number[]).push(8));
  assert.throws(
    () => ((reference.actors[0].position as unknown as number[])[0] = 9),
  );
  assert.throws(() =>
    Object.assign(reference, { modelIdentity: "f".repeat(64) }),
  );
  const moved = {
    ...actor(7),
    position: [8, 0, 1] as [number, number, number],
    animationTime: 0.6,
  };
  enemy.render(gpu.pass, frame, [moved]);
  const latest = await enemy.captureReferenceIdentity(
    enemy.pinCaptureReference(),
  );
  assert.notEqual(latest.poseSha256, identity.poseSha256);
  enemy.renderCapture(gpu.pass, { ...frame, time: 99999 }, reference);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, submittedJoints);
  assert.deepEqual(gpu.writes.at(-1)!.bytes, submittedModels);
  assert.deepEqual(await enemy.captureReferenceIdentity(reference), identity);
  assert.deepEqual(
    await enemy.captureReferenceIdentity(enemy.pinCaptureReference()),
    latest,
  );
  enemy.render(
    gpu.pass,
    frame,
    [{ ...moved, position: [9, 0, 1], animationTime: 0.7 }],
    false,
  );
  assert.deepEqual(
    await enemy.captureReferenceIdentity(enemy.pinCaptureReference()),
    latest,
  );
  assert.deepEqual(enemy.stats, {
    submittedActors: 1,
    submittedTriangles: 1,
    submittedHealthBars: 0,
    submittedHealthBarTriangles: 0,
  });
  enemy.dispose();
});

test("frozen death-blend captures never rewind live animation sampling", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    controlGPU = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load),
    control = await EnemyCharacters.create(
      controlGPU.device,
      f.manifest,
      f.load,
    );
  const living = { ...actor(2), animationTime: 0.5 };
  for (const [characters, target] of [
    [enemy, gpu],
    [control, controlGPU],
  ] as const) {
    characters.render(target.pass, frame, [living]);
    characters.render(target.pass, frame, [
      { ...living, alive: false, deathSeconds: 0.03 },
    ]);
  }
  const reference = enemy.pinCaptureReference(),
    frozen = gpu.writes.at(-2)!.bytes.slice();
  const next = { ...living, alive: false, deathSeconds: 0.08 };
  enemy.render(gpu.pass, frame, [next]);
  control.render(controlGPU.pass, frame, [next]);
  assert.notDeepEqual(gpu.writes.at(-2)!.bytes, frozen);
  enemy.renderCapture(gpu.pass, { ...frame, time: -100 }, reference);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, frozen);
  const after = { ...living, alive: false, deathSeconds: 0.11 };
  enemy.render(gpu.pass, frame, [after]);
  control.render(controlGPU.pass, frame, [after]);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, controlGPU.writes.at(-2)!.bytes);
  assert.deepEqual(gpu.writes.at(-1)!.bytes, controlGPU.writes.at(-1)!.bytes);
  enemy.dispose();
  control.dispose();
});

test("enemy capture admission rejects foreign, forged, stale and disposed references", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    otherGPU = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load),
    other = await EnemyCharacters.create(otherGPU.device, f.manifest, f.load);
  enemy.render(gpu.pass, frame, [actor(1)]);
  other.render(otherGPU.pass, frame, [actor(1)]);
  const reference = enemy.pinCaptureReference();
  assert.throws(
    () => other.renderCapture(otherGPU.pass, frame, reference),
    /current knight capture/,
  );
  assert.throws(
    () => enemy.renderCapture(gpu.pass, frame, { ...reference }),
    /current knight capture/,
  );
  await assert.rejects(
    other.captureReferenceIdentity(reference),
    /current knight capture/,
  );
  const before = gpu.writes.length;
  f.manifest.draws[0].indexCount = 6;
  assert.throws(
    () => enemy.renderCapture(gpu.pass, frame, reference),
    /current knight capture/,
  );
  assert.throws(() => enemy.pinCaptureReference(), /Render the knights/);
  assert.equal(gpu.writes.length, before);
  f.manifest.draws[0].indexCount = 3;
  enemy.render(gpu.pass, frame, []);
  enemy.render(gpu.pass, frame, [actor(1)]);
  assert.throws(
    () => enemy.renderCapture(gpu.pass, frame, reference),
    /current knight capture/,
  );
  const pending = enemy.captureReferenceIdentity(enemy.pinCaptureReference());
  const rejected = assert.rejects(pending, /current knight capture/);
  enemy.render(gpu.pass, frame, [actor(2)]);
  await rejected;
  const current = enemy.pinCaptureReference();
  enemy.dispose();
  assert.throws(() => enemy.renderCapture(gpu.pass, frame, current), /closed/);
  assert.throws(() => enemy.pinCaptureReference(), /closed/);
  await assert.rejects(enemy.captureReferenceIdentity(current), /closed/);
  other.dispose();
});

test("failed enemy submissions preserve the last successful capture and empty scenes can pin", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  enemy.render(gpu.pass, frame, [actor(0)]);
  const accepted = await enemy.captureReferenceIdentity(
    enemy.pinCaptureReference(),
  );
  assert.throws(() =>
    enemy.render(
      {
        ...gpu.pass,
        drawIndexed() {
          throw Error("Encoder closed");
        },
      },
      frame,
      [{ ...actor(0), animationTime: 0.8, position: [4, 0, 0] }],
    ),
  );
  assert.deepEqual(
    await enemy.captureReferenceIdentity(enemy.pinCaptureReference()),
    accepted,
  );
  enemy.render(gpu.pass, { ...frame, enabled: false }, [actor(1)]);
  assert.deepEqual(
    await enemy.captureReferenceIdentity(enemy.pinCaptureReference()),
    accepted,
  );
  enemy.render(gpu.pass, frame, []);
  const empty = enemy.pinCaptureReference();
  assert.deepEqual(empty.actors, []);
  assert.deepEqual(empty.actorIds, []);
  const writes = gpu.writes.length;
  enemy.renderCapture(gpu.pass, frame, empty);
  assert.equal(gpu.writes.length, writes);
  assert.deepEqual(enemy.stats, {
    submittedActors: 0,
    submittedTriangles: 0,
    submittedHealthBars: 0,
    submittedHealthBarTriangles: 0,
  });
  enemy.dispose();
});

test("native-start enemy references match across independent live histories without changing them", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    otherGPU = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load),
    other = await EnemyCharacters.create(otherGPU.device, f.manifest, f.load);
  const start = [{ ...actor(3), animationTime: 0 }];
  enemy.render(gpu.pass, frame, [
    { ...actor(3), animationTime: 0.6, position: [4, 0, -3] },
  ]);
  other.render(otherGPU.pass, frame, [actor(3)]);
  other.render(otherGPU.pass, frame, [
    { ...actor(3), alive: false, deathSeconds: 0.05 },
  ]);
  const before = await enemy.captureReferenceIdentity(
      enemy.pinCaptureReference(),
    ),
    otherBefore = await other.captureReferenceIdentity(
      other.pinCaptureReference(),
    ),
    inputBefore = structuredClone(start),
    reference = enemy.pinNativeStartReference(start),
    otherReference = other.pinNativeStartReference(start);
  assert.deepEqual(
    await enemy.captureReferenceIdentity(reference),
    await other.captureReferenceIdentity(otherReference),
  );
  enemy.renderCapture(gpu.pass, { ...frame, time: 0 }, reference);
  other.renderCapture(otherGPU.pass, { ...frame, time: 0 }, otherReference);
  assert.deepEqual(gpu.writes.at(-2)!.bytes, otherGPU.writes.at(-2)!.bytes);
  assert.deepEqual(gpu.writes.at(-1)!.bytes, otherGPU.writes.at(-1)!.bytes);
  assert.equal(new Float32Array(gpu.writes.at(-2)!.bytes.buffer)[14], 0);
  assert.deepEqual(
    await enemy.captureReferenceIdentity(enemy.pinCaptureReference()),
    before,
  );
  assert.deepEqual(
    await other.captureReferenceIdentity(other.pinCaptureReference()),
    otherBefore,
  );
  assert.deepEqual(start, inputBefore);
  start[0].position[0] = 99;
  assert.deepEqual(reference.actors[0].position, [3, 0, 0]);
  enemy.dispose();
  other.dispose();
});

test("native-start capture admission rejects changed actor sets and noninitial poses", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load),
    native = { ...actor(0), animationTime: 0 };
  assert.throws(
    () => enemy.pinNativeStartReference([native]),
    /Render this world's knights/,
  );
  enemy.render(gpu.pass, frame, [actor(0)]);
  const before = await enemy.captureReferenceIdentity(
      enemy.pinCaptureReference(),
    ),
    writes = gpu.writes.length;
  for (const actors of [
    [],
    Array.from({ length: 17 }, () => native),
    [{ ...native, id: 1 }],
    [{ ...native, alive: false }],
    [{ ...native, deathSeconds: 0.01 }],
    [{ ...native, animationTime: 0.01 }],
    [{ ...native, position: [Infinity, 0, 0] }],
    [{ ...native, position: [20_001, 0, 0] }],
    [{ ...native, position: [0, 0] }],
    [{ ...native, position: [, 0, 0] }],
    [{ ...native, yaw: NaN }],
  ] as EnemyActorPose[][])
    assert.throws(() => enemy.pinNativeStartReference(actors), /starting pose/);
  assert.equal(gpu.writes.length, writes);
  assert.deepEqual(
    await enemy.captureReferenceIdentity(enemy.pinCaptureReference()),
    before,
  );
  const reference = enemy.pinNativeStartReference([native]);
  enemy.render(gpu.pass, frame, []);
  assert.throws(
    () => enemy.renderCapture(gpu.pass, frame, reference),
    /current knight capture/,
  );
  enemy.dispose();
  assert.throws(() => enemy.pinNativeStartReference([native]), /closed/);
});

test("PBR character maps share allocations and generate every real color and normal mip", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU(),
    png = new ArrayBuffer(33),
    bytes = new Uint8Array(png),
    view = new DataView(png);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10]);
  view.setUint32(8, 13);
  view.setUint32(12, 0x49484452);
  view.setUint32(16, 8);
  view.setUint32(20, 2);
  const texture = {
    url: "/private-assets/test-pbr.png",
    byteLength: png.byteLength,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    width: 8,
    height: 2,
  };
  f.manifest.materials[0] = {
    ...f.manifest.materials[0],
    texture,
    normalTexture: texture,
    emissiveTexture: texture,
    emissiveColor: [1, 0.5, 0.25],
    metallic: 0.5,
    roughness: 0.5527864,
  };
  f.manifest.materials.push({
    ...f.manifest.materials[0],
    name: "shared helmet",
  });
  const previous = Object.getOwnPropertyDescriptor(
    globalThis,
    "createImageBitmap",
  );
  let closed = 0,
    imageLoads = 0;
  Object.defineProperty(globalThis, "createImageBitmap", {
    configurable: true,
    value: async () => {
      imageLoads++;
      return {
        width: 8,
        height: 2,
        close() {
          closed++;
        },
      };
    },
  });
  try {
    const renderer = await CharacterRenderer.create(
      gpu.device,
      f.manifest,
      async (url) => (url === texture.url ? png.slice(0) : f.load(url)),
    );
    assert.equal(imageLoads, 2); // Shared by both materials, with separate sRGB and linear storage.
    assert.equal(closed, 2);
    assert.deepEqual(
      gpu.textures.map((t) => [t.width, t.height, t.mipLevelCount, t.format]),
      [
        [8, 2, 4, "rgba8unorm-srgb"],
        [8, 2, 4, "rgba8unorm"],
      ],
    );
    assert.equal(gpu.pipelines, 3);
    assert.equal(gpu.submits, 2);
    assert.deepEqual(gpu.mipDraws, [3, 3, 3, 3, 3, 3]);
    assert.equal(renderer.bytes, 192 + 12 + 464 + 128 + 80 + 2 * 64 + 2 * 92);
    const material = gpu.writes.filter((w) => w.bytes.byteLength === 64)[0];
    const settings = new Float32Array(material.bytes.buffer);
    assert.equal(settings[6], 1); // Authored normal texture enabled.
    assert.equal(settings[7], 1); // PBR path selected.
    assert.equal(settings[8], 0.5);
    assert.ok(Math.abs(settings[9] - 0.5527864) < 1e-6);
    assert.deepEqual(Array.from(settings.slice(12, 15)), [1, 0.5, 0.25]);
    assert.ok(gpu.shaders.some((shader) => shader.includes("raw+=emission")));
    renderer.dispose();
    assert.ok(gpu.textures.every((t) => t.destroyed === 1));
  } finally {
    if (previous)
      Object.defineProperty(globalThis, "createImageBitmap", previous);
    else Reflect.deleteProperty(globalThis, "createImageBitmap");
  }
});

test("enemy health bars cap near-camera width and retain natural distance shrinkage", () => {
  const near = 0.1,
    far = 100,
    f = 1 / Math.tan(1 / 2),
    aspect = 16 / 9;
  const projection = [
    f / aspect,
    0,
    0,
    0,
    0,
    f,
    0,
    0,
    0,
    0,
    far / (near - far),
    -1,
    0,
    0,
    (far * near) / (near - far),
    0,
  ];
  const closeScale = enemyHealthBarScale(projection, [0, 0, -0.2]);
  const projectedHalfWidth = ((0.37 * (f / aspect)) / 0.2) * closeScale;
  assert.ok(closeScale > 0 && closeScale < 0.1);
  assert.ok(Math.abs(projectedHalfWidth - 0.08) < 1e-12);
  assert.ok(Math.abs(projectedHalfWidth * 1280 - 102.4) < 1e-9);
  assert.equal(enemyHealthBarScale(projection, [0, 0, -10]), 1);
  for (const z of [0, 1, -0.05, -101, NaN])
    assert.equal(enemyHealthBarScale(projection, [0, 0, z]), 0);
});

test("four-state zombies share real poses and camera-facing health bars without synthetic run clips", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU();
  f.manifest.name = "test/zombie";
  f.manifest.animationProfile = "quick-mmorpg-zombie-v1";
  f.manifest.clips = ["idle", "walk", "attack", "death"].map((name) => ({
    name: name as "idle" | "walk" | "attack" | "death",
    duration: 1,
    distance: 0,
    loop: name === "idle" || name === "walk",
    tracks: [
      {
        joint: 0,
        translation: [
          { time: 0, value: [0, 0, 0] },
          {
            time: 1,
            value:
              name === "death"
                ? [0, -0.8, 0]
                : name === "attack"
                  ? [1, 0, 0]
                  : name === "walk"
                    ? [0, 0, 2]
                    : [0, 0, 0],
          },
        ],
        rotation: [],
        scale: [],
      },
    ],
  }));
  f.manifest.metadata = { healthBarHeight: 1.3 };
  const enemy = await EnemyCharacters.create(gpu.device, f.manifest, f.load);
  const actors: EnemyActorPose[] = [
    {
      ...actor(7),
      position: [7, 0, 0.5],
      animation: "idle",
      animationTime: 0,
      health: 20,
      maxHealth: 50,
    },
  ];
  enemy.render(gpu.pass, frame, actors);
  assert.deepEqual(gpu.draws.at(-1), [3, 1, 0]);
  assert.deepEqual(gpu.barDraws.at(-1), [12, 1]);
  assert.equal(enemy.stats.submittedHealthBars, 1);
  const healthData = gpu.writes.at(-1)!.bytes;
  const bar = new Float32Array(healthData.buffer);
  assert.ok(Math.abs(bar[1] - 1.3) < 1e-6 && Math.abs(bar[3] - 0.4) < 1e-6);
  actors[0].animation = "walk";
  actors[0].animationTime = 0.2;
  enemy.render(gpu.pass, { ...frame, time: 1 }, actors);
  actors[0].animationTime = 0.4;
  enemy.render(gpu.pass, { ...frame, time: 1.2 }, actors);
  let palette = new Float32Array(
    gpu.writes.filter((w) => w.bytes.byteLength === 128).at(-1)!.bytes.buffer,
  );
  assert.ok(palette[14] > 0.7, "The authored Walk pose was not sampled");
  actors[0].animation = "attack";
  actors[0].animationTime = 0.3;
  enemy.render(gpu.pass, { ...frame, time: 2 }, actors);
  actors[0].animationTime = 0.6;
  enemy.render(gpu.pass, { ...frame, time: 2.2 }, actors);
  palette = new Float32Array(
    gpu.writes.filter((w) => w.bytes.byteLength === 128).at(-1)!.bytes.buffer,
  );
  assert.ok(palette[12] > 0.5, "The authored Attack pose was not sampled");
  const frozen = enemy.pinCaptureReference();
  assert.equal(frozen.actors[0].health, 20);
  actors[0] = {
    ...actors[0],
    alive: false,
    health: 0,
    animation: "death",
    deathSeconds: 3,
    animationTime: 3,
  };
  enemy.render(gpu.pass, { ...frame, time: 4 }, actors);
  enemy.render(gpu.pass, { ...frame, time: 5 }, [
    { ...actors[0], deathSeconds: 4 },
  ]);
  assert.equal(enemy.stats.submittedHealthBars, 0);
  palette = new Float32Array(
    gpu.writes.filter((w) => w.bytes.byteLength === 128).at(-1)!.bytes.buffer,
  );
  assert.ok(
    Math.abs(palette[13] + 0.8) < 1e-6,
    "The authored Death final pose was not retained",
  );
  const writes = gpu.writes.length;
  assert.throws(
    () =>
      enemy.render(gpu.pass, frame, [
        { ...actors[0], alive: true, health: 51 },
      ]),
    /complete world pose/,
  );
  assert.throws(
    () =>
      enemy.render(gpu.pass, frame, [
        { ...actors[0], alive: true, health: 20, animation: "run" },
      ]),
    /complete world pose/,
  );
  assert.equal(gpu.writes.length, writes);
  enemy.dispose();
  assert(gpu.buffers.every((buffer) => buffer.destroyed === 1));
});

test("enemy inspection admits bounded combat health and animation without weakening patrol poses", () => {
  const value = {
    total: 1,
    alive: 1,
    killed: 0,
    actors: [{ ...actor(4), animation: "walk", health: 20, maxHealth: 50 }],
  };
  assert(validEnemySnapshot(value));
  for (const patch of [
    { health: -1 },
    { health: 51 },
    { health: NaN },
    { maxHealth: 0 },
    { maxHealth: Infinity },
    { animation: "fly" },
    { health: undefined },
    { alive: false },
  ])
    assert(
      !validEnemySnapshot({
        ...value,
        actors: [{ ...value.actors[0], ...patch }],
      }),
    );
  assert(
    validEnemySnapshot({
      total: 1,
      alive: 0,
      killed: 1,
      actors: [
        {
          ...actor(4),
          alive: false,
          animation: "death",
          health: 0,
          maxHealth: 50,
        },
      ],
    }),
  );
});

test("character reflections use the world's borrowed IBL and never dispose it", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU();
  let destroyed = 0;
  const borrowed = {
    createView: () => ({}),
    destroy: () => {
      destroyed++;
    },
  } as unknown as GPUTexture;
  const renderer = await CharacterRenderer.create(
    gpu.device,
    f.manifest,
    f.load,
    1,
    {
      ibl: {
        scenarios: new Map([["day", { texture: borrowed, mipMax: 7 }]]),
        fallback: borrowed,
        lut: borrowed,
      },
    },
  );
  renderer.render(
    gpu.pass,
    {
      ...frame,
      specularEnvironment: {
        gain: [2, 3, 4],
        primaryScale: 0.5,
        constantScale: 1,
      },
    },
    { position: [0, 0, 0], yaw: 0, animation: "idle", time: 0, visible: true },
  );
  assert.ok(gpu.boundGroups.includes(2));
  const uniform = gpu.writes.findLast((w) => w.bytes.byteLength === 464)!;
  assert.deepEqual(
    Array.from(new Float32Array(uniform.bytes.buffer).subarray(100, 108)),
    [2, 3, 4, 0, 7, 0.5, 1, 1],
  );
  assert.ok(gpu.shader.includes("texture_2d_array<f32>"));
  assert.ok(gpu.shader.includes("environmentBrdfWeight(f0,brdf)"));
  renderer.dispose();
  assert.equal(destroyed, 0);
  assert.ok(gpu.textures.every((t) => t.destroyed === 1));
});

test("MMORPG pools share each class and select attached weapon draws per actor", async () => {
  globals();
  const f = fixture(),
    gpu = fakeGPU();
  const source = f.manifest.clips[0];
  const manifests = Object.fromEntries(
    (["paladin", "sorceror", "zombie", "warrok"] as const).map((kind) => [
      kind,
      {
        ...structuredClone(f.manifest),
        mmorpgClass: kind,
        ...(kind === "paladin" || kind === "sorceror"
          ? { controller: { profile: "quick-mmorpg-v1", worldScale: 1 / 6 } }
          : { animationProfile: "quick-mmorpg-npc-v1" }),
        clips: (kind === "paladin" || kind === "sorceror"
          ? ["idle", "walk", "run", "attack", "dance", "death"]
          : ["idle", "walk", "attack", "death"]
        ).map((name) => ({ ...structuredClone(source), name })),
      },
    ]),
  ) as Record<MmorpgClass, CharacterManifest>;
  const indices = new Uint32Array([0, 1, 2, 0, 1, 2, 0, 1, 2]);
  const paladin = manifests.paladin;
  paladin.geometry.indices = {
    ...paladin.geometry.indices,
    sha256: createHash("sha256")
      .update(new Uint8Array(indices.buffer))
      .digest("hex"),
    byteLength: indices.byteLength,
  };
  paladin.geometry.indexCount = 9;
  paladin.draws = [
    { material: 0, firstIndex: 0, indexCount: 3 },
    { material: 0, firstIndex: 3, indexCount: 3, part: "weapon.sword1" },
    { material: 0, firstIndex: 6, indexCount: 3, part: "weapon.axe1" },
  ];
  const load = async (url: string) =>
    url === paladin.geometry.indices.url ? indices.buffer : f.load(url);
  // Give every other class its original distinct index file, while retaining shared synthetic positions.
  for (const kind of ["sorceror", "zombie", "warrok"] as const)
    manifests[kind].geometry.indices.url =
      "/private-assets/" + kind + ".indices.bin";
  const models = await MmorpgCharacters.create(
    gpu.device,
    manifests,
    async (url) =>
      url.includes(".indices.bin") && url !== paladin.geometry.indices.url
        ? new Uint32Array([0, 1, 2]).buffer
        : load(url),
  );
  const actors: MmorpgActorPose[] = [
    {
      id: 1,
      class: "paladin",
      name: "A",
      position: [0, 0, 0.5],
      yaw: 0,
      state: "idle",
      actionTime: 0,
      animationTime: 0,
      health: 200,
      maxHealth: 200,
      alive: true,
      weapon: "weapon.sword1",
    },
    {
      id: 2,
      class: "paladin",
      name: "B",
      position: [1, 0, 0.5],
      yaw: 0,
      state: "idle",
      actionTime: 0,
      animationTime: 0,
      health: 200,
      maxHealth: 200,
      alive: true,
      weapon: null,
    },
  ];
  models.render(gpu.pass, frame, actors, 1);
  assert.deepEqual(gpu.draws.slice(-2), [
    [3, 2, 0],
    [3, 1, 3, 0, 0],
  ]);
  assert.equal(models.stats.actors, 2);
  assert.equal(models.stats.triangles, 3);
  actors[1].weapon = "weapon.axe1";
  models.render(gpu.pass, { ...frame, time: 1 }, actors, 1);
  assert.deepEqual(gpu.draws.at(-1), [3, 1, 6, 0, 1]);
  const writes = gpu.writes.length;
  assert.throws(() => models.render(gpu.pass, frame, [actors[0], actors[0]]));
  assert.equal(gpu.writes.length, writes);
  models.dispose();
  assert(gpu.buffers.every((b) => b.destroyed === 1));
});
